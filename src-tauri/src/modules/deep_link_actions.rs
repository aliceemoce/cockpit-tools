//! Deep Link 操作协议模块
//!
//! 扩展 `cockpit-tools://` 协议，支持从外部程序（如 Cursor）触发 Cockpit 操作：
//!
//! - `cockpit-tools://switch/<platform>/<account_id_or_email>` — 切换账号
//! - `cockpit-tools://probe/<platform>/<account_id>` — 对话验活
//! - `cockpit-tools://start-instance/<platform>/<instance_id>` — 启动实例
//! - `cockpit-tools://navigate/<page>` — 前端页面导航
//! - `cockpit-tools://click/<action_id>` — 应用内点击（gui_trigger_click）
//! - `cockpit-tools://screenshot[?path=<output_path>]` — 截图
//! - `cockpit-tools://verify-chat/<platform>/<scope>[/<instance_id>][?path=&screenshot=]` — Cursor 窗内对话验活（CDP，经总控）
//! - `cockpit-tools://ui-state` — 获取 UI 状态（写入日志文件供外部读取）

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Emitter};

static VERIFY_CHAT_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

/// 尝试处理操作类 deep link。返回 true 表示已处理。
pub fn handle_deep_link_actions(app: &AppHandle, url: &str) -> bool {
    let trimmed = url.trim();
    // 只处理 cockpit-tools:// 和 cockpittools:// 的操作类 URL
    let rest = if let Some(stripped) = trimmed.strip_prefix("cockpit-tools://") {
        stripped
    } else if let Some(stripped) = trimmed.strip_prefix("cockpittools://") {
        stripped
    } else {
        return false;
    };

    // 排除已有的 OAuth 回调和外部导入（它们以 import/ 等开头）
    if rest.starts_with("import") || rest.starts_with("callback") || rest.starts_with("zcode") {
        return false;
    }

    // 解析 action/path[?query]
    let (action_path, query) = match rest.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (rest, None),
    };

    let segments: Vec<&str> = action_path.split('/').collect();
    if segments.is_empty() {
        return false;
    }

    let action = segments[0];
    let params: Vec<&str> = segments[1..].to_vec();

    crate::modules::logger::log_info(&format!(
        "[DeepLinkActions] action={}, params={:?}, query={:?}",
        action, params, query
    ));

    match action {
        "switch" => handle_switch(app, &params),
        "probe" => handle_probe(app, &params),
        "start-instance" | "start_instance" => handle_start_instance(app, &params),
        "navigate" => handle_navigate(app, &params),
        "click" => handle_click(app, &params),
        "screenshot" => handle_screenshot(app, query),
        "verify-chat" | "verify_chat" => handle_verify_chat(app, &params, query),
        "ui-state" | "ui_state" => handle_ui_state(app),
        _ => {
            crate::modules::logger::log_warn(&format!(
                "[DeepLinkActions] 未知操作: {}",
                action
            ));
            return false;
        }
    }

    true
}

/// 解析 query string 为 key-value 对（值做 URL 解码）
fn parse_query(query: Option<&str>) -> Vec<(String, String)> {
    match query {
        Some(q) => q
            .split('&')
            .filter_map(|pair| {
                let mut parts = pair.splitn(2, '=');
                let key = parts.next()?.to_string();
                let raw = parts.next().unwrap_or("");
                let value = urlencoding::decode(raw)
                    .map(|v| v.into_owned())
                    .unwrap_or_else(|_| raw.to_string());
                Some((key, value))
            })
            .collect(),
        None => Vec::new(),
    }
}

/// switch/<platform>/<account_id_or_email>
fn handle_switch(app: &AppHandle, params: &[&str]) {
    if params.len() < 2 {
        crate::modules::logger::log_warn(
            "[DeepLinkActions] switch 需要 platform 和 account 参数",
        );
        return;
    }
    let platform = params[0].to_string();
    let account = params[1].to_string();
    let app_clone = app.clone();

    tauri::async_runtime::spawn(async move {
        let result = execute_switch(&platform, &account).await;
        let _ = app_clone.emit(
            "deep-link-action-result",
            serde_json::json!({
                "action": "switch",
                "platform": platform,
                "account": account,
                "success": result.is_ok(),
                "error": result.err(),
            }),
        );
    });
}

async fn execute_switch(platform: &str, account: &str) -> Result<(), String> {
    match platform.to_lowercase().as_str() {
        "cursor" => {
            crate::modules::cursor_account::inject_to_cursor(account)?;
            Ok(())
        }
        _ => Err(format!("平台 '{}' 的 switch 暂未实现", platform)),
    }
}

/// probe/<platform>/<account_id>
fn handle_probe(app: &AppHandle, params: &[&str]) {
    if params.len() < 2 {
        crate::modules::logger::log_warn(
            "[DeepLinkActions] probe 需要 platform 和 account_id 参数",
        );
        return;
    }
    let platform = params[0].to_string();
    let account_id = params[1].to_string();
    let app_clone = app.clone();

    tauri::async_runtime::spawn(async move {
        let result = match platform.to_lowercase().as_str() {
            "cursor" => {
                match crate::modules::cursor_chat_probe::probe_account_chat(&account_id) {
                    Ok(account) => {
                        let outcome = account
                            .chat_probe
                            .as_ref()
                            .map(|p| p.outcome.as_str())
                            .unwrap_or("unknown");
                        Ok(serde_json::json!({ "outcome": outcome, "email": account.email }))
                    }
                    Err(e) => Err(e),
                }
            }
            _ => Err(format!("平台 '{}' 的 probe 暂未实现", platform)),
        };

        let _ = app_clone.emit(
            "deep-link-action-result",
            serde_json::json!({
                "action": "probe",
                "platform": platform,
                "account_id": account_id,
                "success": result.is_ok(),
                "result": result.as_ref().ok(),
                "error": result.err(),
            }),
        );
    });
}

/// start-instance/<platform>/<instance_id>
fn handle_start_instance(app: &AppHandle, params: &[&str]) {
    if params.len() < 2 {
        crate::modules::logger::log_warn(
            "[DeepLinkActions] start-instance 需要 platform 和 instance_id 参数",
        );
        return;
    }
    let platform = params[0].to_string();
    let instance_id = params[1].to_string();
    let app_clone = app.clone();

    tauri::async_runtime::spawn(async move {
        let result = match platform.to_lowercase().as_str() {
            "cursor" => {
                crate::commands::cursor_instance::start_cursor_instance_with_account_switch(
                    instance_id.clone(),
                    None,
                )
                .await
                .map(|_| serde_json::json!({ "instance_id": instance_id }))
            }
            _ => Err(format!("平台 '{}' 的 start-instance 暂未实现", platform)),
        };

        let _ = app_clone.emit(
            "deep-link-action-result",
            serde_json::json!({
                "action": "start-instance",
                "platform": platform,
                "instance_id": instance_id,
                "success": result.is_ok(),
                "error": result.err(),
            }),
        );
    });
}

/// navigate/<page>
fn handle_navigate(app: &AppHandle, params: &[&str]) {
    if params.is_empty() {
        crate::modules::logger::log_warn("[DeepLinkActions] navigate 需要 page 参数");
        return;
    }
    let page = params.join("/");
    if let Some((base, tab)) = page.split_once('/') {
        crate::modules::gui_in_app_click::navigate_page(app, base, Some(tab));
    } else {
        crate::modules::gui_in_app_click::navigate_page(app, &page, None);
    }
}

/// click/<action_id> — 走应用内 gui_trigger_click（data-action-id）
fn handle_click(app: &AppHandle, params: &[&str]) {
    if params.is_empty() {
        crate::modules::logger::log_warn("[DeepLinkActions] click 需要 action_id 参数");
        return;
    }
    let action_id = params.join("/");
    if let Err(e) = crate::modules::gui_in_app_click::trigger_click(app, &action_id) {
        crate::modules::logger::log_warn(&format!(
            "[DeepLinkActions] 应用内点击派发失败: action_id={}, err={}",
            action_id, e
        ));
    }
}

/// screenshot[?path=<output_path>]
fn handle_screenshot(app: &AppHandle, query: Option<&str>) {
    let query_params = parse_query(query);
    let output_path = query_params
        .iter()
        .find(|(k, _)| k == "path")
        .map(|(_, v)| v.clone());

    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        let result =
            crate::commands::screenshot::capture_main_window_screenshot(&app_clone, output_path);

        let _ = app_clone.emit(
            "deep-link-action-result",
            match result {
                Ok(path) => serde_json::json!({
                    "action": "screenshot",
                    "success": true,
                    "path": path,
                }),
                Err(e) => serde_json::json!({
                    "action": "screenshot",
                    "success": false,
                    "error": e,
                }),
            },
        );
    });
}

/// verify-chat/<platform>/<scope>[/<instance_id>][?path=&screenshot=]
/// scope = `instance` | `default`
fn handle_verify_chat(app: &AppHandle, params: &[&str], query: Option<&str>) {
    if params.len() < 2 {
        crate::modules::logger::log_warn(
            "[DeepLinkActions] verify-chat 需要 platform 和 scope 参数",
        );
        return;
    }
    let platform = params[0].to_string();
    let scope = params[1].to_string();
    let instance_id = params.get(2).map(|s| s.to_string());
    let query_pairs = parse_query(query);
    let result_path = query_pairs
        .iter()
        .find(|(k, _)| k == "path")
        .map(|(_, v)| v.clone());
    let screenshot_path = query_pairs
        .iter()
        .find(|(k, _)| k == "screenshot")
        .map(|(_, v)| v.clone());

    if VERIFY_CHAT_IN_FLIGHT
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        crate::modules::logger::log_warn(
            "[DeepLinkActions] verify-chat 已在执行，跳过重复 deep link",
        );
        return;
    }

    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        let _guard = VerifyChatGuard;
        let verify_future = async {
            match platform.to_lowercase().as_str() {
                "cursor" => {
                    execute_verify_cursor_chat(&scope, instance_id.as_deref(), screenshot_path).await
                }
                other => Err(format!("平台 '{}' 的 verify-chat 暂未实现", other)),
            }
        };
        let result = match tokio::time::timeout(std::time::Duration::from_secs(150), verify_future)
            .await
        {
            Ok(inner) => inner,
            Err(_) => Err("verify-chat 总超时(150s)".to_string()),
        };

        let payload = match &result {
            Ok(value) => {
                if let Some(path) = &result_path {
                    let write_result = std::fs::write(
                        path,
                        serde_json::to_string_pretty(value).unwrap_or_default(),
                    );
                    if let Err(e) = write_result {
                        crate::modules::logger::log_error(&format!(
                            "[DeepLinkActions] verify-chat 写结果失败: {}",
                            e
                        ));
                    } else {
                        crate::modules::logger::log_info(&format!(
                            "[DeepLinkActions] verify-chat 结果已写入: {}",
                            path
                        ));
                    }
                }
                serde_json::json!({
                    "action": "verify-chat",
                    "platform": platform,
                    "scope": scope,
                    "instance_id": instance_id,
                    "success": value.get("dialogue_ok").and_then(|v| v.as_bool()).unwrap_or(false),
                    "result": value,
                })
            }
            Err(e) => {
                if let Some(path) = &result_path {
                    let err_payload = serde_json::json!({
                        "action": "verify-chat",
                        "platform": platform,
                        "scope": scope,
                        "instance_id": instance_id,
                        "success": false,
                        "error": e,
                    });
                    let write_result = std::fs::write(
                        path,
                        serde_json::to_string_pretty(&err_payload).unwrap_or_default(),
                    );
                    if let Err(write_err) = write_result {
                        crate::modules::logger::log_error(&format!(
                            "[DeepLinkActions] verify-chat 写错误结果失败: {}",
                            write_err
                        ));
                    } else {
                        crate::modules::logger::log_info(&format!(
                            "[DeepLinkActions] verify-chat 错误结果已写入: {}",
                            path
                        ));
                    }
                }
                serde_json::json!({
                    "action": "verify-chat",
                    "platform": platform,
                    "scope": scope,
                    "instance_id": instance_id,
                    "success": false,
                    "error": e,
                })
            }
        };

        crate::modules::logger::log_info(&format!(
            "[DeepLinkActions] verify-chat 完成 success={}",
            payload
                .get("success")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        ));

        let _ = app_clone.emit("deep-link-action-result", payload);
    });
}

struct VerifyChatGuard;

impl Drop for VerifyChatGuard {
    fn drop(&mut self) {
        VERIFY_CHAT_IN_FLIGHT.store(false, Ordering::SeqCst);
    }
}

async fn execute_verify_cursor_chat(
    scope: &str,
    instance_id: Option<&str>,
    screenshot_path: Option<String>,
) -> Result<serde_json::Value, String> {
    let (user_data_dir, scope_label) = match scope {
        "default" => {
            let dir = crate::modules::cursor_instance::get_default_cursor_user_data_dir()?;
            (dir.to_string_lossy().to_string(), "default".to_string())
        }
        "instance" => {
            let id = instance_id.ok_or("verify-chat instance 需要 instance_id")?;
            let store = crate::modules::cursor_instance::load_instance_store()?;
            let instance = store
                .instances
                .into_iter()
                .find(|item| item.id == id)
                .ok_or_else(|| format!("实例不存在: {}", id))?;
            (instance.user_data_dir, format!("instance:{}", id))
        }
        other => return Err(format!("未知 scope: {}", other)),
    };

    let port = crate::modules::cursor_instance::resolve_cdp_port_for_user_data_dir(&user_data_dir)
        .ok_or_else(|| {
            format!(
                "未找到 {} 的 CDP 端口；请先经总控 start-instance 启动对应 Cursor",
                scope_label
            )
        })?;

    crate::modules::logger::log_info(&format!(
        "[DeepLinkActions] verify-chat {} dir={} port={}",
        scope_label, user_data_dir, port
    ));

    // 等待 CDP 就绪（换号/重启后端口可能短暂无 target，每轮重解析端口）
    let mut port = port;
    for attempt in 0..45 {
        if let Some(fresh) =
            crate::modules::cursor_instance::resolve_cdp_port_for_user_data_dir(&user_data_dir)
        {
            port = fresh;
        }
        if crate::modules::cursor_cdp_control::cdp_port_has_targets(port).await {
            crate::modules::logger::log_info(&format!(
                "[DeepLinkActions] verify-chat CDP 就绪 port={} attempt={}",
                port,
                attempt + 1
            ));
            break;
        }
        if attempt == 44 {
            return Err(format!(
                "CDP 端口无可用 target (port={}, 已等 {}s)",
                port,
                (attempt + 1) * 2
            ));
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }

    let verify = crate::modules::cursor_cdp_control::verify_agents_composer_dialogue(port).await?;

    let mut result = serde_json::json!({
        "scope": scope_label,
        "user_data_dir": user_data_dir,
        "cdp_port": port,
        "dialogue_ok": verify.dialogue_ok,
        "probe_tag": verify.probe_tag,
        "error": verify.error,
        "tail_excerpt": verify.tail_excerpt,
    });

    if let Some(path) = screenshot_path {
        match crate::modules::cursor_cdp_control::take_screenshot(port).await {
            Ok(bytes) => {
                if let Err(e) = std::fs::write(&path, &bytes) {
                    result["screenshot_error"] = serde_json::json!(e.to_string());
                } else {
                    result["screenshot_path"] = serde_json::json!(path);
                }
            }
            Err(e) => {
                result["screenshot_error"] = serde_json::json!(e);
            }
        }
    }

    Ok(result)
}

/// ui-state — 将当前 UI 状态写入临时文件供外部读取
fn handle_ui_state(app: &AppHandle) {
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        let cursor_accounts = crate::modules::cursor_account::list_accounts();
        let account_count = cursor_accounts.len();

        let state = serde_json::json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "platform": "cursor",
            "account_count": account_count,
            "accounts": cursor_accounts.iter().map(|a| {
                serde_json::json!({
                    "id": a.id,
                    "email": a.email,
                    "plan": a.membership_type,
                    "tags": a.tags,
                })
            }).collect::<Vec<_>>(),
        });

        let path = format!(
            "{}/cockpit-ui-state.json",
            std::env::temp_dir().display()
        );
        let write_result =
            std::fs::write(&path, serde_json::to_string_pretty(&state).unwrap_or_default());

        let _ = app_clone.emit(
            "deep-link-action-result",
            serde_json::json!({
                "action": "ui-state",
                "success": write_result.is_ok(),
                "path": path,
                "error": write_result.err().map(|e| e.to_string()),
            }),
        );
    });
}
