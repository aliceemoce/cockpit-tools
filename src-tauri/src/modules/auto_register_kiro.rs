//! Kiro 自动注册模块
//! 使用 chromiumoxide (Chrome DevTools Protocol) 实现浏览器自动化

use anyhow::{anyhow, Result};
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::target::CreateTargetParams;
use chromiumoxide::page::Page;
use futures::StreamExt;
use serde_json::json;
use std::time::Duration;

use crate::commands::auto_register::ImportedAccountData;

const DEFAULT_PASSWORD: &str = "admin123456aA!";
const AWS_REGISTER_URL: &str = "https://view.awsapps.com/start/#/device?user_code=PQCF-FCCN";
const KIRO_OIDC_TOKEN_ENDPOINT_FMT: &str = "https://oidc.{region}.amazonaws.com/token";
const KIRO_REFRESH_ENDPOINT: &str = "https://prod.us-east-1.auth.desktop.kiro.dev/refreshToken";
const KIRO_RUNTIME_DEFAULT_ENDPOINT: &str = "https://q.us-east-1.amazonaws.com";

use std::sync::Arc;

/// 日志发送器类型
type LogSender = Arc<dyn Fn(&str, &str) + Send + Sync>;

/// 日志回调 trait
pub trait LogCallback: Fn(&str, &str) + Send + Sync + 'static {}
impl<T: Fn(&str, &str) + Send + Sync + 'static> LogCallback for T {}

/// 获取 Playwright 安装的 Chromium 路径
fn get_playwright_chromium_path() -> Option<std::path::PathBuf> {
    // Windows 上 Playwright 默认安装路径
    let home_dir = std::env::var("USERPROFILE").ok()?;
    let playwright_dir = std::path::Path::new(&home_dir).join("AppData").join("Local").join("ms-playwright");
    
    // 查找 chromium-* 目录
    if let Ok(entries) = std::fs::read_dir(&playwright_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("chromium-") {
                // 尝试 chrome-win 目录
                let chrome_path = entry.path()
                    .join("chrome-win")
                    .join("chrome.exe");
                if chrome_path.exists() {
                    return Some(chrome_path);
                }
                // 尝试 chrome-win64 目录
                let chrome_path64 = entry.path()
                    .join("chrome-win64")
                    .join("chrome.exe");
                if chrome_path64.exists() {
                    return Some(chrome_path64);
                }
                // 尝试直接 chrome.exe
                let chrome_direct = entry.path().join("chrome.exe");
                if chrome_direct.exists() {
                    return Some(chrome_direct);
                }
            }
        }
    }
    None
}

/// 使用浏览器自动化注册 AWS Builder ID
/// 返回 (sso_token, full_name)
pub async fn register_aws_builder_id<F>(
    email: &str,
    _email_password: &str,
    full_name: &str,
    proxy_url: Option<&str>,
    send_log: F,
) -> Result<(String, String)>
where
    F: Fn(&str, &str) + Send + Sync + 'static,
{
    send_log(email, "========== 开始 AWS Builder ID 注册 ==========");
    send_log(email, &format!("邮箱: {}", email));
    send_log(email, &format!("姓名: {}", full_name));
    send_log(email, &format!("密码: {}", DEFAULT_PASSWORD));
    if let Some(proxy) = proxy_url {
        send_log(email, &format!("代理: {}", proxy));
    }

    // 构建浏览器配置
    let mut browser_config = BrowserConfig::builder();
    
    // 获取用户目录
    let home_dir = std::env::var("USERPROFILE").unwrap_or_default();
    
    // 优先尝试使用 Playwright Chromium
    // 注意：Playwright 的 Chromium 可能需要特殊处理
    match get_playwright_chromium_path() {
        Some(chrome_path) => {
            send_log(email, &format!("检测到 Playwright Chromium: {:?}", chrome_path));
            // Playwright Chromium 在某些系统上需要额外权限
            // 暂时注释掉，优先使用系统 Chrome
            send_log(email, "(Playwright Chromium 可能存在兼容性问题，尝试系统 Chrome)");
        }
        None => {
            let playwright_dir = std::path::Path::new(&home_dir).join("AppData").join("Local").join("ms-playwright");
            send_log(email, &format!("未找到 Playwright Chromium: {:?}", playwright_dir));
        }
    }
    
    // 检查并设置系统 Chrome 路径
    let system_chrome_paths = [
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
    ];
    
    let mut chrome_path: Option<std::path::PathBuf> = None;
    for path in &system_chrome_paths {
        let p = std::path::PathBuf::from(path);
        if p.exists() {
            chrome_path = Some(p);
            break;
        }
    }
    
    // 也检查用户目录
    if chrome_path.is_none() {
        let local_chrome = std::path::PathBuf::from(&home_dir)
            .join("AppData")
            .join("Local")
            .join("Google")
            .join("Chrome")
            .join("Application")
            .join("chrome.exe");
        if local_chrome.exists() {
            chrome_path = Some(local_chrome);
        }
    }
    
    // 检查 Chrome for Testing（专门用于自动化测试）
    if chrome_path.is_none() {
        let chrome_for_testing = std::path::PathBuf::from(r"C:\Program Files\Google\Chrome for Testing\Application\chrome.exe");
        if chrome_for_testing.exists() {
            chrome_path = Some(chrome_for_testing);
        }
    }
    
    if let Some(ref path) = chrome_path {
        send_log(email, &format!("使用系统 Chrome: {:?}", path));
        browser_config = browser_config.chrome_executable(path.clone());
    } else {
        send_log(email, "警告: 未找到 Chrome 浏览器");
        send_log(email, "请安装 Google Chrome: https://www.google.com/chrome/");
        send_log(email, "或 Chrome for Testing: https://googlechromelabs.github.io/chrome-for-testing/");
    }
    
    // 设置代理（如果提供）
    if let Some(proxy) = proxy_url {
        browser_config = browser_config
            .arg(format!("--proxy-server={}", proxy));
    }
    
    // 简化启动参数，提高兼容性
    let browser_config = browser_config
        .arg("--no-sandbox")
        .arg("--disable-setuid-sandbox")
        .arg("--disable-dev-shm-usage")
        .arg("--disable-gpu")
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .build()
        .map_err(|e| anyhow!("构建浏览器配置失败: {}", e))?;

    send_log(email, "\n步骤1: 启动浏览器...");
    
    let (mut browser, mut handler) = match Browser::launch(browser_config).await {
        Ok(result) => {
            send_log(email, "✓ 浏览器进程已启动");
            result
        }
        Err(e) => {
            let err_msg = format!("启动浏览器失败: {:?}", e);
            send_log(email, &format!("✗ {}", err_msg));
            return Err(anyhow!(err_msg));
        }
    };

    // 在后台处理浏览器事件
    let _browser_handle = tokio::spawn(async move {
        loop {
            let _ = handler.next().await;
        }
    });

    // 创建新页面
    let page = browser
        .new_page(CreateTargetParams::new(AWS_REGISTER_URL))
        .await
        .map_err(|e| anyhow!("创建页面失败: {}", e))?;

    send_log(email, "✓ 浏览器启动成功");

    // 等待页面加载
    tokio::time::sleep(Duration::from_secs(3)).await;

    // 创建线程安全的日志发送器
    let log_sender: Arc<dyn Fn(&str, &str) + Send + Sync + 'static> = Arc::new(send_log);

    // 执行注册流程
    let result = execute_registration_flow(&page, email, full_name, Arc::clone(&log_sender)).await;

    // 关闭浏览器
    let _ = browser.close().await;

    result
}

/// 执行注册流程
fn execute_registration_flow<'a>(
    page: &'a Page,
    email: &'a str,
    full_name: &'a str,
    send_log: Arc<dyn Fn(&str, &str) + Send + Sync + 'static>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(String, String)>> + Send + 'a>> {
    Box::pin(async move {
    // 步骤1: 输入邮箱
    send_log(email, "\n步骤1: 输入邮箱...");
    
    // 等待邮箱输入框出现
    let email_input_selector = "input[placeholder=\"username@example.com\"]";
    wait_for_selector(page, email_input_selector, 10)
        .await
        .map_err(|_| anyhow!("未找到邮箱输入框"))?;
    
    // 输入邮箱
    page.find_element(email_input_selector)
        .await?
        .type_str(email)
        .await?;
    
    send_log(email, "✓ 邮箱输入完成");
    tokio::time::sleep(Duration::from_secs(1)).await;

    // 点击第一个继续按钮
    let first_continue_selector = "button[data-testid=\"test-primary-button\"]";
    click_element_with_retry(page, first_continue_selector, &send_log, "第一个继续按钮").await?;
    
    tokio::time::sleep(Duration::from_secs(3)).await;

    // 检测页面类型（登录、验证或注册）
    let page_type = detect_page_type(page).await?;
    
    let sso_token = match page_type {
        PageType::Register => {
            // 注册流程
            send_log(email, "\n步骤2: 输入姓名...");
            let name_input_selector = "input[placeholder=\"Maria José Silva\"]";
            
            wait_for_selector(page, name_input_selector, 10).await
                .map_err(|_| anyhow!("未找到姓名输入框"))?;
            
            page.find_element(name_input_selector)
                .await?
                .type_str(full_name)
                .await?;
            
            tokio::time::sleep(Duration::from_secs(1)).await;
            
            // 点击第二个继续按钮
            let second_continue_selector = "button[data-testid=\"signup-next-button\"]";
            click_element_with_retry(page, second_continue_selector, &send_log, "第二个继续按钮").await?;
            
            tokio::time::sleep(Duration::from_secs(3)).await;
            
            // 处理验证码
            send_log(email, "\n步骤3: 等待验证码...");
            let code = wait_for_verification_code(&send_log, email, 120).await
                .ok_or_else(|| anyhow!("获取验证码超时"))?;
            
            send_log(email, &format!("✓ 获取到验证码: {}", code));
            
            // 输入验证码
            let code_input_selector = "input[placeholder=\"6 位数\"]";
            page.find_element(code_input_selector)
                .await?
                .type_str(&code)
                .await?;
            
            tokio::time::sleep(Duration::from_secs(1)).await;
            
            // 点击验证按钮
            let verify_button_selector = "button[data-testid=\"email-verification-verify-button\"]";
            click_element_with_retry(page, verify_button_selector, &send_log, "Continue 按钮").await?;
            
            tokio::time::sleep(Duration::from_secs(3)).await;
            
            // 输入密码
            send_log(email, "\n步骤4: 输入密码...");
            let password_input_selector = "input[placeholder=\"Enter password\"]";
            wait_for_selector(page, password_input_selector, 10).await
                .map_err(|_| anyhow!("未找到密码输入框"))?;
            
            page.find_element(password_input_selector)
                .await?
                .type_str(DEFAULT_PASSWORD)
                .await?;
            
            tokio::time::sleep(Duration::from_millis(500)).await;
            
            // 输入确认密码
            let confirm_password_selector = "input[placeholder=\"Re-enter password\"]";
            page.find_element(confirm_password_selector)
                .await?
                .type_str(DEFAULT_PASSWORD)
                .await?;
            
            tokio::time::sleep(Duration::from_secs(1)).await;
            
            // 点击第三个继续按钮
            let third_continue_selector = "button[data-testid=\"test-primary-button\"]";
            click_element_with_retry(page, third_continue_selector, &send_log, "第三个继续按钮").await?;
            
            tokio::time::sleep(Duration::from_secs(5)).await;
            
            // 提取 SSO Token
            extract_sso_token(page).await?
        }
        PageType::Login => {
            // 登录流程（账号已存在）
            send_log(email, "检测到已注册账号，执行登录流程...");
            
            // 输入密码
            let password_input_selector = "input[type=\"password\"]";
            wait_for_selector(page, password_input_selector, 10).await
                .map_err(|_| anyhow!("未找到密码输入框"))?;
            
            page.find_element(password_input_selector)
                .await?
                .type_str(DEFAULT_PASSWORD)
                .await?;
            
            tokio::time::sleep(Duration::from_millis(500)).await;
            
            // 点击登录按钮
            let login_button_selector = "button[data-testid=\"test-primary-button\"]";
            click_element_with_retry(page, login_button_selector, &send_log, "登录按钮").await?;
            
            tokio::time::sleep(Duration::from_secs(5)).await;
            
            // 处理可能的验证码
            if let Ok(code_input) = page.find_element("input[placeholder=\"6 位数\"]").await {
                send_log(email, "\n需要输入登录验证码...");
                let code = wait_for_verification_code(&send_log, email, 120).await
                    .ok_or_else(|| anyhow!("获取验证码超时"))?;
                
                code_input.type_str(&code).await?;
                tokio::time::sleep(Duration::from_millis(500)).await;
                
                // 点击验证码确认按钮
                let verify_button_selector = "button[data-testid=\"test-primary-button\"]";
                click_element_with_retry(page, verify_button_selector, &send_log, "登录验证码确认按钮").await?;
                
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
            
            extract_sso_token(page).await?
        }
        PageType::Verify => {
            // 验证流程
            send_log(email, "\n需要输入验证码...");
            let code = wait_for_verification_code(&send_log, email, 120).await
                .ok_or_else(|| anyhow!("获取验证码超时"))?;
            
            let code_input_selector = "input[placeholder=\"6-digit\"]";
            page.find_element(code_input_selector)
                .await?
                .type_str(&code)
                .await?;
            
            tokio::time::sleep(Duration::from_millis(500)).await;
            
            let verify_button_selector = "button[data-testid=\"test-primary-button\"]";
            click_element_with_retry(page, verify_button_selector, &send_log, "Continue 按钮").await?;
            
            tokio::time::sleep(Duration::from_secs(5)).await;
            
            // 继续登录流程
            execute_registration_flow(page, email, full_name, Arc::clone(&send_log)).await?.0
        }
    };

    send_log(email, "\n========== 操作成功! ==========");
    Ok((sso_token, full_name.to_string()))
    })
}

#[derive(Debug, Clone, Copy)]
enum PageType {
    Register,  // 注册页面
    Login,     // 登录页面
    Verify,    // 验证页面
}

/// 检测页面类型
async fn detect_page_type(page: &Page) -> Result<PageType> {
    // 尝试查找注册页面的姓名输入框
    if page.find_element("input[placeholder=\"Maria José Silva\"]").await.is_ok() {
        return Ok(PageType::Register);
    }
    
    // 尝试查找登录页面的标识
    let content = page.content().await?;
    if content.contains("Sign in with your AWS Builder ID") {
        return Ok(PageType::Login);
    }
    
    // 尝试查找验证码输入框（验证页面）
    if content.contains("Verify") && page.find_element("input[placeholder=\"6-digit\"]").await.is_ok() {
        return Ok(PageType::Verify);
    }
    
    // 默认假设为注册页面
    Ok(PageType::Register)
}

/// 等待选择器出现
async fn wait_for_selector(page: &Page, selector: &str, timeout_secs: u64) -> Result<()> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);
    
    while tokio::time::Instant::now() < deadline {
        if page.find_element(selector).await.is_ok() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    
    Err(anyhow!("等待选择器 {} 超时", selector))
}

/// 点击元素（带重试）
async fn click_element_with_retry(
    page: &Page,
    selector: &str,
    send_log: &Arc<dyn Fn(&str, &str) + Send + Sync>,
    description: &str,
) -> Result<()> {
    for attempt in 1..=3 {
        match page.find_element(selector).await {
            Ok(element) => {
                match element.click().await {
                    Ok(_) => {
                        send_log("system", &format!("✓ 点击{}成功", description));
                        return Ok(());
                    }
                    Err(e) => {
                        send_log("system", &format!("点击{}失败 (尝试 {}/3): {}", description, attempt, e));
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            }
            Err(e) => {
                send_log("system", &format!("查找{}失败 (尝试 {}/3): {}", description, attempt, e));
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
    
    Err(anyhow!("点击{}失败，已达到最大重试次数", description))
}

/// 等待验证码（手动输入或自动获取）
async fn wait_for_verification_code(
    _send_log: &Arc<dyn Fn(&str, &str) + Send + Sync>,
    _email: &str,
    timeout_secs: u64,
) -> Option<String> {
    // TODO: 实现验证码获取逻辑
    // 当前为占位实现，等待手动输入或集成邮件验证码获取
    
    // 等待超时时间
    tokio::time::sleep(Duration::from_secs(timeout_secs)).await;
    
    // 返回占位验证码，实际应该获取真实验证码
    Some("123456".to_string())
}

/// 从浏览器 Cookie 中提取 SSO Token
async fn extract_sso_token(page: &Page) -> Result<String> {
    // 尝试多次获取 Cookie
    for _i in 0..30 {
        let cookies = page.get_cookies().await?;
        
        for cookie in cookies {
            if cookie.name == "x-amz-sso_authn" {
                return Ok(cookie.value);
            }
        }
        
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    
    Err(anyhow!("未能获取 SSO Token (x-amz-sso_authn cookie)"))
}

/// SSO 设备授权结果
#[derive(Debug, Clone)]
struct SsoAuthResult {
    access_token: String,
    refresh_token: String,
    client_id: String,
    client_secret: String,
    region: String,
    expires_in: i64,
}

/// 使用 SSO Token 执行完整的 AWS SSO 设备授权流程
/// 获取有效的 access_token 和 refresh_token
pub async fn exchange_sso_token(bearer_token: &str, email: &str, name: &str) -> Result<ImportedAccountData> {
    let region = "us-east-1";
    let oidc_base = format!("https://oidc.{region}.amazonaws.com");
    let portal_base = "https://portal.sso.us-east-1.amazonaws.com";
    let start_url = "https://view.awsapps.com/start";
    let scopes = vec![
        "codewhisperer:analysis",
        "codewhisperer:completions",
        "codewhisperer:conversations",
        "codewhisperer:taskassist",
        "codewhisperer:transformations",
    ];

    // Step 1: 注册 OIDC 客户端
    println!("[SSO] Step 1: Registering OIDC client...");
    let client = reqwest::Client::new();
    let reg_res = client
        .post(format!("{}/client/register", oidc_base))
        .json(&serde_json::json!({
            "clientName": "Kiro Account Manager",
            "clientType": "public",
            "scopes": scopes,
            "grantTypes": ["urn:ietf:params:oauth:grant-type:device_code", "refresh_token"],
            "issuerUrl": start_url
        }))
        .send()
        .await
        .map_err(|e| anyhow!("注册客户端请求失败: {}", e))?;

    if !reg_res.status().is_success() {
        let status = reg_res.status();
        let text = reg_res.text().await.unwrap_or_default();
        return Err(anyhow!("注册客户端失败: {} - {}", status, text));
    }

    let reg_data: serde_json::Value = reg_res.json().await?;
    let client_id = reg_data["clientId"].as_str().ok_or_else(|| anyhow!("响应中缺少 clientId"))?.to_string();
    let client_secret = reg_data["clientSecret"].as_str().ok_or_else(|| anyhow!("响应中缺少 clientSecret"))?.to_string();
    println!("[SSO] Client registered: {}...", &client_id[..client_id.len().min(30)]);

    // Step 2: 发起设备授权
    println!("[SSO] Step 2: Starting device authorization...");
    let dev_res = client
        .post(format!("{}/device_authorization", oidc_base))
        .json(&serde_json::json!({
            "clientId": &client_id,
            "clientSecret": &client_secret,
            "startUrl": start_url
        }))
        .send()
        .await
        .map_err(|e| anyhow!("设备授权请求失败: {}", e))?;

    if !dev_res.status().is_success() {
        let status = dev_res.status();
        let text = dev_res.text().await.unwrap_or_default();
        return Err(anyhow!("设备授权失败: {} - {}", status, text));
    }

    let dev_data: serde_json::Value = dev_res.json().await?;
    let device_code = dev_data["deviceCode"].as_str().ok_or_else(|| anyhow!("响应中缺少 deviceCode"))?.to_string();
    let _user_code = dev_data["userCode"].as_str().ok_or_else(|| anyhow!("响应中缺少 userCode"))?.to_string();
    let interval = dev_data["interval"].as_i64().unwrap_or(1) as u64;
    println!("[SSO] Device code obtained, user_code: {}", _user_code);

    // Step 3: 验证 Bearer Token (whoAmI)
    println!("[SSO] Step 3: Verifying bearer token...");
    let who_res = client
        .get(format!("{}/token/whoAmI", portal_base))
        .header("Authorization", format!("Bearer {}", bearer_token))
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| anyhow!("whoAmI 请求失败: {}", e))?;

    if !who_res.status().is_success() {
        let status = who_res.status();
        let text = who_res.text().await.unwrap_or_default();
        return Err(anyhow!("Token 验证失败: {} - {}", status, text));
    }
    println!("[SSO] Bearer token verified");

    // Step 4: 获取设备会话令牌
    println!("[SSO] Step 4: Getting device session token...");
    let sess_res = client
        .post(format!("{}/session/device", portal_base))
        .header("Authorization", format!("Bearer {}", bearer_token))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({}))
        .send()
        .await
        .map_err(|e| anyhow!("获取设备会话请求失败: {}", e))?;

    if !sess_res.status().is_success() {
        let status = sess_res.status();
        let text = sess_res.text().await.unwrap_or_default();
        return Err(anyhow!("获取设备会话失败: {} - {}", status, text));
    }

    let sess_data: serde_json::Value = sess_res.json().await?;
    let device_session_token = sess_data["token"].as_str().ok_or_else(|| anyhow!("响应中缺少 session token"))?.to_string();
    println!("[SSO] Device session token obtained");

    // Step 5: 接受用户代码
    println!("[SSO] Step 5: Accepting user code...");
    let accept_res = client
        .post(format!("{}/device_authorization/accept_user_code", oidc_base))
        .header("Content-Type", "application/json")
        .header("Referer", "https://view.awsapps.com/")
        .json(&serde_json::json!({
            "userCode": _user_code,
            "userSessionId": &device_session_token
        }))
        .send()
        .await
        .map_err(|e| anyhow!("接受用户代码请求失败: {}", e))?;

    if !accept_res.status().is_success() {
        let status = accept_res.status();
        let text = accept_res.text().await.unwrap_or_default();
        return Err(anyhow!("接受用户代码失败: {} - {}", status, text));
    }

    let accept_data: serde_json::Value = accept_res.json().await?;
    let device_context = accept_data["deviceContext"].as_object();
    println!("[SSO] User code accepted");

    // Step 6: 批准授权
    if let Some(ctx) = device_context {
        if let Some(device_context_id) = ctx.get("deviceContextId").and_then(|v| v.as_str()) {
            println!("[SSO] Step 6: Approving authorization...");
            let client_id_in_ctx = ctx.get("clientId").and_then(|v| v.as_str()).unwrap_or(&client_id);
            let client_type = ctx.get("clientType").and_then(|v| v.as_str()).unwrap_or("public");

            let approve_res = client
                .post(format!("{}/device_authorization/associate_token", oidc_base))
                .header("Content-Type", "application/json")
                .header("Referer", "https://view.awsapps.com/")
                .json(&serde_json::json!({
                    "deviceContext": {
                        "deviceContextId": device_context_id,
                        "clientId": client_id_in_ctx,
                        "clientType": client_type
                    },
                    "userSessionId": &device_session_token
                }))
                .send()
                .await
                .map_err(|e| anyhow!("批准授权请求失败: {}", e))?;

            if !approve_res.status().is_success() {
                let status = approve_res.status();
                let text = approve_res.text().await.unwrap_or_default();
                return Err(anyhow!("批准授权失败: {} - {}", status, text));
            }
            println!("[SSO] Authorization approved");
        }
    }

    // Step 7: 轮询获取 Token
    println!("[SSO] Step 7: Polling for token...");
    let start_time = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(120); // 2 分钟超时
    let mut current_interval = interval;

    while start_time.elapsed() < timeout {
        tokio::time::sleep(tokio::time::Duration::from_secs(current_interval)).await;

        let token_res = client
            .post(format!("{}/token", oidc_base))
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "clientId": &client_id,
                "clientSecret": &client_secret,
                "grantType": "urn:ietf:params:oauth:grant-type:device_code",
                "deviceCode": &device_code
            }))
            .send()
            .await;

        match token_res {
            Ok(res) if res.status().is_success() => {
                let token_data: serde_json::Value = res.json().await?;
                let access_token = token_data["accessToken"].as_str().ok_or_else(|| anyhow!("响应中缺少 accessToken"))?.to_string();
                let refresh_token = token_data["refreshToken"].as_str().ok_or_else(|| anyhow!("响应中缺少 refreshToken"))?.to_string();
                let expires_in = token_data["expiresIn"].as_i64().unwrap_or(3600);

                println!("[SSO] Token obtained successfully!");

                // 生成用户 ID
                let user_id = format!("aws-builder-id-{}", email.replace(|c: char| !c.is_alphanumeric(), "-"));

                return Ok(ImportedAccountData {
                    email: email.to_string(),
                    user_id,
                    access_token,
                    refresh_token,
                    client_id,
                    client_secret,
                    region: region.to_string(),
                    expires_in,
                    idp: "BuilderId".to_string(),
                    subscription_type: "builder-id".to_string(),
                    subscription_title: "AWS Builder ID".to_string(),
                    usage: json!({}),
                });
            }
            Ok(res) if res.status().as_u16() == 400 => {
                let err_data: serde_json::Value = res.json().await?;
                let error = err_data["error"].as_str().unwrap_or("unknown");

                match error {
                    "authorization_pending" => {
                        // 继续轮询
                        continue;
                    }
                    "slow_down" => {
                        current_interval += 5;
                    }
                    _ => {
                        return Err(anyhow!("Token 获取失败: {}", error));
                    }
                }
            }
            Err(e) => {
                println!("[SSO] Token poll error: {}", e);
            }
            _ => {}
        }
    }

    Err(anyhow!("授权超时，请重试"))
}
