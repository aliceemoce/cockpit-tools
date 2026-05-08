use tauri::{AppHandle, Emitter, Manager};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, LazyLock, Mutex};

/// 全局运行的子进程 ID（用于停止注册）
static RUNNING_PROCESS_ID: LazyLock<Arc<Mutex<Option<u32>>>> =
    LazyLock::new(|| Arc::new(Mutex::new(None)));

/// 设置当前运行的子进程 ID
fn set_running_process_id(pid: u32) {
    let mut guard = RUNNING_PROCESS_ID.lock().unwrap();
    *guard = Some(pid);
}

/// 清除当前运行的子进程 ID
fn clear_running_process_id() {
    let mut guard = RUNNING_PROCESS_ID.lock().unwrap();
    *guard = None;
}

/// 终止当前运行的注册进程
pub fn stop_running_process() -> bool {
    let guard = RUNNING_PROCESS_ID.lock().unwrap();
    if let Some(pid) = *guard {
        // 使用系统命令终止进程
        #[cfg(target_os = "windows")]
        let _ = std::process::Command::new("taskkill")
            .args(&["/F", "/PID", &pid.to_string()])
            .output();
        #[cfg(not(target_os = "windows"))]
        let _ = std::process::Command::new("kill")
            .args(&["-9", &pid.to_string()])
            .output();
        return true;
    }
    false
}

/// 停止自动注册命令
#[tauri::command]
pub fn stop_auto_register() -> bool {
    stop_running_process()
}

/// 自动注册参数
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoRegisterKiroParams {
    pub email: String,
    pub email_password: String,
    pub first_name: String,
    pub last_name: String,
    pub proxy_url: Option<String>,
}

/// 自动注册结果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoRegisterResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sso_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 从 SSO Token 导入账号的参数
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFromSsoTokenParams {
    pub bearer_token: String,
    pub region: String,
    pub email: Option<String>,
    pub name: Option<String>,
}

/// 导入结果中的账号数据
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedAccountData {
    pub email: String,
    pub user_id: String,
    pub access_token: String,
    pub refresh_token: String,
    pub client_id: String,
    pub client_secret: String,
    pub region: String,
    pub expires_in: i64,
    pub idp: String,
    pub subscription_type: String,
    pub subscription_title: String,
    pub usage: serde_json::Value,
}

/// 从 SSO Token 导入的结果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFromSsoTokenResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<ImportedAccountData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Kiro 自动注册命令
/// 使用 Node.js + Playwright 脚本完成 AWS Builder ID 注册流程
#[tauri::command]
pub async fn auto_register_kiro(
    app: AppHandle,
    params: AutoRegisterKiroParams,
) -> Result<AutoRegisterResult, String> {
    use std::path::PathBuf;
    use std::process::Stdio;
    use tokio::process::Command;
    

    // Clone app for the closure (AppHandle is cheap to clone)
    let app_for_log = app.clone();

    // 发送日志回调函数
    let send_log = move |email: &str, message: &str| {
        let _ = app_for_log.emit("auto-register-log", serde_json::json!({
            "email": email,
            "message": message,
        }));
    };
    
    send_log(&params.email, "启动 Node.js 自动注册脚本...");
    
    // 查找 Node.js 脚本路径（优先从 Tauri 资源目录查找，但需确保有 node_modules）
    let script_name = "auto_register_kiro.cjs";
    let script_path: PathBuf = if let Ok(resource_dir) = app.path().resource_dir() {
        let resource_script = resource_dir.join("scripts").join(script_name);
        let resource_node_modules = resource_dir.join("scripts/node_modules");
        // 资源目录的脚本必须配套 node_modules 才使用
        if resource_script.exists() && resource_node_modules.exists() {
            send_log(&params.email, "使用资源目录脚本");
            resource_script
        } else if resource_script.exists() {
            send_log(&params.email, "资源目录脚本缺少 node_modules，回退到开发路径");
            // 回退到开发环境路径
            let dev_paths: Vec<PathBuf> = vec![
                PathBuf::from("scripts").join(script_name),
                PathBuf::from("..").join("scripts").join(script_name),
                PathBuf::from("..").join("..").join("scripts").join(script_name),
            ];
            dev_paths.into_iter()
                .find(|p| p.exists())
                .ok_or_else(|| {
                    let err = "未找到带 node_modules 的 auto_register_kiro.cjs 脚本".to_string();
                    send_log(&params.email, &err);
                    err
                })?
        } else {
            // 回退到开发环境路径
            let dev_paths: Vec<PathBuf> = vec![
                PathBuf::from("scripts").join(script_name),
                PathBuf::from("..").join("scripts").join(script_name),
                PathBuf::from("..").join("..").join("scripts").join(script_name),
            ];
            dev_paths.into_iter()
                .find(|p| p.exists())
                .ok_or_else(|| {
                    let err = "未找到 auto_register_kiro.cjs 脚本".to_string();
                    send_log(&params.email, &err);
                    err
                })?
        }
    } else {
        // 回退到开发环境路径
        let dev_paths: Vec<PathBuf> = vec![
            PathBuf::from("scripts").join(script_name),
            PathBuf::from("..").join("scripts").join(script_name),
            PathBuf::from("..").join("..").join("scripts").join(script_name),
        ];
        dev_paths.into_iter()
            .find(|p| p.exists())
            .ok_or_else(|| {
                let err = "未找到 auto_register_kiro.cjs 脚本".to_string();
                send_log(&params.email, &err);
                err
            })?
    };
    
    send_log(&params.email, &format!("脚本路径: {:?}", script_path));
    
    // 尝试检测可用的 Playwright 环境
    let scripts_dir = script_path.parent().unwrap().to_path_buf();
    
    // 方法1: 检查本地 node_modules
    let node_modules_exists = scripts_dir.join("node_modules").exists();
    // 方法2: 检查全局 playwright
    let global_check = Command::new("npx")
        .args(&["playwright", "--version"])
        .output()
        .await;
    let has_global_playwright = global_check.is_ok() && global_check.unwrap().status.success();
    
    if node_modules_exists {
        send_log(&params.email, "使用本地 Playwright");
    } else if has_global_playwright {
        send_log(&params.email, "使用全局 Playwright");
    } else {
        send_log(&params.email, "警告: 未检测到 Playwright，尝试本地安装...");
        // 静默尝试安装
        let scripts_dir_clone = scripts_dir.clone();
        let _ = tokio::spawn(async move {
            let _ = Command::new("npm")
                .args(&["install", "--silent"])
                .current_dir(&scripts_dir_clone)
                .output()
                .await;
            let _ = Command::new("npx")
                .args(&["playwright", "install", "chromium"])
                .current_dir(&scripts_dir_clone)
                .output()
                .await;
        }).await;
    }
    
    // 构建命令参数
    let mut args = vec![
        params.email.clone(),
        params.email_password.clone(),
        params.first_name.clone(),
        params.last_name.clone(),
    ];
    
    // 添加代理 URL（如果有）
    if let Some(proxy) = &params.proxy_url {
        args.push(proxy.clone());
    }
    
    send_log(&params.email, "启动 Playwright 浏览器...");
    
    // 将 UNC 路径转换为标准路径（解决 Windows UNC 路径问题）
    let script_path_str = dunce::simplified(&script_path).to_string_lossy().to_string();
    send_log(&params.email, &format!("标准化脚本路径: {}", script_path_str));
    
    // 执行 Node.js 脚本
    let mut child = Command::new("node")
        .arg(&script_path_str)
        .args(&args)
        .current_dir(&scripts_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            let err = format!("启动 Node.js 进程失败: {}", e);
            send_log(&params.email, &err);
            err
        })?;
    
    // 保存子进程 ID 以便可以终止
    if let Some(pid) = child.id() {
        set_running_process_id(pid);
    }
    
    // 获取 stderr 用于实时日志流
    let stderr = child.stderr.take().ok_or("无法获取 stderr")?;
    let email_clone = params.email.clone();
    let app_clone = app.clone();
    
    // 启动异步任务读取日志
    let log_task = tokio::spawn(async move {
        use tokio::io::{BufReader, AsyncBufReadExt};
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        
        while let Ok(Some(line)) = lines.next_line().await {
            if line.starts_with("[LOG] ") {
                let message = line.trim_start_matches("[LOG] ");
                let _ = app_clone.emit("auto-register-log", serde_json::json!({
                    "email": &email_clone,
                    "message": message,
                }));
            }
        }
    });
    
    // 获取 stdout 用于读取结果
    let stdout = child.stdout.take().ok_or("无法获取 stdout")?;
    let stdout_task = tokio::spawn(async move {
        use tokio::io::{BufReader, AsyncBufReadExt};
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        let mut output = String::new();
        
        while let Ok(Some(line)) = lines.next_line().await {
            output.push_str(&line);
            output.push('\n');
        }
        output
    });
    
    // 等待进程完成
    let status = child.wait().await
        .map_err(|e| format!("等待 Node.js 进程失败: {}", e))?;
    
    // 等待日志任务完成
    let _ = log_task.await;
    
    // 获取 stdout 输出
    let stdout_output = stdout_task.await
        .map_err(|e| format!("读取 stdout 失败: {}", e))?;
    
    if !status.success() {
        clear_running_process_id();
        return Ok(AutoRegisterResult {
            success: false,
            sso_token: None,
            name: None,
            error: Some(format!("脚本执行失败，退出码: {:?}", status.code())),
        });
    }
    
    // 解析 stdout 中的 JSON 结果
    // 只取最后一行非空内容作为 JSON（避免混入其他输出）
    let json_line: String = stdout_output.lines()
        .filter(|line| !line.trim().is_empty())
        .last()
        .map(|s| s.to_string())
        .unwrap_or_else(|| stdout_output.clone());
    
    send_log(&params.email, &format!("原始输出: {}", stdout_output.trim()));
    send_log(&params.email, &format!("解析 JSON: {}", json_line));
    
    let result: serde_json::Value = serde_json::from_str(&json_line)
        .map_err(|e| format!("解析脚本输出失败: {} - 原始输出: {}", e, stdout_output.trim()))?;
    
    let success = result["success"].as_bool().unwrap_or(false);
    let sso_token = result["ssoToken"].as_str().map(|s| s.to_string());
    let name = result["name"].as_str().map(|s| s.to_string());
    let error = result["error"].as_str().map(|s| s.to_string());
    
    send_log(&params.email, &format!("解析结果: success={}, has_token={}, name={:?}", success, sso_token.is_some(), name));
    
    // 清除进程 ID
    clear_running_process_id();
    
    Ok(AutoRegisterResult {
        success,
        sso_token,
        name,
        error,
    })
}

/// 从 SSO Token 导入 Kiro 账号
/// 使用 SSO Token 创建账号记录并保存到系统中
#[tauri::command]
pub async fn import_from_sso_token(
    params: ImportFromSsoTokenParams,
) -> Result<ImportFromSsoTokenResult, String> {
    use crate::modules::auto_register_kiro;
    use crate::modules::kiro_account;
    use crate::models::kiro::KiroOAuthCompletePayload;
    use serde_json::json;
    
    // 提供默认值
    let email = params.email.unwrap_or_else(|| "unknown@example.com".to_string());
    let name = params.name.unwrap_or_default();
    
    match auto_register_kiro::exchange_sso_token(&params.bearer_token, &email, &name).await {
        Ok(account_data) => {
            // 构建 KiroOAuthCompletePayload 并保存账号
            let auth_token = json!({
                "accessToken": account_data.access_token,
                "refreshToken": account_data.refresh_token,
                "clientId": account_data.client_id,
                "clientSecret": account_data.client_secret,
                "region": account_data.region,
                "expiresIn": account_data.expires_in,
                "idp": account_data.idp,
            });
            
            let profile = json!({
                "email": account_data.email,
                "userId": account_data.user_id,
                "name": name,
            });
            
            let payload = KiroOAuthCompletePayload {
                email: account_data.email.clone(),
                user_id: Some(account_data.user_id.clone()),
                login_provider: Some("AWS".to_string()),
                access_token: account_data.access_token.clone(),
                refresh_token: Some(account_data.refresh_token.clone()),
                token_type: Some("Bearer".to_string()),
                expires_at: None,
                idc_region: Some(account_data.region.clone()),
                issuer_url: None,
                client_id: Some(account_data.client_id.clone()),
                scopes: None,
                login_hint: None,
                plan_name: None,
                plan_tier: None,
                credits_total: None,
                credits_used: None,
                bonus_total: None,
                bonus_used: None,
                usage_reset_at: None,
                bonus_expire_days: None,
                kiro_auth_token_raw: Some(auth_token),
                kiro_profile_raw: Some(profile),
                kiro_usage_raw: Some(account_data.usage.clone()),
                status: None,
                status_reason: None,
            };
            
            // 保存账号到系统
            match kiro_account::upsert_account(payload) {
                Ok(saved_account) => {
                    Ok(ImportFromSsoTokenResult {
                        success: true,
                        data: Some(account_data),
                        error: None,
                    })
                }
                Err(e) => {
                    Ok(ImportFromSsoTokenResult {
                        success: false,
                        data: Some(account_data),
                        error: Some(format!("保存账号失败: {}", e)),
                    })
                }
            }
        }
        Err(e) => Ok(ImportFromSsoTokenResult {
            success: false,
            data: None,
            error: Some(e.to_string()),
        }),
    }
}

/// 提交验证码参数
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitVerificationCodeParams {
    pub email: String,
    pub code: String,
}

/// 提交验证码结果
#[derive(Debug, Clone, Serialize)]
pub struct SubmitVerificationCodeResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 提交验证码命令
/// 将验证码写入临时文件，供 Node.js 脚本读取
#[tauri::command]
pub async fn submit_verification_code(
    params: SubmitVerificationCodeParams,
) -> Result<SubmitVerificationCodeResult, String> {
    use std::fs;
    
    // 构建临时文件路径（与 Node.js 脚本一致）
    let temp_dir = std::env::temp_dir();
    let safe_email = params.email.replace(|c: char| !c.is_ascii_alphanumeric(), "_");
    let code_file_path = temp_dir.join(format!("aws_verification_code_{}.txt", safe_email));
    
    // 写入验证码
    match fs::write(&code_file_path, &params.code) {
        Ok(_) => Ok(SubmitVerificationCodeResult {
            success: true,
            error: None,
        }),
        Err(e) => Err(format!("写入验证码文件失败: {}", e)),
    }
}

/// Windsurf 自动注册参数
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoRegisterWindsurfParams {
    pub proxy_url: Option<String>,
    pub email: Option<String>,
    pub name: Option<String>,
    pub browser_path: Option<String>,
}

/// Windsurf 自动注册结果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoRegisterWindsurfResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<WindsurfImportedAccountData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Windsurf 导入的账号数据
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindsurfImportedAccountData {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub email: String,
    pub name: String,
    pub login_provider: String,
}

/// Windsurf 自动注册命令
/// 启动浏览器进行 OAuth 授权流程
#[tauri::command]
pub async fn auto_register_windsurf(
    params: AutoRegisterWindsurfParams,
    app: tauri::AppHandle,
) -> Result<AutoRegisterWindsurfResult, String> {
    use crate::modules::auto_register_windsurf;
    use crate::modules::windsurf_account;
    
    tracing::info!("开始 Windsurf 自动注册流程");
    
    // 运行 Windsurf OAuth 授权
    match auto_register_windsurf::run_windsurf_oauth(params.proxy_url, params.email.clone(), params.name.clone(), params.browser_path.clone(), &app).await {
        Ok(result) => {
            if result.success {
                tracing::info!("Windsurf OAuth 授权成功，开始导入账号");
                
                let access_token = result.access_token.clone().unwrap_or_default();
                let email = params.email.unwrap_or_else(|| "unknown@windsurf.ai".to_string());
                let name = params.name.unwrap_or_else(|| "Windsurf User".to_string());
                
                // 构建账号数据
                let account_data = WindsurfImportedAccountData {
                    access_token: access_token.clone(),
                    token_type: result.token_type.clone().unwrap_or_else(|| "Bearer".to_string()),
                    expires_in: result.expires_in.unwrap_or(3600),
                    email: email.clone(),
                    name: name.clone(),
                    login_provider: "windsurf".to_string(),
                };
                
                // 构建 JSON 并导入账号
                let account_json = serde_json::json!({
                    "email": email,
                    "githubEmail": email,
                    "githubLogin": name,
                    "githubAvatarUrl": null,
                    "accessToken": access_token,
                    "refreshToken": null,
                    "tokenType": "Bearer",
                    "expiresAt": null,
                    "scope": null,
                    "extraRaw": {
                        "autoRegistered": true,
                        "name": name,
                        "importedAt": chrono::Utc::now().to_rfc3339()
                    },
                    "createdAt": chrono::Utc::now().timestamp(),
                    "updatedAt": chrono::Utc::now().timestamp()
                });
                
                // 保存账号到系统
                match windsurf_account::import_from_json(&account_json.to_string()) {
                    Ok(accounts) => {
                        if let Some(account) = accounts.first() {
                            tracing::info!("Windsurf 账号保存成功: {:?}", account.github_email);
                        }
                        Ok(AutoRegisterWindsurfResult {
                            success: true,
                            data: Some(account_data),
                            error: None,
                        })
                    }
                    Err(e) => {
                        tracing::error!("保存 Windsurf 账号失败: {}", e);
                        Ok(AutoRegisterWindsurfResult {
                            success: false,
                            data: Some(account_data),
                            error: Some(format!("保存账号失败: {}", e)),
                        })
                    }
                }
            } else {
                tracing::error!("Windsurf OAuth 授权失败: {:?}", result.error);
                Ok(AutoRegisterWindsurfResult {
                    success: false,
                    data: None,
                    error: result.error,
                })
            }
        }
        Err(e) => {
            tracing::error!("Windsurf 自动注册出错: {}", e);
            Ok(AutoRegisterWindsurfResult {
                success: false,
                data: None,
                error: Some(e.to_string()),
            })
        }
    }
}
