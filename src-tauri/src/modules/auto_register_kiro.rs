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

/// 使用 SSO Token 创建账号
/// AWS Builder ID 的 SSO Token 需要经过 Kiro OAuth 流程才能使用
/// 这里我们直接创建账号，token 将在首次使用时由 Kiro 客户端处理
pub async fn exchange_sso_token(bearer_token: &str, email: &str, name: &str) -> Result<ImportedAccountData> {
    // 生成用户 ID (使用 email 的哈希)
    let user_id = format!("aws-builder-id-{}", email.replace(|c: char| !c.is_alphanumeric(), "-"));
    
    // 注意：AWS SSO Token 不能直接用于 Kiro API
    // 我们将其作为初始 token 存储，Kiro 客户端会在首次使用时处理 OAuth 流程
    Ok(ImportedAccountData {
        email: email.to_string(),
        user_id,
        access_token: bearer_token.to_string(),
        refresh_token: String::new(),
        client_id: String::new(),
        client_secret: String::new(),
        region: "us-east-1".to_string(),
        expires_in: 3600,
        idp: "AWS".to_string(),
        subscription_type: "builder-id".to_string(),
        subscription_title: "AWS Builder ID".to_string(),
        usage: json!({}),
    })
}
