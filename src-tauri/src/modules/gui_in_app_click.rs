//! 应用内点击统一入口：deep link / Tauri 命令只经此模块派发。
//! **派发成功 ≠ 已点击**；`trigger_click_wait` 必须等前端 ack（真点到或明确失败原因）再返回。

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

static CLICK_DEDUPE: Mutex<Option<(String, Instant)>> = Mutex::new(None);
const DEDUPE_WINDOW: Duration = Duration::from_millis(1200);
const RENEWAL_MANUAL_CONTINUE_TIMEOUT_MS: u64 = 45_000;

static REQ_SEQ: AtomicU64 = AtomicU64::new(1);

fn next_request_id() -> String {
    format!("req-{}", REQ_SEQ.fetch_add(1, Ordering::Relaxed))
}

// ── 最近点击结果表（最新 50 条） ────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ClickAckRecord {
    pub action_id: String,
    pub request_id: Option<String>,
    pub success: bool,
    pub detail: Option<String>,
    pub element_info: Option<String>,
    pub at_ms: u128,
}

static RECENT_ACKS: Mutex<Option<VecDeque<ClickAckRecord>>> = Mutex::new(None);
const MAX_RECENT_ACKS: usize = 50;

fn push_ack_record(record: ClickAckRecord) {
    let mut guard = RECENT_ACKS.lock().expect("recent acks lock");
    let deque = guard.get_or_insert_with(VecDeque::new);
    if deque.len() >= MAX_RECENT_ACKS {
        deque.pop_front();
    }
    deque.push_back(record);
}

/// 查询最近点击结果（供 Agent / DeepLink 验证操作是否真实生效）。
pub fn get_recent_ack_results() -> Vec<ClickAckRecord> {
    let guard = RECENT_ACKS.lock().expect("recent acks lock");
    guard
        .as_ref()
        .map(|d| d.iter().cloned().collect())
        .unwrap_or_default()
}

/// 查询指定 action_id 的最新一条 ack。
pub fn get_latest_ack_for(action_id: &str) -> Option<ClickAckRecord> {
    let guard = RECENT_ACKS.lock().expect("recent acks lock");
    guard.as_ref().and_then(|d| {
        d.iter()
            .rev()
            .find(|r| r.action_id == action_id)
            .cloned()
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct ClickDispatchRecord {
    pub action_id: String,
    pub request_id: Option<String>,
    pub success: bool,
    pub outcome_code: String,
    pub detail: Option<String>,
    pub element_info: Option<String>,
    pub waited_ms: u64,
    pub at_ms: u128,
}

static RECENT_DISPATCH_RESULTS: Mutex<Option<VecDeque<ClickDispatchRecord>>> = Mutex::new(None);
const MAX_RECENT_DISPATCH_RESULTS: usize = 50;

fn push_dispatch_record(record: ClickDispatchRecord) {
    let mut guard = RECENT_DISPATCH_RESULTS
        .lock()
        .expect("recent dispatch results lock");
    let deque = guard.get_or_insert_with(VecDeque::new);
    if deque.len() >= MAX_RECENT_DISPATCH_RESULTS {
        deque.pop_front();
    }
    deque.push_back(record);
}

pub fn get_recent_dispatch_results() -> Vec<ClickDispatchRecord> {
    let guard = RECENT_DISPATCH_RESULTS
        .lock()
        .expect("recent dispatch results lock");
    guard
        .as_ref()
        .map(|d| d.iter().cloned().collect())
        .unwrap_or_default()
}

pub fn get_latest_dispatch_for(action_id: &str) -> Option<ClickDispatchRecord> {
    let guard = RECENT_DISPATCH_RESULTS
        .lock()
        .expect("recent dispatch results lock");
    guard.as_ref().and_then(|d| {
        d.iter()
            .rev()
            .find(|r| r.action_id == action_id)
            .cloned()
    })
}

/// 等待中的派发：前端 ack 按 request_id 唤醒。
struct PendingWait {
    tx: mpsc::Sender<ClickAckRecord>,
}

static PENDING: Mutex<Option<HashMap<String, PendingWait>>> = Mutex::new(None);

fn pending_map() -> std::sync::MutexGuard<'static, Option<HashMap<String, PendingWait>>> {
    PENDING.lock().expect("gui click pending lock")
}

fn register_pending(request_id: &str) -> mpsc::Receiver<ClickAckRecord> {
    let (tx, rx) = mpsc::channel();
    let mut guard = pending_map();
    let map = guard.get_or_insert_with(HashMap::new);
    map.insert(request_id.to_string(), PendingWait { tx });
    rx
}

fn take_pending(request_id: &str) -> Option<PendingWait> {
    let mut guard = pending_map();
    guard.as_mut().and_then(|m| m.remove(request_id))
}

fn clear_pending(request_id: &str) {
    let _ = take_pending(request_id);
}

/// 成功路径返回给命令面的结构化结果（仅 success=true）。
#[derive(Debug, Clone, Serialize)]
pub struct ClickResult {
    pub action_id: String,
    pub request_id: String,
    pub success: bool,
    pub detail: Option<String>,
    pub element_info: Option<String>,
    pub waited_ms: u64,
}

fn fail_msg(code: &str, detail: &str) -> String {
    format!("{code}: {detail}")
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn record_dispatch_result(
    action_id: &str,
    request_id: Option<&str>,
    success: bool,
    outcome_code: &str,
    detail: Option<String>,
    element_info: Option<String>,
    waited_ms: u64,
) {
    push_dispatch_record(ClickDispatchRecord {
        action_id: action_id.to_string(),
        request_id: request_id.map(|v| v.to_string()),
        success,
        outcome_code: outcome_code.to_string(),
        detail,
        element_info,
        waited_ms,
        at_ms: now_ms(),
    });
}

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
        .filter(|(_, p)| p.name().eq_ignore_ascii_case("cockpit-tools.exe"))
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

fn emit_trigger_click(app: &AppHandle, action_id: &str, request_id: &str) -> Result<(), String> {
    let payload = serde_json::json!({
        "action_id": action_id,
        "request_id": request_id,
    });
    app.emit("gui:trigger-click", payload)
        .map_err(|e| fail_msg("emit_failed", &format!("emit gui:trigger-click failed: {e}")))
}

fn default_timeout_ms(action_id: &str) -> u64 {
    if action_id.starts_with("cursor-instance-start-") {
        20_000
    } else if action_id.starts_with("cursor-renewal-") || action_id.starts_with("nav-cursor-") {
        // 续费台/ Cursor 子 Tab：账号池大时前端积压，5s 常 timeout_no_ack，拉号/无感/四开关点不到。
        RENEWAL_MANUAL_CONTINUE_TIMEOUT_MS
    } else {
        5_000
    }
}

fn classify_ack_failure(detail: Option<&str>, element_info: Option<&str>) -> String {
    let d = detail.unwrap_or("").to_ascii_lowercase();
    let e = element_info.unwrap_or("").to_ascii_lowercase();
    if d.contains("blocked_by_whitelist") {
        return "blocked_by_whitelist".to_string();
    }
    if e.contains("[disabled]") || d.contains("element disabled") || d.contains("disabled") {
        return "element_disabled".to_string();
    }
    if d.contains("not found") || d.contains("element not found") {
        return "element_not_found".to_string();
    }
    "click_failed".to_string()
}

// ── 枚举回执（模块七）────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct EnumerateAckRecord {
    pub request_id: String,
    pub success: bool,
    pub route: Option<String>,
    pub total_clickable: usize,
    pub legacy_action_ids: Vec<String>,
    pub actions: serde_json::Value,
    pub detail: Option<String>,
    pub at_ms: u128,
}

static LAST_ENUMERATE: Mutex<Option<EnumerateAckRecord>> = Mutex::new(None);
static ENUM_PENDING: Mutex<Option<HashMap<String, mpsc::Sender<EnumerateAckRecord>>>> =
    Mutex::new(None);

pub fn get_last_enumerate_result() -> Option<EnumerateAckRecord> {
    LAST_ENUMERATE
        .lock()
        .expect("last enumerate lock")
        .clone()
}

fn register_enumerate_pending(request_id: &str) -> mpsc::Receiver<EnumerateAckRecord> {
    let (tx, rx) = mpsc::channel();
    let mut guard = ENUM_PENDING.lock().expect("enum pending lock");
    let map = guard.get_or_insert_with(HashMap::new);
    map.insert(request_id.to_string(), tx);
    rx
}

fn take_enumerate_pending(request_id: &str) -> Option<mpsc::Sender<EnumerateAckRecord>> {
    let mut guard = ENUM_PENDING.lock().expect("enum pending lock");
    guard.as_mut().and_then(|m| m.remove(request_id))
}

/// 前端枚举回执：写入最近结果并唤醒等待方；同时合并进 %TEMP%/cockpit-ui-state.json。
pub fn enumerate_ack_from_frontend(
    request_id: Option<String>,
    success: bool,
    route: Option<String>,
    total_clickable: usize,
    legacy_action_ids: Vec<String>,
    actions: serde_json::Value,
    detail: Option<String>,
) {
    let rid = request_id
        .filter(|s| !s.is_empty())
        .unwrap_or_else(next_request_id);
    let record = EnumerateAckRecord {
        request_id: rid.clone(),
        success,
        route,
        total_clickable,
        legacy_action_ids,
        actions: actions.clone(),
        detail: detail.clone(),
        at_ms: now_ms(),
    };
    {
        let mut guard = LAST_ENUMERATE.lock().expect("last enumerate lock");
        *guard = Some(record.clone());
    }
    if let Some(tx) = take_enumerate_pending(&rid) {
        let _ = tx.send(record.clone());
    }
    write_enumerate_into_ui_state(&record);
    if success {
        crate::modules::logger::log_info(&format!(
            "[GuiClick] 枚举完成 request_id={} total={} legacy={}",
            rid,
            total_clickable,
            record.legacy_action_ids.len()
        ));
    } else {
        crate::modules::logger::log_warn(&format!(
            "[GuiClick] 枚举失败 request_id={} detail={:?}",
            rid, detail
        ));
    }
}

fn write_enumerate_into_ui_state(record: &EnumerateAckRecord) {
    let path = std::env::temp_dir().join("cockpit-ui-state.json");
    let mut root = match std::fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str::<serde_json::Value>(&raw)
            .unwrap_or_else(|_| serde_json::json!({})),
        Err(_) => serde_json::json!({}),
    };
    if !root.is_object() {
        root = serde_json::json!({});
    }
    root["timestamp"] = serde_json::json!(chrono::Utc::now().to_rfc3339());
    root["enumerated_actions"] = serde_json::json!({
        "request_id": record.request_id,
        "success": record.success,
        "route": record.route,
        "total_clickable": record.total_clickable,
        "legacy_action_ids": record.legacy_action_ids,
        "actions": record.actions,
        "detail": record.detail,
        "at_ms": record.at_ms,
    });
    let _ = std::fs::write(
        &path,
        serde_json::to_string_pretty(&root).unwrap_or_default(),
    );
}

/// 点击回执写入 %TEMP%/cockpit-ui-state.json（含白名单拦截标记，便于批量脚本判定）。
fn write_last_click_into_ui_state(
    action_id: &str,
    success: bool,
    detail: Option<&str>,
    element_info: Option<&str>,
    blocked_by_whitelist: bool,
    danger_kind: Option<&str>,
) {
    let path = std::env::temp_dir().join("cockpit-ui-state.json");
    let mut root = match std::fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str::<serde_json::Value>(&raw)
            .unwrap_or_else(|_| serde_json::json!({})),
        Err(_) => serde_json::json!({}),
    };
    if !root.is_object() {
        root = serde_json::json!({});
    }
    root["timestamp"] = serde_json::json!(chrono::Utc::now().to_rfc3339());
    root["last_click"] = serde_json::json!({
        "action_id": action_id,
        "success": success,
        "detail": detail,
        "element_info": element_info,
        "blocked_by_whitelist": blocked_by_whitelist,
        "danger_kind": danger_kind,
        "at_ms": now_ms(),
    });
    let _ = std::fs::write(
        &path,
        serde_json::to_string_pretty(&root).unwrap_or_default(),
    );
}

/// 派发枚举并等待前端回执（写入 cockpit-ui-state.json）。
pub fn trigger_enumerate_wait(
    app: &AppHandle,
    timeout_ms: Option<u64>,
) -> Result<EnumerateAckRecord, String> {
    let request_id = next_request_id();
    let timeout = Duration::from_millis(timeout_ms.unwrap_or(8_000));
    let rx = register_enumerate_pending(&request_id);
    let payload = serde_json::json!({ "request_id": request_id });
    if let Err(e) = app.emit("gui:enumerate-actions", payload) {
        let _ = take_enumerate_pending(&request_id);
        return Err(fail_msg(
            "emit_failed",
            &format!("emit gui:enumerate-actions failed: {e}"),
        ));
    }
    crate::modules::logger::log_info(&format!(
        "[GuiClick] 已派发枚举 request_id={} timeout_ms={}",
        request_id,
        timeout.as_millis()
    ));
    match rx.recv_timeout(timeout) {
        Ok(ack) => {
            if ack.success {
                Ok(ack)
            } else {
                Err(fail_msg(
                    "enumerate_failed",
                    ack.detail.as_deref().unwrap_or("enumerate failed"),
                ))
            }
        }
        Err(_) => {
            let _ = take_enumerate_pending(&request_id);
            Err(fail_msg(
                "timeout_no_ack",
                &format!("enumerate request_id={request_id} 前端无回执"),
            ))
        }
    }
}

/// H4-1：从最近一次枚举回执里取 action_id 对应的元素描述，作为危险判定的 element_hint。
///
/// 以前这里恒传 `None`，Rust 只看裸 id，前端却看「id + 文案」，于是同一控件一端拦一端放。
/// 现在双端共用 `shared_click_guard` 的同一套判据，并且 Rust 侧也拿到元素信息。
///
/// 取不到就返回 `None`（判据退化为「只看 id」），**绝不因此放行**：动作词表里命中仍会拦。
pub fn element_hint_for(action_id: &str) -> Option<String> {
    let action_id = action_id.trim();
    if action_id.is_empty() {
        return None;
    }
    let record = get_last_enumerate_result()?;
    let actions = record.actions.as_array()?;
    for entry in actions {
        let id_match = entry
            .get("id")
            .and_then(|v| v.as_str())
            .map(|v| v == action_id)
            .unwrap_or(false);
        let legacy_match = entry
            .get("legacyActionId")
            .and_then(|v| v.as_str())
            .map(|v| v == action_id)
            .unwrap_or(false);
        if !id_match && !legacy_match {
            continue;
        }
        if let Some(info) = entry.get("elementInfo").and_then(|v| v.as_str()) {
            if !info.is_empty() {
                return Some(info.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::element_hint_for;
    use crate::modules::gui_in_app_click::enumerate_ack_from_frontend;
    use serde_json::json;

    fn seed_enumerate(actions: serde_json::Value) {
        enumerate_ack_from_frontend(
            Some("req-h4-1-test".to_string()),
            true,
            Some("/_root".to_string()),
            actions.as_array().map(|a| a.len()).unwrap_or(0),
            vec![],
            actions,
            Some("seed".to_string()),
        );
    }

    /// H4-1 问题 2：Rust 侧以前恒传 None，看不到元素信息。现在必须能取到。
    #[test]
    fn element_hint_is_resolved_from_last_enumerate() {
        seed_enumerate(json!([
            {
                "id": "enum/_root/input-自动重置机器码-1a2b-3",
                "legacyActionId": "cursor-renewal-xubei-toggle-machine-id",
                "elementInfo": "input type=checkbox \"自动重置机器码\" kind=toggle"
            },
            {
                "id": "enum/_root/button-删除账号-x1-7",
                "legacyActionId": "cursor-renewal-delete-account",
                "elementInfo": "button \"删除账号\" kind=action"
            }
        ]));

        // 按 legacy id 查
        assert_eq!(
            element_hint_for("cursor-renewal-xubei-toggle-machine-id").as_deref(),
            Some("input type=checkbox \"自动重置机器码\" kind=toggle")
        );
        // 按 enum id 查
        assert_eq!(
            element_hint_for("enum/_root/button-删除账号-x1-7").as_deref(),
            Some("button \"删除账号\" kind=action")
        );
        // 查不到 → None（退化为只看 id，动作词命中仍拦）
        assert_eq!(element_hint_for("no-such-action").as_deref(), None);
        assert_eq!(element_hint_for("").as_deref(), None);
    }

    /// 端到端：拿到 hint 后，偏好开关放行、真动作拦截 —— 双端同判据。
    #[test]
    fn guard_uses_element_hint_from_enumerate() {
        seed_enumerate(json!([
            {
                "id": "enum/_root/input-自动重置机器码-1a2b-3",
                "legacyActionId": "cursor-renewal-xubei-toggle-machine-id",
                "elementInfo": "input type=checkbox \"自动重置机器码\" kind=toggle"
            },
            {
                "id": "enum/_root/button-删除账号-x1-7",
                "legacyActionId": "cursor-renewal-delete-account",
                "elementInfo": "button \"删除账号\" kind=action"
            }
        ]));

        let toggle_id = "cursor-renewal-xubei-toggle-machine-id";
        let g = crate::modules::click_action_whitelist::evaluate_click_guard(
            toggle_id,
            element_hint_for(toggle_id).as_deref(),
        );
        assert!(g.allowed, "配置开关应可点");
        assert!(!g.dangerous, "配置开关不应判危险");

        let delete_id = "cursor-renewal-delete-account";
        let g = crate::modules::click_action_whitelist::evaluate_click_guard(
            delete_id,
            element_hint_for(delete_id).as_deref(),
        );
        assert!(!g.allowed, "真危险动作必须仍拦");
        assert_eq!(
            g.danger_kind,
            Some(crate::modules::click_action_whitelist::DangerKind::Delete)
        );
    }
}

/// 派发并**等待**前端真点击/明确失败。禁止 emit 即 Ok。
pub fn trigger_click_wait(
    app: &AppHandle,
    action_id: &str,
    timeout_ms: Option<u64>,
) -> Result<ClickResult, String> {
    let action_id = action_id.trim();
    if action_id.is_empty() {
        let msg = fail_msg("empty_action_id", "action_id 为空");
        record_dispatch_result(
            action_id,
            None,
            false,
            "empty_action_id",
            Some(msg.clone()),
            None,
            0,
        );
        return Err(msg);
    }

    // 模块七：危险操作默认拦截（删号/退出/重置/杀进程等），显式白名单才放行。
    // H4-1：不再恒传 None —— 从最近一次枚举回执取元素信息，保证与前端同一判据。
    let element_hint = element_hint_for(action_id);
    let guard = crate::modules::click_action_whitelist::evaluate_click_guard(
        action_id,
        element_hint.as_deref(),
    );
    if !guard.allowed {
        let msg = guard
            .block_reason
            .clone()
            .unwrap_or_else(|| fail_msg("blocked_by_whitelist", action_id));
        let element_info = Some(format!("action_id={action_id}"));
        record_dispatch_result(
            action_id,
            None,
            false,
            "blocked_by_whitelist",
            Some(msg.clone()),
            element_info.clone(),
            0,
        );
        push_ack_record(ClickAckRecord {
            action_id: action_id.to_string(),
            request_id: None,
            success: false,
            detail: Some(msg.clone()),
            element_info: element_info.clone(),
            at_ms: now_ms(),
        });
        write_last_click_into_ui_state(
            action_id,
            false,
            Some(msg.as_str()),
            element_info.as_deref(),
            true,
            guard.danger_kind.map(|k| k.as_str()),
        );
        crate::modules::logger::log_warn(&format!(
            "[GuiClick] 白名单拦截 action_id={} reason={}",
            action_id, msg
        ));
        return Err(msg);
    }

    if should_dedupe(action_id) {
        let latest = get_latest_ack_for(action_id);
        let hint = latest
            .as_ref()
            .map(|r| {
                format!(
                    "最近同 id ack success={} detail={:?} elem={:?} at_ms={}",
                    r.success, r.detail, r.element_info, r.at_ms
                )
            })
            .unwrap_or_else(|| "无最近同 id ack".to_string());
        crate::modules::logger::log_warn(&format!(
            "[GuiClick] 去重跳过（非已点击）action_id={} {}",
            action_id, hint
        ));
        let msg = fail_msg(
            "deduped_skipped",
            &format!("action_id={action_id} 在去重窗口内重复派发，不得冒充已点击；{hint}"),
        );
        record_dispatch_result(
            action_id,
            None,
            false,
            "deduped_skipped",
            Some(msg.clone()),
            None,
            0,
        );
        return Err(msg);
    }

    let request_id = next_request_id();
    let timeout = Duration::from_millis(timeout_ms.unwrap_or_else(|| default_timeout_ms(action_id)));
    let rx = register_pending(&request_id);
    let started = Instant::now();

    crate::modules::logger::log_info(&format!(
        "[GuiClick] 派发 action_id={} request_id={} timeout_ms={}（等待 ack）",
        action_id,
        request_id,
        timeout.as_millis()
    ));

    if action_id.starts_with("cursor-instance-start-") {
        // Cursor 多开在 Cursor 账号页的应用多开子 Tab，不是总控 instances 页。
        navigate_page(app, "cursor", Some("instances"));
        let app_clone = app.clone();
        let id = action_id.to_string();
        let rid = request_id.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(2800)).await;
            match emit_trigger_click(&app_clone, &id, &rid) {
                Ok(()) => crate::modules::logger::log_info(&format!(
                    "[GuiClick] 已 emit action_id={} request_id={}（多开页延迟后）",
                    id, rid
                )),
                Err(e) => {
                    crate::modules::logger::log_warn(&format!(
                        "[GuiClick] 派发失败 action_id={} request_id={} err={}",
                        id, rid, e
                    ));
                    // 唤醒等待方，避免干等到超时无原因
                    if let Some(pending) = take_pending(&rid) {
                        let _ = pending.tx.send(ClickAckRecord {
                            action_id: id,
                            request_id: Some(rid),
                            success: false,
                            detail: Some(e),
                            element_info: None,
                            at_ms: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_millis())
                                .unwrap_or(0),
                        });
                    }
                }
            }
        });
    } else if let Err(e) = emit_trigger_click(app, action_id, &request_id) {
        clear_pending(&request_id);
        record_dispatch_result(
            action_id,
            Some(&request_id),
            false,
            "emit_failed",
            Some(e.clone()),
            None,
            started.elapsed().as_millis() as u64,
        );
        return Err(e);
    }

    match rx.recv_timeout(timeout) {
        Ok(ack) => {
            let waited_ms = started.elapsed().as_millis() as u64;
            if ack.success {
                record_dispatch_result(
                    action_id,
                    Some(&request_id),
                    true,
                    "clicked",
                    ack.detail.clone(),
                    ack.element_info.clone(),
                    waited_ms,
                );
                crate::modules::logger::log_info(&format!(
                    "[GuiClick] 已点击 action_id={} request_id={} waited_ms={} elem={:?} detail={:?}",
                    action_id, request_id, waited_ms, ack.element_info, ack.detail
                ));
                Ok(ClickResult {
                    action_id: action_id.to_string(),
                    request_id,
                    success: true,
                    detail: ack.detail,
                    element_info: ack.element_info,
                    waited_ms,
                })
            } else {
                let code = if ack
                    .detail
                    .as_deref()
                    .unwrap_or("")
                    .starts_with("emit_failed")
                {
                    "emit_failed".to_string()
                } else {
                    classify_ack_failure(ack.detail.as_deref(), ack.element_info.as_deref())
                };
                let msg = fail_msg(
                    &code,
                    &format!(
                        "action_id={action_id} request_id={request_id} waited_ms={waited_ms} detail={:?} elem={:?}；请核对窗是否活着、是否在正确页、锚点是否存在",
                        ack.detail, ack.element_info
                    ),
                );
                record_dispatch_result(
                    action_id,
                    Some(&request_id),
                    false,
                    &code,
                    Some(msg.clone()),
                    ack.element_info.clone(),
                    waited_ms,
                );
                crate::modules::logger::log_warn(&format!("[GuiClick] 失败原因={msg}"));
                Err(msg)
            }
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            clear_pending(&request_id);
            let waited_ms = started.elapsed().as_millis() as u64;
            let latest = get_latest_ack_for(action_id);
            let old = latest
                .as_ref()
                .map(|r| {
                    format!(
                        "有旧 ack success={} detail={:?}（可能不是本轮 request_id）",
                        r.success, r.detail
                    )
                })
                .unwrap_or_else(|| "无旧 ack".to_string());
            let msg = fail_msg(
                "timeout_no_ack",
                &format!(
                    "action_id={action_id} request_id={request_id} waited_ms={waited_ms}；前端无回执（多半 WebView 未起/监听未挂/错页未挂载控件）；{old}"
                ),
            );
            record_dispatch_result(
                action_id,
                Some(&request_id),
                false,
                "timeout_no_ack",
                Some(msg.clone()),
                None,
                waited_ms,
            );
            crate::modules::logger::log_warn(&format!("[GuiClick] 失败原因={msg}"));
            Err(msg)
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            clear_pending(&request_id);
            let msg = fail_msg(
                "timeout_no_ack",
                &format!(
                    "action_id={action_id} request_id={request_id} pending 通道断开，未收到 ack"
                ),
            );
            record_dispatch_result(
                action_id,
                Some(&request_id),
                false,
                "timeout_no_ack",
                Some(msg.clone()),
                None,
                started.elapsed().as_millis() as u64,
            );
            Err(msg)
        }
    }
}

/// 兼容旧调用：内部走 wait（默认超时）。成功仅表示前端已真点击。
pub fn trigger_click(app: &AppHandle, action_id: &str) -> Result<(), String> {
    trigger_click_wait(app, action_id, None).map(|_| ())
}

/// 前端回执：控件是否找到并 click()，含元素信息用于验证。
pub fn ack_from_frontend(action_id: String, success: bool, detail: Option<String>) {
    ack_from_frontend_enhanced(action_id, None, success, detail, None);
}

/// 前端回执（增强版）：含 request_id / 元素信息。
pub fn ack_from_frontend_enhanced(
    action_id: String,
    request_id: Option<String>,
    success: bool,
    detail: Option<String>,
    element_info: Option<String>,
) {
    let action_id = action_id.trim().to_string();
    let extra = detail
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| format!(" ({})", s))
        .unwrap_or_default();
    if success {
        crate::modules::logger::log_info(&format!(
            "[GuiClick] 前端已点击 action_id={} request_id={:?}{} elem={:?}",
            action_id, request_id, extra, element_info,
        ));
    } else {
        crate::modules::logger::log_warn(&format!(
            "[GuiClick] 前端未点到控件 action_id={} request_id={:?}{} elem={:?}",
            action_id, request_id, extra, element_info,
        ));
    }
    let record = ClickAckRecord {
        action_id: action_id.clone(),
        request_id: request_id.clone(),
        success,
        detail: detail.clone(),
        element_info: element_info.clone(),
        at_ms: now_ms(),
    };
    push_ack_record(record.clone());
    let blocked = detail
        .as_deref()
        .map(|d| d.contains("blocked_by_whitelist"))
        .unwrap_or(false);
    write_last_click_into_ui_state(
        &action_id,
        success,
        detail.as_deref(),
        element_info.as_deref(),
        blocked,
        None,
    );

    if let Some(rid) = request_id.as_deref().filter(|s| !s.is_empty()) {
        if let Some(pending) = take_pending(rid) {
            let _ = pending.tx.send(record);
        }
    }
}
