//! UI 状态导出命令模块
//!
//! 提供 `get_ui_state` Tauri command，返回当前应用的完整状态 JSON，
//! 包括账号列表、当前活跃账号、配额缓存、活跃实例等信息。
//! 供外部自动化验收和 CLI `cockpit ui-state` 使用。

use tauri::AppHandle;

/// 获取当前 UI 状态的结构化 JSON。
///
/// 返回内容包括：
/// - `timestamp`: 当前时间戳
/// - `cursor_accounts`: Cursor 账号列表摘要
/// - `cursor_current_id`: 当前活跃 Cursor 账号 ID
/// - `qoder_accounts`: Qoder 账号列表摘要
/// - `trae_accounts`: Trae 账号列表摘要
/// - `codex_accounts`: Codex 账号列表摘要
/// - `windsurf_accounts`: Windsurf 账号列表摘要
/// - `kiro_accounts`: Kiro 账号列表摘要
/// - `github_copilot_accounts`: GitHub Copilot 账号列表摘要
/// - `codebuddy_accounts`: Codebuddy 账号列表摘要
/// - `codebuddy_cn_accounts`: Codebuddy CN 账号列表摘要
/// - `zed_accounts`: Zed 账号列表摘要
/// - `workbuddy_accounts`: Workbuddy 账号列表摘要
/// - `screenshot_path`: 截图保存路径（如果刚截过图）
#[tauri::command]
pub fn get_ui_state(_app: AppHandle) -> Result<serde_json::Value, String> {
    let cursor_accounts = crate::modules::cursor_account::list_accounts();
    let qoder_accounts = crate::modules::qoder_account::list_accounts();
    let trae_accounts = crate::modules::trae_account::list_accounts();
    let codex_accounts = crate::modules::codex_account::list_accounts();
    let windsurf_accounts = crate::modules::windsurf_account::list_accounts();
    let kiro_accounts = crate::modules::kiro_account::list_accounts();
    let copilot_accounts = crate::modules::github_copilot_account::list_accounts();
    let codebuddy_accounts = crate::modules::codebuddy_account::list_accounts();
    let codebuddy_cn_accounts = crate::modules::codebuddy_cn_account::list_accounts();
    let zed_accounts = crate::modules::zed_account::list_accounts();
    let workbuddy_accounts = crate::modules::workbuddy_account::list_accounts();

    let state = serde_json::json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "app_version": env!("CARGO_PKG_VERSION"),
        "platforms": {
            "cursor": {
                "account_count": cursor_accounts.len(),
                "accounts": cursor_accounts.iter().map(|a| {
                    serde_json::json!({
                        "id": a.id,
                        "email": a.email,
                        "membership_type": a.membership_type,
                        "tags": a.tags,
                    })
                }).collect::<Vec<_>>(),
            },
            "qoder": {
                "account_count": qoder_accounts.len(),
                "accounts": qoder_accounts.iter().map(|a| {
                    serde_json::json!({
                        "id": a.id,
                        "email": a.email,
                        "plan_type": a.plan_type,
                        "tags": a.tags,
                    })
                }).collect::<Vec<_>>(),
            },
            "trae": {
                "account_count": trae_accounts.len(),
                "accounts": trae_accounts.iter().map(|a| {
                    serde_json::json!({
                        "id": a.id,
                        "email": a.email,
                        "plan_type": a.plan_type,
                        "tags": a.tags,
                    })
                }).collect::<Vec<_>>(),
            },
            "codex": {
                "account_count": codex_accounts.len(),
                "accounts": codex_accounts.iter().map(|a| {
                    serde_json::json!({
                        "id": a.id,
                        "email": a.email,
                        "plan_type": a.plan_type,
                        "tags": a.tags,
                    })
                }).collect::<Vec<_>>(),
            },
            "windsurf": {
                "account_count": windsurf_accounts.len(),
                "accounts": windsurf_accounts.iter().map(|a| {
                    serde_json::json!({
                        "id": a.id,
                        "github_login": a.github_login,
                        "copilot_plan": a.copilot_plan,
                        "tags": a.tags,
                    })
                }).collect::<Vec<_>>(),
            },
            "kiro": {
                "account_count": kiro_accounts.len(),
                "accounts": kiro_accounts.iter().map(|a| {
                    serde_json::json!({
                        "id": a.id,
                        "email": a.email,
                        "plan_tier": a.plan_tier,
                        "tags": a.tags,
                    })
                }).collect::<Vec<_>>(),
            },
            "github_copilot": {
                "account_count": copilot_accounts.len(),
                "accounts": copilot_accounts.iter().map(|a| {
                    serde_json::json!({
                        "id": a.id,
                        "github_login": a.github_login,
                        "copilot_plan": a.copilot_plan,
                        "tags": a.tags,
                    })
                }).collect::<Vec<_>>(),
            },
            "codebuddy": {
                "account_count": codebuddy_accounts.len(),
                "accounts": codebuddy_accounts.iter().map(|a| {
                    serde_json::json!({
                        "id": a.id,
                        "email": a.email,
                        "plan_type": a.plan_type,
                        "tags": a.tags,
                    })
                }).collect::<Vec<_>>(),
            },
            "codebuddy_cn": {
                "account_count": codebuddy_cn_accounts.len(),
                "accounts": codebuddy_cn_accounts.iter().map(|a| {
                    serde_json::json!({
                        "id": a.id,
                        "email": a.email,
                        "plan_type": a.plan_type,
                        "tags": a.tags,
                    })
                }).collect::<Vec<_>>(),
            },
            "zed": {
                "account_count": zed_accounts.len(),
                "accounts": zed_accounts.iter().map(|a| {
                    serde_json::json!({
                        "id": a.id,
                        "github_login": a.github_login,
                        "plan_raw": a.plan_raw,
                        "tags": a.tags,
                    })
                }).collect::<Vec<_>>(),
            },
            "workbuddy": {
                "account_count": workbuddy_accounts.len(),
                "accounts": workbuddy_accounts.iter().map(|a| {
                    serde_json::json!({
                        "id": a.id,
                        "email": a.email,
                        "plan_type": a.plan_type,
                        "tags": a.tags,
                    })
                }).collect::<Vec<_>>(),
            },
        },
    });

    // 同时写入临时文件供 CLI 读取
    let path = format!(
        "{}/cockpit-ui-state.json",
        std::env::temp_dir().display()
    );
    let _ = std::fs::write(&path, serde_json::to_string_pretty(&state).unwrap_or_default());

    crate::modules::logger::log_info(&format!(
        "[UIState] 状态已导出到: {}",
        path
    ));

    Ok(state)
}

/// 将 UI 状态导出到指定文件路径
#[tauri::command]
pub fn export_ui_state(
    app: AppHandle,
    output_path: Option<String>,
) -> Result<String, String> {
    let state = get_ui_state(app)?;
    let path = output_path.unwrap_or_else(|| {
        format!(
            "{}/cockpit-ui-state.json",
            std::env::temp_dir().display()
        )
    });
    serde_json::to_string_pretty(&state)
        .map_err(|e| format!("序列化失败: {}", e))
        .and_then(|json| {
            std::fs::write(&path, json).map_err(|e| format!("写入失败: {}", e))?;
            Ok(path)
        })
}
