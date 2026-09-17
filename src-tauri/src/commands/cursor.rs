use std::time::Instant;
use tauri::{AppHandle, Emitter};

use crate::models::cursor::{CursorAccount, CursorAccountListPage};
use crate::modules::{cursor_account, cursor_oauth, logger, renewal_apps_auto_update, renewal_console_status, wuxian_seamless_server, wuyou_native, xubei_cursor_injector, xubei_renewal_prefs, xubei_switch_client};

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
pub async fn list_cursor_accounts() -> Result<Vec<CursorAccount>, String> {
    tauri::async_runtime::spawn_blocking(cursor_account::list_accounts_for_ui)
        .await
        .map_err(|e| format!("list_cursor_accounts join: {e}"))
}

/// 0012：当前账号实时额度（强制拉 usage，查不到返回 queried=false）
#[tauri::command]
pub async fn refresh_cursor_current_account_realtime(
) -> Result<crate::models::cursor::CursorCurrentQuotaSnapshot, String> {
    Ok(cursor_account::current_account_quota_realtime().await)
}

#[tauri::command]
pub async fn list_cursor_accounts_page(
    offset: usize,
    limit: Option<usize>,
) -> Result<CursorAccountListPage, String> {
    let limit = limit.unwrap_or(200);
    tauri::async_runtime::spawn_blocking(move || {
        cursor_account::list_accounts_page_for_ui(offset, limit)
    })
    .await
    .map_err(|e| format!("list_cursor_accounts_page join: {e}"))
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
    max_duration_secs: Option<u64>,
) -> Result<i32, String> {
    let started_at = Instant::now();
    let limit = max_count.and_then(|n| if n > 0 { Some(n as usize) } else { None });
    // 0012：允许「不按条数截断、只按墙钟预算」的滚动刷新，保证 4482 大池长期能扫完。
    // 未指定时限时沿用自动刷新默认时限，避免一轮占死 guard。
    let max_duration = match max_duration_secs {
        Some(0) => None,
        Some(secs) => Some(std::time::Duration::from_secs(secs)),
        None => {
            if limit.is_some() {
                Some(std::time::Duration::from_secs(
                    cursor_account::CURSOR_AUTO_REFRESH_MAX_DURATION_SECS,
                ))
            } else {
                Some(std::time::Duration::from_secs(
                    cursor_account::CURSOR_AUTO_REFRESH_MAX_DURATION_SECS,
                ))
            }
        }
    };
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

/// 续杯管家无感换号：云端拉号 + 写 wuxian 热替换 + 默认 Cursor（一体，对齐管家「无感换号」）。
#[tauri::command]
pub async fn switch_cursor_account_from_xubei_seamless(
    app: AppHandle,
) -> Result<CursorAccount, String> {
    let started_at = Instant::now();
    logger::log_info("[Cursor Command] 续杯无感换号开始");
    let account =
        tokio::task::spawn_blocking(xubei_switch_client::pull_and_xubei_seamless_switch)
            .await
            .map_err(|e| format!("续杯无感换号任务失败: {e}"))??;
    emit_cursor_accounts_changed(&app, &account.id, "xubei_seamless_switch");
    let app_for_tray = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = crate::modules::tray::update_tray_menu(&app_for_tray);
    });
    logger::log_info(&format!(
        "[Cursor Command] 续杯无感换号完成: id={}, email={}, elapsed={}ms",
        account.id,
        account.email,
        started_at.elapsed().as_millis()
    ));
    Ok(account)
}

/// 续杯管家池内换号：从总控账号池轮换选号 + wuxian 热替换（不云端拉号、不走默认 Play）。
#[tauri::command]
pub async fn switch_cursor_account_from_xubei_pool(
    app: AppHandle,
) -> Result<CursorAccount, String> {
    let started_at = Instant::now();
    logger::log_info("[Cursor Command] 续杯池内换号开始");
    let account =
        tokio::task::spawn_blocking(xubei_switch_client::switch_pool_account_via_xubei)
            .await
            .map_err(|e| format!("续杯池内换号任务失败: {e}"))??;
    emit_cursor_accounts_changed(&app, &account.id, "xubei_pool_switch");
    let app_for_tray = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = crate::modules::tray::update_tray_menu(&app_for_tray);
    });
    logger::log_info(&format!(
        "[Cursor Command] 续杯池内换号完成: id={}, email={}, elapsed={}ms",
        account.id,
        account.email,
        started_at.elapsed().as_millis()
    ));
    Ok(account)
}

/// 续杯管家云端拉号：只进总控账号池，不写 Cursor / 不碰无感通道。
#[tauri::command]
pub async fn pull_cursor_account_from_xubei(app: AppHandle) -> Result<CursorAccount, String> {
    let started_at = Instant::now();
    logger::log_info("[Cursor Command] 续杯拉号开始");
    let account = tokio::task::spawn_blocking(xubei_switch_client::pull_account_into_pool)
        .await
        .map_err(|e| format!("拉号任务失败: {e}"))??;
    emit_cursor_accounts_changed(&app, &account.id, "xubei_pull");
    let app_for_tray = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = crate::modules::tray::update_tray_menu(&app_for_tray);
    });
    logger::log_info(&format!(
        "[Cursor Command] 续杯拉号完成: id={}, email={}, elapsed={}ms",
        account.id,
        account.email,
        started_at.elapsed().as_millis()
    ));
    Ok(account)
}

/// 只读：续杯/虚备 get-token 当前邮箱（续费控制台状态条）。
#[tauri::command]
pub async fn read_wuxian_get_token_email() -> Result<Option<String>, String> {
    tokio::task::spawn_blocking(cursor_account::read_wuxian_get_token_email)
        .await
        .map_err(|e| format!("get-token 读取任务失败: {e}"))?
}

/// 续费控制台三程序只读状态（替换前端占位假数据）。
#[tauri::command]
pub async fn get_renewal_console_status() -> Result<renewal_console_status::RenewalConsoleStatus, String> {
    tokio::task::spawn_blocking(renewal_console_status::collect_renewal_console_status)
        .await
        .map_err(|e| format!("状态收集失败: {e}"))?
}

/// 无忧原生能力探测（ASAR 指纹 / 传统切号就绪）。
#[tauri::command]
pub async fn get_wuyou_native_status() -> Result<wuyou_native::WuyouNativeStatus, String> {
    tokio::task::spawn_blocking(wuyou_native::collect_status)
        .await
        .map_err(|e| format!("无忧状态收集失败: {e}"))
}

/// 无忧传统切号：指纹 fail-closed 后走 Cockpit 默认实例切号链。
#[tauri::command]
pub async fn wuyou_traditional_switch(app: AppHandle, account_id: String) -> Result<String, String> {
    let status = tokio::task::spawn_blocking(wuyou_native::require_traditional_switch_ready)
        .await
        .map_err(|e| format!("无忧前置检查失败: {e}"))??;
    logger::log_info(&format!(
        "[Wuyou Switch] fingerprint_ok={} account_id={}",
        status.fingerprint_verified, account_id
    ));
    inject_cursor_account(app, account_id).await
}

/// 续杯管家续费台偏好只读（四开关）。
#[tauri::command]
pub async fn get_xubei_renewal_prefs() -> Result<xubei_renewal_prefs::XubeiRenewalPrefs, String> {
    tokio::task::spawn_blocking(xubei_renewal_prefs::read_xubei_renewal_prefs)
        .await
        .map_err(|e| format!("偏好读取失败: {e}"))?
}

/// 续杯管家续费台偏好写入（对齐 wuxian + cursor-switch-assistant/config.json）。
#[tauri::command]
pub async fn set_xubei_renewal_pref(
    key: String,
    value: bool,
) -> Result<xubei_renewal_prefs::XubeiRenewalPrefs, String> {
    let pref_key = match key.as_str() {
        "seamless_enabled" | "seamlessEnabled" => {
            xubei_renewal_prefs::XubeiRenewalPrefKey::SeamlessEnabled
        }
        "auto_switch" | "autoSwitch" => xubei_renewal_prefs::XubeiRenewalPrefKey::AutoSwitch,
        "auto_reset_machine" | "autoResetMachine" => {
            xubei_renewal_prefs::XubeiRenewalPrefKey::AutoResetMachine
        }
        "auto_send_continue" | "autoSendContinue" => {
            xubei_renewal_prefs::XubeiRenewalPrefKey::AutoSendContinue
        }
        other => return Err(format!("未知续杯偏好键: {other}")),
    };
    logger::log_info(&format!(
        "[Cursor Command] 续杯偏好写入: key={key}, value={value}"
    ));
    xubei_renewal_prefs::set_xubei_renewal_pref(pref_key, value)
}

/// 三续费程序自动更新：运行 scripts/sync_external_renewal_apps.ps1 并返回报告。
/// 须异步 + spawn_blocking：同步命令会占满阻塞线程，进续费台后切走时整窗卡死。
#[tauri::command]
pub async fn sync_renewal_apps_auto_update(
    launch_xubei: Option<bool>,
) -> renewal_apps_auto_update::RenewalAppsSyncResult {
    let launch = launch_xubei.unwrap_or(false);
    match tauri::async_runtime::spawn_blocking(move || {
        renewal_apps_auto_update::sync_renewal_apps_now(launch)
    })
    .await
    {
        Ok(result) => result,
        Err(err) => renewal_apps_auto_update::RenewalAppsSyncResult {
            ok: false,
            report_path: String::new(),
            report: None,
            error: Some(format!("续费程序同步任务失败: {}", err)),
        },
    }
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

/// 指定 Cursor 路径：打开文件对话框让用户选择 Cursor.exe 或 Cursor 安装目录。
#[tauri::command]
pub async fn pick_cursor_path(app: AppHandle) -> Result<String, String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog()
        .file()
        .add_filter("Cursor 可执行文件", &["exe"])
        .add_filter("所有文件", &["*"])
        .set_title("选择 Cursor.exe 或 Cursor 安装目录")
        .pick_file(move |path| {
            let _ = tx.send(path);
        });
    // 等待回调
    let file_path = rx.recv_timeout(std::time::Duration::from_secs(120))
        .map_err(|_| "等待文件选择超时")?;
    match file_path {
        Some(path) => {
            use tauri_plugin_dialog::FilePath;
            let path_str = match path {
                FilePath::Url(url) => url.to_string(),
                FilePath::Path(p) => p.to_string_lossy().to_string(),
            };
            logger::log_info(&format!("[Cursor Command] 指定 Cursor 路径: {}", path_str));
            // 保存到配置
            let cfg_dir = dirs::home_dir()
                .ok_or_else(|| "无法获取主目录".to_string())?
                .join(".cursor-switch-assistant");
            if !cfg_dir.exists() {
                std::fs::create_dir_all(&cfg_dir).map_err(|e| format!("创建配置目录失败: {}", e))?;
            }
            let cfg_path = cfg_dir.join("cursor_path.json");
            let doc = serde_json::json!({ "cursor_path": path_str, "updated_at": chrono::Utc::now().to_rfc3339() });
            crate::modules::atomic_write::write_string_atomic(
                &cfg_path,
                &serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?,
            )
            .map_err(|e| format!("保存 Cursor 路径失败: {}", e))?;
            Ok(path_str)
        }
        None => Err("未选择文件".to_string()),
    }
}

/// 手动发送继续：向 wuxian 本地服务发送 resume 信号。
#[tauri::command]
pub async fn manual_send_continue() -> Result<String, String> {
    logger::log_info("[Cursor Command] 手动发送继续");
    // alone：ensure-start + HTTP 扫口全放阻塞线程，禁止在 async worker 上同步扫端口卡 UI
    tokio::task::spawn_blocking(cursor_account::manual_send_continue_to_xubei)
        .await
        .map_err(|e| format!("手动发送继续任务失败: {e}"))?
}

/// 重置 Cursor 机器码（对齐续杯管家「重置机器码」按钮）。
#[tauri::command]
pub async fn reset_cursor_machine_id() -> Result<String, String> {
    logger::log_info("[Cursor Command] 重置机器码");
    tokio::task::spawn_blocking(cursor_account::reset_cursor_machine_id_live)
        .await
        .map_err(|e| format!("重置机器码任务失败: {e}"))?
}

/// 续杯 Basic 注入（对齐管家「注入」：workbench i0/i1/i2 + exthost + main）。
#[tauri::command]
pub async fn inject_xubei_basic(allow_live_write: Option<bool>) -> Result<String, String> {
    let allow = allow_live_write.unwrap_or(false);
    logger::log_info(&format!(
        "[Cursor Command] 续杯 Basic 注入开始 allow_live_write={allow}"
    ));
    tokio::task::spawn_blocking(move || {
        // alone：ensure-start 放阻塞线程，避免 async worker 同步扫口
        let _ = wuxian_seamless_server::start(None);
        xubei_cursor_injector::inject_basic(allow)
    })
        .await
        .map_err(|e| format!("注入任务失败: {e}"))?
}

/// 还原续杯注入（对齐续杯管家「还原」按钮：从 .bak 恢复被注入文件）。
#[tauri::command]
pub async fn restore_cursor_injection() -> Result<String, String> {
    logger::log_info("[Cursor Command] 还原续杯注入");
    tokio::task::spawn_blocking(cursor_account::restore_cursor_injection)
        .await
        .map_err(|e| format!("还原任务失败: {e}"))?
}

/// 启动 Cockpit 内置 wuxian 无感 HTTP 服务。
#[tauri::command]
pub fn start_wuxian_seamless_server() -> Result<serde_json::Value, String> {
    match wuxian_seamless_server::start(None) {
        Ok(port) => Ok(serde_json::json!({
            "ok": true,
            "port": port,
            "adoptedExternal": wuxian_seamless_server::is_adopted_external(),
        })),
        Err(e) => Err(e),
    }
}

/// 停止 Cockpit 内置 wuxian 无感 HTTP 服务。
#[tauri::command]
pub fn stop_wuxian_seamless_server() -> Result<serde_json::Value, String> {
    wuxian_seamless_server::stop();
    Ok(serde_json::json!({ "ok": true }))
}

/// 查询 wuxian 无感服务状态。
#[tauri::command]
pub fn get_wuxian_seamless_server_status() -> serde_json::Value {
    serde_json::json!({
        "running": wuxian_seamless_server::is_running(),
        "port": wuxian_seamless_server::active_port(),
        "adoptedExternal": wuxian_seamless_server::is_adopted_external(),
    })
}

/// 激活卡密（对齐续杯管家「激活卡密」）。
#[tauri::command]
pub async fn activate_card_key(card_key: String) -> Result<serde_json::Value, String> {
    logger::log_info(&format!(
        "[Cursor Command] 激活卡密: key={}…",
        &card_key[..card_key.len().min(6)]
    ));
    tokio::task::spawn_blocking(move || xubei_switch_client::activate_card_key(&card_key))
        .await
        .map_err(|e| format!("卡密激活任务失败: {e}"))?
}

/// 查询本地卡密激活状态。
#[tauri::command]
pub fn verify_card_key() -> Result<serde_json::Value, String> {
    xubei_switch_client::verify_card_key()
}

/// 修改续杯管家密码。
#[tauri::command]
pub async fn change_xubei_password(
    old_password: String,
    new_password: String,
) -> Result<serde_json::Value, String> {
    logger::log_info("[Cursor Command] 修改续杯密码");
    tokio::task::spawn_blocking(move || {
        xubei_switch_client::change_xubei_password(&old_password, &new_password)
    })
    .await
    .map_err(|e| format!("修改密码任务失败: {e}"))?
}

/// 退出续杯管家登录。
#[tauri::command]
pub fn logout_xubei() -> Result<serde_json::Value, String> {
    logger::log_info("[Cursor Command] 退出续杯登录");
    xubei_switch_client::logout_xubei()
}
