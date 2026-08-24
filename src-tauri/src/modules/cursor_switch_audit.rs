use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;

use serde::Serialize;
use uuid::Uuid;

use crate::models::cursor::CursorAccount;
use crate::modules::logger;

const AUDIT_FILE_NAME: &str = "cursor_switch_audit.jsonl";

static AUDIT_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SwitchAuditPhase {
    SwitchStart,
    Pick,
    ProbePre,
    Inject,
    Launch,
    ProbePost,
    RefreshPost,
    StickCheck,
    UiErrorMark,
}

#[derive(Debug, Clone, Serialize)]
pub struct CursorSwitchAuditCtx {
    pub switch_trace_id: String,
    pub instance_id: String,
}

#[derive(Debug, Clone, Serialize)]
struct SwitchAuditRecord {
    ts: String,
    switch_trace_id: String,
    phase: SwitchAuditPhase,
    instance_id: Option<String>,
    account_id: Option<String>,
    email: Option<String>,
    pool: Option<String>,
    remaining_pct: Option<i32>,
    auto_used_pct: Option<i32>,
    total_used_pct: Option<i32>,
    outcome: String,
    error: Option<String>,
}

impl CursorSwitchAuditCtx {
    pub fn new(instance_id: impl Into<String>) -> Self {
        Self {
            switch_trace_id: Uuid::new_v4().to_string(),
            instance_id: instance_id.into(),
        }
    }
}

fn audit_log_path() -> Result<std::path::PathBuf, String> {
    Ok(logger::get_log_dir()?.join(AUDIT_FILE_NAME))
}

fn usage_fields(account: &CursorAccount) -> (Option<i32>, Option<i32>, Option<i32>) {
    crate::modules::cursor_account::cursor_switch_usage_snapshot(account)
}

fn append_record(record: SwitchAuditRecord) {
    let Ok(path) = audit_log_path() else {
        return;
    };
    let Ok(json) = serde_json::to_string(&record) else {
        return;
    };
    let _guard = AUDIT_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{}", json);
    }
}

fn write_event(
    ctx: Option<&CursorSwitchAuditCtx>,
    phase: SwitchAuditPhase,
    instance_id: Option<&str>,
    account: Option<&CursorAccount>,
    pool: Option<&str>,
    outcome: impl Into<String>,
    error: Option<String>,
) {
    let (remaining_pct, auto_used_pct, total_used_pct) = account
        .map(usage_fields)
        .unwrap_or((None, None, None));
    let pool_label = pool.map(str::to_string);
    let record = SwitchAuditRecord {
        ts: chrono::Local::now().to_rfc3339(),
        switch_trace_id: ctx
            .map(|value| value.switch_trace_id.clone())
            .unwrap_or_else(|| "no-trace".to_string()),
        phase,
        instance_id: instance_id
            .map(str::to_string)
            .or_else(|| ctx.map(|value| value.instance_id.clone())),
        account_id: account.map(|value| value.id.clone()),
        email: account.map(|value| value.email.clone()),
        pool: pool_label,
        remaining_pct,
        auto_used_pct,
        total_used_pct,
        outcome: outcome.into(),
        error,
    };
    append_record(record);
}

pub fn write_switch_start(ctx: &CursorSwitchAuditCtx, forced_account_id: Option<&str>) {
    write_event(
        Some(ctx),
        SwitchAuditPhase::SwitchStart,
        Some(&ctx.instance_id),
        None,
        None,
        if forced_account_id.is_some() {
            "explicit_row"
        } else {
            "auto_rotation"
        },
        forced_account_id.map(str::to_string),
    );
    logger::log_info(&format!(
        "[Cursor SwitchAudit] switch_start: switch_trace_id={}, instance_id={}, mode={}",
        ctx.switch_trace_id,
        ctx.instance_id,
        if forced_account_id.is_some() {
            "explicit_row"
        } else {
            "auto_rotation"
        }
    ));
}

pub fn write_pick(
    ctx: &CursorSwitchAuditCtx,
    account: &CursorAccount,
    pool: &str,
    candidates: usize,
) {
    write_event(
        Some(ctx),
        SwitchAuditPhase::Pick,
        Some(&ctx.instance_id),
        Some(account),
        Some(pool),
        format!("ok:candidates={}", candidates),
        None,
    );
}

pub fn write_pick_no_candidates(ctx: &CursorSwitchAuditCtx, error: &str) {
    write_event(
        Some(ctx),
        SwitchAuditPhase::Pick,
        Some(&ctx.instance_id),
        None,
        None,
        "no_candidates",
        Some(error.to_string()),
    );
}

pub fn write_probe_pre(
    ctx: &CursorSwitchAuditCtx,
    account: &CursorAccount,
    outcome: &str,
    error: Option<&str>,
) {
    write_event(
        Some(ctx),
        SwitchAuditPhase::ProbePre,
        Some(&ctx.instance_id),
        Some(account),
        None,
        outcome,
        error.map(str::to_string),
    );
}

pub fn write_inject(ctx: &CursorSwitchAuditCtx, account: &CursorAccount, outcome: &str, error: Option<&str>) {
    write_event(
        Some(ctx),
        SwitchAuditPhase::Inject,
        Some(&ctx.instance_id),
        Some(account),
        None,
        outcome,
        error.map(str::to_string),
    );
    if outcome == "ok" {
        logger::log_info(&format!(
            "[Cursor Switch] 切号落盘完成: switch_trace_id={}, account_id={}, email={}",
            ctx.switch_trace_id, account.id, account.email
        ));
    }
}

pub fn write_probe_post(
    ctx: &CursorSwitchAuditCtx,
    account: &CursorAccount,
    outcome: &str,
    error: Option<&str>,
) {
    write_event(
        Some(ctx),
        SwitchAuditPhase::ProbePost,
        Some(&ctx.instance_id),
        Some(account),
        None,
        outcome,
        error.map(str::to_string),
    );
    if outcome == "ok" {
        logger::log_info(&format!(
            "[Cursor Switch] 切号验收通过: switch_trace_id={}, account_id={}, email={}",
            ctx.switch_trace_id, account.id, account.email
        ));
    } else {
        logger::log_warn(&format!(
            "[Cursor Switch] 切号验收失败: switch_trace_id={}, account_id={}, email={}, 疑似登录页/会话失效: {}",
            ctx.switch_trace_id,
            account.id,
            account.email,
            error.unwrap_or(outcome)
        ));
    }
}

pub fn write_launch(ctx: &CursorSwitchAuditCtx, account: &CursorAccount, outcome: &str, error: Option<&str>) {
    write_event(
        Some(ctx),
        SwitchAuditPhase::Launch,
        Some(&ctx.instance_id),
        Some(account),
        None,
        outcome,
        error.map(str::to_string),
    );
}

pub fn write_refresh_post(
    switch_trace_id: &str,
    instance_id: &str,
    account: &CursorAccount,
    outcome: &str,
    error: Option<&str>,
) {
    let ctx = CursorSwitchAuditCtx {
        switch_trace_id: switch_trace_id.to_string(),
        instance_id: instance_id.to_string(),
    };
    write_event(
        Some(&ctx),
        SwitchAuditPhase::RefreshPost,
        Some(instance_id),
        Some(account),
        None,
        outcome,
        error.map(str::to_string),
    );
}

pub fn write_stick_check(
    ctx: &CursorSwitchAuditCtx,
    account: &CursorAccount,
    outcome: &str,
    error: Option<&str>,
) {
    write_event(
        Some(ctx),
        SwitchAuditPhase::StickCheck,
        Some(&ctx.instance_id),
        Some(account),
        None,
        outcome,
        error.map(str::to_string),
    );
    if outcome == "ok" {
        logger::log_info(&format!(
            "[Cursor Switch] 粘号复查通过: switch_trace_id={}, email={}",
            ctx.switch_trace_id, account.email
        ));
    } else {
        logger::log_warn(&format!(
            "[Cursor Switch] 粘号复查失败: switch_trace_id={}, expected={}, {}",
            ctx.switch_trace_id,
            account.email,
            error.unwrap_or(outcome)
        ));
    }
}

pub fn write_ui_error_mark(account_id: &str, email: &str, error: &str) {
    let account = CursorAccount {
        id: account_id.to_string(),
        email: email.to_string(),
        auth_id: None,
        name: None,
        tags: None,
        access_token: String::new(),
        refresh_token: None,
        membership_type: None,
        subscription_status: None,
        sign_up_type: None,
        cursor_auth_raw: None,
        cursor_usage_raw: None,
        status: None,
        status_reason: None,
        quota_query_last_error: Some(error.to_string()),
        quota_query_last_error_at: None,
        usage_updated_at: None,
        chat_probe: None,
        created_at: 0,
        last_used: 0,
    };
    write_event(
        None,
        SwitchAuditPhase::UiErrorMark,
        None,
        Some(&account),
        None,
        "marked",
        Some(error.to_string()),
    );
    logger::log_warn(&format!(
        "[Cursor SwitchAudit] ui_error_mark: account_id={}, email={}, error={}",
        account_id, email, error
    ));
}
