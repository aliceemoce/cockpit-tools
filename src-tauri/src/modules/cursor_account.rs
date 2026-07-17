use base64::Engine as _;
use rand::seq::SliceRandom;
use rand::RngCore;
use rusqlite::{Connection, OptionalExtension};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256, Sha512};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use uuid::Uuid;

use crate::models::cursor::{CursorAccount, CursorAccountIndex, CursorImportPayload};
use crate::modules::{account, logger};

const ACCOUNTS_INDEX_FILE: &str = "cursor_accounts.json";
const ACCOUNTS_DIR: &str = "cursor_accounts";
const LOCAL_IMPORT_BACKUPS_DIR: &str = "cursor_local_import_backups";
const CURSOR_QUOTA_ALERT_COOLDOWN_SECONDS: i64 = 10 * 60;
const CURSOR_ACCESS_TOKEN_REFRESH_THRESHOLD_SECONDS: i64 = 5 * 60;
const CURSOR_USAGE_QUERY_MIN_INTERVAL_SECONDS: i64 = 60;
const CURSOR_INDEX_MAINTENANCE_DELAY_SECS: u64 = 10 * 60;
const ARCHIVED_DUPLICATES_DIR: &str = "_archived_duplicates";
/// 用户可见：统一配额失败文案（禁止「会话已过期/失效」，见 HR-20260626-005 / HR-20260701-002）
const CURSOR_UI_QUOTA_QUERY_FAILED: &str = "配额查询失败";
const CURSOR_UI_QUOTA_AUTH_REIMPORT: &str = "配额查询失败，请重新导入账号";

fn cursor_quota_auth_reimport_for(email: &str) -> String {
    format!("{}: {}", CURSOR_UI_QUOTA_AUTH_REIMPORT, email)
}

lazy_static::lazy_static! {
    static ref CURSOR_ACCOUNT_INDEX_LOCK: Mutex<()> = Mutex::new(());
    static ref CURSOR_QUOTA_ALERT_LAST_SENT: Mutex<HashMap<String, i64>> = Mutex::new(HashMap::new());
    static ref CURSOR_INDEX_MAINTENANCE_DONE: AtomicBool = AtomicBool::new(false);
    static ref CURSOR_INDEX_MAINTENANCE_SCHEDULED: AtomicBool = AtomicBool::new(false);
    static ref CURSOR_REFRESH_IN_FLIGHT: Mutex<HashSet<String>> = Mutex::new(HashSet::new());
    /// 本进程内最近一次刷新尝试时间（成功/失败/transient 都记），用于最旧优先调度时给失败号冷却。
    static ref CURSOR_REFRESH_RECENT_ATTEMPTS: Mutex<HashMap<String, i64>> = Mutex::new(HashMap::new());
}

/// 自动全量刷新每轮默认最多刷多少个（仍串行；下一轮继续挑最旧）。
pub const CURSOR_AUTO_REFRESH_BATCH_SIZE: usize = 120;
/// 自动全量刷新每轮墙钟上限（秒），避免一轮占满整个 interval。
pub const CURSOR_AUTO_REFRESH_MAX_DURATION_SECS: u64 = 8 * 60;

pub fn index_maintenance_completed() -> bool {
    CURSOR_INDEX_MAINTENANCE_DONE.load(Ordering::Acquire)
}

/// Runs directory reconcile + quota-pool dedupe once, 10 minutes after first schedule.
pub fn schedule_index_maintenance_once() {
    if CURSOR_INDEX_MAINTENANCE_SCHEDULED.swap(true, Ordering::AcqRel) {
        return;
    }
    logger::log_info(&format!(
        "[Cursor Account] 已安排索引维护: delay={}s (单次)",
        CURSOR_INDEX_MAINTENANCE_DELAY_SECS
    ));
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(CURSOR_INDEX_MAINTENANCE_DELAY_SECS));
        if let Err(err) = run_index_maintenance_once() {
            logger::log_warn(&format!("[Cursor Account] 索引维护失败: {}", err));
        }
    });
}

fn now_ts() -> i64 {
    chrono::Utc::now().timestamp()
}

fn normalize_status_value(value: Option<&str>) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_ascii_lowercase())
        }
    })
}

fn is_banned_status(value: Option<&str>) -> bool {
    matches!(
        normalize_status_value(value).as_deref(),
        Some("banned") | Some("ban") | Some("forbidden")
    )
}

fn is_banned_reason(value: Option<&str>) -> bool {
    let Some(reason) = normalize_status_value(value) else {
        return false;
    };
    reason.contains("banned")
        || reason.contains("forbidden")
        || reason.contains("suspended")
        || reason.contains("disabled")
        || reason.contains("封禁")
        || reason.contains("禁用")
}

pub fn is_banned_account(account: &CursorAccount) -> bool {
    is_banned_status(account.status.as_deref())
        || is_banned_reason(account.status_reason.as_deref())
}

// ---------------------------------------------------------------------------
// Storage helpers
// ---------------------------------------------------------------------------

fn get_data_dir() -> Result<PathBuf, String> {
    account::get_data_dir()
}

fn get_accounts_dir() -> Result<PathBuf, String> {
    let base = get_data_dir()?;
    let dir = base.join(ACCOUNTS_DIR);
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| format!("创建 Cursor 账号目录失败: {}", e))?;
    }
    Ok(dir)
}

fn get_accounts_index_path() -> Result<PathBuf, String> {
    Ok(get_data_dir()?.join(ACCOUNTS_INDEX_FILE))
}

fn get_local_import_backups_dir() -> Result<PathBuf, String> {
    let dir = get_data_dir()?.join(LOCAL_IMPORT_BACKUPS_DIR);
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| format!("创建 Cursor 本机导入备份目录失败: {}", e))?;
    }
    Ok(dir)
}

pub fn accounts_index_path_string() -> Result<String, String> {
    Ok(get_accounts_index_path()?.to_string_lossy().to_string())
}

fn normalize_account_id(account_id: &str) -> Result<String, String> {
    let trimmed = account_id.trim();
    if trimmed.is_empty() {
        return Err("账号 ID 不能为空".to_string());
    }

    if trimmed.contains('/') || trimmed.contains('\\') || trimmed.contains("..") {
        return Err("账号 ID 非法，包含路径字符".to_string());
    }

    let valid = trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.');
    if !valid {
        return Err("账号 ID 非法，仅允许字母/数字/._-".to_string());
    }

    Ok(trimmed.to_string())
}

fn resolve_account_file_path(account_id: &str) -> Result<PathBuf, String> {
    let normalized = normalize_account_id(account_id)?;
    Ok(get_accounts_dir()?.join(format!("{}.json", normalized)))
}

#[derive(serde::Serialize, Deserialize, Clone)]
struct CursorLocalImportBackupSnapshot {
    created_at_ms: i64,
    email: String,
    action: String,
    payload: CursorImportPayload,
    account: CursorAccount,
}

fn sanitize_backup_file_segment(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for ch in value.chars() {
        let normalized = if ch.is_ascii_alphanumeric() { ch } else { '_' };
        output.push(normalized.to_ascii_lowercase());
    }
    let trimmed = output.trim_matches('_').to_string();
    if trimmed.is_empty() {
        "unknown".to_string()
    } else {
        trimmed
    }
}

fn write_local_import_backup(
    payload: &CursorImportPayload,
    outcome: &UpsertAccountOutcome,
) -> Result<PathBuf, String> {
    let backup_dir = get_local_import_backups_dir()?;
    let timestamp_ms = chrono::Utc::now().timestamp_millis();
    let email_segment = sanitize_backup_file_segment(payload.email.as_str());
    let file_name = format!(
        "{}-{}-{}.json",
        timestamp_ms, email_segment, outcome.account.id
    );
    let backup_path = backup_dir.join(file_name);
    let snapshot = CursorLocalImportBackupSnapshot {
        created_at_ms: timestamp_ms,
        email: payload.email.clone(),
        action: if outcome.created {
            "created".to_string()
        } else {
            "updated".to_string()
        },
        payload: payload.clone(),
        account: outcome.account.clone(),
    };
    let content = serde_json::to_string_pretty(&snapshot)
        .map_err(|e| format!("序列化 Cursor 本机导入备份失败: {}", e))?;
    crate::modules::atomic_write::write_string_atomic(&backup_path, &content)
        .map_err(|e| format!("写入 Cursor 本机导入备份失败: {}", e))?;
    Ok(backup_path)
}

fn record_import_backup(
    payload: &CursorImportPayload,
    outcome: &UpsertAccountOutcome,
) -> Result<PathBuf, String> {
    let backup_path = write_local_import_backup(payload, outcome)?;
    crate::modules::cursor_import_backup_sync::schedule_local_import_backup_upload(
        backup_path.clone(),
    );
    logger::log_info(&format!(
        "[Cursor Account] 导入备份已写入: email={}, backup={}",
        outcome.account.email,
        backup_path.display()
    ));
    Ok(backup_path)
}

pub fn upsert_import_payload(payload: CursorImportPayload) -> Result<CursorAccount, String> {
    let payload_for_backup = payload.clone();
    let outcome = upsert_account_with_outcome(payload)?;
    record_import_backup(&payload_for_backup, &outcome)?;
    Ok(outcome.account)
}

fn load_local_import_backup_snapshots() -> Vec<CursorLocalImportBackupSnapshot> {
    let backup_dir = match get_local_import_backups_dir() {
        Ok(dir) => dir,
        Err(err) => {
            logger::log_warn(&format!(
                "[Cursor Account] 读取本机导入备份目录失败，跳过补扫: {}",
                err
            ));
            return Vec::new();
        }
    };

    let entries = match fs::read_dir(&backup_dir) {
        Ok(value) => value,
        Err(err) => {
            logger::log_warn(&format!(
                "[Cursor Account] 枚举本机导入备份失败: path={}, error={}",
                backup_dir.display(),
                err
            ));
            return Vec::new();
        }
    };

    let mut snapshots = Vec::new();
    for entry in entries {
        let Ok(item) = entry else {
            continue;
        };
        let path = item.path();
        if !path.is_file() {
            continue;
        }
        let is_json = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("json"))
            .unwrap_or(false);
        if !is_json {
            continue;
        }
        let content = match fs::read_to_string(&path) {
            Ok(value) => value,
            Err(err) => {
                logger::log_warn(&format!(
                    "[Cursor Account] 读取本机导入备份失败: path={}, error={}",
                    path.display(),
                    err
                ));
                continue;
            }
        };
        match serde_json::from_str::<CursorLocalImportBackupSnapshot>(&content) {
            Ok(snapshot) => snapshots.push(snapshot),
            Err(err) => {
                logger::log_warn(&format!(
                    "[Cursor Account] 解析本机导入备份失败: path={}, error={}",
                    path.display(),
                    err
                ));
            }
        }
    }

    snapshots.sort_by_key(|snapshot| snapshot.created_at_ms);
    snapshots
}

// ---------------------------------------------------------------------------
// Account file operations
// ---------------------------------------------------------------------------

pub fn load_account(account_id: &str) -> Option<CursorAccount> {
    let account_path = resolve_account_file_path(account_id).ok()?;
    if !account_path.exists() {
        return None;
    }
    let content = fs::read_to_string(&account_path).ok()?;
    match crate::modules::secure_account_storage::deserialize_account_file::<CursorAccount>(&account_path, &content) {
        Ok((account, needs_rotation)) => {
            if needs_rotation {
                let account_for_rewrite = account.clone();
                crate::modules::deferred_account_rewrite::schedule_account_rewrite_if_unchanged(
                    "cursor",
                    account_for_rewrite.id.clone(),
                    account_path.clone(),
                    content.as_bytes(),
                    move || {
                        crate::modules::secure_account_storage::serialize_account_file(
                            "cursor",
                            &account_for_rewrite,
                        )
                    },
                );
            }
            Some(account)
        }
        Err(_) => None,
    }
}

fn save_account_file(account: &CursorAccount) -> Result<(), String> {
    let path = resolve_account_file_path(account.id.as_str())?;
    let content =
        crate::modules::secure_account_storage::serialize_account_file("cursor", account)?;
    crate::modules::atomic_write::write_string_atomic(&path, &content)
        .map_err(|e| format!("保存账号失败: {}", e))?;
    Ok(())
}

/// 持久化账号详情（供对话验活等模块写回 `chat_probe` 等字段）。
pub fn persist_account(account: &CursorAccount) -> Result<(), String> {
    save_account_file(account)?;
    let mut index = load_account_index();
    refresh_summary(&mut index, account);
    save_account_index(&index)?;
    Ok(())
}

fn delete_account_file(account_id: &str) -> Result<(), String> {
    let path = resolve_account_file_path(account_id)?;
    if path.exists() {
        crate::modules::atomic_write::remove_file_locked(&path)
            .map_err(|e| format!("删除账号文件失败: {}", e))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Index operations
// ---------------------------------------------------------------------------

fn load_account_index() -> CursorAccountIndex {
    let path = match get_accounts_index_path() {
        Ok(p) => p,
        Err(_) => return CursorAccountIndex::new(),
    };

    if !path.exists() {
        return CursorAccountIndex::new();
    }

    match fs::read_to_string(path.as_path()) {
        Ok(content) => match crate::modules::atomic_write::parse_json_with_auto_restore::<
            CursorAccountIndex,
        >(&path, &content)
        {
            Ok(index) => index,
            Err(err) => {
                logger::log_warn(&format!(
                    "[Cursor Account] 账号索引解析失败，使用空索引兜底: path={}, error={}",
                    path.display(),
                    err
                ));
                CursorAccountIndex::new()
            }
        },
        Err(err) => {
            logger::log_warn(&format!(
                "[Cursor Account] 读取账号索引失败，使用空索引兜底: path={}, error={}",
                path.display(),
                err
            ));
            CursorAccountIndex::new()
        }
    }
}

fn load_account_index_checked() -> Result<CursorAccountIndex, String> {
    let path = get_accounts_index_path()?;
    if !path.exists() {
        return Ok(CursorAccountIndex::new());
    }

    let content = match fs::read_to_string(path.as_path()) {
        Ok(content) => content,
        Err(err) => {
            if !collect_account_ids_from_directory().is_empty() {
                logger::log_warn(&format!(
                    "[Cursor Account] 读取账号索引失败，将按账号目录补扫恢复: path={}, error={}",
                    path.display(),
                    err
                ));
                return Ok(CursorAccountIndex::new());
            }
            return Err(format!("读取账号索引失败: {}", err));
        }
    };

    if content.trim().is_empty() {
        return Ok(CursorAccountIndex::new());
    }

    match crate::modules::atomic_write::parse_json_with_auto_restore::<CursorAccountIndex>(
        &path, &content,
    ) {
        Ok(index) => Ok(index),
        Err(err) => {
            if !collect_account_ids_from_directory().is_empty() {
                logger::log_warn(&format!(
                    "[Cursor Account] 账号索引解析失败，将按账号目录补扫恢复: path={}, error={}",
                    path.display(),
                    err
                ));
                return Ok(CursorAccountIndex::new());
            }
            Err(crate::error::file_corrupted_error(
                ACCOUNTS_INDEX_FILE,
                &path.to_string_lossy(),
                &err.to_string(),
            ))
        }
    }
}

fn save_account_index(index: &CursorAccountIndex) -> Result<(), String> {
    let path = get_accounts_index_path()?;
    let content =
        serde_json::to_string_pretty(index).map_err(|e| format!("序列化账号索引失败: {}", e))?;
    crate::modules::atomic_write::write_string_atomic(&path, &content)
        .map_err(|e| format!("写入账号索引失败: {}", e))?;
    Ok(())
}

fn refresh_summary(index: &mut CursorAccountIndex, account: &CursorAccount) {
    if let Some(summary) = index.accounts.iter_mut().find(|item| item.id == account.id) {
        *summary = account.summary();
        return;
    }
    index.accounts.push(account.summary());
}

fn upsert_account_record(account: CursorAccount) -> Result<CursorAccount, String> {
    let _lock = CURSOR_ACCOUNT_INDEX_LOCK
        .lock()
        .map_err(|_| "获取 Cursor 账号锁失败".to_string())?;
    let mut index = load_account_index();
    save_account_file(&account)?;
    refresh_summary(&mut index, &account);
    save_account_index(&index)?;
    Ok(account)
}

fn persist_quota_query_error(account_id: &str, message: &str) {
    let Some(mut account) = load_account(account_id) else {
        return;
    };
    account.quota_query_last_error = Some(message.to_string());
    account.quota_query_last_error_at = Some(chrono::Utc::now().timestamp_millis());
    let email = account.email.clone();
    let _ = upsert_account_record(account);
    logger::log_warn(&format!(
        "[Cursor Account] 标记账号失败(将显示红色): account_id={}, error={}",
        account_id, message
    ));
    crate::modules::cursor_switch_audit::write_ui_error_mark(account_id, &email, message);
}

pub(crate) fn is_cursor_transient_quota_error(message: &str) -> bool {
    let lower = message.to_lowercase();
    lower.contains("error sending request")
        || lower.contains("timed out")
        || lower.contains("timeout")
        || lower.contains("connection")
        || lower.contains("connect error")
        || lower.contains("dns")
        || lower.contains("resolve")
        || lower.contains("proxy")
        || lower.contains("tunnel")
        || lower.contains("502")
        || lower.contains("503")
        || lower.contains("504")
}

fn is_cursor_auth_quota_error(message: &str) -> bool {
    let lower = message.to_lowercase();
    lower.contains(CURSOR_UI_QUOTA_QUERY_FAILED)
        || lower.contains("会话已过期")
        || lower.contains("会话已失效")
        || lower.contains("未认证")
        || lower.contains("请重新导入")
        || lower.contains("请重新登录")
        || lower.contains("登录会话已过期")
        || lower.contains("授权已失效")
        || lower.contains("凭证无效")
        || lower.contains("凭证失效")
        || lower.contains("token已失效")
        || lower.contains("token 已失效")
        || lower.contains("登录授权已过期")
        || lower.contains("refresh token 已失效")
        || lower.contains("session expired")
        || lower.contains("invalid credentials")
        || lower.contains("unauthenticated")
        || lower.contains("re-import")
        || lower.contains("re-login")
}

/// 镜像账号总览 `isAbnormalAccount`（CursorAccountsPage.tsx）。
pub fn is_cursor_overview_abnormal(account: &CursorAccount) -> bool {
    is_banned_account(account)
        || account
            .status
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case("error"))
        || account
            .quota_query_last_error
            .as_deref()
            .is_some_and(is_cursor_auth_quota_error)
}

/// 镜像账号总览 `resolveRemainingQuotaPercent`。
pub fn cursor_overview_remaining_percent(account: &CursorAccount) -> Option<i32> {
    cursor_switch_remaining_percent(account)
}

/// 多开/实例自动挑号：与账号总览同一套「有效且有额度」判定。
pub fn ensure_cursor_overview_pickable(account: &CursorAccount) -> Result<(), String> {
    if is_cursor_overview_abnormal(account) {
        if is_banned_account(account) {
            return Err(format!("Cursor 账号已被标记不可用: {}", account.email));
        }
        if account
            .quota_query_last_error
            .as_deref()
            .is_some_and(is_cursor_auth_quota_error)
        {
            return Err(cursor_quota_auth_reimport_for(&account.email));
        }
        return Err(format!("Cursor 账号状态异常: {}", account.email));
    }
    if !has_nirvana_switch_ready_tokens(account) {
        return Err(format!(
            "Cursor 账号缺少可切号 token，请重新导入账号: {}",
            account.email
        ));
    }
    match cursor_overview_remaining_percent(account) {
        None => Err(format!(
            "Cursor 账号配额不可用（与总览一致），已跳过: {}",
            account.email
        )),
        Some(remaining) if remaining <= 0 => Err(format!(
            "Cursor 账号额度已耗尽（与总览一致），已跳过: {}",
            account.email
        )),
        Some(_) => Ok(()),
    }
}

/// 切号前拉实时额度并复用总览 pick 判定，避免磁盘缓存 remaining 与 API 已耗尽仍 inject。
pub async fn refresh_and_ensure_overview_pickable(account_id: &str) -> Result<CursorAccount, String> {
    let refreshed = refresh_account_fast_async(account_id).await?;
    ensure_cursor_overview_pickable(&refreshed.account)?;
    Ok(refreshed.account)
}

/// 用户显式选号 Play：尽力刷新；不因配额/封禁/磁盘失败标记阻断（仅缺 token 硬拦）。自动轮换仍走 pickable。
pub async fn refresh_for_forced_account_switch(account_id: &str) -> Result<CursorAccount, String> {
    let account = match refresh_account_fast_async(account_id).await {
        Ok(refreshed) => refreshed.account,
        Err(err) if is_cursor_transient_quota_error(&err) => load_account(account_id)
            .ok_or_else(|| format!("Cursor 账号不存在: {}", account_id))?,
        Err(err) => {
            logger::log_warn(&format!(
                "[Cursor Switch] 手动选号刷新失败，仍继续切号: id={}, error={}",
                account_id, err
            ));
            load_account(account_id)
                .ok_or_else(|| format!("Cursor 账号不存在: {}", account_id))?
        }
    };
    ensure_cursor_manual_pick_allowed(&account)?;
    Ok(account)
}

/// 用户手动点 Play/多开指定账号：只校验能否落盘 token，不拦 pending/失败标记/额度耗尽/封禁。
pub fn ensure_cursor_manual_pick_allowed(account: &CursorAccount) -> Result<(), String> {
    if !has_nirvana_switch_ready_tokens(account) {
        return Err(format!(
            "Cursor 账号缺少可切号 token，请重新导入账号: {}",
            account.email
        ));
    }
    Ok(())
}

fn ensure_cursor_switch_allowed(account: &CursorAccount, manual_user_pick: bool) -> Result<(), String> {
    if manual_user_pick {
        return ensure_cursor_manual_pick_allowed(account);
    }
    ensure_cursor_switch_ready_account(account)
}

pub fn account_has_auth_failure_marker(account: &CursorAccount) -> bool {
    account
        .quota_query_last_error
        .as_deref()
        .is_some_and(is_cursor_auth_quota_error)
        || account
            .status_reason
            .as_deref()
            .is_some_and(is_cursor_auth_quota_error)
        || account
            .status
            .as_deref()
            .is_some_and(is_cursor_auth_quota_error)
}

pub struct CursorRefreshResult {
    pub account: CursorAccount,
    pub persisted: bool,
}

fn cursor_accounts_differ_for_refresh(before: &CursorAccount, after: &CursorAccount) -> bool {
    before.quota_query_last_error != after.quota_query_last_error
        || before.quota_query_last_error_at != after.quota_query_last_error_at
        || before.cursor_usage_raw != after.cursor_usage_raw
        || before.usage_updated_at != after.usage_updated_at
        || before.last_used != after.last_used
        || before.access_token != after.access_token
        || before.refresh_token != after.refresh_token
        || before.membership_type != after.membership_type
        || before.subscription_status != after.subscription_status
        || before.sign_up_type != after.sign_up_type
        || before.cursor_auth_raw != after.cursor_auth_raw
}

pub fn cursor_refresh_persisted(before: &CursorAccount, after: &CursorAccount) -> bool {
    cursor_accounts_differ_for_refresh(before, after)
}

// ---------------------------------------------------------------------------
// Identity helpers
// ---------------------------------------------------------------------------

fn normalize_non_empty(value: Option<&str>) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn normalize_email_identity(value: Option<&str>) -> Option<String> {
    normalize_non_empty(value).and_then(|raw| {
        let lowered = raw.to_lowercase();
        if lowered.contains('@') {
            Some(lowered)
        } else {
            None
        }
    })
}

fn normalize_token_identity(value: Option<&str>) -> Option<String> {
    normalize_non_empty(value)
}

fn normalize_auth_identity(value: Option<&str>) -> Option<String> {
    normalize_non_empty(value)
}

/// 无忧 Lc/Kh：vscdb 里 `cursorAuth/accessToken` = 裸 JWT，`cursorAuth/refreshToken` = 完整 `user_id::jwt` session。
fn bare_jwt_from_token(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("token 为空".to_string());
    }
    if let Some((_, jwt)) = trimmed.split_once("::") {
        let jwt = jwt.trim();
        if jwt.is_empty() {
            return Err("session token 缺少 JWT 部分".to_string());
        }
        return Ok(jwt.to_string());
    }
    Ok(trimmed.to_string())
}

fn resolve_vscdb_auth_tokens_from_parts(
    access_token: &str,
    refresh_token: Option<&str>,
) -> Result<(String, String), String> {
    let access_raw = access_token.trim();
    if access_raw.is_empty() {
        return Err("access_token 为空".to_string());
    }

    let refresh_raw = refresh_token.map(str::trim).filter(|v| !v.is_empty());

    let session_token = if let Some(rt) = refresh_raw {
        if rt.contains("::") {
            rt.to_string()
        } else {
            let jwt = bare_jwt_from_token(access_raw)?;
            let user_id = extract_workos_user_id(&jwt).ok_or_else(|| {
                "无法从 access_token 解析 WorkOS user_id，请重新导入账号".to_string()
            })?;
            format!("{}::{}", user_id, jwt)
        }
    } else if access_raw.contains("::") {
        access_raw.to_string()
    } else {
        let user_id = extract_workos_user_id(access_raw).ok_or_else(|| {
            "账号缺少 refresh_token，且无法从 access_token 构建 session，请删除后重新导入"
                .to_string()
        })?;
        format!("{}::{}", user_id, access_raw)
    };

    let access_jwt = bare_jwt_from_token(
        session_token
            .split_once("::")
            .map(|(_, jwt)| jwt)
            .unwrap_or(access_raw),
    )?;

    if access_jwt.split('.').count() < 2 {
        return Err("access_token 不是有效 JWT，请重新导入账号".to_string());
    }

    Ok((access_jwt, session_token))
}

fn resolve_vscdb_auth_tokens(account: &CursorAccount) -> Result<(String, String), String> {
    resolve_vscdb_auth_tokens_from_parts(
        account.access_token.as_str(),
        account.refresh_token.as_deref(),
    )
    .map_err(|err| format!("账号 {} ({}): {}", account.email, account.id, err))
}

fn normalize_account_tokens(account: &mut CursorAccount) -> Result<(), String> {
    let (access_jwt, session_token) = resolve_vscdb_auth_tokens(account)?;
    account.access_token = access_jwt.clone();
    account.refresh_token = Some(session_token.clone());
    upsert_cursor_auth_raw_string(account, "accessToken", Some(access_jwt));
    upsert_cursor_auth_raw_string(account, "refreshToken", Some(session_token));
    Ok(())
}

// checkpoint_vscdb 已移除——13GB+ DB 上 WAL checkpoint 不可靠，
// 改为在 switch_tokens_in_profile_db / write_cursor_auth_fields_to_conn
// 中使用 DELETE journal mode + BEGIN/COMMIT 确保写入持久化。

fn decode_access_token_payload(access_token: &str) -> Option<serde_json::Value> {
    let parts: Vec<&str> = access_token.split('.').collect();
    if parts.len() < 2 {
        return None;
    }

    let payload_b64 = parts[1].replace('-', "+").replace('_', "/");
    let padded = match payload_b64.len() % 4 {
        2 => format!("{}==", payload_b64),
        3 => format!("{}=", payload_b64),
        _ => payload_b64,
    };

    let decoded = base64::engine::general_purpose::STANDARD
        .decode(padded)
        .ok()?;
    serde_json::from_slice(&decoded).ok()
}

fn extract_auth_id_from_access_token(access_token: &str) -> Option<String> {
    let value = decode_access_token_payload(access_token)?;
    normalize_non_empty(value.get("sub").and_then(|raw| raw.as_str()))
}

fn extract_access_token_exp(access_token: &str) -> Option<i64> {
    let value = decode_access_token_payload(access_token)?;
    value.get("exp").and_then(|raw| raw.as_i64())
}

fn access_token_needs_refresh(access_token: &str) -> bool {
    let Some(exp) = extract_access_token_exp(access_token) else {
        return true;
    };
    exp <= now_ts() + CURSOR_ACCESS_TOKEN_REFRESH_THRESHOLD_SECONDS
}

fn extract_auth_id_from_raw_value(raw: Option<&Value>) -> Option<String> {
    let obj = raw.and_then(|value| value.as_object())?;

    normalize_auth_identity(
        obj.get("authId")
            .and_then(|value| value.as_str())
            .or_else(|| obj.get("auth_id").and_then(|value| value.as_str()))
            .or_else(|| obj.get("workosId").and_then(|value| value.as_str()))
            .or_else(|| obj.get("workos_id").and_then(|value| value.as_str())),
    )
}

fn is_cursor_placeholder_auth_id(value: &str) -> bool {
    value.starts_with("user_cursor_")
}

fn normalize_quota_pool_auth_id(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || is_cursor_placeholder_auth_id(trimmed) {
        return None;
    }

    if let Some(user_id) = extract_workos_user_id(trimmed) {
        return Some(user_id);
    }

    if trimmed.starts_with("auth0|") {
        let user_id = trimmed.rsplit('|').next().unwrap_or(trimmed);
        if user_id.starts_with("user_") {
            return Some(user_id.to_string());
        }
    }

    if trimmed.starts_with("user_") {
        return Some(trimmed.to_string());
    }

    Some(trimmed.to_string())
}

fn resolve_quota_pool_id(account: &CursorAccount) -> Option<String> {
    extract_workos_id(account)
        .as_deref()
        .and_then(normalize_quota_pool_auth_id)
        .or_else(|| {
            extract_workos_user_id(account.access_token.as_str())
                .as_deref()
                .and_then(normalize_quota_pool_auth_id)
        })
        .or_else(|| {
            extract_auth_id_from_raw_value(account.cursor_auth_raw.as_ref())
                .as_deref()
                .and_then(normalize_quota_pool_auth_id)
        })
        .or_else(|| {
            account
                .auth_id
                .as_deref()
                .and_then(normalize_quota_pool_auth_id)
        })
}

fn backfill_quota_pool_auth_id(account: &mut CursorAccount) {
    let Some(pool_id) = resolve_quota_pool_id(account) else {
        return;
    };
    account.auth_id = Some(pool_id.clone());
    upsert_cursor_auth_raw_string(account, "authId", Some(pool_id.clone()));
    upsert_cursor_auth_raw_string(account, "workosId", Some(pool_id));
}

fn resolve_payload_auth_id(payload: &CursorImportPayload) -> Option<String> {
    normalize_auth_identity(payload.auth_id.as_deref())
        .or_else(|| extract_auth_id_from_raw_value(payload.cursor_auth_raw.as_ref()))
        .or_else(|| extract_auth_id_from_access_token(payload.access_token.as_str()))
}

fn resolve_account_auth_id(account: &CursorAccount) -> Option<String> {
    normalize_auth_identity(account.auth_id.as_deref())
        .or_else(|| extract_auth_id_from_raw_value(account.cursor_auth_raw.as_ref()))
        .or_else(|| extract_auth_id_from_access_token(account.access_token.as_str()))
}

fn cursor_auth_raw_object_mut(account: &mut CursorAccount) -> &mut serde_json::Map<String, Value> {
    if !matches!(account.cursor_auth_raw, Some(Value::Object(_))) {
        account.cursor_auth_raw = Some(Value::Object(serde_json::Map::new()));
    }

    match account.cursor_auth_raw.as_mut() {
        Some(Value::Object(obj)) => obj,
        _ => unreachable!("cursor_auth_raw 应始终为对象"),
    }
}

fn upsert_cursor_auth_raw_string(account: &mut CursorAccount, key: &str, value: Option<String>) {
    let Some(text) = normalize_non_empty(value.as_deref()) else {
        return;
    };
    cursor_auth_raw_object_mut(account).insert(key.to_string(), Value::String(text));
}

fn upsert_cursor_auth_raw_bool(account: &mut CursorAccount, key: &str, value: Option<bool>) {
    let Some(flag) = value else {
        return;
    };
    cursor_auth_raw_object_mut(account).insert(key.to_string(), Value::Bool(flag));
}

fn normalize_cursor_sign_up_type(value: Option<&str>) -> Option<String> {
    let raw = normalize_non_empty(value)?;
    match raw.as_str() {
        "SIGN_UP_TYPE_AUTH_0" => Some("Auth_0".to_string()),
        "SIGN_UP_TYPE_GOOGLE" => Some("Google".to_string()),
        "SIGN_UP_TYPE_GITHUB" => Some("Github".to_string()),
        "SIGN_UP_TYPE_WORKOS" => Some("WorkOS".to_string()),
        _ => Some(raw),
    }
}

/// 仅以「共享同一 Cursor 额度池」判定重复：相同 workosId 或相同 JWT sub。
/// 同邮箱但 workosId 不同 → 视为不同账号，不合并。
fn extract_workos_id(account: &CursorAccount) -> Option<String> {
    let raw = account.cursor_auth_raw.as_ref()?;
    let obj = raw.as_object()?;
    for key in ["workosId", "workos_id"] {
        if let Some(value) = obj.get(key).and_then(|v| v.as_str()) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn extract_access_token_subject(access_token: &str) -> Option<String> {
    let payload = decode_access_token_payload(access_token)?;
    payload
        .get("sub")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn cursor_quota_pool_key(account: &CursorAccount) -> Option<String> {
    if let Some(workos_id) = extract_workos_id(account) {
        return Some(format!("workos:{}", workos_id));
    }
    extract_access_token_subject(&account.access_token).map(|sub| format!("sub:{}", sub))
}

fn accounts_are_duplicates(left: &CursorAccount, right: &CursorAccount) -> bool {
    let left_email = normalize_email_identity(Some(left.email.as_str()));
    let right_email = normalize_email_identity(Some(right.email.as_str()));
    matches!(
        (left_email.as_ref(), right_email.as_ref()),
        (Some(le), Some(re)) if le == re
    )
}

fn archive_duplicate_account_file(account_id: &str) -> Result<(), String> {
    let accounts_dir = get_accounts_dir()?;
    let src = accounts_dir.join(format!("{}.json", account_id));
    if !src.is_file() {
        return Ok(());
    }
    let archive_dir = accounts_dir.join(ARCHIVED_DUPLICATES_DIR);
    fs::create_dir_all(&archive_dir).map_err(|e| format!("创建重复账号归档目录失败: {}", e))?;
    let mut dst = archive_dir.join(format!("{}.json", account_id));
    if dst.exists() {
        dst = archive_dir.join(format!("{}_{}.json", account_id, now_ts()));
    }
    fs::rename(&src, &dst)
        .map_err(|e| format!("归档重复账号文件失败: id={}, error={}", account_id, e))?;
    logger::log_info(&format!(
        "[Cursor Account] 重复账号文件已归档: id={}, path={}",
        account_id,
        dst.display()
    ));
    Ok(())
}

// ---------------------------------------------------------------------------
// Merge helpers
// ---------------------------------------------------------------------------

fn merge_string_list(
    primary: Option<Vec<String>>,
    secondary: Option<Vec<String>>,
) -> Option<Vec<String>> {
    let mut merged = Vec::new();
    let mut seen = HashSet::new();

    for source in [primary, secondary] {
        if let Some(values) = source {
            for value in values {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let key = trimmed.to_lowercase();
                if seen.insert(key) {
                    merged.push(trimmed.to_string());
                }
            }
        }
    }

    if merged.is_empty() {
        None
    } else {
        Some(merged)
    }
}

fn fill_if_empty_string(target: &mut String, source: &str) {
    if target.trim().is_empty() {
        let incoming = source.trim();
        if !incoming.is_empty() {
            *target = incoming.to_string();
        }
    }
}

fn fill_if_none<T: Clone>(target: &mut Option<T>, source: &Option<T>) {
    if target.is_none() {
        *target = source.clone();
    }
}

fn merge_duplicate_account(primary: &mut CursorAccount, duplicate: &CursorAccount) {
    fill_if_empty_string(&mut primary.email, duplicate.email.as_str());
    fill_if_empty_string(&mut primary.access_token, duplicate.access_token.as_str());

    fill_if_none(&mut primary.auth_id, &duplicate.auth_id);
    fill_if_none(&mut primary.name, &duplicate.name);
    fill_if_none(&mut primary.refresh_token, &duplicate.refresh_token);
    fill_if_none(&mut primary.membership_type, &duplicate.membership_type);
    fill_if_none(
        &mut primary.subscription_status,
        &duplicate.subscription_status,
    );
    fill_if_none(&mut primary.sign_up_type, &duplicate.sign_up_type);
    fill_if_none(&mut primary.cursor_auth_raw, &duplicate.cursor_auth_raw);
    fill_if_none(&mut primary.cursor_usage_raw, &duplicate.cursor_usage_raw);
    fill_if_none(&mut primary.status, &duplicate.status);
    fill_if_none(&mut primary.status_reason, &duplicate.status_reason);

    primary.tags = merge_string_list(primary.tags.clone(), duplicate.tags.clone());
    primary.created_at = primary.created_at.min(duplicate.created_at);
    primary.last_used = primary.last_used.max(duplicate.last_used);
}

fn choose_primary_account_index(group: &[usize], accounts: &[CursorAccount]) -> usize {
    group
        .iter()
        .copied()
        .max_by(|left, right| {
            let left_account = &accounts[*left];
            let right_account = &accounts[*right];
            left_account
                .last_used
                .cmp(&right_account.last_used)
                .then_with(|| right_account.created_at.cmp(&left_account.created_at))
        })
        .unwrap_or(group[0])
}

fn collect_account_ids_from_directory() -> Vec<String> {
    let accounts_dir = match get_accounts_dir() {
        Ok(dir) => dir,
        Err(err) => {
            logger::log_warn(&format!(
                "[Cursor Account] 获取账号目录失败，跳过目录补扫: {}",
                err
            ));
            return Vec::new();
        }
    };

    let entries = match fs::read_dir(&accounts_dir) {
        Ok(value) => value,
        Err(err) => {
            logger::log_warn(&format!(
                "[Cursor Account] 读取账号目录失败，跳过目录补扫: path={}, error={}",
                accounts_dir.display(),
                err
            ));
            return Vec::new();
        }
    };

    let mut ids = Vec::new();
    for entry in entries {
        let Ok(item) = entry else {
            continue;
        };
        let path = item.path();
        if !path.is_file() {
            continue;
        }

        let is_json = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("json"))
            .unwrap_or(false);
        if !is_json {
            continue;
        }

        let Some(stem) = path.file_stem().and_then(|name| name.to_str()) else {
            continue;
        };
        let Ok(account_id) = normalize_account_id(stem) else {
            logger::log_warn(&format!(
                "[Cursor Account] 检测到非法账号文件名，已忽略: file={}",
                path.display()
            ));
            continue;
        };
        ids.push(account_id);
    }

    ids.sort();
    ids.dedup();
    ids
}

fn normalize_account_index(index: &mut CursorAccountIndex) -> Vec<CursorAccount> {
    let mut loaded_accounts = Vec::new();
    let mut seen_account_ids = HashSet::new();
    let mut seen_summary_ids = HashSet::new();

    for summary in &index.accounts {
        if !seen_summary_ids.insert(summary.id.clone()) {
            continue;
        }
        if let Some(account) = load_account(&summary.id) {
            if seen_account_ids.insert(account.id.clone()) {
                loaded_accounts.push(account);
            }
        }
    }

    let mut recovered_count = 0usize;
    for account_id in collect_account_ids_from_directory() {
        if seen_account_ids.contains(&account_id) {
            continue;
        }
        if let Some(account) = load_account(&account_id) {
            if seen_account_ids.insert(account.id.clone()) {
                if !seen_summary_ids.contains(&account_id) {
                    recovered_count += 1;
                }
                loaded_accounts.push(account);
            }
        }
    }
    if recovered_count > 0 {
        logger::log_warn(&format!(
            "[Cursor Account] 检测到索引缺失，已从账号目录恢复 {} 个账号",
            recovered_count
        ));
    }

    if loaded_accounts.len() <= 1 {
        index.accounts = loaded_accounts
            .iter()
            .map(|account| account.summary())
            .collect();
        return loaded_accounts;
    }

    let mut parents: Vec<usize> = (0..loaded_accounts.len()).collect();

    fn find(parents: &mut [usize], idx: usize) -> usize {
        let parent = parents[idx];
        if parent == idx {
            return idx;
        }
        let root = find(parents, parent);
        parents[idx] = root;
        root
    }

    fn union(parents: &mut [usize], left: usize, right: usize) {
        let left_root = find(parents, left);
        let right_root = find(parents, right);
        if left_root != right_root {
            parents[right_root] = left_root;
        }
    }

    let total = loaded_accounts.len();
    for left in 0..total {
        for right in (left + 1)..total {
            if accounts_are_duplicates(&loaded_accounts[left], &loaded_accounts[right]) {
                union(&mut parents, left, right);
            }
        }
    }

    let mut grouped: HashMap<usize, Vec<usize>> = HashMap::new();
    for idx in 0..total {
        let root = find(&mut parents, idx);
        grouped.entry(root).or_default().push(idx);
    }

    let mut processed_roots = HashSet::new();
    let mut normalized_accounts = Vec::new();
    let mut removed_ids = Vec::new();
    for idx in 0..total {
        let root = find(&mut parents, idx);
        if !processed_roots.insert(root) {
            continue;
        }
        let Some(group) = grouped.get(&root) else {
            continue;
        };

        if group.len() == 1 {
            normalized_accounts.push(loaded_accounts[group[0]].clone());
            continue;
        }

        let primary_idx = choose_primary_account_index(group, &loaded_accounts);
        let mut primary = loaded_accounts[primary_idx].clone();
        for member in group {
            if *member == primary_idx {
                continue;
            }
            merge_duplicate_account(&mut primary, &loaded_accounts[*member]);
            removed_ids.push(loaded_accounts[*member].id.clone());
        }

        normalized_accounts.push(primary);
    }

    if !removed_ids.is_empty() {
        for account in &normalized_accounts {
            if let Err(err) = save_account_file(account) {
                logger::log_warn(&format!(
                    "[Cursor Account] 保存邮箱去重账号失败: id={}, error={}",
                    account.id, err
                ));
            }
        }
        for account_id in &removed_ids {
            if let Err(err) = archive_duplicate_account_file(account_id) {
                logger::log_warn(&format!(
                    "[Cursor Account] 归档重复邮箱账号失败: id={}, error={}",
                    account_id, err
                ));
            }
        }
        logger::log_warn(&format!(
            "[Cursor Account] 邮箱去重: merged_count={}, removed_ids={}",
            removed_ids.len(),
            removed_ids.join(",")
        ));
    }

    index.accounts = normalized_accounts
        .iter()
        .map(|account| account.summary())
        .collect();
    normalized_accounts
}

fn list_accounts_from_index(index: &CursorAccountIndex) -> Vec<CursorAccount> {
    let mut accounts = Vec::new();
    let mut seen = HashSet::new();
    for summary in &index.accounts {
        if !seen.insert(summary.id.clone()) {
            continue;
        }
        if let Some(mut account) = load_account(&summary.id) {
            backfill_quota_pool_auth_id(&mut account);
            accounts.push(account);
        }
    }
    accounts
}

fn run_index_maintenance_once() -> Result<(), String> {
    if index_maintenance_completed() {
        return Ok(());
    }
    let _lock = CURSOR_ACCOUNT_INDEX_LOCK
        .lock()
        .map_err(|_| "获取 Cursor 账号锁失败".to_string())?;
    logger::log_info("[Cursor Account] 开始单次索引维护(目录补扫 + 邮箱去重合并)");
    let started = std::time::Instant::now();
    let mut index = load_account_index();
    let had_index_accounts = !index.accounts.is_empty();
    let accounts = normalize_account_index(&mut index);
    if had_index_accounts && accounts.is_empty() {
        logger::log_warn("[Cursor Account] 索引维护后无有效账号，保留原索引不写回");
    } else if let Err(err) = save_account_index(&index) {
        logger::log_warn(&format!("[Cursor Account] 索引维护保存失败: {}", err));
    }
    CURSOR_INDEX_MAINTENANCE_DONE.store(true, Ordering::Release);
    logger::log_info(&format!(
        "[Cursor Account] 索引维护完成: accounts={}, elapsed={}ms",
        accounts.len(),
        started.elapsed().as_millis()
    ));
    Ok(())
}

fn collect_live_account_emails() -> HashSet<String> {
    let mut emails = HashSet::new();
    for account_id in collect_account_ids_from_directory() {
        let Some(account) = load_account(&account_id) else {
            continue;
        };
        if let Some(email) = normalize_email_identity(Some(account.email.as_str())) {
            emails.insert(email);
        }
    }
    emails
}

// ---------------------------------------------------------------------------
// CRUD
// ---------------------------------------------------------------------------

pub fn list_accounts() -> Vec<CursorAccount> {
    let _lock = CURSOR_ACCOUNT_INDEX_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut index = load_account_index();
    let had_index_accounts = !index.accounts.is_empty();
    let index_before_normalize = serde_json::to_vec(&index).ok();
    let accounts = normalize_account_index(&mut index);
    if had_index_accounts && accounts.is_empty() {
        logger::log_warn(
            "[Cursor Account] 账号索引中存在账号，但详情文件均无法读取，已跳过空索引写回",
        );
        return accounts;
    }
    let index_changed = index_before_normalize
        .as_ref()
        .map(|before| Some(before.as_slice()) != serde_json::to_vec(&index).ok().as_deref())
        .unwrap_or(true);
    if index_changed {
        if let Err(err) = save_account_index(&index) {
            logger::log_warn(&format!("[Cursor Account] 保存账号索引失败: {}", err));
        }
    }
    accounts
}

pub fn list_accounts_checked() -> Result<Vec<CursorAccount>, String> {
    let _lock = CURSOR_ACCOUNT_INDEX_LOCK
        .lock()
        .map_err(|_| "获取 Cursor 账号锁失败".to_string())?;
    let mut index = load_account_index_checked()?;
    let had_index_accounts = !index.accounts.is_empty();
    let index_before_normalize = serde_json::to_vec(&index).ok();
    let accounts = normalize_account_index(&mut index);
    if had_index_accounts && accounts.is_empty() {
        return Err("Cursor 账号索引中存在账号，但详情文件均无法读取；已保留前端缓存，请从账号备份或本地账号文件恢复。".to_string());
    }
    let index_changed = index_before_normalize
        .as_ref()
        .map(|before| Some(before.as_slice()) != serde_json::to_vec(&index).ok().as_deref())
        .unwrap_or(true);
    if index_changed {
        if let Err(err) = save_account_index(&index) {
            logger::log_warn(&format!("[Cursor Account] 保存账号索引失败: {}", err));
        }
    }
    Ok(accounts)
}

/// 返回排序后的账号列表：
/// 1. 按剩余 Credits 从高到低
/// 2. 配额查询失败的排到最后
/// 3. 同额度的按刷新顺序排列，最后刷新的排最前面
/// 4. 显示真实额度池 ID（auth_id）
pub fn list_accounts_sorted() -> Vec<CursorAccount> {
    let mut accounts = list_accounts();
    sort_accounts_for_display(&mut accounts);
    accounts
}

fn sort_accounts_for_display(accounts: &mut [CursorAccount]) {
    accounts.sort_by(|left, right| {
        let left_remaining = average_quota_percentage(&extract_quota_metrics(left)) as i32;
        let right_remaining = average_quota_percentage(&extract_quota_metrics(right)) as i32;
        let left_failed = has_quota_query_failed(left);
        let right_failed = has_quota_query_failed(right);
        // 失败的排最后
        left_failed
            .cmp(&right_failed)
            // 剩余 Credits 从高到低
            .then_with(|| right_remaining.cmp(&left_remaining))
            // 同额度的，最后刷新的排最前面
            .then_with(|| {
                let left_ts = left.usage_updated_at.unwrap_or(0);
                let right_ts = right.usage_updated_at.unwrap_or(0);
                right_ts.cmp(&left_ts)
            })
    });
}

/// 获取账号的真实额度池显示 ID
pub fn quota_pool_id_display(account: &CursorAccount) -> Option<String> {
    resolve_quota_pool_id(account).or_else(|| account.auth_id.clone())
}

pub(crate) fn has_quota_query_failed(account: &CursorAccount) -> bool {
    account
        .quota_query_last_error
        .as_ref()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

pub fn is_cursor_switch_ready_account(account: &CursorAccount) -> bool {
    if is_banned_account(account) {
        return false;
    }
    if !has_nirvana_switch_ready_tokens(account) {
        return false;
    }
    if has_quota_query_failed(account) || account_has_auth_failure_marker(account) {
        return false;
    }
    true
}

pub fn ensure_cursor_switch_ready_account(account: &CursorAccount) -> Result<(), String> {
    if is_cursor_switch_ready_account(account) {
        return Ok(());
    }
    if is_banned_account(account) {
        return Err(format!("Cursor 账号已被标记不可用: {}", account.email));
    }
    if !has_nirvana_switch_ready_tokens(account) {
        return Err(format!(
            "Cursor 账号缺少可切号 token，请重新导入账号: {}",
            account.email
        ));
    }
    if account_has_auth_failure_marker(account) {
        return Err(cursor_quota_auth_reimport_for(&account.email));
    }
    if let Some(error) = account.quota_query_last_error.as_deref() {
        let error = error.trim();
        if !error.is_empty() {
            return Err(format!(
                "Cursor 账号{}，已跳过切号: {}",
                CURSOR_UI_QUOTA_QUERY_FAILED, account.email
            ));
        }
    }
    Err(format!("Cursor 账号当前不可切号: {}", account.email))
}

/// 切号后异步验收用：探测 token 是否仍被 API 接受（不阻断切号链）。
pub async fn probe_cursor_account_live_auth(account_id: &str) -> Result<(), String> {
    let account = load_account(account_id).ok_or_else(|| format!("Cursor 账号不存在: {}", account_id))?;
    let (access_token, _) = nirvana_kh_auth_tokens(&account)?;
    let client = build_cursor_http_client()?;
    match fetch_user_meta_with_client(&client, &access_token).await {
        Ok(_) => Ok(()),
        Err(err) => {
            if is_cursor_transient_quota_error(&err) {
                return Ok(());
            }
            if is_cursor_auth_quota_error(&err) {
                persist_quota_query_error(account_id, &err);
            }
            Err(err)
        }
    }
}

struct UpsertAccountOutcome {
    account: CursorAccount,
    created: bool,
}

fn apply_payload(
    account: &mut CursorAccount,
    payload: CursorImportPayload,
    resolved_auth_id: Option<String>,
) {
    let incoming_email = payload.email.trim().to_string();
    if !incoming_email.is_empty() {
        account.email = incoming_email;
    } else if !account.email.contains('@') {
        account.email.clear();
    }
    account.name = payload.name;
    account.access_token = payload.access_token;
    account.refresh_token = payload.refresh_token;
    if let Err(err) = normalize_account_tokens(account) {
        logger::log_warn(&format!(
            "[Cursor Account] 导入 token 规范化失败，保留原始值: email={}, error={}",
            account.email, err
        ));
    }
    account.membership_type = payload.membership_type;
    account.subscription_status = payload.subscription_status;
    account.sign_up_type = payload.sign_up_type;
    account.cursor_auth_raw = payload.cursor_auth_raw;
    account.cursor_usage_raw = payload.cursor_usage_raw;
    if let Some(auth_id) = resolved_auth_id {
        account.auth_id = Some(auth_id.clone());
        upsert_cursor_auth_raw_string(account, "authId", Some(auth_id));
    }
    account.status = payload.status;
    account.status_reason = payload.status_reason;
    account.last_used = now_ts();
    backfill_quota_pool_auth_id(account);
}

fn upsert_account_with_outcome(
    payload: CursorImportPayload,
) -> Result<UpsertAccountOutcome, String> {
    let _lock = CURSOR_ACCOUNT_INDEX_LOCK
        .lock()
        .map_err(|_| "获取 Cursor 账号锁失败".to_string())?;

    let now = now_ts();
    let mut index = load_account_index();
    let incoming_auth_id = resolve_payload_auth_id(&payload);
    let incoming_email = normalize_email_identity(Some(payload.email.as_str()));

    // 邮箱优先身份：导入只认邮箱，避免跨邮箱覆盖同 id。
    let identity_seed = incoming_email
        .clone()
        .or_else(|| incoming_auth_id.clone())
        .or_else(|| normalize_token_identity(Some(payload.access_token.as_str())))
        .unwrap_or_else(|| "cursor_user".to_string())
        .to_lowercase();
    let mut generated_id = format!("cursor_{:x}", md5::compute(identity_seed.as_bytes()));

    let account_id = index
        .accounts
        .iter()
        .filter_map(|item| load_account(&item.id))
        .find(|account| {
            let existing_email = normalize_email_identity(Some(account.email.as_str()));
            matches!(
                (existing_email.as_ref(), incoming_email.as_ref()),
                (Some(ex_email), Some(in_email)) if ex_email == in_email
            )
        })
        .map(|account| account.id)
        .unwrap_or_else(|| {
            if let Some(existing) = load_account(&generated_id) {
                let existing_email = normalize_email_identity(Some(existing.email.as_str()));
                if existing_email != incoming_email {
                    generated_id = format!(
                        "cursor_{:x}",
                        md5::compute(format!("{}::{}", identity_seed, Uuid::new_v4()).as_bytes())
                    );
                }
            }
            generated_id
        });

    let existing = load_account(&account_id);
    let created = existing.is_none();
    let tags = existing.as_ref().and_then(|acc| acc.tags.clone());
    let created_at = existing.as_ref().map(|acc| acc.created_at).unwrap_or(now);

    let mut account = existing.unwrap_or(CursorAccount {
        id: account_id.clone(),
        email: payload.email.clone(),
        auth_id: incoming_auth_id.clone(),
        name: payload.name.clone(),
        tags,
        access_token: payload.access_token.clone(),
        refresh_token: payload.refresh_token.clone(),
        membership_type: payload.membership_type.clone(),
        subscription_status: payload.subscription_status.clone(),
        sign_up_type: payload.sign_up_type.clone(),
        cursor_auth_raw: payload.cursor_auth_raw.clone(),
        cursor_usage_raw: payload.cursor_usage_raw.clone(),
        status: payload.status.clone(),
        status_reason: payload.status_reason.clone(),
        quota_query_last_error: None,
        quota_query_last_error_at: None,
        usage_updated_at: None,
        chat_probe: None,
        created_at,
        last_used: now,
    });

    apply_payload(&mut account, payload, incoming_auth_id);
    account.id = account_id;
    account.created_at = created_at;
    account.quota_query_last_error = None;
    account.quota_query_last_error_at = None;
    account.last_used = now;

    save_account_file(&account)?;
    refresh_summary(&mut index, &account);
    save_account_index(&index)?;

    logger::log_info(&format!(
        "[Cursor Account] 账号已保存: action={}, id={}, email={}",
        if created { "created" } else { "updated" },
        account.id,
        account.email
    ));
    Ok(UpsertAccountOutcome { account, created })
}

pub fn upsert_account(payload: CursorImportPayload) -> Result<CursorAccount, String> {
    Ok(upsert_account_with_outcome(payload)?.account)
}

pub fn remove_account(account_id: &str) -> Result<(), String> {
    let _lock = CURSOR_ACCOUNT_INDEX_LOCK
        .lock()
        .map_err(|_| "获取 Cursor 账号锁失败".to_string())?;
    let mut index = load_account_index();
    index.accounts.retain(|item| item.id != account_id);
    save_account_index(&index)?;
    delete_account_file(account_id)?;
    Ok(())
}

pub fn remove_accounts(account_ids: &[String]) -> Result<(), String> {
    for id in account_ids {
        remove_account(id)?;
    }
    Ok(())
}

pub fn update_account_tags(account_id: &str, tags: Vec<String>) -> Result<CursorAccount, String> {
    let mut account = load_account(account_id).ok_or_else(|| "账号不存在".to_string())?;
    account.tags = Some(tags);
    account.last_used = now_ts();
    let updated = account.clone();
    upsert_account_record(account)?;
    Ok(updated)
}

// ---------------------------------------------------------------------------
// Import / Export
// ---------------------------------------------------------------------------

fn clone_object_value(value: Option<&Value>) -> Option<Value> {
    value.and_then(|raw| {
        if raw.is_object() {
            Some(raw.clone())
        } else {
            None
        }
    })
}

fn extract_string(obj: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(value) = obj.get(*key) {
            if let Some(text) = value.as_str().map(str::trim).filter(|v| !v.is_empty()) {
                return Some(text.to_string());
            }
        }
    }
    None
}

fn payload_from_import_value(raw: Value) -> Result<CursorImportPayload, String> {
    let obj = raw
        .as_object()
        .ok_or_else(|| "Cursor 导入 JSON 必须是对象".to_string())?;

    let email = extract_string(obj, &["email", "cachedEmail", "cursor_email"])
        .ok_or_else(|| "缺少 email 字段".to_string())?;
    let access_token = extract_string(
        obj,
        &[
            "access_token",
            "accessToken",
            "token",
            "cursor_access_token",
        ],
    )
    .ok_or_else(|| "缺少 access_token 字段".to_string())?;

    let name = extract_string(obj, &["name", "displayName"]);
    let refresh_token = extract_string(
        obj,
        &["refresh_token", "refreshToken", "cursor_refresh_token"],
    );
    let membership_type = extract_string(
        obj,
        &[
            "membership_type",
            "membershipType",
            "stripeMembershipType",
            "plan",
        ],
    );
    let subscription_status = extract_string(
        obj,
        &[
            "subscription_status",
            "subscriptionStatus",
            "stripeSubscriptionStatus",
        ],
    );
    let sign_up_type = extract_string(obj, &["sign_up_type", "signUpType", "cachedSignUpType"]);
    let status = extract_string(obj, &["status"]);
    let status_reason = extract_string(obj, &["status_reason", "statusReason"]);

    let cursor_auth_raw = clone_object_value(obj.get("cursor_auth_raw"))
        .or_else(|| clone_object_value(obj.get("cursorAuthRaw")));
    let cursor_usage_raw = clone_object_value(obj.get("cursor_usage_raw"))
        .or_else(|| clone_object_value(obj.get("cursorUsageRaw")));
    let auth_id = extract_string(obj, &["auth_id", "authId", "workos_id", "workosId"])
        .or_else(|| extract_auth_id_from_raw_value(cursor_auth_raw.as_ref()))
        .or_else(|| extract_auth_id_from_access_token(access_token.as_str()));

    Ok(CursorImportPayload {
        email,
        auth_id,
        name,
        access_token,
        refresh_token,
        membership_type,
        subscription_status,
        sign_up_type,
        cursor_auth_raw,
        cursor_usage_raw,
        status,
        status_reason,
    })
}

fn payload_from_cursor_account(account: CursorAccount) -> Result<CursorImportPayload, String> {
    let email = account.email.trim().to_string();
    if email.is_empty() {
        return Err("缺少 email 字段".to_string());
    }

    let access_token = account.access_token.trim().to_string();
    if access_token.is_empty() {
        return Err("缺少 access_token 字段".to_string());
    }

    Ok(CursorImportPayload {
        email,
        auth_id: account.auth_id,
        name: account.name,
        access_token,
        refresh_token: account.refresh_token,
        membership_type: account.membership_type,
        subscription_status: account.subscription_status,
        sign_up_type: account.sign_up_type,
        cursor_auth_raw: account.cursor_auth_raw,
        cursor_usage_raw: account.cursor_usage_raw,
        status: account.status,
        status_reason: account.status_reason,
    })
}

fn payloads_from_import_json_value(value: Value) -> Result<Vec<CursorImportPayload>, String> {
    match value {
        Value::Array(items) => {
            if items.is_empty() {
                return Err("导入数组为空".to_string());
            }
            let mut payloads = Vec::with_capacity(items.len());
            for (idx, item) in items.into_iter().enumerate() {
                let payload = payload_from_import_value(item)
                    .map_err(|e| format!("第 {} 条 Cursor 账号解析失败: {}", idx + 1, e))?;
                payloads.push(payload);
            }
            Ok(payloads)
        }
        Value::Object(mut obj) => {
            let object_value = Value::Object(obj.clone());
            if let Ok(payload) = payload_from_import_value(object_value) {
                return Ok(vec![payload]);
            }

            if let Some(accounts) = obj
                .remove("accounts")
                .or_else(|| obj.remove("items"))
                .and_then(|raw| raw.as_array().cloned())
            {
                if accounts.is_empty() {
                    return Err("导入数组为空".to_string());
                }
                let mut payloads = Vec::with_capacity(accounts.len());
                for (idx, item) in accounts.into_iter().enumerate() {
                    let payload = payload_from_import_value(item)
                        .map_err(|e| format!("第 {} 条 Cursor 账号解析失败: {}", idx + 1, e))?;
                    payloads.push(payload);
                }
                return Ok(payloads);
            }

            Err("无法解析 Cursor 导入对象".to_string())
        }
        _ => Err("Cursor 导入 JSON 必须是对象或数组".to_string()),
    }
}

pub fn import_from_json(json_content: &str) -> Result<Vec<CursorAccount>, String> {
    if let Ok(account) = serde_json::from_str::<CursorAccount>(json_content) {
        let payload = payload_from_cursor_account(account)
            .map_err(|e| format!("Cursor 账号解析失败: {}", e))?;
        let saved = upsert_import_payload(payload)?;
        logger::log_info("[Cursor Account] 从 JSON 导入完成: total=1");
        return Ok(vec![saved]);
    }

    if let Ok(accounts) = serde_json::from_str::<Vec<CursorAccount>>(json_content) {
        let mut result = Vec::new();
        for (idx, account) in accounts.into_iter().enumerate() {
            let payload = payload_from_cursor_account(account)
                .map_err(|e| format!("第 {} 条 Cursor 账号解析失败: {}", idx + 1, e))?;
            result.push(upsert_import_payload(payload)?);
        }
        logger::log_info(&format!(
            "[Cursor Account] 从 JSON 导入完成: total={}",
            result.len()
        ));
        return Ok(result);
    }

    if let Ok(value) = serde_json::from_str::<Value>(json_content) {
        if let Ok(payloads) = payloads_from_import_json_value(value) {
            let mut result = Vec::with_capacity(payloads.len());
            for payload in payloads {
                result.push(upsert_import_payload(payload)?);
            }
            logger::log_info(&format!(
                "[Cursor Account] 从 JSON 导入完成: total={}",
                result.len()
            ));
            return Ok(result);
        }
    }

    Err("无法解析 JSON 内容".to_string())
}

pub fn export_accounts(account_ids: &[String]) -> Result<String, String> {
    let accounts: Vec<CursorAccount> = account_ids
        .iter()
        .filter_map(|id| load_account(id))
        .collect();
    serde_json::to_string_pretty(&accounts).map_err(|e| format!("序列化失败: {}", e))
}

// ---------------------------------------------------------------------------
// Local import (read from Cursor's state.vscdb)
// ---------------------------------------------------------------------------

pub fn get_default_cursor_data_dir() -> Result<PathBuf, String> {
    #[cfg(target_os = "macos")]
    {
        let home = dirs::home_dir().ok_or("无法获取用户主目录")?;
        return Ok(home.join("Library/Application Support/Cursor"));
    }

    #[cfg(target_os = "windows")]
    {
        let appdata =
            std::env::var("APPDATA").map_err(|_| "无法获取 APPDATA 环境变量".to_string())?;
        return Ok(PathBuf::from(appdata).join("Cursor"));
    }

    #[cfg(target_os = "linux")]
    {
        let home = dirs::home_dir().ok_or("无法获取用户主目录")?;
        return Ok(home.join(".config/Cursor"));
    }

    #[allow(unreachable_code)]
    Err("Cursor 账号导入仅支持 macOS、Windows 和 Linux".to_string())
}

pub fn get_default_cursor_state_db_path() -> Result<PathBuf, String> {
    Ok(get_default_cursor_data_dir()?
        .join("User")
        .join("globalStorage")
        .join("state.vscdb"))
}

pub fn get_default_cursor_storage_json_path() -> Result<PathBuf, String> {
    Ok(get_default_cursor_data_dir()?
        .join("User")
        .join("globalStorage")
        .join("storage.json"))
}

pub fn get_default_cursor_machine_id_path() -> Result<PathBuf, String> {
    Ok(get_default_cursor_data_dir()?.join("machineId"))
}

fn read_vscdb_item(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row("SELECT value FROM ItemTable WHERE key = ?1", [key], |row| {
        row.get::<_, String>(0)
    })
    .optional()
    .ok()
    .flatten()
    .and_then(|v| {
        let trimmed = v.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

pub fn read_local_cursor_auth() -> Result<Option<CursorImportPayload>, String> {
    let db_path = get_default_cursor_state_db_path()?;
    if !db_path.exists() {
        return Ok(None);
    }

    let conn = Connection::open(&db_path)
        .map_err(|e| format!("打开 Cursor 本地数据库失败({}): {}", db_path.display(), e))?;

    let access_token = match read_vscdb_item(&conn, "cursorAuth/accessToken") {
        Some(t) => t,
        None => return Ok(None),
    };

    let email = read_vscdb_item(&conn, "cursorAuth/cachedEmail").unwrap_or_default();
    if email.is_empty() {
        return Ok(None);
    }

    let refresh_token = read_vscdb_item(&conn, "cursorAuth/refreshToken");
    let auth_id = read_vscdb_item(&conn, "cursorAuth/authId")
        .or_else(|| extract_auth_id_from_access_token(access_token.as_str()));
    let membership_type = read_vscdb_item(&conn, "cursorAuth/stripeMembershipType");
    let subscription_status = read_vscdb_item(&conn, "cursorAuth/stripeSubscriptionStatus");
    let sign_up_type = read_vscdb_item(&conn, "cursorAuth/cachedSignUpType");

    let mut auth_raw = serde_json::Map::new();
    auth_raw.insert(
        "accessToken".to_string(),
        Value::String(access_token.clone()),
    );
    if let Some(ref rt) = refresh_token {
        auth_raw.insert("refreshToken".to_string(), Value::String(rt.clone()));
    }
    if let Some(ref auth_id_value) = auth_id {
        auth_raw.insert("authId".to_string(), Value::String(auth_id_value.clone()));
    }
    auth_raw.insert("cachedEmail".to_string(), Value::String(email.clone()));
    if let Some(ref mt) = membership_type {
        auth_raw.insert(
            "stripeMembershipType".to_string(),
            Value::String(mt.clone()),
        );
    }
    if let Some(ref ss) = subscription_status {
        auth_raw.insert(
            "stripeSubscriptionStatus".to_string(),
            Value::String(ss.clone()),
        );
    }
    if let Some(ref st) = sign_up_type {
        auth_raw.insert("cachedSignUpType".to_string(), Value::String(st.clone()));
    }

    Ok(Some(CursorImportPayload {
        email,
        auth_id,
        name: None,
        access_token,
        refresh_token,
        membership_type,
        subscription_status,
        sign_up_type,
        cursor_auth_raw: Some(Value::Object(auth_raw)),
        cursor_usage_raw: None,
        status: None,
        status_reason: None,
    }))
}

pub fn import_from_local() -> Result<Option<CursorAccount>, String> {
    let payload = match read_local_cursor_auth()? {
        Some(p) => p,
        None => return Ok(None),
    };
    let account = upsert_import_payload(payload)?;
    logger::log_info(&format!(
        "[Cursor Account] 从本地导入成功: id={}, email={}",
        account.id, account.email
    ));
    Ok(Some(account))
}

// ---------------------------------------------------------------------------
// Inject (write auth fields back to Cursor's state.vscdb)
// ---------------------------------------------------------------------------

fn write_cursor_auth_fields_to_conn(
    conn: &Connection,
    account: &CursorAccount,
) -> Result<(), String> {
    // 切换到 DELETE journal mode 确保写入可靠持久化（13GB+ DB 的 WAL checkpoint 不可靠）
    conn.execute_batch("PRAGMA journal_mode=DELETE;")
        .map_err(|e| format!("切换 journal_mode=DELETE 失败: {}", e))?;
    conn.execute_batch("BEGIN;")
        .map_err(|e| format!("BEGIN transaction 失败: {}", e))?;

    let write_result = (|| {
        upsert_vscdb_item(conn, "cursorAuth/accessToken", &account.access_token)?;
        if let Some(ref rt) = account.refresh_token {
            upsert_vscdb_item(conn, "cursorAuth/refreshToken", rt)?;
        }
        upsert_vscdb_item(conn, "cursorAuth/cachedEmail", &account.email)?;
        let sign_up = account
            .sign_up_type
            .as_deref()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or("Auth_0");
        upsert_vscdb_item(conn, "cursorAuth/cachedSignUpType", sign_up)?;
        if let Some(ref auth_id) = account.auth_id {
            if !auth_id.trim().is_empty() {
                upsert_vscdb_item(conn, "cursorAuth/authId", auth_id)?;
                upsert_vscdb_item(conn, "cursorAuth/workosId", auth_id)?;
            }
        }
        if let Some(ref mt) = account.membership_type {
            upsert_vscdb_item(conn, "cursorAuth/stripeMembershipType", mt)?;
        }
        if let Some(ref ss) = account.subscription_status {
            upsert_vscdb_item(conn, "cursorAuth/stripeSubscriptionStatus", ss)?;
        }
        upsert_vscdb_item(conn, "cursor.accessToken", &account.access_token)?;
        upsert_vscdb_item(conn, "cursor.email", &account.email)?;
        Ok(())
    })();

    match write_result {
        Ok(()) => {
            conn.execute_batch("COMMIT;")
                .map_err(|e| format!("COMMIT 失败: {}", e))?;
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK;");
            let _ = conn.execute_batch("PRAGMA journal_mode=WAL;");
            return Err(e);
        }
    }

    // 恢复 WAL mode
    conn.execute_batch("PRAGMA journal_mode=WAL;")
        .map_err(|e| format!("恢复 journal_mode=WAL 失败: {}", e))?;
    Ok(())
}

fn upsert_vscdb_item(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO ItemTable (key, value) VALUES (?1, ?2)",
        (key, value),
    )
    .map_err(|e| format!("写入 {} 失败: {}", key, e))?;
    Ok(())
}

fn random_sha256_hex() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let hash = Sha256::digest(bytes);
    format!("{:x}", hash)
}

fn random_sha512_hex() -> String {
    let mut bytes = [0u8; 64];
    rand::thread_rng().fill_bytes(&mut bytes);
    let hash = Sha512::digest(bytes);
    format!("{:x}", hash)
}

fn random_dev_device_id() -> String {
    Uuid::new_v4().to_string().to_lowercase()
}

fn random_sqm_id() -> String {
    format!("{{{}}}", Uuid::new_v4().to_string().to_uppercase())
}

fn upsert_storage_json_ids(
    storage_json_path: &PathBuf,
    ids: &HashMap<&str, String>,
) -> Result<(), String> {
    let parent = storage_json_path
        .parent()
        .ok_or_else(|| "storage.json 路径无效".to_string())?;
    if !parent.exists() {
        fs::create_dir_all(parent).map_err(|e| format!("创建 storage.json 目录失败: {}", e))?;
    }

    let mut obj = serde_json::Map::<String, Value>::new();
    if storage_json_path.exists() {
        let raw = fs::read_to_string(storage_json_path).map_err(|e| {
            format!(
                "读取 storage.json 失败({}): {}",
                storage_json_path.display(),
                e
            )
        })?;
        if !raw.trim().is_empty() {
            if let Ok(Value::Object(parsed)) = serde_json::from_str::<Value>(&raw) {
                obj = parsed;
            }
        }
    }

    obj.insert(
        "telemetry.machineId".to_string(),
        Value::String(ids["telemetry.machineId"].clone()),
    );
    obj.insert(
        "telemetry.macMachineId".to_string(),
        Value::String(ids["telemetry.macMachineId"].clone()),
    );
    obj.insert(
        "telemetry.devDeviceId".to_string(),
        Value::String(ids["telemetry.devDeviceId"].clone()),
    );
    obj.insert(
        "telemetry.sqmId".to_string(),
        Value::String(ids["telemetry.sqmId"].clone()),
    );

    let content = serde_json::to_string_pretty(&Value::Object(obj))
        .map_err(|e| format!("序列化 storage.json 失败: {}", e))?;
    crate::modules::atomic_write::write_string_atomic(storage_json_path, &content)
        .map_err(|e| format!("写入 storage.json 失败: {}", e))?;
    Ok(())
}

fn upsert_state_vscdb_ids(
    state_db_path: &PathBuf,
    ids: &HashMap<&str, String>,
) -> Result<(), String> {
    if !state_db_path.exists() {
        return Ok(());
    }

    let conn = Connection::open(state_db_path).map_err(|e| {
        format!(
            "打开 Cursor state.vscdb 失败({}): {}",
            state_db_path.display(),
            e
        )
    })?;
    upsert_vscdb_item(
        &conn,
        "storage.serviceMachineId",
        &ids["storage.serviceMachineId"],
    )?;
    upsert_vscdb_item(&conn, "telemetry.machineId", &ids["telemetry.machineId"])?;
    upsert_vscdb_item(
        &conn,
        "telemetry.macMachineId",
        &ids["telemetry.macMachineId"],
    )?;
    upsert_vscdb_item(
        &conn,
        "telemetry.devDeviceId",
        &ids["telemetry.devDeviceId"],
    )?;
    upsert_vscdb_item(&conn, "telemetry.sqmId", &ids["telemetry.sqmId"])?;
    Ok(())
}

const SWITCH_AUTH_DELETE_KEYS: &[&str] = &[
    "cursorAuth/accessToken",
    "cursorAuth/refreshToken",
    "cursorAuth/cachedEmail",
    "cursorAuth/cachedSignUpType",
    "cursorAuth/stripeMembershipType",
];

fn remove_vscdb_sidecars(db_path: &Path) {
    let db = db_path.to_string_lossy();
    let _ = fs::remove_file(format!("{db}-wal"));
    let _ = fs::remove_file(format!("{db}-shm"));
}

/// 切号注入前清理旧 auth 键，避免残留 membership/authId 与新 token 冲突导致登录页。
fn clear_switch_auth_keys_for_profile(profile_dir: &Path) -> Result<(), String> {
    let db_path = profile_dir
        .join("User")
        .join("globalStorage")
        .join("state.vscdb");
    if !db_path.exists() {
        return Ok(());
    }

    remove_vscdb_sidecars(&db_path);
    let conn = Connection::open(&db_path)
        .map_err(|e| format!("打开 Cursor state.vscdb 失败({}): {}", db_path.display(), e))?;

    for key in SWITCH_AUTH_DELETE_KEYS {
        conn.execute("DELETE FROM ItemTable WHERE key = ?1", [*key])
            .map_err(|e| format!("删除 {} 失败: {}", key, e))?;
    }
    for key in ["cursorAuth/authId", "cursorAuth/workosId"] {
        let _ = conn.execute("DELETE FROM ItemTable WHERE key = ?1", [key]);
    }
    Ok(())
}

/// 无忧传统切号 `i()`：close → Kh → Gh → Jh → Yh → Nc（默认 profile）。
fn nirvana_traditional_switch_steps(account_id: &str, manual_user_pick: bool) -> Result<(), String> {
    let account =
        load_account(account_id).ok_or_else(|| format!("Cursor 账号不存在: {}", account_id))?;
    ensure_cursor_switch_allowed(&account, manual_user_pick)?;
    logger::log_info(&format!("[Cursor Switch] 无忧传统切号: {}", account.email));

    let default_dir = get_default_cursor_data_dir()?;
    let default_dir_str = default_dir.to_string_lossy().to_string();
    // 对齐 r2：仅关闭默认 profile 的 Cursor，保留其它多开实例（禁止 taskkill 全杀）。
    crate::modules::cursor_instance::close_cursor(&[default_dir_str], 20)?;

    switch_tokens_in_profile_db(&default_dir, account_id, manual_user_pick)?;
    reset_storage_json_ids_for_profile(&default_dir)?;
    reset_machine_id_file_for_profile(&default_dir)?;

    if let Ok(cursor_exe) = crate::modules::cursor_instance::resolve_cursor_launch_path() {
        crate::modules::cursor_switch_align::apply_nirvana_traditional_switch_patches(&cursor_exe);
    }

    logger::log_info(&format!(
        "[Cursor Switch] 无忧传统路径换号完成: email={}, profile={}",
        account.email,
        default_dir.display()
    ));
    Ok(())
}

/// 账号总览 Play 与多开实例启动的切号落盘。`manual_user_pick=true` 时为用户点选，不拦配额/失败标记。
pub fn switch_cursor_account_to_profile(
    account_id: &str,
    profile_dir: &Path,
    manual_user_pick: bool,
) -> Result<(), String> {
    if crate::modules::cursor_instance::is_default_cursor_profile_dir(profile_dir) {
        return nirvana_traditional_switch_steps(account_id, manual_user_pick);
    }

    let account =
        load_account(account_id).ok_or_else(|| format!("Cursor 账号不存在: {}", account_id))?;
    ensure_cursor_switch_allowed(&account, manual_user_pick)?;
    let profile_dir_str = profile_dir.to_string_lossy().to_string();
    crate::modules::cursor_instance::close_cursor_profile_strict(&profile_dir_str, 20)?;
    crate::modules::cursor_instance::ensure_state_db_for_injection(profile_dir)?;

    switch_tokens_in_profile_db(profile_dir, account_id, manual_user_pick)?;
    reset_storage_json_ids_for_profile(profile_dir)?;
    reset_machine_id_file_for_profile(profile_dir)?;

    if let Ok(cursor_exe) = crate::modules::cursor_instance::resolve_cursor_launch_path() {
        crate::modules::cursor_switch_align::apply_nirvana_traditional_switch_patches_main_js_only(
            &cursor_exe,
        );
    }

    logger::log_info(&format!(
        "[Cursor Switch] 无忧传统路径换号完成(多开): email={}, profile={}",
        account.email,
        profile_dir.display()
    ));
    Ok(())
}

/// 无忧 `switchTokensInDb`（Kh）：`accessToken`/`refreshToken` 原样裸 JWT（与默认 profile 一致，不做 `user_id::jwt` 转换）。
fn nirvana_kh_auth_tokens(account: &CursorAccount) -> Result<(String, String), String> {
    if let Some(raw) = account.cursor_auth_raw.as_ref() {
        if let Some(at) = raw.get("accessToken").and_then(|v| v.as_str()) {
            let access = at.trim();
            if !access.is_empty() {
                let refresh = raw
                    .get("refreshToken")
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or(access);
                return Ok((access.to_string(), refresh.to_string()));
            }
        }
    }

    let access = account.access_token.trim();
    if access.is_empty() {
        return Err(format!("账号 {} access_token 为空", account.email));
    }
    let refresh = account
        .refresh_token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(access);
    Ok((access.to_string(), refresh.to_string()))
}

fn verify_switch_tokens_written(db_path: &Path) -> Result<(), String> {
    let conn = Connection::open(db_path)
        .map_err(|e| format!("切号落盘校验打开数据库失败({}): {}", db_path.display(), e))?;
    let access = read_vscdb_item(&conn, "cursorAuth/accessToken")
        .ok_or_else(|| "切号落盘校验失败: cursorAuth/accessToken 未写入".to_string())?;
    let refresh = read_vscdb_item(&conn, "cursorAuth/refreshToken")
        .ok_or_else(|| "切号落盘校验失败: cursorAuth/refreshToken 未写入".to_string())?;
    let email = read_vscdb_item(&conn, "cursorAuth/cachedEmail")
        .ok_or_else(|| "切号落盘校验失败: cursorAuth/cachedEmail 未写入".to_string())?;
    if !email.contains('@') {
        return Err(format!("切号落盘校验失败: cachedEmail 无效: {}", email));
    }
    logger::log_info(&format!(
        "[Cursor Switch] 切号落盘校验通过: access_len={}, refresh_len={}, email_len={}",
        access.len(),
        refresh.len(),
        email.len()
    ));
    Ok(())
}

/// 对齐无忧 `switchTokensInDb`（Kh）：删旧 auth → 重置 state.vscdb telemetry → 原样写 token。
pub fn switch_tokens_in_profile_db(
    profile_dir: &Path,
    account_id: &str,
    manual_user_pick: bool,
) -> Result<(), String> {
    let account =
        load_account(account_id).ok_or_else(|| format!("Cursor 账号不存在: {}", account_id))?;
    ensure_cursor_switch_allowed(&account, manual_user_pick)?;
    let (access_token, refresh_token) = nirvana_kh_auth_tokens(&account)?;
    let db_path = profile_dir
        .join("User")
        .join("globalStorage")
        .join("state.vscdb");
    if !db_path.exists() {
        return Err(format!("Cursor state.vscdb 不存在: {}", db_path.display()));
    }

    remove_vscdb_sidecars(&db_path);
    let conn = Connection::open(&db_path)
        .map_err(|e| format!("打开 Cursor state.vscdb 失败({}): {}", db_path.display(), e))?;

    // 切换到 DELETE journal mode：13GB+ 的 DB 在 WAL 模式下 wal_checkpoint 不可靠，
    // DELETE mode 的写入直接进主 DB 文件，不依赖 checkpoint flush。
    conn.execute_batch("PRAGMA journal_mode=DELETE;")
        .map_err(|e| format!("切换 journal_mode=DELETE 失败: {}", e))?;
    conn.execute_batch("BEGIN;")
        .map_err(|e| format!("BEGIN transaction 失败: {}", e))?;

    let write_result = (|| {
        for key in SWITCH_AUTH_DELETE_KEYS {
            conn.execute("DELETE FROM ItemTable WHERE key = ?1", [*key])
                .map_err(|e| format!("删除 {} 失败: {}", key, e))?;
        }
        for key in ["cursor.accessToken", "cursor.email"] {
            conn.execute("DELETE FROM ItemTable WHERE key = ?1", [key])
                .map_err(|e| format!("删除 {} 失败: {}", key, e))?;
        }
        for key in ["cursorAuth/authId", "cursorAuth/workosId"] {
            conn.execute("DELETE FROM ItemTable WHERE key = ?1", [key])
                .map_err(|e| format!("删除 {} 失败: {}", key, e))?;
        }

        let ids = build_cursor_fingerprint_ids();
        upsert_vscdb_item(
            &conn,
            "storage.serviceMachineId",
            &ids["storage.serviceMachineId"],
        )?;
        upsert_vscdb_item(&conn, "telemetry.machineId", &ids["telemetry.machineId"])?;
        upsert_vscdb_item(
            &conn,
            "telemetry.macMachineId",
            &ids["telemetry.macMachineId"],
        )?;
        upsert_vscdb_item(
            &conn,
            "telemetry.devDeviceId",
            &ids["telemetry.devDeviceId"],
        )?;
        upsert_vscdb_item(&conn, "telemetry.sqmId", &ids["telemetry.sqmId"])?;

        upsert_vscdb_item(&conn, "cursorAuth/accessToken", &access_token)?;
        upsert_vscdb_item(&conn, "cursorAuth/refreshToken", &refresh_token)?;
        upsert_vscdb_item(&conn, "cursorAuth/cachedEmail", &account.email)?;
        upsert_vscdb_item(&conn, "cursorAuth/cachedSignUpType", "Auth_0")?;

        if let Some(ref auth_id) = resolve_quota_pool_id(&account) {
            upsert_vscdb_item(&conn, "cursorAuth/authId", auth_id)?;
            upsert_vscdb_item(&conn, "cursorAuth/workosId", auth_id)?;
        }
        if let Some(ref mt) = account.membership_type {
            upsert_vscdb_item(&conn, "cursorAuth/stripeMembershipType", mt)?;
        }
        if let Some(ref ss) = account.subscription_status {
            upsert_vscdb_item(&conn, "cursorAuth/stripeSubscriptionStatus", ss)?;
        }
        Ok(())
    })();

    match write_result {
        Ok(()) => {
            conn.execute_batch("COMMIT;")
                .map_err(|e| format!("COMMIT 失败: {}", e))?;
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK;");
            let _ = conn.execute_batch("PRAGMA journal_mode=WAL;");
            return Err(e);
        }
    }

    // 恢复 WAL mode 供 Cursor 正常使用
    conn.execute_batch("PRAGMA journal_mode=WAL;")
        .map_err(|e| format!("恢复 journal_mode=WAL 失败: {}", e))?;
    drop(conn);

    verify_switch_tokens_written(&db_path)?;

    logger::log_info(&format!(
        "[Cursor Switch] switchTokensInDb 完成: email={}, db={}, token=nirvana_raw, journal_mode=delete_then_wal",
        account.email,
        db_path.display()
    ));
    Ok(())
}

/// 对齐无忧 `resetStorageJsonIds`（Gh）：独立生成 telemetry 并写入 storage.json。
pub fn reset_storage_json_ids_for_profile(profile_dir: &Path) -> Result<(), String> {
    let storage_json = profile_dir
        .join("User")
        .join("globalStorage")
        .join("storage.json");
    let ids = build_cursor_fingerprint_ids();
    upsert_storage_json_ids(&storage_json, &ids)?;
    logger::log_info(&format!(
        "[Cursor Switch] storage.json telemetry 已重置: {}",
        storage_json.display()
    ));
    Ok(())
}

/// 对齐无忧 `resetMachineIdFile`（Jh）：写入新的 machineId 文件（UUID v4）。
pub fn reset_machine_id_file_for_profile(profile_dir: &Path) -> Result<(), String> {
    let machine_id_path = profile_dir.join("machineId");
    let parent = machine_id_path
        .parent()
        .ok_or_else(|| "machineId 路径无效".to_string())?;
    if !parent.exists() {
        fs::create_dir_all(parent).map_err(|e| format!("创建 machineId 目录失败: {}", e))?;
    }
    let value = random_dev_device_id();
    crate::modules::atomic_write::write_string_atomic(&machine_id_path, &value)
        .map_err(|e| format!("写入 machineId 失败: {}", e))?;
    logger::log_info(&format!(
        "[Cursor Switch] machineId 文件已重置: {}",
        machine_id_path.display()
    ));
    Ok(())
}

fn build_cursor_fingerprint_ids() -> HashMap<&'static str, String> {
    let mut ids = HashMap::<&str, String>::new();
    ids.insert("storage.serviceMachineId", Uuid::new_v4().to_string());
    ids.insert("telemetry.machineId", random_sha256_hex());
    ids.insert("telemetry.macMachineId", random_sha512_hex());
    ids.insert("telemetry.devDeviceId", random_dev_device_id());
    ids.insert("telemetry.sqmId", random_sqm_id());
    ids
}

fn hard_reset_cursor_fingerprint_state_at_paths(
    storage_json: &Path,
    machine_id_path: &Path,
    state_db: &Path,
) -> Result<(), String> {
    let ids = build_cursor_fingerprint_ids();
    upsert_storage_json_ids(&storage_json.to_path_buf(), &ids)?;

    let machine_parent = machine_id_path
        .parent()
        .ok_or_else(|| "machineId 路径无效".to_string())?;
    if !machine_parent.exists() {
        fs::create_dir_all(machine_parent)
            .map_err(|e| format!("创建 machineId 目录失败: {}", e))?;
    }
    crate::modules::atomic_write::write_string_atomic(
        &machine_id_path.to_path_buf(),
        &ids["telemetry.devDeviceId"],
    )
    .map_err(|e| format!("写入 machineId 失败: {}", e))?;

    upsert_state_vscdb_ids(&state_db.to_path_buf(), &ids)?;
    Ok(())
}

pub fn hard_reset_cursor_fingerprint_state() -> Result<(), String> {
    let storage_json = get_default_cursor_storage_json_path()?;
    let machine_id_path = get_default_cursor_machine_id_path()?;
    let state_db = get_default_cursor_state_db_path()?;
    hard_reset_cursor_fingerprint_state_at_paths(&storage_json, &machine_id_path, &state_db)?;
    logger::log_info("[Cursor Switch] 已执行本地指纹重置（state.vscdb/storage.json/machineId）");
    Ok(())
}

pub fn hard_reset_cursor_fingerprint_state_for_profile(profile_dir: &Path) -> Result<(), String> {
    let storage_json = profile_dir
        .join("User")
        .join("globalStorage")
        .join("storage.json");
    let machine_id_path = profile_dir.join("machineId");
    let state_db = profile_dir
        .join("User")
        .join("globalStorage")
        .join("state.vscdb");
    hard_reset_cursor_fingerprint_state_at_paths(&storage_json, &machine_id_path, &state_db)?;
    logger::log_info(&format!(
        "[Cursor Switch] 已执行实例 profile 指纹重置: {}",
        profile_dir.display()
    ));
    Ok(())
}

/// 对齐无忧 `writeTokenToDb`（Lc）：仅回写 token，不做指纹重置/关进程（TokenKeeper 保活用）。
pub fn inject_to_cursor(account_id: &str) -> Result<(), String> {
    let account =
        load_account(account_id).ok_or_else(|| format!("Cursor 账号不存在: {}", account_id))?;
    let db_path = get_default_cursor_state_db_path()?;
    if !db_path.exists() {
        return Err(format!("Cursor state.vscdb 不存在: {}", db_path.display()));
    }

    let conn =
        Connection::open(&db_path).map_err(|e| format!("打开 Cursor 本地数据库失败: {}", e))?;
    write_cursor_auth_fields_to_conn(&conn, &account)?;

    logger::log_info(&format!(
        "[Cursor Account] 注入成功: id={}, email={}",
        account.id, account.email
    ));
    Ok(())
}

pub fn inject_to_cursor_at_path(db_path: &std::path::Path, account_id: &str) -> Result<(), String> {
    let account =
        load_account(account_id).ok_or_else(|| format!("Cursor 账号不存在: {}", account_id))?;
    if !db_path.exists() {
        return Err(format!("Cursor state.vscdb 不存在: {}", db_path.display()));
    }

    let conn =
        Connection::open(db_path).map_err(|e| format!("打开 Cursor 本地数据库失败: {}", e))?;
    write_cursor_auth_fields_to_conn(&conn, &account)?;

    logger::log_info(&format!(
        "[Cursor Account] 注入成功(自定义路径): id={}, email={}, path={}",
        account.id,
        account.email,
        db_path.display()
    ));
    Ok(())
}

// ---------------------------------------------------------------------------
// Cursor usage API
// ---------------------------------------------------------------------------

const CURSOR_USAGE_SUMMARY_URL: &str = "https://cursor.com/api/usage-summary";
const CURSOR_GET_USER_META_URL: &str = "https://api2.cursor.sh/aiserver.v1.AuthService/GetUserMeta";
const CURSOR_FULL_STRIPE_PROFILE_URL: &str = "https://api2.cursor.sh/auth/full_stripe_profile";
const CURSOR_STRIPE_PROFILE_URL: &str = "https://api2.cursor.sh/auth/stripe_profile";
// 与官方 Cursor 客户端保持一致：使用 api2.cursor.sh/oauth/token 和内置 client_id 交换新 token。
const CURSOR_OAUTH_TOKEN_URL: &str = "https://api2.cursor.sh/oauth/token";
const CURSOR_AUTH_CLIENT_ID: &str = "KbZUR41cY7W6zRSdpSUJ7I7mLYBKOCmB";

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CursorUserMetaResponse {
    email: Option<String>,
    sign_up_type: Option<String>,
    workos_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CursorStripeProfileResponse {
    membership_type: Option<String>,
    individual_membership_type: Option<String>,
    subscription_status: Option<String>,
    team_membership_type: Option<String>,
    is_team_member: Option<bool>,
    is_enterprise: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
struct CursorRefreshTokenResponse {
    #[serde(alias = "accessToken")]
    access_token: Option<String>,
    #[serde(alias = "refreshToken")]
    refresh_token: Option<String>,
    #[serde(default, alias = "shouldLogout")]
    should_logout: bool,
}

fn build_cursor_http_client() -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder().timeout(std::time::Duration::from_secs(6));

    let config = crate::modules::config::get_user_config();
    if config.global_proxy_enabled && !config.global_proxy_url.trim().is_empty() {
        let proxy_url = config.global_proxy_url.trim();
        if let Ok(proxy) = reqwest::Proxy::all(proxy_url) {
            builder = builder.proxy(proxy);
            logger::log_info(&format!(
                "[Cursor Client] HTTP 客户端已启用代理: {}",
                proxy_url
            ));
        }
    }

    builder
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))
}

fn extract_workos_user_id(jwt: &str) -> Option<String> {
    let value = decode_access_token_payload(jwt)?;
    let sub = value.get("sub")?.as_str()?;
    let user_id = sub.rsplit('|').next().unwrap_or(sub);
    if user_id.starts_with("user_") {
        Some(user_id.to_string())
    } else {
        None
    }
}

fn build_session_cookie(access_token: &str) -> Option<String> {
    let user_id = extract_workos_user_id(access_token)?;
    Some(format!(
        "WorkosCursorSessionToken={}%3A%3A{}",
        user_id, access_token
    ))
}

fn resolve_membership_from_stripe_profile(profile: &CursorStripeProfileResponse) -> Option<String> {
    let membership = normalize_non_empty(profile.membership_type.as_deref());
    let individual = normalize_non_empty(profile.individual_membership_type.as_deref());

    if let Some(individual_value) = individual.as_ref() {
        if !individual_value.eq_ignore_ascii_case("free")
            && !matches!(
                membership.as_deref(),
                Some(value) if value.eq_ignore_ascii_case("enterprise")
            )
        {
            return Some(individual_value.clone());
        }
    }

    membership.or(individual)
}

async fn exchange_refresh_token_with_client(
    client: &reqwest::Client,
    refresh_token: &str,
) -> Result<CursorRefreshTokenResponse, String> {
    let response = client
        .post(CURSOR_OAUTH_TOKEN_URL)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "grant_type": "refresh_token",
            "client_id": CURSOR_AUTH_CLIENT_ID,
            "refresh_token": refresh_token,
        }))
        .send()
        .await
        .map_err(|e| format!("请求 Cursor token 刷新接口失败: {}", e))?;

    let status = response.status().as_u16();
    let body = response
        .text()
        .await
        .map_err(|e| format!("读取 Cursor token 刷新响应失败: {}", e))?;

    if status == 401 || status == 403 {
        return Err("Cursor refresh token 已过期或无效，请重新导入账号".to_string());
    }
    if status != 200 {
        let detail = body.trim();
        return Err(if detail.is_empty() {
            format!("Cursor token 刷新接口返回异常状态码: {}", status)
        } else {
            format!(
                "Cursor token 刷新接口返回异常状态码: {}, body_len={}",
                status,
                body.len()
            )
        });
    }

    serde_json::from_str::<CursorRefreshTokenResponse>(&body)
        .map_err(|e| format!("解析 Cursor token 刷新响应失败: {}", e))
}

async fn refresh_account_access_token_with_client(
    client: &reqwest::Client,
    account: &mut CursorAccount,
) -> Result<bool, String> {
    let Some(refresh_token) = normalize_non_empty(account.refresh_token.as_deref()) else {
        return Ok(false);
    };

    let response = exchange_refresh_token_with_client(client, refresh_token.as_str()).await?;
    if response.should_logout {
        return Err("Cursor refresh token 已失效，请重新导入账号".to_string());
    }

    let new_access_token = normalize_non_empty(response.access_token.as_deref())
        .ok_or_else(|| "Cursor token 刷新响应缺少 access_token".to_string())?;
    let new_refresh_token =
        normalize_non_empty(response.refresh_token.as_deref()).or(Some(refresh_token));

    account.access_token = new_access_token.clone();
    account.refresh_token = new_refresh_token.clone();
    upsert_cursor_auth_raw_string(account, "accessToken", Some(new_access_token));
    upsert_cursor_auth_raw_string(account, "refreshToken", new_refresh_token);
    let _ = normalize_account_tokens(account);
    Ok(true)
}

async fn fetch_user_meta_with_client(
    client: &reqwest::Client,
    access_token: &str,
) -> Result<CursorUserMetaResponse, String> {
    let response = client
        .post(CURSOR_GET_USER_META_URL)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({}))
        .send()
        .await
        .map_err(|e| format!("请求 Cursor user meta 失败: {}", e))?;

    let status = response.status().as_u16();
    if status == 401 {
        return Err(CURSOR_UI_QUOTA_QUERY_FAILED.to_string());
    }
    if status == 403 {
        return Err("Cursor 用户信息查询被限流，请稍后重试".to_string());
    }
    if status != 200 {
        return Err(format!("Cursor user meta API 返回异常状态码: {}", status));
    }

    let body = response
        .text()
        .await
        .map_err(|e| format!("读取 Cursor user meta 响应失败: {}", e))?;

    serde_json::from_str::<CursorUserMetaResponse>(&body)
        .map_err(|e| format!("解析 Cursor user meta JSON 失败: {}", e))
}

async fn fetch_stripe_profile_with_client(
    client: &reqwest::Client,
    access_token: &str,
) -> Result<Option<CursorStripeProfileResponse>, String> {
    let full_response = client
        .get(CURSOR_FULL_STRIPE_PROFILE_URL)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("请求 Cursor full stripe profile 失败: {}", e))?;

    let full_status = full_response.status().as_u16();
    if full_status == 401 || full_status == 403 {
        return Err(CURSOR_UI_QUOTA_QUERY_FAILED.to_string());
    }
    if full_status == 200 {
        let body = full_response
            .text()
            .await
            .map_err(|e| format!("读取 Cursor full stripe profile 响应失败: {}", e))?;
        let profile = serde_json::from_str::<CursorStripeProfileResponse>(&body)
            .map_err(|e| format!("解析 Cursor full stripe profile JSON 失败: {}", e))?;
        return Ok(Some(profile));
    }

    let fallback_response = client
        .get(CURSOR_STRIPE_PROFILE_URL)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("请求 Cursor stripe profile 失败: {}", e))?;

    let fallback_status = fallback_response.status().as_u16();
    if fallback_status == 401 || fallback_status == 403 {
        return Err(CURSOR_UI_QUOTA_QUERY_FAILED.to_string());
    }
    if fallback_status != 200 {
        return Ok(None);
    }

    let body = fallback_response
        .text()
        .await
        .map_err(|e| format!("读取 Cursor stripe profile 响应失败: {}", e))?;

    let parsed = serde_json::from_str::<serde_json::Value>(&body)
        .map_err(|e| format!("解析 Cursor stripe profile JSON 失败: {}", e))?;

    match parsed {
        Value::Object(_) => serde_json::from_value::<CursorStripeProfileResponse>(parsed)
            .map(Some)
            .map_err(|e| format!("解析 Cursor stripe profile 对象失败: {}", e)),
        Value::String(text) => {
            if text.trim().is_empty() {
                Ok(None)
            } else {
                Ok(Some(CursorStripeProfileResponse {
                    membership_type: Some("pro".to_string()),
                    individual_membership_type: None,
                    subscription_status: None,
                    team_membership_type: None,
                    is_team_member: None,
                    is_enterprise: None,
                }))
            }
        }
        _ => Ok(None),
    }
}

async fn fetch_usage_summary_with_client(
    client: &reqwest::Client,
    access_token: &str,
) -> Result<serde_json::Value, String> {
    let cookie = build_session_cookie(access_token)
        .ok_or_else(|| "无法从 accessToken 解析 WorkOS 用户 ID".to_string())?;

    let response = client
        .get(CURSOR_USAGE_SUMMARY_URL)
        .header("Accept", "application/json")
        .header("Cookie", &cookie)
        .header(
            "User-Agent",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)",
        )
        .send()
        .await
        .map_err(|e| format!("请求 Cursor usage API 失败: {}", e))?;

    let status = response.status().as_u16();
    if status == 401 {
        return Err(CURSOR_UI_QUOTA_QUERY_FAILED.to_string());
    }
    if status == 403 {
        return Err("Cursor 配额查询被限流，请稍后重试".to_string());
    }
    if status != 200 {
        return Err(format!("Cursor usage API 返回异常状态码: {}", status));
    }

    let body = response
        .text()
        .await
        .map_err(|e| format!("读取 Cursor usage 响应失败: {}", e))?;

    serde_json::from_str::<serde_json::Value>(&body)
        .map_err(|e| format!("解析 Cursor usage JSON 失败: {}", e))
}

// ---------------------------------------------------------------------------
// Refresh (updates our own account storage + fetches usage from official APIs)
// ---------------------------------------------------------------------------

async fn refresh_account_async_once(account_id: &str) -> Result<CursorRefreshResult, String> {
    let existing = load_account(account_id).ok_or_else(|| "账号不存在".to_string())?;
    logger::log_info(&format!(
        "[Cursor Refresh] 开始刷新账号: id={}, email={}",
        existing.id, existing.email
    ));

    let client = build_cursor_http_client()?;
    let mut account = existing.clone();

    if access_token_needs_refresh(&account.access_token) {
        match refresh_account_access_token_with_client(&client, &mut account).await {
            Ok(true) => {
                logger::log_info(&format!(
                    "[Cursor Refresh] access token 刷新成功: id={}",
                    account.id
                ));
            }
            Ok(false) => {}
            Err(err) => {
                logger::log_warn(&format!(
                    "[Cursor Refresh] access token 刷新失败，继续使用现有 token: id={}, error={}",
                    account.id, err
                ));
            }
        }
    }

    let access_token = account.access_token.clone();
    let (meta_result, stripe_result, usage_result) = tokio::join!(
        fetch_user_meta_with_client(&client, &access_token),
        fetch_stripe_profile_with_client(&client, &access_token),
        fetch_usage_summary_with_client(&client, &access_token),
    );

    match meta_result {
        Ok(meta) => {
            if let Some(api_email) = normalize_email_identity(meta.email.as_deref()) {
                upsert_cursor_auth_raw_string(&mut account, "cachedEmail", Some(api_email));
            }

            if let Some(sign_up_type) = normalize_cursor_sign_up_type(meta.sign_up_type.as_deref())
            {
                account.sign_up_type = Some(sign_up_type.clone());
                upsert_cursor_auth_raw_string(&mut account, "cachedSignUpType", Some(sign_up_type));
            }

            upsert_cursor_auth_raw_string(&mut account, "workosId", meta.workos_id.clone());

            logger::log_info(&format!(
                "[Cursor Refresh] 用户信息拉取成功: id={}, email={}",
                account.id, account.email
            ));
        }
        Err(err) => {
            logger::log_warn(&format!(
                "[Cursor Refresh] 用户信息拉取失败: id={}, error={}",
                account.id, err
            ));
        }
    }

    match stripe_result {
        Ok(Some(profile)) => {
            if let Some(membership_type) = resolve_membership_from_stripe_profile(&profile) {
                account.membership_type = Some(membership_type.clone());
                upsert_cursor_auth_raw_string(
                    &mut account,
                    "stripeMembershipType",
                    Some(membership_type),
                );
            }

            let subscription_status = normalize_non_empty(profile.subscription_status.as_deref());
            if let Some(status) = subscription_status.clone() {
                account.subscription_status = Some(status);
            }
            upsert_cursor_auth_raw_string(
                &mut account,
                "stripeSubscriptionStatus",
                subscription_status,
            );
            upsert_cursor_auth_raw_string(
                &mut account,
                "teamMembershipType",
                normalize_non_empty(profile.team_membership_type.as_deref()),
            );
            upsert_cursor_auth_raw_bool(&mut account, "isTeamMember", profile.is_team_member);
            upsert_cursor_auth_raw_bool(&mut account, "isEnterprise", profile.is_enterprise);

            logger::log_info(&format!(
                "[Cursor Refresh] 订阅信息拉取成功: id={}",
                account.id
            ));
        }
        Ok(None) => {
            logger::log_warn(&format!(
                "[Cursor Refresh] 未获取到订阅信息: id={}",
                account.id
            ));
        }
        Err(err) => {
            logger::log_warn(&format!(
                "[Cursor Refresh] 订阅信息拉取失败: id={}, error={}",
                account.id, err
            ));
        }
    }

    let mut usage_refreshed = false;
    match usage_result {
        Ok(usage) => {
            if let Some(mt) = usage.get("membershipType").and_then(|v| v.as_str()) {
                if !mt.is_empty() {
                    account.membership_type = Some(mt.to_string());
                }
            }
            account.cursor_usage_raw = Some(usage);
            account.quota_query_last_error = None;
            account.quota_query_last_error_at = None;
            usage_refreshed = true;
            logger::log_info(&format!(
                "[Cursor Refresh] API 配额拉取成功: id={}",
                account.id
            ));
        }
        Err(err) => {
            if is_cursor_transient_quota_error(&err) {
                logger::log_warn(&format!(
                    "[Cursor Refresh] transient 失败，跳过写盘: id={}, error={}",
                    account.id, err
                ));
                return Ok(CursorRefreshResult {
                    account: existing,
                    persisted: false,
                });
            }
            logger::log_warn(&format!(
                "[Cursor Refresh] API 配额拉取失败: id={}, error={}",
                account.id, err
            ));
            account.quota_query_last_error = Some(err);
            account.quota_query_last_error_at = Some(chrono::Utc::now().timestamp_millis());
        }
    }

    let refreshed_at = now_ts();
    if usage_refreshed {
        account.usage_updated_at = Some(refreshed_at);
    }
    account.last_used = refreshed_at;
    backfill_quota_pool_auth_id(&mut account);
    let updated = account.clone();
    upsert_account_record(account)?;
    logger::log_info(&format!(
        "[Cursor Refresh] 刷新完成: id={}, email={}",
        updated.id, updated.email
    ));
    Ok(CursorRefreshResult {
        account: updated.clone(),
        persisted: cursor_accounts_differ_for_refresh(&existing, &updated),
    })
}

pub async fn refresh_account_async(account_id: &str) -> Result<CursorRefreshResult, String> {
    let result = refresh_account_async_once(account_id).await;
    if let Err(err) = &result {
        if !is_cursor_transient_quota_error(err) {
            persist_quota_query_error(account_id, err);
        }
    }
    result
}

/// UI 热路径：与全量刷新共用 upstream 判定逻辑，避免 quota_only 漏标失败。
pub async fn refresh_account_fast_async(account_id: &str) -> Result<CursorRefreshResult, String> {
    {
        let mut in_flight = CURSOR_REFRESH_IN_FLIGHT
            .lock()
            .map_err(|_| "获取 Cursor 刷新锁失败".to_string())?;
        if !in_flight.insert(account_id.to_string()) {
            logger::log_info(&format!(
                "[Cursor Refresh] 跳过重复刷新(已在进行): id={}",
                account_id
            ));
            let account = load_account(account_id).ok_or_else(|| "账号不存在".to_string())?;
            return Ok(CursorRefreshResult {
                account,
                persisted: false,
            });
        }
    }

    let result = refresh_account_async(account_id).await;
    CURSOR_REFRESH_IN_FLIGHT
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(account_id);
    result
}

fn mark_refresh_attempted(account_id: &str) {
    let now = now_ts();
    let Ok(mut map) = CURSOR_REFRESH_RECENT_ATTEMPTS.lock() else {
        return;
    };
    map.insert(account_id.to_string(), now);
    let prune_before = now.saturating_sub(6 * 60 * 60);
    map.retain(|_, ts| *ts >= prune_before);
}

fn recent_refresh_attempt_ts(account_id: &str) -> i64 {
    CURSOR_REFRESH_RECENT_ATTEMPTS
        .lock()
        .ok()
        .and_then(|map| map.get(account_id).copied())
        .unwrap_or(0)
}

/// 调度键越小越优先刷：从未成功拉过用量 / 用量最旧 timestamp 在前；
/// 刚失败或刚尝试过的用尝试时间做冷却，避免死号占满每一轮（成功写盘则 usage_updated_at 变大，自然排到后面）。
fn refresh_schedule_ts(account: &CursorAccount) -> i64 {
    let usage = account.usage_updated_at.unwrap_or(0);
    let err_at = account
        .quota_query_last_error_at
        .map(|ms| ms / 1000)
        .unwrap_or(0);
    let recent = recent_refresh_attempt_ts(&account.id);
    let last_touch = recent.max(err_at);
    if last_touch > usage {
        last_touch
    } else {
        usage
    }
}

/// 按用量新鲜度升序串行刷新（最旧优先）。
/// `max_count`：本轮最多刷几个；`None` = 全部。
/// `max_duration`：墙钟时限；超时提前结束，下一轮仍会挑当前最旧的。
pub async fn refresh_tokens_stale_first(
    max_count: Option<usize>,
    max_duration: Option<Duration>,
) -> Result<Vec<(String, Result<CursorRefreshResult, String>)>, String> {
    let mut active_accounts: Vec<CursorAccount> = list_accounts()
        .into_iter()
        .filter(|account| !is_banned_account(account))
        .collect();

    active_accounts.sort_by(|left, right| {
        refresh_schedule_ts(left)
            .cmp(&refresh_schedule_ts(right))
            .then_with(|| left.id.cmp(&right.id))
    });

    let total_active = active_accounts.len();
    if let Some(limit) = max_count {
        if active_accounts.len() > limit {
            active_accounts.truncate(limit);
        }
    }

    logger::log_info(&format!(
        "[Cursor Refresh] 最旧优先刷新开始: active={}, this_round={}, max_count={:?}, max_duration_secs={:?}, first_id={:?}, last_id={:?}",
        total_active,
        active_accounts.len(),
        max_count,
        max_duration.map(|d| d.as_secs()),
        active_accounts.first().map(|a| a.id.as_str()),
        active_accounts.last().map(|a| a.id.as_str()),
    ));

    let started = std::time::Instant::now();
    let mut results = Vec::with_capacity(active_accounts.len());
    for account in active_accounts {
        if let Some(max_dur) = max_duration {
            if started.elapsed() >= max_dur {
                logger::log_info(&format!(
                    "[Cursor Refresh] 达到时限提前结束: elapsed={}ms, done={}",
                    started.elapsed().as_millis(),
                    results.len()
                ));
                break;
            }
        }
        let id = account.id.clone();
        mark_refresh_attempted(&id);
        let result = refresh_account_async(&id).await;
        results.push((id, result));
    }
    Ok(results)
}

pub async fn refresh_all_tokens(
) -> Result<Vec<(String, Result<CursorRefreshResult, String>)>, String> {
    // 全量仍串行；改为最旧优先，避免每轮只按索引从头扫、旧号长期得不到成功写回。
    refresh_tokens_stale_first(None, None).await
}

// ---------------------------------------------------------------------------
// Quota alert
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default)]
struct CursorUsagePercent {
    total_used: Option<i32>,
    auto_used: Option<i32>,
    api_used: Option<i32>,
}

fn clamp_percent(value: f64) -> i32 {
    if !value.is_finite() {
        return 0;
    }
    if value <= 0.0 {
        return 0;
    }
    if value >= 100.0 {
        return 100;
    }
    value.round() as i32
}

fn pick_number(value: Option<&Value>, keys: &[&str]) -> Option<f64> {
    let obj = value?.as_object()?;
    for key in keys {
        let Some(raw) = obj.get(*key) else {
            continue;
        };
        if let Some(n) = raw.as_f64() {
            if n.is_finite() {
                return Some(n);
            }
            continue;
        }
        if let Some(text) = raw.as_str() {
            if let Ok(parsed) = text.trim().parse::<f64>() {
                if parsed.is_finite() {
                    return Some(parsed);
                }
            }
        }
    }
    None
}

fn read_usage_percent(account: &CursorAccount) -> CursorUsagePercent {
    let Some(raw) = account.cursor_usage_raw.as_ref() else {
        return CursorUsagePercent::default();
    };

    let raw_obj = match raw.as_object() {
        Some(value) => value,
        None => return CursorUsagePercent::default(),
    };

    let plan_value = raw_obj
        .get("individualUsage")
        .and_then(|value| value.as_object())
        .and_then(|value| value.get("plan"))
        .or_else(|| {
            raw_obj
                .get("individual_usage")
                .and_then(|value| value.as_object())
                .and_then(|value| value.get("plan"))
        })
        .or_else(|| raw_obj.get("planUsage"))
        .or_else(|| raw_obj.get("plan_usage"));

    let total_direct = pick_number(plan_value, &["totalPercentUsed", "total_percent_used"]);
    let auto_direct = pick_number(plan_value, &["autoPercentUsed", "auto_percent_used"]);
    let api_direct = pick_number(plan_value, &["apiPercentUsed", "api_percent_used"]);

    let used = pick_number(plan_value, &["used", "totalSpend", "total_spend"]);
    let limit = pick_number(plan_value, &["limit"]);
    let total_ratio = match (used, limit) {
        (Some(used_val), Some(limit_val)) if limit_val > 0.0 => {
            Some((used_val / limit_val) * 100.0)
        }
        _ => None,
    };

    // breakdown.total 是已用量合计，不是上限；与主仓库一致，只信 totalPercentUsed / used÷limit。
    CursorUsagePercent {
        total_used: total_direct.or(total_ratio).map(clamp_percent),
        auto_used: auto_direct.map(clamp_percent),
        api_used: api_direct.map(clamp_percent),
    }
}

pub(crate) fn extract_quota_metrics(account: &CursorAccount) -> Vec<(String, i32)> {
    let usage = read_usage_percent(account);
    let mut metrics = Vec::new();

    if let Some(used) = usage.total_used {
        metrics.push(("Total Usage".to_string(), 100 - used.clamp(0, 100)));
    }
    if let Some(used) = usage.auto_used {
        metrics.push(("Auto + Composer".to_string(), 100 - used.clamp(0, 100)));
    }
    if let Some(used) = usage.api_used {
        metrics.push(("API Usage".to_string(), 100 - used.clamp(0, 100)));
    }

    metrics
}

fn average_quota_percentage(metrics: &[(String, i32)]) -> f64 {
    if metrics.is_empty() {
        return 0.0;
    }
    let sum: i32 = metrics.iter().map(|(_, pct)| *pct).sum();
    sum as f64 / metrics.len() as f64
}

const FULL_QUOTA_REMAINING_THRESHOLD: f64 = 99.0;
const SWITCH_FULL_POOL_REMAINING_MIN: i32 = 99;

/// 与卡片进度条一致：max(total/auto/api 已用%)。验活 ok 且 total 已满时换号池看 api 剩余。
fn cursor_switch_max_used_percent(account: &CursorAccount, usage: &CursorUsagePercent) -> Option<i32> {
    if let Some(probe) = account.chat_probe.as_ref() {
        if probe.outcome == "rate_limited" {
            return Some(100);
        }
        if probe.outcome == "ok" {
            let total = usage.total_used?;
            if total >= 100 {
                if let Some(api_used) = usage.api_used {
                    return Some(api_used.clamp(0, 100));
                }
            }
        }
    }

    let mut used_candidates = Vec::new();
    if let Some(used) = usage.total_used {
        used_candidates.push(used);
    }
    if let Some(used) = usage.auto_used {
        used_candidates.push(used);
    }
    if let Some(used) = usage.api_used {
        used_candidates.push(used);
    }
    used_candidates.into_iter().max()
}

/// 与账号总览 UI `resolveRemainingQuotaPercent` 对齐：100 - max(各维度已用%)。
/// Agent 验活结果优先：ok 视为有剩余；rate_limited 视为用尽。
pub fn cursor_switch_remaining_percent(account: &CursorAccount) -> Option<i32> {
    if let Some(probe) = account.chat_probe.as_ref() {
        if probe.outcome == "ok" {
            return Some(
                cursor_switch_remaining_percent_from_usage(account)
                    .unwrap_or(1)
                    .max(1),
            );
        }
        if probe.outcome == "rate_limited" {
            return Some(0);
        }
    }
    cursor_switch_remaining_percent_from_usage(account)
}

fn cursor_switch_remaining_percent_from_usage(account: &CursorAccount) -> Option<i32> {
    if has_quota_query_failed(account) {
        return None;
    }
    let usage = read_usage_percent(account);
    let max_used = cursor_switch_max_used_percent(account, &usage)?;
    Some(100 - max_used.clamp(0, 100))
}

/// total/auto/api 按 Agent 对话口径判断是否用尽（api 维度优先于过期 total）。
pub fn is_cursor_quota_exhausted_for_switch(account: &CursorAccount) -> bool {
    if let Some(probe) = account.chat_probe.as_ref() {
        if probe.outcome == "ok" {
            return false;
        }
        if probe.outcome == "rate_limited" {
            return true;
        }
    }
    match cursor_switch_remaining_percent_from_usage(account) {
        Some(remaining) => remaining <= 0,
        None => has_quota_query_failed(account),
    }
}

pub fn cursor_switch_usage_snapshot(account: &CursorAccount) -> (Option<i32>, Option<i32>, Option<i32>) {
    let usage = read_usage_percent(account);
    (
        cursor_switch_remaining_percent(account),
        usage.auto_used,
        usage.total_used,
    )
}

#[derive(Debug, Clone)]
pub struct CursorRotationPick {
    pub account_id: String,
    pub pool: &'static str,
    pub candidates: usize,
    pub remaining_pct: i32,
}

fn plan_limit_value(account: &CursorAccount) -> Option<f64> {
    let raw = account.cursor_usage_raw.as_ref()?;
    let raw_obj = raw.as_object()?;
    let plan_value = raw_obj
        .get("individualUsage")
        .and_then(|value| value.as_object())
        .and_then(|value| value.get("plan"))
        .or_else(|| {
            raw_obj
                .get("individual_usage")
                .and_then(|value| value.as_object())
                .and_then(|value| value.get("plan"))
        })
        .or_else(|| raw_obj.get("planUsage"))
        .or_else(|| raw_obj.get("plan_usage"));
    pick_number(plan_value, &["limit"])
}

pub fn has_effective_plan_budget(account: &CursorAccount) -> bool {
    plan_limit_value(account).is_some_and(|limit| limit > 0.0)
}

fn has_nirvana_switch_ready_tokens(account: &CursorAccount) -> bool {
    nirvana_kh_auth_tokens(account).is_ok()
}

pub fn is_full_quota_account(account: &CursorAccount) -> bool {
    if !is_cursor_switch_ready_account(account) {
        return false;
    }
    if !has_effective_plan_budget(account) {
        return false;
    }
    let metrics = extract_quota_metrics(account);
    if metrics.is_empty() {
        return false;
    }
    average_quota_percentage(&metrics) >= FULL_QUOTA_REMAINING_THRESHOLD
}

fn normalize_quota_alert_threshold(value: i32) -> i32 {
    value.clamp(0, 100)
}

pub(crate) fn resolve_current_account_id(accounts: &[CursorAccount]) -> Option<String> {
    crate::modules::provider_current_state::resolve_existing_current_account_id(
        "cursor",
        accounts.iter().map(|account| account.id.as_str()),
    )
}

pub fn resolve_current_account_id_for_refresh() -> Option<String> {
    let accounts = list_accounts();
    resolve_current_account_id(&accounts)
}

#[derive(Debug, Clone)]
pub struct CursorLocalProfileSyncResult {
    pub imported: bool,
    pub current_updated: bool,
    /// 同 id 账号邮箱等字段被本地 profile 更新（须通知前端重拉列表）
    pub profile_updated: bool,
    pub account: Option<CursorAccount>,
}

fn find_account_for_import_payload(payload: &CursorImportPayload) -> Option<CursorAccount> {
    let incoming_email = normalize_email_identity(Some(payload.email.as_str()));

    list_accounts().into_iter().find(|account| {
        let existing_email = normalize_email_identity(Some(account.email.as_str()));
        matches!(
            (existing_email.as_ref(), incoming_email.as_ref()),
            (Some(ex_email), Some(in_email)) if ex_email == in_email
        )
    })
}

fn local_import_payload_matches_account(
    account: &CursorAccount,
    payload: &CursorImportPayload,
) -> bool {
    normalize_email_identity(Some(account.email.as_str()))
        == normalize_email_identity(Some(payload.email.as_str()))
        && normalize_token_identity(Some(account.access_token.as_str()))
            == normalize_token_identity(Some(payload.access_token.as_str()))
}

/// TokenKeeper 每 20s 读默认 profile `state.vscdb`：新号入库、current 对齐本地登录态。
pub fn sync_local_cursor_from_default_profile() -> Result<CursorLocalProfileSyncResult, String> {
    let payload = match read_local_cursor_auth()? {
        Some(value) => value,
        None => {
            return Ok(CursorLocalProfileSyncResult {
                imported: false,
                current_updated: false,
                profile_updated: false,
                account: None,
            });
        }
    };

    let payload_for_backup = payload.clone();
    let mut profile_updated = false;
    let (account, imported) =
        if let Some(existing) = find_account_for_import_payload(&payload) {
            if local_import_payload_matches_account(&existing, &payload) {
                (existing, false)
            } else {
                let email_changed = normalize_email_identity(Some(existing.email.as_str()))
                    != normalize_email_identity(Some(payload.email.as_str()));
                let outcome = upsert_account_with_outcome(payload)?;
                record_import_backup(&payload_for_backup, &outcome)?;
                profile_updated = !outcome.created && email_changed;
                (outcome.account, outcome.created)
            }
        } else {
            let outcome = upsert_account_with_outcome(payload)?;
            record_import_backup(&payload_for_backup, &outcome)?;
            (outcome.account, outcome.created)
        };

    let stored_current =
        crate::modules::provider_current_state::get_current_account_id("cursor").ok().flatten();
    let mut current_updated = false;
    if stored_current.as_deref() != Some(account.id.as_str()) {
        crate::modules::provider_current_state::set_current_account_id(
            "cursor",
            Some(account.id.as_str()),
        )?;
        current_updated = true;
    }

    if imported || current_updated || profile_updated {
        logger::log_info(&format!(
            "[Cursor Account] 本地 profile 同步: imported={}, current_updated={}, profile_updated={}, id={}, email={}",
            imported, current_updated, profile_updated, account.id, account.email
        ));
    }

    if !imported && !current_updated && !profile_updated {
        return Ok(CursorLocalProfileSyncResult {
            imported: false,
            current_updated: false,
            profile_updated: false,
            account: None,
        });
    }

    Ok(CursorLocalProfileSyncResult {
        imported,
        current_updated,
        profile_updated,
        account: Some(account),
    })
}

fn remaining_credits_sort_key(account: &CursorAccount) -> (i32, i64) {
    let remaining = average_quota_percentage(&extract_quota_metrics(account)) as i32;
    let refreshed_at = account.usage_updated_at.unwrap_or(0);
    (remaining, refreshed_at)
}

/// 实例 Play / 多开启动：每次强制轮换（排除当前绑定），满额池均匀随机，否则好号按剩余额度加权随机。
/// 按 usage-summary 剩余额度选号；不把 chat_probe=ok 单独抬成优先池（HR-20260717-003）。
pub fn pick_cursor_rotation_account(
    exclude_ids: &HashSet<String>,
) -> Result<CursorRotationPick, String> {
    let mut full_pool: Vec<(CursorAccount, i32)> = Vec::new();
    let mut good_pool: Vec<(CursorAccount, i32)> = Vec::new();
    let mut excluded_exhausted = 0usize;

    for account in list_accounts() {
        if exclude_ids.contains(&account.id) {
            continue;
        }
        if is_cursor_overview_abnormal(&account) {
            continue;
        }
        if !has_nirvana_switch_ready_tokens(&account) {
            continue;
        }
        let Some(remaining) = cursor_overview_remaining_percent(&account) else {
            continue;
        };
        if remaining <= 0 {
            excluded_exhausted += 1;
            continue;
        }
        if remaining >= SWITCH_FULL_POOL_REMAINING_MIN {
            full_pool.push((account, remaining));
        } else {
            good_pool.push((account, remaining));
        }
    }

    if excluded_exhausted > 0 {
        logger::log_info(&format!(
            "[Cursor Switch] pick 排除: exhausted={}",
            excluded_exhausted
        ));
    }

    let mut rng = rand::thread_rng();

    let (use_pool_name, pool): (&str, &Vec<(CursorAccount, i32)>) = if !full_pool.is_empty() {
        ("full", &full_pool)
    } else {
        ("good", &good_pool)
    };

    if pool.is_empty() {
        return Err("没有可用的 Cursor 轮换账号".to_string());
    }

    if let Some((account, remaining)) = pool.choose(&mut rng).cloned() {
        let candidates = pool.len();
        logger::log_info(&format!(
            "[Cursor Switch] pick: pool={} candidates={} picked={} email={} remaining={}% auto={:?}% total={:?}% chat_probe={:?}",
            use_pool_name,
            candidates,
            account.id,
            account.email,
            remaining,
            cursor_switch_usage_snapshot(&account).1,
            cursor_switch_usage_snapshot(&account).2,
            account.chat_probe.as_ref().map(|p| p.outcome.as_str()),
        ));
        return Ok(CursorRotationPick {
            account_id: account.id,
            pool: match use_pool_name {
                "full" => "full",
                _ => "good",
            },
            candidates,
            remaining_pct: remaining,
        });
    }

    Err("没有可用的 Cursor 轮换账号".to_string())
}

pub fn pick_full_quota_account(exclude_ids: &HashSet<String>) -> Option<String> {
    let mut candidates: Vec<CursorAccount> = list_accounts()
        .into_iter()
        .filter(|account| !exclude_ids.contains(&account.id))
        .filter(|account| is_full_quota_account(account))
        .collect();

    if candidates.is_empty() {
        return None;
    }

    candidates.sort_by(|left, right| {
        remaining_credits_sort_key(right).cmp(&remaining_credits_sort_key(left))
    });

    candidates.first().map(|account| account.id.clone())
}

pub fn pick_highest_remaining_credits_account(exclude_ids: &HashSet<String>) -> Option<String> {
    let mut candidates: Vec<CursorAccount> = list_accounts()
        .into_iter()
        .filter(|account| !exclude_ids.contains(&account.id))
        .filter(|account| is_cursor_switch_ready_account(account))
        .collect();

    if candidates.is_empty() {
        return None;
    }

    // 排序：剩余 Credits 高 → 低；同额度最后刷新优先；失败排最后
    candidates.sort_by(|left, right| {
        let left_remaining = average_quota_percentage(&extract_quota_metrics(left)) as i32;
        let right_remaining = average_quota_percentage(&extract_quota_metrics(right)) as i32;
        let left_failed = has_quota_query_failed(left);
        let right_failed = has_quota_query_failed(right);
        left_failed
            .cmp(&right_failed)
            .then_with(|| right_remaining.cmp(&left_remaining))
            .then_with(|| {
                let left_ts = left.usage_updated_at.unwrap_or(0);
                let right_ts = right.usage_updated_at.unwrap_or(0);
                right_ts.cmp(&left_ts)
            })
    });

    candidates.first().map(|account| account.id.clone())
}

fn pick_quota_alert_recommendation(
    accounts: &[CursorAccount],
    current_id: &str,
) -> Option<CursorAccount> {
    let mut candidates: Vec<CursorAccount> = accounts
        .iter()
        .filter(|account| account.id != current_id)
        .filter(|account| !is_banned_account(account))
        .filter(|account| !extract_quota_metrics(account).is_empty())
        .cloned()
        .collect();

    if candidates.is_empty() {
        return None;
    }

    candidates.sort_by(|a, b| {
        let avg_a = average_quota_percentage(&extract_quota_metrics(a));
        let avg_b = average_quota_percentage(&extract_quota_metrics(b));
        avg_b
            .partial_cmp(&avg_a)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.last_used.cmp(&b.last_used))
    });

    candidates.into_iter().next()
}

fn display_email(account: &CursorAccount) -> String {
    let trimmed = account.email.trim();
    if trimmed.is_empty() {
        account.id.clone()
    } else {
        trimmed.to_string()
    }
}

fn build_quota_alert_cooldown_key(account_id: &str, threshold: i32) -> String {
    format!("cursor:{}:{}", account_id, threshold)
}

fn should_emit_quota_alert(cooldown_key: &str, now: i64) -> bool {
    let Ok(mut state) = CURSOR_QUOTA_ALERT_LAST_SENT.lock() else {
        return true;
    };

    if let Some(last_sent) = state.get(cooldown_key) {
        if now - *last_sent < CURSOR_QUOTA_ALERT_COOLDOWN_SECONDS {
            return false;
        }
    }

    state.insert(cooldown_key.to_string(), now);
    true
}

fn clear_quota_alert_cooldown(account_id: &str, threshold: i32) {
    if let Ok(mut state) = CURSOR_QUOTA_ALERT_LAST_SENT.lock() {
        state.remove(&build_quota_alert_cooldown_key(account_id, threshold));
    }
}

pub fn run_quota_alert_if_needed(
) -> Result<Option<crate::modules::account::QuotaAlertPayload>, String> {
    let cfg = crate::modules::config::get_user_config();
    if !cfg.cursor_quota_alert_enabled {
        return Ok(None);
    }

    let threshold = normalize_quota_alert_threshold(cfg.cursor_quota_alert_threshold);
    let accounts = list_accounts();
    let current_id = match resolve_current_account_id(&accounts) {
        Some(id) => id,
        None => return Ok(None),
    };

    let current = match accounts.iter().find(|account| account.id == current_id) {
        Some(account) => account,
        None => return Ok(None),
    };
    if is_banned_account(current) {
        return Ok(None);
    }

    let metrics = extract_quota_metrics(current);
    if metrics.is_empty() {
        clear_quota_alert_cooldown(&current_id, threshold);
        return Ok(None);
    }

    let low_models: Vec<(String, i32)> = metrics
        .into_iter()
        .filter(|(_, pct)| *pct <= threshold)
        .collect();
    if low_models.is_empty() {
        clear_quota_alert_cooldown(&current_id, threshold);
        return Ok(None);
    }

    let now = chrono::Utc::now().timestamp();
    let cooldown_key = build_quota_alert_cooldown_key(&current_id, threshold);
    if !should_emit_quota_alert(&cooldown_key, now) {
        return Ok(None);
    }

    let recommendation = pick_quota_alert_recommendation(&accounts, &current_id);
    let lowest_percentage = low_models.iter().map(|(_, pct)| *pct).min().unwrap_or(0);
    let payload = crate::modules::account::QuotaAlertPayload {
        platform: "cursor".to_string(),
        current_account_id: current_id,
        current_email: display_email(current),
        threshold,
        threshold_display: None,
        lowest_percentage,
        low_models: low_models.into_iter().map(|(name, _)| name).collect(),
        recommended_account_id: recommendation.as_ref().map(|account| account.id.clone()),
        recommended_email: recommendation.as_ref().map(display_email),
        triggered_at: now,
    };

    crate::modules::account::dispatch_quota_alert(&payload);
    Ok(Some(payload))
}

#[cfg(test)]
mod cursor_overview_pick_tests {
    use super::*;
    use crate::models::cursor::CursorAccount;

    fn account_with_usage(id: &str, total: i32, auto: i32) -> CursorAccount {
        CursorAccount {
            id: id.into(),
            email: format!("{id}@test.com"),
            auth_id: None,
            name: None,
            tags: None,
            access_token: "eyJhbGciOiJIUzI1NiJ9.e30.sig".into(),
            refresh_token: Some("eyJhbGciOiJIUzI1NiJ9.e30.sig".into()),
            membership_type: None,
            subscription_status: None,
            sign_up_type: None,
            cursor_auth_raw: None,
            cursor_usage_raw: Some(serde_json::json!({
                "individualUsage": {
                    "plan": {
                        "totalPercentUsed": total,
                        "autoPercentUsed": auto
                    }
                }
            })),
            status: None,
            status_reason: None,
            quota_query_last_error: None,
            quota_query_last_error_at: None,
            usage_updated_at: Some(super::now_ts()),
            chat_probe: None,
            created_at: 0,
            last_used: 0,
        }
    }

    #[test]
    fn remaining_percent_matches_ui_max_used() {
        let account = account_with_usage("a", 79, 100);
        assert_eq!(cursor_overview_remaining_percent(&account), Some(0));
        assert!(ensure_cursor_overview_pickable(&account).is_err());
    }

    #[test]
    fn free_zero_used_counts_as_full_pool() {
        let account = account_with_usage("b", 0, 0);
        assert_eq!(cursor_overview_remaining_percent(&account), Some(100));
        assert!(ensure_cursor_overview_pickable(&account).is_ok());
    }

    #[test]
    fn session_expired_is_abnormal_and_not_pickable() {
        let mut account = account_with_usage("c", 0, 0);
        account.quota_query_last_error = Some(CURSOR_UI_QUOTA_QUERY_FAILED.into());
        assert!(is_cursor_overview_abnormal(&account));
        assert!(cursor_overview_remaining_percent(&account).is_none());
        assert!(ensure_cursor_overview_pickable(&account).is_err());
    }

    #[test]
    fn manual_pick_allows_quota_error_when_tokens_present() {
        let mut account = account_with_usage("d", 0, 0);
        account.quota_query_last_error = Some(CURSOR_UI_QUOTA_QUERY_FAILED.into());
        account.cursor_auth_raw = Some(serde_json::json!({
            "accessToken": "eyJhbGciOiJIUzI1NiJ9.test",
            "refreshToken": "eyJhbGciOiJIUzI1NiJ9.refresh"
        }));
        assert!(ensure_cursor_switch_ready_account(&account).is_err());
        assert!(ensure_cursor_manual_pick_allowed(&account).is_ok());
        assert!(ensure_cursor_switch_allowed(&account, true).is_ok());
        assert!(ensure_cursor_switch_allowed(&account, false).is_err());
    }
}

#[cfg(test)]
mod cursor_rotation_pick_tests {
    use super::*;
    use crate::models::cursor::CursorAccount;

    fn account_with_usage(id: &str, total: i32, auto: i32) -> CursorAccount {
        account_with_usage_dims(id, total, auto, None)
    }

    fn account_with_usage_dims(id: &str, total: i32, auto: i32, api: Option<i32>) -> CursorAccount {
        let mut plan = serde_json::json!({
            "totalPercentUsed": total,
            "autoPercentUsed": auto
        });
        if let Some(api_used) = api {
            plan.as_object_mut()
                .expect("plan object")
                .insert("apiPercentUsed".into(), serde_json::json!(api_used));
        }
        CursorAccount {
            id: id.into(),
            email: format!("{id}@test.com"),
            auth_id: None,
            name: None,
            tags: None,
            access_token: "eyJhbGciOiJIUzI1NiJ9.e30.sig".into(),
            refresh_token: Some("eyJhbGciOiJIUzI1NiJ9.e30.sig".into()),
            membership_type: None,
            subscription_status: None,
            sign_up_type: None,
            cursor_auth_raw: None,
            cursor_usage_raw: Some(serde_json::json!({
                "individualUsage": {
                    "plan": plan
                }
            })),
            status: None,
            status_reason: None,
            quota_query_last_error: None,
            quota_query_last_error_at: None,
            usage_updated_at: Some(super::now_ts()),
            chat_probe: None,
            created_at: 0,
            last_used: 0,
        }
    }

    #[test]
    fn remaining_percent_matches_ui_max_used() {
        let account = account_with_usage("a", 79, 100);
        assert_eq!(cursor_switch_remaining_percent(&account), Some(0));
        assert!(is_cursor_quota_exhausted_for_switch(&account));
    }

    #[test]
    fn total_full_but_api_open_counts_as_bar_exhausted() {
        let account = account_with_usage_dims("chatable", 100, 100, Some(0));
        assert_eq!(cursor_switch_remaining_percent(&account), Some(0));
        assert!(is_cursor_quota_exhausted_for_switch(&account));
    }

    #[test]
    fn total_full_api_open_with_probe_ok_uses_api_remaining() {
        let mut account = account_with_usage_dims("probed", 100, 100, Some(0));
        account.chat_probe = Some(crate::models::cursor::CursorChatProbe {
            outcome: "ok".into(),
            probed_at: 1,
            detail: None,
            duration_ms: None,
            status_email: None,
            request_id: None,
        });
        assert_eq!(cursor_switch_remaining_percent(&account), Some(100));
        assert!(!is_cursor_quota_exhausted_for_switch(&account));
    }

    #[test]
    fn total_and_api_full_counts_as_exhausted() {
        let account = account_with_usage_dims("blocked", 100, 100, Some(100));
        assert_eq!(cursor_switch_remaining_percent(&account), Some(0));
        assert!(is_cursor_quota_exhausted_for_switch(&account));
    }

    #[test]
    fn free_zero_used_counts_as_full_pool() {
        let account = account_with_usage("b", 0, 0);
        assert_eq!(cursor_switch_remaining_percent(&account), Some(100));
        assert!(!is_cursor_quota_exhausted_for_switch(&account));
    }
}

#[cfg(test)]
mod cursor_auth_token_tests {
    use super::*;
    use crate::models::cursor::CursorAccount;

    fn sample_jwt() -> String {
        "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJhdXRoMHx1c2VyXzAxSFFGR0g4WjY4WjY4WjY4WiIsImV4cCI6OTk5OTk5OTk5fQ.sig".to_string()
    }

    #[test]
    fn switch_ready_when_raw_empty_but_access_token_present() {
        let jwt = sample_jwt();
        let account = CursorAccount {
            id: "t".into(),
            email: "a@b.com".into(),
            auth_id: None,
            name: None,
            tags: None,
            access_token: jwt,
            refresh_token: Some("refresh".into()),
            membership_type: None,
            subscription_status: None,
            sign_up_type: None,
            cursor_auth_raw: Some(serde_json::json!({})),
            cursor_usage_raw: Some(serde_json::json!({
                "individualUsage": { "plan": { "totalPercentUsed": 0, "autoPercentUsed": 0 } }
            })),
            status: None,
            status_reason: None,
            quota_query_last_error: None,
            quota_query_last_error_at: None,
            usage_updated_at: None,
            chat_probe: None,
            created_at: 0,
            last_used: 0,
        };
        assert!(has_nirvana_switch_ready_tokens(&account));
        assert!(ensure_cursor_overview_pickable(&account).is_ok());
    }

    #[test]
    fn nirvana_kh_keeps_raw_refresh_token() {
        let jwt = sample_jwt();
        let account = CursorAccount {
            id: "t".into(),
            email: "a@b.com".into(),
            auth_id: None,
            name: None,
            tags: None,
            access_token: jwt.clone(),
            refresh_token: Some(jwt.clone()),
            membership_type: None,
            subscription_status: None,
            sign_up_type: None,
            cursor_auth_raw: None,
            cursor_usage_raw: None,
            status: None,
            status_reason: None,
            quota_query_last_error: None,
            quota_query_last_error_at: None,
            usage_updated_at: None,
            chat_probe: None,
            created_at: 0,
            last_used: 0,
        };
        let (access, refresh) = nirvana_kh_auth_tokens(&account).expect("raw");
        assert_eq!(access, jwt);
        assert_eq!(refresh, jwt);
        assert!(!refresh.contains("::"));
    }

    #[test]
    fn builds_session_when_refresh_missing() {
        let account = CursorAccount {
            id: "t".into(),
            email: "a@b.com".into(),
            auth_id: None,
            name: None,
            tags: None,
            access_token: sample_jwt(),
            refresh_token: None,
            membership_type: None,
            subscription_status: None,
            sign_up_type: None,
            cursor_auth_raw: None,
            cursor_usage_raw: None,
            status: None,
            status_reason: None,
            quota_query_last_error: None,
            quota_query_last_error_at: None,
            usage_updated_at: None,
            chat_probe: None,
            created_at: 0,
            last_used: 0,
        };
        let (access, session) = resolve_vscdb_auth_tokens(&account).expect("session");
        assert!(!access.contains("::"));
        assert!(session.contains("::"));
        assert!(session.contains("user_"));
    }

    #[test]
    fn splits_session_access_token() {
        let jwt = sample_jwt();
        let session = format!("user_01TEST::{jwt}");
        let (access, out) =
            resolve_vscdb_auth_tokens_from_parts(&session, Some(&session)).expect("ok");
        assert_eq!(access, jwt);
        assert_eq!(out, session);
    }

    #[test]
    fn test_zero_breakdown_total_keeps_server_percent() {
        // breakdown.total==0 只表示尚未用量；须保留 totalPercentUsed=0，禁止强制 100
        let usage_json = serde_json::json!({
            "isUnlimited": false,
            "individualUsage": {
                "plan": {
                    "limit": 0.0,
                    "totalPercentUsed": 0,
                    "autoPercentUsed": 0,
                    "apiPercentUsed": 0,
                    "breakdown": {
                        "included": 0.0,
                        "bonus": 0.0,
                        "total": 0.0
                    }
                },
                "onDemand": {
                    "enabled": false,
                    "limit": 0.0
                }
            }
        });
        let account = CursorAccount {
            id: "test_zero_quota".into(),
            email: "zero@quota.com".into(),
            auth_id: None,
            name: None,
            tags: None,
            access_token: "fake_token".into(),
            refresh_token: None,
            membership_type: None,
            subscription_status: None,
            sign_up_type: None,
            cursor_auth_raw: None,
            cursor_usage_raw: Some(usage_json),
            status: None,
            status_reason: None,
            quota_query_last_error: None,
            quota_query_last_error_at: None,
            usage_updated_at: None,
            chat_probe: None,
            created_at: 0,
            last_used: 0,
        };
        let usage = read_usage_percent(&account);
        assert_eq!(usage.total_used, Some(0));
        assert_eq!(usage.auto_used, Some(0));
        assert_eq!(usage.api_used, Some(0));
    }

    #[test]
    fn test_free_bonus_quota_not_forced_exhausted() {
        // FREE：included=0 但 bonus/total>0 → 必须保留真实百分比，禁止强制 100
        let usage_json = serde_json::json!({
            "isUnlimited": false,
            "individualUsage": {
                "plan": {
                    "limit": 0.0,
                    "totalPercentUsed": 67.0,
                    "autoPercentUsed": 100.0,
                    "apiPercentUsed": 0.0,
                    "breakdown": {
                        "included": 0.0,
                        "bonus": 134.0,
                        "total": 134.0
                    }
                },
                "onDemand": {
                    "enabled": false,
                    "limit": null
                }
            }
        });
        let account = CursorAccount {
            id: "test_bonus_quota".into(),
            email: "bonus@quota.com".into(),
            auth_id: None,
            name: None,
            tags: None,
            access_token: "fake_token".into(),
            refresh_token: None,
            membership_type: Some("free".into()),
            subscription_status: None,
            sign_up_type: None,
            cursor_auth_raw: None,
            cursor_usage_raw: Some(usage_json),
            status: None,
            status_reason: None,
            quota_query_last_error: None,
            quota_query_last_error_at: None,
            usage_updated_at: None,
            chat_probe: None,
            created_at: 0,
            last_used: 0,
        };
        let usage = read_usage_percent(&account);
        assert_eq!(usage.total_used, Some(67));
        assert_eq!(usage.auto_used, Some(100));
        assert_eq!(usage.api_used, Some(0));
    }
}

