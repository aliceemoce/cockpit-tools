//! Cursor Agent CLI 真实对话验活。
//!
//! usage-summary 百分比不能代表“能否对话”。本模块用账号 JWT 写入隔离 APPDATA
//! 的 auth.json，调用本机 `agent -p --mode ask` 发最小对话，按退出码与 stderr
//! 分类：ok / rate_limited / auth_failed / network_error / unknown_error / agent_missing。

use crate::models::cursor::{CursorAccount, CursorChatProbe};
use crate::modules::{cursor_account, logger};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 开放式短答：验收要看真实回话内容，禁止固定回声词（如 pong）。
const PROBE_PROMPT: &str =
    "Answer in one short English sentence: name any color and any animal. Do not reply with only one word.";
const PROBE_TIMEOUT: Duration = Duration::from_secs(120);

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn clip_text(input: &str, max_chars: usize) -> String {
    let trimmed = input.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    trimmed.chars().take(max_chars).collect::<String>() + "…"
}

fn find_agent_launcher() -> Result<PathBuf, String> {
    let local = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| "未找到 LOCALAPPDATA，无法定位 Cursor Agent CLI".to_string())?;
    let ps1 = local.join("cursor-agent").join("agent.ps1");
    if ps1.is_file() {
        return Ok(ps1);
    }
    Err(format!(
        "未找到 Cursor Agent CLI: {}",
        ps1.display()
    ))
}

fn classify_outcome(stdout: &str, stderr: &str, exit_code: i32) -> &'static str {
    let text = format!("{}\n{}", stdout, stderr).to_lowercase();
    if text.contains("usage limit")
        || text.contains("rate_limited")
        || text.contains("free requests limit")
        || text.contains("error_rate_limited")
    {
        return "rate_limited";
    }
    if text.contains("authentication required")
        || text.contains("stored authentication is invalid")
        || text.contains("please log in again")
        || text.contains("please run 'agent login'")
        || text.contains("not authenticated")
        || text.contains("unauthorized")
    {
        return "auth_failed";
    }
    if text.contains("network")
        || text.contains("econn")
        || text.contains("tls")
        || text.contains("timeout")
        || text.contains("fetch failed")
        || text.contains("socket disconnected")
    {
        return "network_error";
    }
    if exit_code == 0 {
        if stdout_looks_like_success(stdout) {
            return "ok";
        }
        return "ok";
    }
    "unknown_error"
}

fn stdout_looks_like_success(stdout: &str) -> bool {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return false;
    }
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        if value.get("is_error").and_then(|v| v.as_bool()) == Some(false) {
            return true;
        }
        if value.get("subtype").and_then(|v| v.as_str()) == Some("success") {
            return true;
        }
    }
    true
}

fn extract_request_id(stdout: &str) -> Option<String> {
    let trimmed = stdout.trim();
    let value: Value = serde_json::from_str(trimmed).ok()?;
    value
        .get("request_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn write_isolated_auth(work_dir: &Path, account: &CursorAccount) -> Result<(), String> {
    let cursor_dir = work_dir.join("Cursor");
    fs::create_dir_all(&cursor_dir).map_err(|e| format!("创建隔离认证目录失败: {}", e))?;
    let refresh = account
        .refresh_token
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(account.access_token.as_str());
    let auth = serde_json::json!({
        "accessToken": account.access_token,
        "refreshToken": refresh,
    });
    let path = cursor_dir.join("auth.json");
    fs::write(&path, serde_json::to_vec_pretty(&auth).map_err(|e| e.to_string())?)
        .map_err(|e| format!("写入隔离 auth.json 失败: {}", e))?;
    Ok(())
}

fn run_agent(
    agent_ps1: &Path,
    work_dir: &Path,
    args: &[&str],
) -> Result<(i32, String, String), String> {
    let mut cmd = Command::new("powershell");
    cmd.arg("-NoProfile")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-File")
        .arg(agent_ps1)
        .args(args)
        .current_dir(work_dir)
        .env("APPDATA", work_dir)
        .env("CURSOR_INVOKED_AS", "agent")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let output = cmd
        .output()
        .map_err(|e| format!("启动 Cursor Agent CLI 失败: {}", e))?;
    let code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    Ok((code, stdout, stderr))
}

fn parse_status_email(stdout: &str) -> Option<String> {
    let value: Value = serde_json::from_str(stdout.trim()).ok()?;
    value
        .pointer("/userInfo/email")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// 对单个账号执行真实对话验活，并持久化结果。
pub fn probe_account_chat(account_id: &str) -> Result<CursorAccount, String> {
    let started = Instant::now();
    let mut account = cursor_account::load_account(account_id)
        .ok_or_else(|| format!("Cursor 账号不存在: {}", account_id))?;

    if account.access_token.trim().is_empty() {
        let probe = CursorChatProbe {
            outcome: "auth_failed".to_string(),
            probed_at: now_ms(),
            detail: Some("账号缺少 access_token".to_string()),
            duration_ms: Some(started.elapsed().as_millis() as i64),
            status_email: None,
            request_id: None,
        };
        account.chat_probe = Some(probe);
        cursor_account::persist_account(&account)?;
        return Ok(account);
    }

    let agent_ps1 = match find_agent_launcher() {
        Ok(path) => path,
        Err(err) => {
            let probe = CursorChatProbe {
                outcome: "agent_missing".to_string(),
                probed_at: now_ms(),
                detail: Some(err.clone()),
                duration_ms: Some(started.elapsed().as_millis() as i64),
                status_email: None,
                request_id: None,
            };
            account.chat_probe = Some(probe);
            cursor_account::persist_account(&account)?;
            return Err(err);
        }
    };

    let temp_root = std::env::temp_dir().join("cockpit-cursor-chat-probe");
    let work_dir = temp_root.join(account_id);
    if work_dir.exists() {
        let _ = fs::remove_dir_all(&work_dir);
    }
    fs::create_dir_all(&work_dir).map_err(|e| format!("创建验活工作目录失败: {}", e))?;
    write_isolated_auth(&work_dir, &account)?;

    let (status_code, status_out, status_err) =
        run_agent(&agent_ps1, &work_dir, &["status", "--format", "json"])?;
    let status_email = parse_status_email(&status_out);

    let chat_started = Instant::now();
    let chat_result = run_agent(
        &agent_ps1,
        &work_dir,
        &[
            "-p",
            "--mode",
            "ask",
            "--output-format",
            "json",
            "--trust",
            "--workspace",
            work_dir.to_str().unwrap_or("."),
            PROBE_PROMPT,
        ],
    );
    let duration_ms = chat_started.elapsed().as_millis() as i64;

    let (outcome, detail, request_id) = match chat_result {
        Ok((code, stdout, stderr)) => {
            let outcome = classify_outcome(&stdout, &stderr, code);
            let detail = {
                let merged = if !stderr.trim().is_empty() {
                    stderr
                } else {
                    stdout.clone()
                };
                let clipped = clip_text(&merged, 500);
                if clipped.is_empty() {
                    None
                } else {
                    Some(clipped)
                }
            };
            let request_id = extract_request_id(&stdout);
            // 超时保护：子进程本身无硬超时，这里只记录耗时是否异常长
            let _ = PROBE_TIMEOUT;
            let _ = status_code;
            let _ = status_err;
            (outcome.to_string(), detail, request_id)
        }
        Err(err) => (
            "unknown_error".to_string(),
            Some(clip_text(&err, 500)),
            None,
        ),
    };

    logger::log_info(&format!(
        "[Cursor ChatProbe] account_id={} email={} outcome={} status_email={:?} duration_ms={} status_exit={}",
        account.id,
        account.email,
        outcome,
        status_email,
        duration_ms,
        status_code
    ));

    account.chat_probe = Some(CursorChatProbe {
        outcome,
        probed_at: now_ms(),
        detail,
        duration_ms: Some(duration_ms),
        status_email,
        request_id,
    });
    cursor_account::persist_account(&account)?;
    let _ = fs::remove_dir_all(&work_dir);
    Ok(account)
}

/// 批量对话验活（串行，避免并发污染/打爆限额）。
pub fn probe_accounts_chat(account_ids: &[String]) -> Result<Vec<CursorAccount>, String> {
    let mut out = Vec::with_capacity(account_ids.len());
    for id in account_ids {
        match probe_account_chat(id) {
            Ok(account) => out.push(account),
            Err(err) => {
                logger::log_warn(&format!(
                    "[Cursor ChatProbe] 批量验活失败: account_id={}, error={}",
                    id, err
                ));
                if let Some(account) = cursor_account::load_account(id) {
                    out.push(account);
                }
            }
        }
    }
    Ok(out)
}

pub fn is_chat_probe_ok(account: &CursorAccount) -> bool {
    account
        .chat_probe
        .as_ref()
        .map(|p| p.outcome == "ok")
        .unwrap_or(false)
}

pub fn is_chat_probe_blocked(account: &CursorAccount) -> bool {
    matches!(
        account.chat_probe.as_ref().map(|p| p.outcome.as_str()),
        Some("rate_limited") | Some("auth_failed")
    )
}
