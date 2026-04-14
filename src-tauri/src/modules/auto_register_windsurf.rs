//! Windsurf 自动注册模块
//! 提供 Windsurf OAuth 自动授权功能

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::process::Stdio;
use tauri::{Manager, Emitter};

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
        "auto_register_windsurf_stealth.js"
    } else {
        "auto_register_windsurf.js"
    };
    
    // 查找 Node.js 脚本路径（从多个可能位置查找）
    let script_paths: Vec<PathBuf> = vec![
        PathBuf::from(format!("scripts/{}", script_name)),
        PathBuf::from(format!("../scripts/{}", script_name)),
        PathBuf::from(format!("../../scripts/{}", script_name)),
    ];

    let script_path: PathBuf = script_paths.into_iter()
        .find(|p| p.exists())
        .ok_or_else(|| {
            let err = format!("未找到 {} 脚本", script_name);
            log_error(&err);
            anyhow!(err)
        })?;

    log_info(&format!("脚本路径: {:?}", script_path));

    let scripts_dir = script_path.parent().unwrap().to_path_buf();

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

    // 执行 Node.js 脚本
    let mut child = tokio::process::Command::new("node")
        .arg(&script_path)
        .args(&args)
        .current_dir(&scripts_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow!("启动 Node.js 进程失败: {}", e))?;

    // 获取 stderr 用于实时日志流
    let stderr = child.stderr.take().ok_or_else(|| anyhow!("无法获取 stderr"))?;
    let app_clone = app.clone();

    // 启动异步任务读取日志
    let log_task = tokio::spawn(async move {
        use tokio::io::{AsyncBufReadExt, BufReader};
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
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
    let (stdout_result, _) = tokio::join!(stdout_task, log_task);
    let output = stdout_result.map_err(|e| anyhow!("读取 stdout 失败: {}", e))?;

    // 等待进程结束
    let status = child
        .wait()
        .await
        .map_err(|e| anyhow!("等待进程结束失败: {}", e))?;

    log_info(&format!("Node.js 进程退出码: {:?}", status.code()));

    // 解析结果
    if output.trim().is_empty() {
        return Err(anyhow!("脚本输出为空"));
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
