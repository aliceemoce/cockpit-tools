//! 应用内点击统一入口：deep link / Tauri 命令只经此模块派发，禁止把 emit 成功谎报为「已点击」。
//! 前端点击结果通过 `gui_click_ack` 回写 app.log。

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter};

static CLICK_DEDUPE: Mutex<Option<(String, Instant)>> = Mutex::new(None);

const DEDUPE_WINDOW: Duration = Duration::from_millis(1200);

/// 副实例 on_open_url 会点空 WebView；多进程时只让最早启动的 cockpit-tools 处理 deep link。
pub fn should_skip_deeplink_on_open_url() -> bool {
    use sysinfo::{ProcessRefreshKind, System, UpdateKind};
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
    );
    let my_pid = std::process::id();
    let mut cockpit_pids: Vec<u32> = sys
        .processes()
        .iter()
        .filter(|(_, p)| {
            p.name()
                .eq_ignore_ascii_case("cockpit-tools.exe")
        })
        .map(|(pid, _)| pid.as_u32())
        .collect();
    if cockpit_pids.len() <= 1 {
        return false;
    }
    cockpit_pids.sort_unstable();
    let primary_pid = cockpit_pids[0];
    if my_pid != primary_pid {
        crate::modules::logger::log_info(&format!(
            "[GuiClick] 副实例 pid={} 跳过 on_open_url，由主实例 pid={} 经 SingleInstance 处理",
            my_pid, primary_pid
        ));
        return true;
    }
    false
}

fn should_dedupe(action_id: &str) -> bool {
    let mut guard = CLICK_DEDUPE.lock().expect("gui click dedupe lock");
    let now = Instant::now();
    if let Some((last_id, last_at)) = guard.as_ref() {
        if last_id == action_id && now.duration_since(*last_at) < DEDUPE_WINDOW {
            return true;
        }
    }
    *guard = Some((action_id.to_string(), now));
    false
}

pub fn navigate_page(app: &AppHandle, page: &str, tab: Option<&str>) {
    let mut payload = serde_json::json!({ "page": page });
    if let Some(tab) = tab.filter(|s| !s.is_empty()) {
        payload["tab"] = serde_json::json!(tab);
    }
    let _ = app.emit("deep-link-navigate", payload);
    crate::modules::logger::log_info(&format!(
        "[GuiClick] 已派发导航 page={} tab={:?}（等待前端切页）",
        page, tab
    ));
}

fn emit_trigger_click(app: &AppHandle, action_id: &str) -> Result<(), String> {
    app.emit("gui:trigger-click", action_id)
        .map_err(|e| format!("emit gui:trigger-click failed: {e}"))
}

/// 派发应用内点击；日志写「已派发」，真正点中与否以前端 ack 为准。
pub fn trigger_click(app: &AppHandle, action_id: &str) -> Result<(), String> {
    let action_id = action_id.trim();
    if action_id.is_empty() {
        return Err("action_id 为空".to_string());
    }
    if should_dedupe(action_id) {
        crate::modules::logger::log_info(&format!(
            "[GuiClick] 去重跳过重复派发 action_id={}",
            action_id
        ));
        return Ok(());
    }

    if action_id.starts_with("cursor-instance-start-") {
        // Cursor 多开在 Cursor 账号页的应用多开子 Tab，不是总控 instances 页。
        navigate_page(app, "cursor", Some("instances"));
        let app_clone = app.clone();
        let id = action_id.to_string();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(2800)).await;
            match emit_trigger_click(&app_clone, &id) {
                Ok(()) => crate::modules::logger::log_info(&format!(
                    "[GuiClick] 已派发 action_id={}（多开页，等待前端 ack）",
                    id
                )),
                Err(e) => crate::modules::logger::log_warn(&format!(
                    "[GuiClick] 派发失败 action_id={} err={}",
                    id, e
                )),
            }
        });
        return Ok(());
    }

    emit_trigger_click(app, action_id)?;
    crate::modules::logger::log_info(&format!(
        "[GuiClick] 已派发 action_id={}（等待前端 ack）",
        action_id
    ));
    Ok(())
}

/// 前端回执：控件是否找到并 click()。
pub fn ack_from_frontend(action_id: String, success: bool, detail: Option<String>) {
    let action_id = action_id.trim();
    let extra = detail
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| format!(" ({})", s))
        .unwrap_or_default();
    if success {
        crate::modules::logger::log_info(&format!(
            "[GuiClick] 前端已点击 action_id={}{}",
            action_id, extra
        ));
    } else {
        crate::modules::logger::log_warn(&format!(
            "[GuiClick] 前端未点到控件 action_id={}{}",
            action_id, extra
        ));
    }
}
