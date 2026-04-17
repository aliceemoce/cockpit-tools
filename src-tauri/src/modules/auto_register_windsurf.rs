//! Windsurf 自动注册模块
//! 提供 Windsurf OAuth 自动授权功能

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::process::Stdio;
use std::sync::{Arc, LazyLock, Mutex};
use tauri::{Manager, Emitter};

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

/// 终止当前运行的 Windsurf 注册进程
pub fn stop_running_process() -> bool {
    let guard = RUNNING_PROCESS_ID.lock().unwrap();
    if let Some(pid) = *guard {
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

/// Windsurf 自动注册脚本结果
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindsurfRegisterResult {
    pub success: bool,
    #[serde(rename = "accessToken")]
    pub access_token: Option<String>,
    #[serde(rename = "tokenType")]
    pub token_type: Option<String>,
    #[serde(rename = "expiresIn")]
    pub expires_in: Option<i64>,
    pub error: Option<String>,
}

/// 运行 Windsurf 自动注册脚本
pub async fn run_windsurf_oauth(
    proxy_url: Option<String>,
    email: Option<String>,
    name: Option<String>,
    browser_path: Option<String>,
    app: &tauri::AppHandle,
) -> Result<WindsurfRegisterResult> {
    use std::path::PathBuf;

    // 确定使用哪个脚本
    let use_stealth = browser_path.as_ref().map(|b| b == "stealth").unwrap_or(false);
    
    let script_name = if use_stealth {
        "auto_register_windsurf_stealth.cjs"
    } else {
        "auto_register_windsurf.cjs"
    };
    
    // 查找 Node.js 脚本路径（优先从 Tauri 资源目录查找，但需确保有 node_modules）
    let script_path: PathBuf = if let Ok(resource_dir) = app.path().resource_dir() {
        let resource_script = resource_dir.join("scripts").join(&script_name);
        let resource_node_modules = resource_dir.join("scripts/node_modules");
        // 资源目录的脚本必须配套 node_modules 才使用
        if resource_script.exists() && resource_node_modules.exists() {
            log_info("使用资源目录脚本");
            resource_script
        } else if resource_script.exists() {
            log_info("资源目录脚本缺少 node_modules，回退到开发路径");
            // 回退到开发环境路径
            let dev_paths: Vec<PathBuf> = vec![
                PathBuf::from("scripts").join(&script_name),
                PathBuf::from("..").join("scripts").join(&script_name),
                PathBuf::from("..").join("..").join("scripts").join(&script_name),
            ];
            dev_paths.into_iter()
                .find(|p| p.exists())
                .ok_or_else(|| {
                    let err = format!("未找到带 node_modules 的 {} 脚本", script_name);
                    log_error(&err);
                    anyhow!(err)
                })?
        } else {
            // 回退到开发环境路径
            let dev_paths: Vec<PathBuf> = vec![
                PathBuf::from("scripts").join(&script_name),
                PathBuf::from("..").join("scripts").join(&script_name),
                PathBuf::from("..").join("..").join("scripts").join(&script_name),
            ];
            dev_paths.into_iter()
                .find(|p| p.exists())
                .ok_or_else(|| {
                    let err = format!("未找到 {} 脚本", script_name);
                    log_error(&err);
                    anyhow!(err)
                })?
        }
    } else {
        // 回退到开发环境路径
        let dev_paths: Vec<PathBuf> = vec![
            PathBuf::from("scripts").join(&script_name),
            PathBuf::from("..").join("scripts").join(&script_name),
            PathBuf::from("..").join("..").join("scripts").join(&script_name),
        ];
        dev_paths.into_iter()
            .find(|p| p.exists())
            .ok_or_else(|| {
                let err = format!("未找到 {} 脚本", script_name);
                log_error(&err);
                anyhow!(err)
            })?
    };

    log_info(&format!("脚本路径: {:?}", script_path));

    let mut args = vec![];

    // 添加代理 URL（如果有）
    if let Some(proxy) = &proxy_url {
        args.push(proxy.clone());
    } else {
        args.push(String::new()); // 占位符
    }

    // 添加邮箱（如果有）
    if let Some(email_str) = &email {
        args.push(email_str.clone());
        
        // 解析名字，如果没有提供则使用默认值
        let (first_name, last_name) = if let Some(name_str) = &name {
            let parts: Vec<&str> = name_str.split_whitespace().collect();
            if parts.len() >= 2 {
                (parts[0].to_string(), parts[1..].join(" "))
            } else if parts.len() == 1 {
                (parts[0].to_string(), "User".to_string())
            } else {
                ("User".to_string(), "Name".to_string())
            }
        } else {
            ("User".to_string(), "Name".to_string())
        };
        
        args.push(first_name);
        args.push(last_name);
    }
    
    // 添加浏览器路径（如果是 stealth 模式，传递 'stealth' 作为标记）
    if let Some(browser) = &browser_path {
        if !browser.is_empty() && browser != "stealth" {
            args.push(browser.clone());
        } else if browser == "stealth" {
            args.push("stealth".to_string());
        }
    }

    log_info("启动 Windsurf OAuth 授权浏览器...");

    // 将 UNC 路径转换为标准路径（解决 Windows UNC 路径问题）
    let script_path_str = dunce::simplified(&script_path).to_string_lossy().to_string();
    log_info(&format!("标准化脚本路径: {}", script_path_str));

    // 执行 Node.js 脚本
    let mut child = tokio::process::Command::new("node")
        .arg(&script_path_str)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow!("启动 Node.js 进程失败: {}", e))?;

    // 保存子进程 ID 以便可以终止
    if let Some(pid) = child.id() {
        set_running_process_id(pid);
    }

    // 获取 stderr 用于实时日志流和错误收集
    let stderr = child.stderr.take().ok_or_else(|| anyhow!("无法获取 stderr"))?;
    let app_clone = app.clone();

    // 启动异步任务读取日志和错误
    let log_task = tokio::spawn(async move {
        use tokio::io::{AsyncBufReadExt, BufReader};
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        let mut stderr_output = String::new();

        while let Ok(Some(line)) = lines.next_line().await {
            stderr_output.push_str(&line);
            stderr_output.push('\n');

            // 发送 [LOG] 前缀的日志到前端
            if line.starts_with("[LOG] ") {
                let message = line.trim_start_matches("[LOG] ");
                let _ = app_clone.emit(
                    "auto-register-log",
                    json!({
                        "platform": "windsurf",
                        "message": message,
                    }),
                );
            }
        }

        stderr_output
    });

    // 获取 stdout 用于读取结果
    let stdout = child.stdout.take().ok_or_else(|| anyhow!("无法获取 stdout"))?;
    let stdout_task = tokio::spawn(async move {
        use tokio::io::{AsyncBufReadExt, BufReader};
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        let mut output = String::new();

        while let Ok(Some(line)) = lines.next_line().await {
            output.push_str(&line);
        }

        output
    });

    // 等待任务完成
    let (stdout_result, stderr_result) = tokio::join!(stdout_task, log_task);
    let output = stdout_result.map_err(|e| anyhow!("读取 stdout 失败: {}", e))?;
    let stderr_output = stderr_result.unwrap_or_default();

    // 等待进程结束
    let status = child
        .wait()
        .await
        .map_err(|e| anyhow!("等待进程结束失败: {}", e))?;

    log_info(&format!("Node.js 进程退出码: {:?}", status.code()));

    // 如果进程失败且有 stderr 输出，记录错误信息
    if !status.success() && !stderr_output.is_empty() {
        log_error(&format!("脚本 stderr 输出:\n{}", stderr_output));
    }

    // 清除进程 ID
    clear_running_process_id();

    // 解析结果
    if output.trim().is_empty() {
        if stderr_output.trim().is_empty() {
            return Err(anyhow!("脚本输出为空"));
        } else {
            return Err(anyhow!("脚本执行失败: {}", stderr_output.trim()));
        }
    }

    match serde_json::from_str::<WindsurfRegisterResult>(&output) {
        Ok(result) => {
            if result.success {
                log_info("Windsurf OAuth 授权成功");
                Ok(result)
            } else {
                Err(anyhow!(
                    "Windsurf OAuth 授权失败: {}",
                    result.error.unwrap_or_else(|| "未知错误".to_string())
                ))
            }
        }
        Err(e) => {
            log_error(&format!("解析脚本输出失败: {}, 输出: {}", e, output));
            Err(anyhow!("解析脚本输出失败: {}", e))
        }
    }
}

fn log_info(message: &str) {
    crate::modules::logger::log_info(&format!("[Windsurf Auto Register] {}", message));
}

fn log_error(message: &str) {
    crate::modules::logger::log_error(&format!("[Windsurf Auto Register] {}", message));
}
