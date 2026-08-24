use std::time::Instant;
use tauri::{AppHandle, Emitter};

use crate::models::cursor::CursorAccount;
use crate::modules::{cursor_account, cursor_oauth, logger};

fn emit_cursor_accounts_changed(app: &AppHandle, account_id: &str, reason: &str) {
    let _ = app.emit(
        "accounts:changed",
        serde_json::json!({
            "platformId": "cursor",
            "accountId": account_id,
            "reason": reason,
        }),
    );
}

#[tauri::command]
pub fn list_cursor_accounts() -> Result<Vec<CursorAccount>, String> {
    Ok(cursor_account::list_accounts())
}

#[tauri::command]
pub fn delete_cursor_account(account_id: String) -> Result<(), String> {
    cursor_account::remove_account(&account_id)
}

#[tauri::command]
pub fn delete_cursor_accounts(account_ids: Vec<String>) -> Result<(), String> {
    cursor_account::remove_accounts(&account_ids)
}

#[tauri::command]
pub async fn import_cursor_from_json(json_content: String) -> Result<Vec<CursorAccount>, String> {
    let accounts = cursor_account::import_from_json(&json_content)?;
    // JSON 导入后异步刷新每个账号的在线信息
    let mut refreshed = Vec::new();
    for account in accounts {
        match cursor_account::refresh_account_async(&account.id).await {
            Ok(result) => refreshed.push(result.account),
            Err(e) => {
                logger::log_warn(&format!(
                    "[Cursor Import] JSON导入账号在线刷新失败（保留本地字段）: id={}, error={}",
                    account.id, e
                ));
                refreshed.push(account);
            }
        }
    }
    Ok(refreshed)
}

#[tauri::command]
pub async fn import_cursor_from_local(app: AppHandle) -> Result<Vec<CursorAccount>, String> {
    match cursor_account::import_from_local()? {
        Some(mut account) => {
            // 本地导入后立即在线拉取完整信息（user_meta + stripe_profile + usage_summary）
            match cursor_account::refresh_account_async(&account.id).await {
                Ok(refreshed) => {
                    account = refreshed.account;
                    logger::log_info(&format!(
                        "[Cursor Import] 本地导入 + 在线全字段刷新成功: id={}, email={}",
                        account.id, account.email
                    ));
                }
                Err(e) => {
                    logger::log_warn(&format!(
                        "[Cursor Import] 本地导入成功但在线刷新失败（已保留本地字段）: id={}, email={}, error={}",
                        account.id, account.email, e
                    ));
                }
            }
            let _ = crate::modules::tray::update_tray_menu(&app);
            Ok(vec![account])
        }
        None => Err("未找到本地 Cursor 登录信息".to_string()),
    }
}

#[tauri::command]
pub fn export_cursor_accounts(account_ids: Vec<String>) -> Result<String, String> {
    cursor_account::export_accounts(&account_ids)
}

#[tauri::command]
pub async fn refresh_cursor_token(
    app: AppHandle,
    account_id: String,
) -> Result<CursorAccount, String> {
    let started_at = Instant::now();
    logger::log_info(&format!(
        "[Cursor Command] 手动刷新账号开始: account_id={}",
        account_id
    ));

    match cursor_account::refresh_account_fast_async(&account_id).await {
        Ok(refreshed) => {
            let account = refreshed.account;
            if refreshed.persisted {
                emit_cursor_accounts_changed(&app, &account.id, "refresh");
            }
            if let Err(e) = cursor_account::run_quota_alert_if_needed() {
                logger::log_warn(&format!("[QuotaAlert][Cursor] 预警检查失败: {}", e));
            }
            let app_for_tray = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = crate::modules::tray::update_tray_menu(&app_for_tray);
            });
            logger::log_info(&format!(
                "[Cursor Command] 刷新完成: account_id={}, email={}, elapsed={}ms, mode=fast_usage_only, persisted={}",
                account.id,
                account.email,
                started_at.elapsed().as_millis(),
                refreshed.persisted
            ));
            Ok(account)
        }
        Err(err) => {
            logger::log_warn(&format!(
                "[Cursor Command] 刷新失败: account_id={}, elapsed={}ms, error={}",
                account_id,
                started_at.elapsed().as_millis(),
                err
            ));
            Err(err)
        }
    }
}

#[tauri::command]
pub async fn refresh_all_cursor_tokens(
    app: AppHandle,
    max_count: Option<i32>,
) -> Result<i32, String> {
    let started_at = Instant::now();
    let limit = max_count.and_then(|n| if n > 0 { Some(n as usize) } else { None });
    // 自动刷新传入 max_count 时加墙钟上限，避免一轮扫完全库占死 guard。
    let max_duration = limit.map(|_| {
        std::time::Duration::from_secs(cursor_account::CURSOR_AUTO_REFRESH_MAX_DURATION_SECS)
    });
    logger::log_info(&format!(
        "[Cursor Command] 批量刷新开始: max_count={:?}, max_duration_secs={:?}",
        limit,
        max_duration.map(|d| d.as_secs())
    ));

    let results = cursor_account::refresh_tokens_stale_first(limit, max_duration).await?;

    let mut success_count = 0usize;
    let mut persisted_any = false;
    for (_id, result) in results {
        match result {
            Ok(refreshed) => {
                success_count += 1;
                if refreshed.persisted {
                    persisted_any = true;
                }
            }
            Err(_) => {}
        }
    }

    if persisted_any {
        emit_cursor_accounts_changed(&app, "", "refresh-batch-complete");
    }

    if success_count > 0 {
        if let Err(e) = cursor_account::run_quota_alert_if_needed() {
            logger::log_warn(&format!(
                "[QuotaAlert][Cursor] 全量刷新后预警检查失败: {}",
                e
            ));
        }
    }

    let app_for_tray = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = crate::modules::tray::update_tray_menu(&app_for_tray);
    });
    logger::log_info(&format!(
        "[Cursor Command] 批量刷新完成: success={}, elapsed={}ms",
        success_count,
        started_at.elapsed().as_millis()
    ));
    Ok(success_count as i32)
}

#[tauri::command]
pub fn add_cursor_account_with_token(
    app: AppHandle,
    access_token: String,
) -> Result<CursorAccount, String> {
    let email = "unknown".to_string();
    let payload = crate::models::cursor::CursorImportPayload {
        email,
        auth_id: None,
        name: None,
        access_token,
        refresh_token: None,
        membership_type: None,
        subscription_status: None,
        sign_up_type: None,
        cursor_auth_raw: None,
        cursor_usage_raw: None,
        status: None,
        status_reason: None,
    };
    let account = cursor_account::upsert_import_payload(payload)?;
    let _ = crate::modules::tray::update_tray_menu(&app);
    Ok(account)
}

#[tauri::command]
pub async fn update_cursor_account_tags(
    account_id: String,
    tags: Vec<String>,
) -> Result<CursorAccount, String> {
    cursor_account::update_account_tags(&account_id, tags)
}

#[tauri::command]
pub fn get_cursor_accounts_index_path() -> Result<String, String> {
    cursor_account::accounts_index_path_string()
}

#[tauri::command]
pub fn cursor_oauth_login_start() -> Result<cursor_oauth::CursorOAuthStartResponse, String> {
    logger::log_info("[Cursor Command] OAuth 登录开始");
    cursor_oauth::start_login()
}

#[tauri::command]
pub async fn cursor_oauth_login_complete(
    app: AppHandle,
    login_id: String,
) -> Result<CursorAccount, String> {
    logger::log_info(&format!(
        "[Cursor Command] OAuth 等待完成: login_id={}",
        login_id
    ));
    let payload = cursor_oauth::complete_login(&login_id).await?;
    let mut account = cursor_account::upsert_import_payload(payload)?;

    match cursor_account::refresh_account_async(&account.id).await {
        Ok(refreshed) => account = refreshed.account,
        Err(e) => {
            logger::log_warn(&format!("[Cursor OAuth] 登录后自动刷新配额失败: {}", e));
        }
    }

    let _ = crate::modules::tray::update_tray_menu(&app);
    logger::log_info(&format!(
        "[Cursor Command] OAuth 登录完成: account_id={}, email={}",
        account.id, account.email
    ));
    Ok(account)
}

#[tauri::command]
pub fn cursor_oauth_login_cancel(login_id: Option<String>) -> Result<(), String> {
    logger::log_info(&format!(
        "[Cursor Command] OAuth 取消: login_id={}",
        login_id.as_deref().unwrap_or("<none>")
    ));
    cursor_oauth::cancel_login(login_id.as_deref())
}

#[tauri::command]
pub async fn inject_cursor_account(app: AppHandle, account_id: String) -> Result<String, String> {
    let started_at = Instant::now();
    logger::log_info(&format!(
        "[Cursor Switch] 开始切换账号: account_id={}",
        account_id
    ));

    let account = cursor_account::load_account(&account_id)
        .ok_or_else(|| format!("Cursor account not found: {}", account_id))?;

    let launch_warning =
        match crate::commands::cursor_instance::start_cursor_instance_with_account_switch(
            "__default__".to_string(),
            Some(account_id.clone()),
        )
        .await
        {
            Ok(_) => None,
            Err(err) => {
                if err.starts_with("APP_PATH_NOT_FOUND:") || err.contains("启动 Cursor 失败") {
                    Some(err)
                } else {
                    return Err(err);
                }
            }
        };

    let _ =
        crate::modules::provider_current_state::set_current_account_id("cursor", Some(&account_id));

    let app_for_tray = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = crate::modules::tray::update_tray_menu(&app_for_tray);
    });

    if let Some(err) = launch_warning {
        logger::log_warn(&format!(
            "[Cursor Switch] 切号完成但启动失败: account_id={}, email={}, elapsed={}ms, error={}",
            account.id,
            account.email,
            started_at.elapsed().as_millis(),
            err
        ));
        if err.contains("未找到 Cursor") || err.contains("APP_PATH_NOT_FOUND") {
            let _ = app.emit(
                "app:path_missing",
                serde_json::json!({ "app": "cursor", "retry": { "kind": "default" } }),
            );
        }
        Ok(format!("切换完成，但 Cursor 启动失败: {}", err))
    } else {
        logger::log_info(&format!(
            "[Cursor Switch] 切号流程完成: account_id={}, email={}, elapsed={}ms (验收以 probe_post 日志为准)",
            account.id,
            account.email,
            started_at.elapsed().as_millis()
        ));
        Ok(format!("切换完成: {}", account.email))
    }
}

/// 默认自动换号：与多开「启动」同构（forced_account_id=None），写默认 profile。
#[tauri::command]
pub async fn inject_cursor_account_auto(app: AppHandle) -> Result<String, String> {
    let started_at = Instant::now();
    logger::log_info("[Cursor Switch] 开始默认自动换号");

    let view = match crate::commands::cursor_instance::start_cursor_instance_with_account_switch(
        "__default__".to_string(),
        None,
    )
    .await
    {
        Ok(v) => v,
        Err(err) => {
            if err.starts_with("APP_PATH_NOT_FOUND:") || err.contains("启动 Cursor 失败") {
                logger::log_warn(&format!(
                    "[Cursor Switch] 自动换号写库完成但启动失败: elapsed={}ms, error={}",
                    started_at.elapsed().as_millis(),
                    err
                ));
                if err.contains("未找到 Cursor") || err.contains("APP_PATH_NOT_FOUND") {
                    let _ = app.emit(
                        "app:path_missing",
                        serde_json::json!({ "app": "cursor", "retry": { "kind": "default" } }),
                    );
                }
                return Ok(format!("自动换号完成，但 Cursor 启动失败: {}", err));
            }
            return Err(err);
        }
    };

    let account_id = view
        .bind_account_id
        .clone()
        .or_else(|| {
            crate::modules::provider_current_state::get_current_account_id("cursor")
                .ok()
                .flatten()
        })
        .unwrap_or_default();
    if !account_id.is_empty() {
        let _ = crate::modules::provider_current_state::set_current_account_id(
            "cursor",
            Some(&account_id),
        );
    }
    let email = cursor_account::load_account(&account_id)
        .map(|a| a.email)
        .unwrap_or_else(|| account_id.clone());

    let app_for_tray = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = crate::modules::tray::update_tray_menu(&app_for_tray);
    });

    logger::log_info(&format!(
        "[Cursor Switch] 默认自动换号完成: account_id={}, email={}, elapsed={}ms",
        account_id,
        email,
        started_at.elapsed().as_millis()
    ));
    Ok(format!("自动换号完成: {}", email))
}

#[tauri::command]
pub async fn probe_cursor_account_chat(
    app: AppHandle,
    account_id: String,
) -> Result<CursorAccount, String> {
    let started_at = Instant::now();
    logger::log_info(&format!(
        "[Cursor Command] 对话验活开始: account_id={}",
        account_id
    ));
    let account = tauri::async_runtime::spawn_blocking(move || {
        crate::modules::cursor_chat_probe::probe_account_chat(&account_id)
    })
    .await
    .map_err(|e| format!("对话验活任务失败: {}", e))??;

    emit_cursor_accounts_changed(&app, &account.id, "chat_probe");
    logger::log_info(&format!(
        "[Cursor Command] 对话验活完成: account_id={}, email={}, outcome={:?}, elapsed={}ms",
        account.id,
        account.email,
        account.chat_probe.as_ref().map(|p| p.outcome.as_str()),
        started_at.elapsed().as_millis()
    ));
    Ok(account)
}

#[tauri::command]
pub async fn probe_cursor_accounts_chat(
    app: AppHandle,
    account_ids: Vec<String>,
) -> Result<Vec<CursorAccount>, String> {
    let started_at = Instant::now();
    logger::log_info(&format!(
        "[Cursor Command] 批量对话验活开始: count={}",
        account_ids.len()
    ));
    let accounts = tauri::async_runtime::spawn_blocking(move || {
        crate::modules::cursor_chat_probe::probe_accounts_chat(&account_ids)
    })
    .await
    .map_err(|e| format!("批量对话验活任务失败: {}", e))??;

    for account in &accounts {
        emit_cursor_accounts_changed(&app, &account.id, "chat_probe");
    }
    logger::log_info(&format!(
        "[Cursor Command] 批量对话验活完成: count={}, elapsed={}ms",
        accounts.len(),
        started_at.elapsed().as_millis()
    ));
    Ok(accounts)
}
