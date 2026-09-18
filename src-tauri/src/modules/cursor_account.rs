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
use std::time::{Duration, Instant};
use uuid::Uuid;

use crate::models::cursor::{
    CursorAccount, CursorAccountIndex, CursorAccountListPage, CursorCurrentQuotaSnapshot,
    CursorImportPayload,
};
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
    /// 0013：本进程内已知的「零额度号」（free，plan.limit=0），仅用于状态变更日志。
    static ref CURSOR_ZERO_BUDGET_SEEN: Mutex<HashSet<String>> = Mutex::new(HashSet::new());
    static ref AUTO_SWITCH_LAST_AT: Mutex<HashMap<String, Instant>> = Mutex::new(HashMap::new());
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
    match crate::modules::secure_account_storage::deserialize_account_file::<CursorAccount>(
        &account_path,
        &content,
    ) {
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

/// 按邮箱定位池内账号（续杯 get-token / check-usage 用）。大小写不敏感。
pub fn find_account_id_by_email(email: &str) -> Option<String> {
    let needle = normalize_email_identity(Some(email))?;
    list_accounts()
        .into_iter()
        .find(|account| {
            normalize_email_identity(Some(account.email.as_str())).as_ref() == Some(&needle)
        })
        .map(|account| account.id)
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
    let lock_wait_started = std::time::Instant::now();
    let _lock = CURSOR_ACCOUNT_INDEX_LOCK
        .lock()
        .map_err(|_| "获取 Cursor 账号锁失败".to_string())?;
    let lock_wait_ms = lock_wait_started.elapsed().as_millis();
    if lock_wait_ms >= 200 {
        logger::log_warn(&format!(
            "[Cursor Perf] 等账号索引锁 {}ms",
            lock_wait_ms
        ));
    }
    let mut index = load_account_index();
    save_account_file(&account)?;
    refresh_summary(&mut index, &account);
    save_account_index(&index)?;
    Ok(account)
}

/// 0012：批量刷新专用——只写账号详情文件，不动整表索引。
/// 索引（4482 条 / 1.1MB）由批末 `flush_account_index_for` 统一写一次，
/// 否则每刷一个账号都重写全表索引，单账号被拖到数十秒，一轮扫不完池子。
fn upsert_account_record_deferred(account: CursorAccount) -> Result<CursorAccount, String> {
    save_account_file(&account)?;
    Ok(account)
}

/// 0012：批末统一回写索引——读一次索引、批量更新 summary、写一次盘。
pub fn flush_account_index_for(accounts: &[CursorAccount]) {
    if accounts.is_empty() {
        return;
    }
    // 拿不到锁时绝不静默丢弃整批：原实现 `let Ok(_lock) = ... else { warn; return }`
    // 会把整轮刷新结果吞掉（界面表现「刷新了额度没变 / 账号像被吞」）。
    // 退化为逐个 upsert_record 兜底，保证每条都落盘。
    match CURSOR_ACCOUNT_INDEX_LOCK.lock() {
        Ok(_lock) => {
            let mut index = load_account_index();
            for account in accounts {
                refresh_summary(&mut index, account);
            }
            if let Err(err) = save_account_index(&index) {
                logger::log_warn(&format!("[Cursor Refresh] 批末回写索引失败: {}", err));
            }
        }
        Err(_) => {
            logger::log_warn(
                "[Cursor Refresh] 批末回写取锁失败，退化为逐个 upsert 兜底，避免整批丢弃",
            );
            for account in accounts {
                if let Err(err) = upsert_account_record(account.clone()) {
                    logger::log_warn(&format!(
                        "[Cursor Refresh] 兜底 upsert 失败: id={}, err={}",
                        account.id, err
                    ));
                }
            }
        }
    }
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

pub(crate) fn is_cursor_auth_quota_error(message: &str) -> bool {
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
        // 0012 §2.3「换号必须配合查询成功」：token 已被服务端拒绝（401 / 会话失效）
        // 的号一律禁止落盘切号，否则会把失效凭据写进 state.vscdb 与 auth.json，
        // 导致 DSH 读到无法使用的账号。
        Err(err) if is_cursor_auth_quota_error(&err) => {
            let email = load_account(account_id)
                .map(|a| a.email)
                .unwrap_or_else(|| "<unknown>".to_string());
            logger::log_warn(&format!(
                "[Cursor Switch] 手动选号被服务端拒绝(会话失效)，中止切号: id={}, email={}, error={}",
                account_id, email, err
            ));
            return Err(format!(
                "该账号会话已失效，无法切号（请换一个可用账号）: {}",
                email
            ));
        }
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
    /// 0012：本次是否**真的**拉到了 usage（成功才为 true）。
    /// 调用方可据此区分「实时查到」与「只读到磁盘旧值」，禁止把未查到的当成满额度。
    pub usage_refreshed: bool,
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
        // 0020：不再对每个 entry 调用 `path.is_file()`（9843 个文件 = 9843 次 stat，
        // 实测这一项独占 4873ms）。改为纯文件名判断：零 stat、零完整路径拼接。
        // 若目录中存在以 .json 结尾的子目录，其后续 `load_account` 读取会失败并返回 None，无副作用。
        let file_name = item.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        let bytes = name.as_bytes();
        if bytes.len() <= 5 || !bytes[bytes.len() - 5..].eq_ignore_ascii_case(b".json") {
            continue;
        }
        let stem = &name[..name.len() - 5];
        let Ok(account_id) = normalize_account_id(stem) else {
            logger::log_warn(&format!(
                "[Cursor Account] 检测到非法账号文件名，已忽略: file={}",
                name
            ));
            continue;
        };
        ids.push(account_id);
    }

    ids.sort();
    ids.dedup();
    ids
}

/// 0019：并发读取账号详情。
///
/// 动机（本机实测）：单条 `load_account()` 约 15ms（其中路径解析约 2.2ms、读文件约 2.5ms、
/// 解密+双层 JSON 约 5.6ms，其余为 exists 等固定开销），串行读 4472 条约 **57 秒**。
/// 本机 20 核，且详情文件之间完全独立（纯读、无共享写），适合分块并发。
///
/// 返回 `(原始下标, 账号)`，调用方据此保留既有顺序与去重语义。
fn load_accounts_concurrently(ids: &[String]) -> Vec<(usize, CursorAccount)> {
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(1, 16);

    if ids.len() < 32 || workers <= 1 {
        return ids
            .iter()
            .enumerate()
            .filter_map(|(idx, id)| load_account(id).map(|account| (idx, account)))
            .collect();
    }

    let chunk_size = ids.len().div_ceil(workers);
    let mut out: Vec<(usize, CursorAccount)> = Vec::with_capacity(ids.len());
    std::thread::scope(|scope| {
        let handles: Vec<_> = ids
            .chunks(chunk_size)
            .enumerate()
            .map(|(chunk_idx, chunk)| {
                let base = chunk_idx * chunk_size;
                scope.spawn(move || {
                    chunk
                        .iter()
                        .enumerate()
                        .filter_map(|(i, id)| load_account(id).map(|account| (base + i, account)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        for handle in handles {
            if let Ok(part) = handle.join() {
                out.extend(part);
            }
        }
    });
    out.sort_by_key(|(idx, _)| *idx);
    out
}

/// 0021：判断索引是否**真的需要**跑全量维护（`normalize_account_index`）。
///
/// 背景：`list_accounts()` 每次调用都会跑一遍完整维护——读出全部详情、扫账号目录、
/// 做邮箱去重合并、重建并写回索引。但维护动作是**幂等**的：当
///
/// 1. 索引内没有重复 id；
/// 2. 索引内没有重复邮箱（去重合并已做过）；
/// 3. 账号目录里没有游离账号文件（不存在"索引缺失需补扫"）；
///
/// 三者同时成立时，跑与不跑维护的结果**完全一致**。本函数用 O(n) 哈希检测给出该判定，
/// 让"打开账号页"这条热路径不再每次都做整表维护。
fn index_needs_normalize(index: &CursorAccountIndex) -> bool {
    let mut seen_ids: HashSet<&str> = HashSet::new();
    let mut seen_emails: HashSet<String> = HashSet::new();
    for summary in &index.accounts {
        if !seen_ids.insert(summary.id.as_str()) {
            return true;
        }
        if let Some(email) = normalize_email_identity(Some(summary.email.as_str())) {
            if !seen_emails.insert(email) {
                return true;
            }
        }
    }
    for account_id in collect_account_ids_from_directory() {
        if !seen_ids.contains(account_id.as_str()) {
            return true;
        }
    }
    false
}

fn normalize_account_index(index: &mut CursorAccountIndex) -> Vec<CursorAccount> {
    let mut seen_summary_ids = HashSet::new();
    let mut candidate_ids: Vec<String> = Vec::new();

    for summary in &index.accounts {
        if seen_summary_ids.insert(summary.id.clone()) {
            candidate_ids.push(summary.id.clone());
        }
    }
    let summary_candidate_count = candidate_ids.len();

    let mut recovered_count = 0usize;
    for account_id in collect_account_ids_from_directory() {
        if seen_summary_ids.contains(&account_id) {
            continue;
        }
        candidate_ids.push(account_id);
    }

    // 0019：候选 id 先定序去重，再并发读取，最后按原顺序回收，保持既有语义不变。
    let mut loaded_accounts = Vec::new();
    let mut seen_account_ids = HashSet::new();
    for (candidate_idx, account) in load_accounts_concurrently(&candidate_ids) {
        if seen_account_ids.insert(account.id.clone()) {
            if candidate_idx >= summary_candidate_count {
                recovered_count += 1;
            }
            loaded_accounts.push(account);
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
    // 0020：原实现是 O(n²) 两两比较（4471² / 2 ≈ 1000 万次，实测约 3.6 秒）。
    // `accounts_are_duplicates` 只比较「规范化邮箱」，因此按邮箱分组、仅在组内两两比较，
    // 判定结果与原实现完全等价，复杂度降为 O(n)。
    let mut email_groups: HashMap<String, Vec<usize>> = HashMap::new();
    for (idx, account) in loaded_accounts.iter().enumerate() {
        if let Some(email) = normalize_email_identity(Some(account.email.as_str())) {
            email_groups.entry(email).or_default().push(idx);
        }
    }
    for group in email_groups.values() {
        if group.len() < 2 {
            continue;
        }
        for i in 0..group.len() {
            for j in (i + 1)..group.len() {
                if accounts_are_duplicates(&loaded_accounts[group[i]], &loaded_accounts[group[j]]) {
                    union(&mut parents, group[i], group[j]);
                }
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

    // 0021 快速路径：索引已一致（无重复 id/邮箱、目录无游离账号）→ 整表维护是幂等的，
    // 直接并发读取即可，不再"每次打开都做整的"。
    if had_index_accounts && !index_needs_normalize(&index) {
        let ids: Vec<String> = index
            .accounts
            .iter()
            .map(|summary| summary.id.clone())
            .collect();
        let loaded: Vec<CursorAccount> = load_accounts_concurrently(&ids)
            .into_iter()
            .map(|(_, account)| account)
            .collect();
        if !loaded.is_empty() {
            return loaded;
        }
        // 详情全部读不到 → 落回原路径，保留既有告警与"不写回空索引"语义
    }

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

/// 前端列表分页：按索引切片读详情并去令牌；不每次全表 normalize，避免四千号一次堵死。
pub fn list_accounts_page_for_ui(offset: usize, limit: usize) -> CursorAccountListPage {
    let limit = limit.clamp(1, 500);
    let _lock = CURSOR_ACCOUNT_INDEX_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut index = load_account_index();
    // 与 list_accounts() 对齐：只要索引需要整表维护（含目录里的游离账号），
    // 先 normalize 再计数，否则页面 total 会小于磁盘真实账号数（表现为「账号变少」）。
    let had_index_accounts = !index.accounts.is_empty();
    if !had_index_accounts || index_needs_normalize(&index) {
        let index_before_normalize = serde_json::to_vec(&index).ok();
        let normalized = normalize_account_index(&mut index);
        if had_index_accounts && normalized.is_empty() {
            // 与 list_accounts() 同语义：详情全读不到时不写回空索引
            logger::log_warn(
                "[Cursor Account] 账号索引中存在账号，但详情文件均无法读取，已跳过空索引写回",
            );
        } else {
            let index_changed = index_before_normalize
                .as_ref()
                .map(|before| Some(before.as_slice()) != serde_json::to_vec(&index).ok().as_deref())
                .unwrap_or(true);
            if index_changed {
                if let Err(err) = save_account_index(&index) {
                    logger::log_warn(&format!(
                        "[Cursor Account] 分页补扫后保存失败: {}",
                        err
                    ));
                }
            }
        }
    }
    let total = index.accounts.len();
    if offset >= total {
        return CursorAccountListPage {
            accounts: Vec::new(),
            total,
            offset,
            next_offset: offset,
            has_more: false,
        };
    }
    let end = (offset + limit).min(total);
    let slice = &index.accounts[offset..end];
    // 0019：本页详情并发读取（本机 20 核；单条约 15ms，200 条串行 0.9~2.5s）。
    let page_ids: Vec<String> = slice
        .iter()
        .map(|summary| summary.id.clone())
        .collect();
    let loaded: HashMap<usize, CursorAccount> = load_accounts_concurrently(&page_ids)
        .into_iter()
        .collect();
    let mut accounts = Vec::with_capacity(slice.len());
    for (page_idx, summary) in slice.iter().enumerate() {
        if let Some(account) = loaded.get(&page_idx) {
            accounts.push(account.clone().for_ui_list());
        } else {
            accounts.push(
                CursorAccount {
                    id: summary.id.clone(),
                    email: summary.email.clone(),
                    auth_id: summary.auth_id.clone(),
                    name: None,
                    tags: summary.tags.clone(),
                    access_token: String::new(),
                    refresh_token: None,
                    membership_type: summary.membership_type.clone(),
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
                    created_at: summary.created_at,
                    last_used: summary.last_used,
                }
                .for_ui_list(),
            );
        }
    }
    CursorAccountListPage {
        accounts,
        total,
        offset,
        next_offset: end,
        has_more: end < total,
    }
}

/// 兼容整表 list：去令牌后返回（仍可能慢；前端应优先分页）。
pub fn list_accounts_for_ui() -> Vec<CursorAccount> {
    list_accounts()
        .into_iter()
        .map(CursorAccount::for_ui_list)
        .collect()
}

pub fn list_accounts_checked() -> Result<Vec<CursorAccount>, String> {
    let _lock = CURSOR_ACCOUNT_INDEX_LOCK
        .lock()
        .map_err(|_| "获取 Cursor 账号锁失败".to_string())?;
    let mut index = load_account_index_checked()?;
    let had_index_accounts = !index.accounts.is_empty();

    // 0021 快速路径：同上，索引一致时跳过整表维护。
    if had_index_accounts && !index_needs_normalize(&index) {
        let ids: Vec<String> = index
            .accounts
            .iter()
            .map(|summary| summary.id.clone())
            .collect();
        let loaded: Vec<CursorAccount> = load_accounts_concurrently(&ids)
            .into_iter()
            .map(|(_, account)| account)
            .collect();
        if !loaded.is_empty() {
            return Ok(loaded);
        }
    }

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
    let _ = sync_cursor_auth_json(&account.access_token, account.refresh_token.as_deref());
    Ok(())
}

/// 同步写入 %APPDATA%\Cursor\auth.json，供 DSH（DeepSeek Harness）及外部工具通过文件监听器感知最新凭据
pub fn sync_cursor_auth_json(access_token: &str, refresh_token: Option<&str>) -> Result<(), String> {
    let cursor_root = cursor_appdata_root()?;
    fs::create_dir_all(&cursor_root).map_err(|e| format!("创建 Cursor AppData 失败: {}", e))?;
    let rt = refresh_token.unwrap_or(access_token);
    let doc = serde_json::json!({
        "accessToken": access_token,
        "refreshToken": rt,
    });
    let content = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    crate::modules::atomic_write::write_string_atomic(
        &cursor_root.join("auth.json"),
        &content,
    )
    .map_err(|e| format!("写入 Cursor auth.json 失败: {}", e))?;
    logger::log_info("[Cursor Switch] 已同步写入 %APPDATA%\\Cursor\\auth.json (供 DSH 监听同步)");
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

/// 续杯/虚备已在 main.js 打补丁时，禁止 Cockpit 重写 storage.json / machineId（保留「重置机器码」）。
fn should_skip_fingerprint_reset_for_third_party_renewal() -> bool {
    crate::modules::cursor_instance::resolve_cursor_launch_path()
        .ok()
        .map(|exe| {
            crate::modules::cursor_switch_align::cursor_main_js_has_third_party_renewal_patch(&exe)
        })
        .unwrap_or(false)
}

/// 无忧传统切号 `i()`：close → Kh → Gh → Jh → Yh → Nc（默认 profile）。
fn nirvana_traditional_switch_steps(account_id: &str, manual_user_pick: bool) -> Result<(), String> {
    let account =
        load_account(account_id).ok_or_else(|| format!("Cursor 账号不存在：{}", account_id))?;
    ensure_cursor_switch_allowed(&account, manual_user_pick)?;
    logger::log_info(&format!("[Cursor Switch] 无忧传统切号：{}", account.email));

    let default_dir = get_default_cursor_data_dir()?;
    // 多开逻辑：不关闭 Cursor，直接切换 token
    switch_tokens_in_profile_db(&default_dir, account_id, manual_user_pick)?;

    if should_skip_fingerprint_reset_for_third_party_renewal() {
        logger::log_info(
            "[Cursor Switch] main.js 含续杯/虚备补丁，跳过 storage.json 与 machineId 文件重置（保留续杯重置机器码）",
        );
    } else {
        reset_storage_json_ids_for_profile(&default_dir)?;
        reset_machine_id_file_for_profile(&default_dir)?;
    }

    if let Ok(cursor_exe) = crate::modules::cursor_instance::resolve_cursor_launch_path() {
        crate::modules::cursor_switch_align::apply_nirvana_traditional_switch_patches_main_js_only(
            &cursor_exe,
        );
    }

    logger::log_info(&format!(
        "[Cursor Switch] 无忧传统路径换号完成（多开模式，不关 Cursor）: email={}, profile={}",
        account.email,
        default_dir.display()
    ));
    Ok(())
}

/// 虚备无感热替换权威落盘路径（注入 JS 轮询 get-token 读此文件）。
fn wuxian_seamless_state_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "无法获取用户主目录".to_string())?;
    Ok(home.join(".wuxian-assistant").join("seamless_state.json"))
}

fn wuxian_auto_switch_pref_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "无法获取用户主目录".to_string())?;
    Ok(home.join(".wuxian-assistant").join("auto_switch_pref.json"))
}

fn cursor_appdata_root() -> Result<PathBuf, String> {
    let appdata = std::env::var("APPDATA").map_err(|_| "APPDATA 未设置".to_string())?;
    Ok(PathBuf::from(appdata).join("Cursor"))
}

/// 热路径落盘用的 auto_switch 有效值。
/// - 用户 pref 为 true：原样跟 pref 写 true
/// - 读到 false（污染/历史硬编码残留）：只升级为 true，禁止回写 false 固化偷关
/// 历史病根：曾硬编码 `auto_switch_pref.enabled=false` 与 `config.auto_switch=false`。
/// `emit_audit=true` 时写 `[偷关防御][换号链路]` 审计；单测可关。
fn hot_path_effective_auto_switch(user_auto_switch: bool, emit_audit: bool) -> bool {
    if user_auto_switch {
        true
    } else {
        if emit_audit {
            crate::modules::logger::log_warn(
                "[偷关防御][换号链路] 读到 auto_switch=false 污染值，按升级为 true 处理，不回写 false",
            );
        }
        true
    }
}

/// 与 `apply_xubei_seamless_hot_path` 双写字段同构：pref.enabled + seamless config.auto_switch。
/// 单测用此构造回归「禁止落盘 false」；生产路径必须与此一致。
fn hot_path_auto_switch_disk_payload(user_auto_switch: bool) -> (serde_json::Value, serde_json::Value) {
    let v = hot_path_effective_auto_switch(user_auto_switch, /*emit_audit*/ false);
    (
        serde_json::json!({ "enabled": v }),
        serde_json::json!({ "auto_switch": v }),
    )
}

/// 对齐续杯管家 `apply_account`：写 seamless_state + wx_*（保留用户四开关设置，不自动关闭）。
fn apply_xubei_seamless_hot_path(
    email: &str,
    access_token: &str,
    refresh_token: &str,
    ids: &HashMap<&'static str, String>,
) -> Result<(), String> {
    let state_path = wuxian_seamless_state_path()?;
    if let Some(parent) = state_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建 wuxian 目录失败: {}", e))?;
    }

    let mut old_ids: Option<serde_json::Map<String, Value>> = None;
    let mut machine_ids_rev: i64 = 1;
    if state_path.exists() {
        if let Ok(raw) = fs::read_to_string(&state_path) {
            if let Ok(Value::Object(old)) = serde_json::from_str::<Value>(&raw) {
                if let Some(Value::Object(m)) = old.get("machineIds").cloned() {
                    old_ids = Some(m);
                }
                if let Some(v) = old.get("machineIdsRev").and_then(|x| x.as_i64()) {
                    machine_ids_rev = v + 1;
                }
            }
        }
    }

    let machine_ids = serde_json::json!({
        "machineId": ids["telemetry.machineId"],
        "macMachineId": ids["telemetry.macMachineId"],
        "devDeviceId": ids["telemetry.devDeviceId"],
        "sqmId": ids["telemetry.sqmId"],
    });

    let mut mappings: Vec<Value> = Vec::new();
    if let Some(ref old) = old_ids {
        for key in ["machineId", "macMachineId"] {
            let o = old.get(key).and_then(|v| v.as_str());
            let n = machine_ids.get(key).and_then(|v| v.as_str());
            if let (Some(o), Some(n)) = (o, n) {
                if o.len() == n.len() && o != n {
                    mappings.push(serde_json::json!({ "old": o, "new": n }));
                }
            }
        }
    }

    // 读取用户当前四开关偏好；auto_switch 经升级后再落盘（禁写 false）
    let user_prefs = crate::modules::xubei_renewal_prefs::read_xubei_renewal_prefs()
        .unwrap_or(crate::modules::xubei_renewal_prefs::XubeiRenewalPrefs {
            seamless_enabled: true,
            auto_switch: true,
            auto_reset_machine: true,
            auto_send_continue: true,
        });
    // 一律跟用户 pref；读到 false 只升级。历史病根：硬编码 enabled/auto_switch=false。
    let effective_auto_switch =
        hot_path_effective_auto_switch(user_prefs.auto_switch, /*emit_audit*/ true);
    let pref_path = wuxian_auto_switch_pref_path()?;
    let pref_body = serde_json::json!({ "enabled": effective_auto_switch });
    crate::modules::atomic_write::write_string_atomic(
        &pref_path,
        &serde_json::to_string_pretty(&pref_body).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("写入 auto_switch_pref 失败: {}", e))?;

    // 对齐原版 apply 后：偏好开启则落 pending_resume，供注入 AutoResume 轮询 /api/pending-resume
    // config.auto_switch 必须与 auto_switch_pref 同写 effective_auto_switch，
    // 禁止 seamless_state 残留 false 而 pref 已升 true（双写不一致会让 HTTP/FO 侧读到假关）。
    let mut st = serde_json::json!({
        "config": { "enabled": user_prefs.seamless_enabled, "auto_switch": effective_auto_switch },
        "accessToken": access_token,
        "refreshToken": refresh_token,
        "email": email,
        "is_new": !mappings.is_empty(),
        "machineIds": machine_ids,
        "machineIdsRev": machine_ids_rev,
        "mappings": mappings,
        "reset_ok": true,
        "updated_at": chrono::Utc::now().timestamp_millis() as f64 / 1000.0,
        "source": "cockpit-tools",
    });
    if user_prefs.auto_send_continue {
        if let Some(obj) = st.as_object_mut() {
            obj.insert("pending_resume".into(), serde_json::json!(true));
            obj.insert("resume_text".into(), serde_json::json!("继续"));
        }
    }
    crate::modules::atomic_write::write_string_atomic(
        &state_path,
        &serde_json::to_string_pretty(&st).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("写入 seamless_state 失败: {}", e))?;

    let cursor_root = cursor_appdata_root()?;
    fs::create_dir_all(&cursor_root).map_err(|e| format!("创建 Cursor AppData 失败: {}", e))?;
    crate::modules::atomic_write::write_string_atomic(
        &cursor_root.join("wx_token.txt"),
        access_token,
    )
    .map_err(|e| format!("写入 wx_token 失败: {}", e))?;
    let mid_doc = serde_json::json!({
        "machineId": ids["telemetry.machineId"],
        "macMachineId": ids["telemetry.macMachineId"],
    });
    crate::modules::atomic_write::write_string_atomic(
        &cursor_root.join("wx_mid.json"),
        &serde_json::to_string(&mid_doc).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("写入 wx_mid 失败: {}", e))?;
    let eh_doc = serde_json::json!({ "mappings": mappings });
    crate::modules::atomic_write::write_string_atomic(
        &cursor_root.join("wx_eh_map.json"),
        &serde_json::to_string(&eh_doc).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("写入 wx_eh_map 失败: {}", e))?;

    crate::modules::atomic_write::write_string_atomic(
        &cursor_root.join("machineId"),
        &ids["telemetry.machineId"],
    )
    .map_err(|e| format!("写入 machineId 失败: {}", e))?;
    crate::modules::atomic_write::write_string_atomic(
        &cursor_root.join("machineid.json"),
        &serde_json::to_string_pretty(&mid_doc).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("写入 machineid.json 失败: {}", e))?;
    let gs = cursor_root.join("User").join("globalStorage");
    fs::create_dir_all(&gs).map_err(|e| format!("创建 globalStorage 失败: {}", e))?;
    crate::modules::atomic_write::write_string_atomic(
        &gs.join("machine-id"),
        &ids["telemetry.machineId"],
    )
    .map_err(|e| format!("写入 machine-id 失败: {}", e))?;

    let _ = sync_cursor_auth_json(access_token, Some(refresh_token));

    logger::log_info(&format!(
        "[Cursor Switch] 续杯无感热替换落盘完成: email={}, state={}, seamless={}, auto_switch={}, auto_reset={}, auto_continue={}",
        email,
        state_path.display(),
        user_prefs.seamless_enabled,
        effective_auto_switch,
        user_prefs.auto_reset_machine,
        user_prefs.auto_send_continue,
    ));
    Ok(())
}

fn reassert_xubei_hot_path_until_stuck(
    email: &str,
    access_token: &str,
    refresh_token: &str,
    ids: &HashMap<&'static str, String>,
) -> Result<(), String> {
    // WP-4 / 模块三：事件/漂移驱动。禁止固定 20×500ms 盲写 + 24×500ms 盲守（≈22s）。
    // 一致即返回；仅 get-token 与期望 email 不一致时才重写。通常 <2s，异常才拉长。
    let expected = email.trim().to_lowercase();
    const MAX_ROUNDS: u32 = 12;
    const SLEEP_MS: u64 = 150;

    apply_xubei_seamless_hot_path(email, access_token, refresh_token, ids)?;

    for round in 1..=MAX_ROUNDS {
        match read_wuxian_get_token_email() {
            Ok(Some(got)) if got.trim().to_lowercase() == expected => {
                logger::log_info(&format!(
                    "[Cursor Switch] 续杯无感粘号确认（漂移驱动）round={}, email={}（一致，立即返回）",
                    round, email
                ));
                return Ok(());
            }
            Ok(Some(got)) => {
                logger::log_warn(&format!(
                    "[Cursor Switch] 续杯无感仍被盖回 round={}/{}, get-token={}, 期望={}，重写",
                    round, MAX_ROUNDS, got, email
                ));
                apply_xubei_seamless_hot_path(email, access_token, refresh_token, ids)?;
            }
            Ok(None) => {
                logger::log_warn(&format!(
                    "[Cursor Switch] 续杯无感 get-token 空 round={}/{}, 期望={}，重写",
                    round, MAX_ROUNDS, email
                ));
                apply_xubei_seamless_hot_path(email, access_token, refresh_token, ids)?;
            }
            Err(err) => {
                logger::log_warn(&format!(
                    "[Cursor Switch] 续杯无感 get-token 读失败 round={}/{}: {}，重写",
                    round, MAX_ROUNDS, err
                ));
                apply_xubei_seamless_hot_path(email, access_token, refresh_token, ids)?;
            }
        }
        std::thread::sleep(Duration::from_millis(SLEEP_MS));
    }

    match read_wuxian_get_token_email() {
        Ok(Some(got)) if got.trim().to_lowercase() == expected => Ok(()),
        Ok(Some(got)) => Err(format!(
            "续杯无感未粘住: 期望={} get-token={}",
            email, got
        )),
        Ok(None) => Err(format!("续杯无感未粘住: get-token 空, 期望={}", email)),
        Err(err) => Err(format!("续杯无感未粘住: 读失败: {}", err)),
    }
}

fn spawn_xubei_hot_path_keeper(
    email: String,
    access_token: String,
    refresh_token: String,
    ids: HashMap<&'static str, String>,
) {
    let _ = std::thread::Builder::new()
        .name("xubei-hot-path-keeper".into())
        .spawn(move || {
            let expected = email.trim().to_lowercase();
            // 仅漂移才写；轮询从 1s×45 降噪为 2s×24（总监视窗≈48s，唤醒次数减半）
            for round in 1..=24u32 {
                std::thread::sleep(Duration::from_secs(2));
                let need = match read_wuxian_get_token_email() {
                    Ok(Some(got)) => got.trim().to_lowercase() != expected,
                    Ok(None) => true,
                    Err(_) => true,
                };
                if need {
                    if let Err(err) =
                        apply_xubei_seamless_hot_path(&email, &access_token, &refresh_token, &ids)
                    {
                        logger::log_warn(&format!(
                            "[Cursor Switch] 续杯无感续盖失败 round={}: {}",
                            round, err
                        ));
                    } else {
                        logger::log_info(&format!(
                            "[Cursor Switch] 续杯无感续盖 round={}, email={}",
                            round, email
                        ));
                    }
                }
            }
        });
}

const XUBEI_SEAMLESS_SWITCH_TAG: &str = "续杯无感换号";

/// 续杯管家式无感换号：写 wuxian 热替换通道 + 默认 profile 库（不关窗）。
pub fn xubei_seamless_switch_account(
    account_id: &str,
    manual_user_pick: bool,
) -> Result<(), String> {
    // 对齐原版 on_seamless：先起本地无感服务，然后直接 apply；不先卡 get-token
    match crate::modules::wuxian_seamless_server::start(None) {
        Ok(port) => logger::log_info(&format!(
            "[Cursor Switch] wuxian 就绪 port={} adopted={}",
            port,
            crate::modules::wuxian_seamless_server::is_adopted_external()
        )),
        Err(e) => logger::log_warn(&format!(
            "[Cursor Switch] wuxian 启动失败（仍继续写盘，对齐原版 apply_account）: {e}"
        )),
    }

    let account =
        load_account(account_id).ok_or_else(|| format!("Cursor 账号不存在: {}", account_id))?;
    ensure_cursor_switch_allowed(&account, manual_user_pick)?;
    let (access_token, refresh_token) = nirvana_kh_auth_tokens(&account)?;
    let default_dir = get_default_cursor_data_dir()?;
    let ids = build_cursor_fingerprint_ids();
    logger::log_info(&format!(
        "[Cursor Switch] 续杯无感换号(热替换+写库): email={}, profile={}",
        account.email,
        default_dir.display()
    ));

    apply_xubei_seamless_hot_path(
        &account.email,
        &access_token,
        &refresh_token,
        &ids,
    )?;
    switch_tokens_in_profile_db_live(&default_dir, account_id, manual_user_pick)?;
    let storage_json = default_dir
        .join("User")
        .join("globalStorage")
        .join("storage.json");
    if should_skip_fingerprint_reset_for_third_party_renewal() {
        logger::log_info(
            "[Cursor Switch] main.js 含续杯补丁，续杯无感路径跳过 storage.json 指纹写入",
        );
    } else {
        upsert_storage_json_ids(&storage_json, &ids)?;
    }
    reassert_xubei_hot_path_until_stuck(
        &account.email,
        &access_token,
        &refresh_token,
        &ids,
    )?;
    spawn_xubei_hot_path_keeper(
        account.email.clone(),
        access_token,
        refresh_token,
        ids.clone(),
    );

    let mut tags = account.tags.clone().unwrap_or_default();
    if !tags.iter().any(|t| t == XUBEI_SEAMLESS_SWITCH_TAG) {
        tags.push(XUBEI_SEAMLESS_SWITCH_TAG.to_string());
        let _ = update_account_tags(&account.id, tags);
    }

    logger::log_info(&format!(
        "[Cursor Switch] 续杯无感换号完成: email={}",
        account.email
    ));
    crate::modules::renewal_console_status::invalidate_renewal_console_status_cache();
    Ok(())
}

/// Cockpit 内置 wuxian 服务 auto-switch 回调：对齐原版 —— 云端 /api/v1/switch + apply，
/// 不再只从空池轮换（关管家后池空会直接「拉不到号」）。
pub fn xubei_seamless_switch_from_wuxian_state() -> Result<String, String> {
    logger::log_info("[WuxianServer] auto-switch：走云端拉号+无感写入（对齐管家 do_switch）");
    let account = crate::modules::xubei_switch_client::pull_and_xubei_seamless_switch()?;
    logger::log_info(&format!(
        "[WuxianServer] auto-switch 完成: email={}",
        account.email
    ));
    Ok(account.email)
}

fn wuxian_candidate_ports() -> Vec<u16> {
    const DEFAULT: &[u16] = &[14520, 14521, 14522, 14523, 14524, 35420, 35421, 35422, 35423, 35424];
    let mut ports = Vec::with_capacity(DEFAULT.len() + 1);
    let active = crate::modules::wuxian_seamless_server::active_port();
    if active != 0 {
        ports.push(active);
    }
    for &p in DEFAULT {
        if !ports.contains(&p) {
            ports.push(p);
        }
    }
    ports
}

fn tcp_port_open(port: u16, timeout_ms: u64) -> bool {
    use std::net::{SocketAddr, TcpStream};
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    TcpStream::connect_timeout(&addr, Duration::from_millis(timeout_ms)).is_ok()
}

/// HTTP get-token 探测：任一端口成功响应即视为可达（邮箱可空 → `Ok(None)`）；
/// 全部端口无成功响应 → `Err`（不可达）。不含 seamless_state 文件回退。
pub fn probe_wuxian_get_token_http() -> Result<Option<String>, String> {
    let ports = wuxian_candidate_ports();
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_millis(250))
        .timeout(Duration::from_millis(800))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;
    let mut saw_http_ok = false;
    for port in ports {
        if !tcp_port_open(port, 120) {
            continue;
        }
        let url = format!("http://127.0.0.1:{}/api/get-token", port);
        match client.get(&url).send() {
            Ok(resp) if resp.status().is_success() => {
                saw_http_ok = true;
                let body = resp.text().unwrap_or_default();
                if let Ok(v) = serde_json::from_str::<Value>(&body) {
                    if let Some(email) = v.get("email").and_then(|x| x.as_str()) {
                        let email = email.trim();
                        if !email.is_empty() {
                            return Ok(Some(email.to_string()));
                        }
                    }
                }
            }
            _ => continue,
        }
    }
    if saw_http_ok {
        Ok(None)
    } else {
        Err("get-token 服务不可达".to_string())
    }
}

/// 只读：续杯/虚备 get-token 当前邮箱（对照用）。禁止写入对方通道。
/// 优先 HTTP；HTTP 不可达时回退读 seamless_state。
pub fn read_wuxian_get_token_email() -> Result<Option<String>, String> {
    match probe_wuxian_get_token_http() {
        Ok(v) => return Ok(v),
        Err(_) => {}
    }
    let state_path = wuxian_seamless_state_path()?;
    if !state_path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&state_path)
        .map_err(|e| format!("读 seamless_state 失败: {}", e))?;
    let v: Value =
        serde_json::from_str(&raw).map_err(|e| format!("解析 seamless_state 失败: {}", e))?;
    Ok(v.get("email")
        .and_then(|x| x.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty()))
}

/// 手动发送继续：对齐原版 —— 写 `seamless_state.pending_resume` + `resume_text=继续`。
/// 原版不扫 HTTP `/api/resume-continue`（管家也无此路由）。
pub fn manual_send_continue_to_xubei() -> Result<String, String> {
    let _ = crate::modules::wuxian_seamless_server::start(None);

    let state_path = wuxian_seamless_state_path()?;
    let mut st = if state_path.exists() {
        let raw = fs::read_to_string(&state_path)
            .map_err(|e| format!("读 seamless_state 失败: {e}"))?;
        serde_json::from_str::<Value>(&raw).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };
    let email = st
        .get("email")
        .and_then(|x| x.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());

    if let Some(obj) = st.as_object_mut() {
        obj.insert("pending_resume".into(), serde_json::json!(true));
        obj.insert("resume_text".into(), serde_json::json!("继续"));
    } else {
        st = serde_json::json!({
            "pending_resume": true,
            "resume_text": "继续",
            "email": email,
        });
    }

    if let Some(parent) = state_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建 wuxian 目录失败: {e}"))?;
    }
    crate::modules::atomic_write::write_string_atomic(
        &state_path,
        &serde_json::to_string_pretty(&st).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("写入 seamless_state pending_resume 失败: {e}"))?;

    let msg = format!("已标记发送继续: {email}");
    logger::log_info(&format!("[Xubei Continue] {msg}"));
    Ok(msg)
}

/// 重置本机 Cursor 机器码：对齐续杯管家「重置机器码」按钮。
pub fn reset_cursor_machine_id_live() -> Result<String, String> {
    hard_reset_cursor_fingerprint_state()?;
    let msg = "机器码已重置 (storage.json / machineId / state.vscdb)";
    logger::log_info(&format!("[Xubei Reset] {}", msg));
    Ok(msg.to_string())
}

/// 还原续杯注入：从 .bak 备份恢复被注入的 Cursor 文件。
/// 对齐原版 `cursor_injector.restore()` — workbench / exthost / main / util 四个文件。
pub fn restore_cursor_injection() -> Result<String, String> {
    use crate::modules::cursor_instance;

    let exe = cursor_instance::resolve_cursor_launch_path()
        .map_err(|_| "未找到 Cursor 安装路径")?;
    let install_root = exe
        .parent()
        .ok_or_else(|| "无法解析 Cursor 安装根目录")?;
    let app_root = install_root.join("resources").join("app");

    // 与原版 resolve_paths() 一致的四个注入文件
    let targets: Vec<(&str, std::path::PathBuf)> = vec![
        (
            "workbench",
            app_root.join("out/vs/workbench/workbench.desktop.main.js"),
        ),
        (
            "exthost",
            app_root.join("out/vs/workbench/api/node/extensionHostProcess.js"),
        ),
        ("main", app_root.join("out/main.js")),
    ];

    let mut steps: Vec<String> = Vec::new();
    let mut any_work = false;

    for (label, path) in &targets {
        if !path.exists() {
            steps.push(format!("{label}: 文件不存在，跳过"));
            continue;
        }
        let bak = path.with_extension(format!(
            "{}.bak",
            path.extension().unwrap_or_default().to_str().unwrap_or("js")
        ));
        // 修正：bak 路径是 <file>.bak（如 workbench.desktop.main.js.bak）
        let bak = std::path::PathBuf::from(format!("{}.bak", path.display()));
        if bak.exists() {
            std::fs::copy(&bak, path).map_err(|e| {
                format!("{label}: 从 .bak 还原失败: {e}")
            })?;
            steps.push(format!("{label}: 已从 .bak 还原"));
            any_work = true;
        } else {
            steps.push(format!("{label}: 无 .bak 备份，跳过"));
        }
    }

    // util: alwaysLocalSingletonMain.js（glob 搜索）
    if let Ok(entries) = glob_simple(&app_root, "alwaysLocalSingletonMain.js") {
        for util_path in entries {
            let bak = std::path::PathBuf::from(format!("{}.bak", util_path.display()));
            if bak.exists() {
                if let Err(e) = std::fs::copy(&bak, &util_path) {
                    steps.push(format!("util: 从 .bak 还原失败: {e}"));
                } else {
                    steps.push("util: 已从 .bak 还原".into());
                    any_work = true;
                }
            } else {
                steps.push("util: 无 .bak 备份，跳过".into());
            }
        }
    }

    let summary = if any_work {
        "还原完成（已从备份恢复注入文件）"
    } else {
        "当前未注入，无需还原"
    };
    logger::log_info(&format!("[Xubei Restore] {summary}: {:?}", steps));
    Ok(format!("{summary}\n{}", steps.join("\n")))
}

/// 简易 glob：在 dir 下递归搜索匹配文件名的文件。
fn glob_simple(dir: &std::path::Path, filename: &str) -> Result<Vec<std::path::PathBuf>, ()> {
    let mut results = Vec::new();
    fn walk(dir: &std::path::Path, filename: &str, out: &mut Vec<std::path::PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, filename, out);
                } else if path.file_name().map(|n| n == filename).unwrap_or(false) {
                    out.push(path);
                }
            }
        }
    }
    walk(dir, filename, &mut results);
    Ok(results)
}

/// 默认 profile 无感：写本机默认 Cursor 库与 storage，不关窗。
/// 默认 Play（`manual_user_pick=false`）：不抢续杯热通道。
/// 显式切号（`manual_user_pick=true`）：写库成功后若 get-token HTTP 可达，再对齐热通道。
/// 无忧传统路径仍由 `nirvana_traditional_switch_steps` 保留，勿删。
fn default_seamless_switch_steps(account_id: &str, manual_user_pick: bool) -> Result<(), String> {
    let account =
        load_account(account_id).ok_or_else(|| format!("Cursor 账号不存在: {}", account_id))?;
    ensure_cursor_switch_allowed(&account, manual_user_pick)?;
    let default_dir = get_default_cursor_data_dir()?;
    let ids = build_cursor_fingerprint_ids();
    logger::log_info(&format!(
        "[Cursor Switch] 默认无感(只写本库{}): email={}, profile={}",
        if manual_user_pick {
            "；显式切号可后续对齐热通道"
        } else {
            "，不碰续杯通道"
        },
        account.email,
        default_dir.display()
    ));
    // Cursor 仍占用 state.vscdb 时不得切 journal / 删 -wal。
    switch_tokens_in_profile_db_live(&default_dir, account_id, manual_user_pick)?;
    let storage_json = default_dir
        .join("User")
        .join("globalStorage")
        .join("storage.json");
    if should_skip_fingerprint_reset_for_third_party_renewal() {
        logger::log_info(
            "[Cursor Switch] main.js 含续杯/虚备补丁，默认无感路径跳过 storage.json 指纹写入（保留续杯重置机器码）",
        );
    } else {
        upsert_storage_json_ids(&storage_json, &ids)?;
    }

    // 显式切号 + 默认 profile：写库后若管家 get-token HTTP 可达，对齐热通道，避免 stick 仍读到旧号。
    // 默认 Play 不走此分支，避免抢热通道。
    if manual_user_pick {
        match probe_wuxian_get_token_http() {
            Ok(_) => {
                let (access_token, refresh_token) = nirvana_kh_auth_tokens(&account)?;
                apply_xubei_seamless_hot_path(
                    &account.email,
                    &access_token,
                    &refresh_token,
                    &ids,
                )?;
                logger::log_info(&format!(
                    "[Cursor Switch] 显式切号已对齐续杯热通道: email={}",
                    account.email
                ));
            }
            Err(err) => {
                logger::log_info(&format!(
                    "[Cursor Switch] 显式切号 get-token 不可达，仅写默认库: {}",
                    err
                ));
            }
        }
    }

    logger::log_info(&format!(
        "[Cursor Switch] 默认无感完成: email={}",
        account.email
    ));
    Ok(())
}

/// 读默认 profile 当前 `cursorAuth/cachedEmail`（粘号复查用）。
pub fn read_default_cached_email() -> Result<Option<String>, String> {
    let db_path = get_default_cursor_state_db_path()?;
    if !db_path.exists() {
        return Ok(None);
    }
    let conn = Connection::open(&db_path)
        .map_err(|e| format!("打开 Cursor state.vscdb 失败({}): {}", db_path.display(), e))?;
    Ok(read_vscdb_item(&conn, "cursorAuth/cachedEmail").filter(|e| !e.trim().is_empty()))
}

/// 多开实例目录内的热换落点（禁止写全局家目录，避免串默认窗）。
fn multi_profile_seamless_dir(profile_dir: &Path) -> PathBuf {
    profile_dir.join("cockpit-seamless")
}

/// 对齐续杯投递：在本 profile 写 active_token / state，供后续注入轮询；不写 `~/.cursor-renewal`。
fn apply_multi_profile_seamless_files(
    profile_dir: &Path,
    email: &str,
    access_token: &str,
    refresh_token: &str,
    ids: &HashMap<&'static str, String>,
) -> Result<(), String> {
    let dir = multi_profile_seamless_dir(profile_dir);
    fs::create_dir_all(&dir).map_err(|e| format!("创建多开热换目录失败: {}", e))?;

    let machine_ids = serde_json::json!({
        "machineId": ids["telemetry.machineId"],
        "macMachineId": ids["telemetry.macMachineId"],
        "devDeviceId": ids["telemetry.devDeviceId"],
        "sqmId": ids["telemetry.sqmId"],
    });
    let st = serde_json::json!({
        "accessToken": access_token,
        "refreshToken": refresh_token,
        "email": email,
        "machineIds": machine_ids,
        "updated_at": chrono::Utc::now().timestamp_millis() as f64 / 1000.0,
        "source": "cockpit-tools-multi",
    });
    crate::modules::atomic_write::write_string_atomic(
        &dir.join("state.json"),
        &serde_json::to_string_pretty(&st).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("写入多开 state.json 失败: {}", e))?;
    crate::modules::atomic_write::write_string_atomic(&dir.join("active_token"), access_token)
        .map_err(|e| format!("写入多开 active_token 失败: {}", e))?;
    crate::modules::atomic_write::write_string_atomic(
        &dir.join("machine_id_override.json"),
        &serde_json::to_string(&serde_json::json!({ "mappings": [] })).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("写入多开 machine_id_override 失败: {}", e))?;
    Ok(())
}

fn build_multi_cdp_auth_push_js(
    email: &str,
    access_token: &str,
    refresh_token: &str,
    ids: &HashMap<&'static str, String>,
) -> Result<String, String> {
    let auth = serde_json::json!({
        "accessToken": access_token,
        "refreshToken": refresh_token,
        "email": email,
        "machineIds": {
            "machineId": ids["telemetry.machineId"],
            "macMachineId": ids["telemetry.macMachineId"],
            "devDeviceId": ids["telemetry.devDeviceId"],
            "sqmId": ids["telemetry.sqmId"],
        },
    });
    let auth_lit = serde_json::to_string(&auth).map_err(|e| e.to_string())?;
    Ok(format!(
        r#"(function(){{
  var auth={auth};
  window.__cockpitSeamlessAuth=auth;
  var okStore=false, errStore=null;
  try{{
    if(window.__cockpitFooterSyncTimer){{
      try{{ clearInterval(window.__cockpitFooterSyncTimer); }}catch(_ct){{}}
      window.__cockpitFooterSyncTimer=null;
      window.__cockpitFooterSync=false;
    }}
  }}catch(_cl){{}}
  try{{
    if(window.store&&typeof window.store.set==='function'){{
      window.store.set('cursorAuth/accessToken', auth.accessToken, -1);
      window.store.set('cursorAuth/refreshToken', auth.refreshToken, -1);
      window.store.set('cursorAuth/cachedEmail', auth.email, -1);
      if(auth.machineIds){{
        window.store.set('telemetry.devDeviceId', auth.machineIds.devDeviceId, -1);
        window.store.set('telemetry.machineId', auth.machineIds.machineId, -1);
        window.store.set('telemetry.macMachineId', auth.machineIds.macMachineId, -1);
        window.store.set('telemetry.sqmId', auth.machineIds.sqmId, -1);
      }}
      okStore=true;
    }}
  }}catch(e){{errStore=String(e);}}
  var gotEmail=(window.store&&window.store.get)?window.store.get('cursorAuth/cachedEmail',-1):null;
  return {{ok:true, okStore:okStore, errStore:errStore, email:gotEmail||auth.email, hasSeamless:!!window.__cockpitSeamlessAuth}};
}})()"#,
        auth = auth_lit
    ))
}

fn build_multi_cdp_auth_verify_js(expected_email: &str) -> String {
    let want = expected_email.trim().to_lowercase();
    format!(
        r#"(function(){{
  var want={want_lit};
  var got=((window.store&&window.store.get('cursorAuth/cachedEmail',-1))||'').toLowerCase();
  var seam=(window.__cockpitSeamlessAuth&&window.__cockpitSeamlessAuth.email||'').toLowerCase();
  return {{ok:got===want||seam===want, email:got||seam, want:want}};
}})()"#,
        want_lit = serde_json::to_string(&want).unwrap_or_else(|_| "\"\"".to_string())
    )
}

/// 经 CDP 把认证态推进运行中多开窗的 `window.store` + `__cockpitSeamlessAuth`。
fn push_multi_auth_via_cdp(
    profile_dir: &Path,
    email: &str,
    access_token: &str,
    refresh_token: &str,
    ids: &HashMap<&'static str, String>,
) -> Result<Value, String> {
    let dir_s = profile_dir.to_string_lossy().to_string();
    let port = crate::modules::cursor_instance::resolve_cdp_port_for_user_data_dir(&dir_s)
        .ok_or_else(|| format!("多开无感：未找到 CDP 端口 profile={}", dir_s))?;
    let js = build_multi_cdp_auth_push_js(email, access_token, refresh_token, ids)?;
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("创建 CDP 运行时失败: {}", e))?;
    let result = rt.block_on(async {
        crate::modules::cursor_cdp_control::execute_javascript(port, &js).await
    })?;
    logger::log_info(&format!(
        "[Cursor Switch] 多开无感 CDP 热推: port={}, email={}, result={}",
        port, email, result
    ));
    Ok(result)
}

fn spawn_multi_cdp_auth_keeper(profile_dir: PathBuf, email: String) {
    let _ = std::thread::Builder::new()
        .name("multi-cdp-auth-keeper".into())
        .spawn(move || {
            let expected = email.trim().to_lowercase();
            let dir_s = profile_dir.to_string_lossy().to_string();
            for round in 1..=5u32 {
                std::thread::sleep(Duration::from_secs(2));
                let Some(port) =
                    crate::modules::cursor_instance::resolve_cdp_port_for_user_data_dir(&dir_s)
                else {
                    logger::log_warn(&format!(
                        "[Cursor Switch] 多开 CDP 校验跳过 round={}: 无 CDP 端口",
                        round
                    ));
                    continue;
                };
                let js = build_multi_cdp_auth_verify_js(&email);
                let rt = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => {
                        logger::log_warn(&format!(
                            "[Cursor Switch] 多开 CDP 校验运行时失败 round={}: {}",
                            round, e
                        ));
                        continue;
                    }
                };
                match rt.block_on(async {
                    crate::modules::cursor_cdp_control::execute_javascript(port, &js).await
                }) {
                    Ok(v) => {
                        let got = v
                            .get("value")
                            .and_then(|x| x.get("email"))
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .trim()
                            .to_lowercase();
                        let ok = v
                            .get("value")
                            .and_then(|x| x.get("ok"))
                            .and_then(|x| x.as_bool())
                            .unwrap_or(false);
                        if ok && got == expected {
                            logger::log_info(&format!(
                                "[Cursor Switch] 多开 CDP 热态已对齐 round={}, email={}",
                                round, email
                            ));
                            break;
                        }
                        logger::log_warn(&format!(
                            "[Cursor Switch] 多开 CDP 热态未对齐 round={}, want={}, got={}, raw={}",
                            round, email, got, v
                        ));
                    }
                    Err(err) => {
                        logger::log_warn(&format!(
                            "[Cursor Switch] 多开 CDP 校验失败 round={}: {}",
                            round, err
                        ));
                    }
                }
            }
        });
}

/// 账号总览 Play 与多开实例启动的切号落盘。`manual_user_pick=true` 时为用户点选，不拦配额/失败标记。
pub fn switch_cursor_account_to_profile(
    account_id: &str,
    profile_dir: &Path,
    manual_user_pick: bool,
) -> Result<(), String> {
    if crate::modules::cursor_instance::is_default_cursor_profile_dir(profile_dir) {
        return default_seamless_switch_steps(account_id, manual_user_pick);
    }

    let account =
        load_account(account_id).ok_or_else(|| format!("Cursor 账号不存在: {}", account_id))?;
    ensure_cursor_switch_allowed(&account, manual_user_pick)?;
    let (access_token, refresh_token) = nirvana_kh_auth_tokens(&account)?;
    // 多开无感：对齐续杯「不关窗 + 热写认证态」语义；禁止关多开窗冷写。
    // 不得写全局 ~/.cursor-renewal 或 wx_*（会串默认窗）；只写本 profile + CDP 推进运行窗。
    crate::modules::cursor_instance::ensure_state_db_for_injection(profile_dir)?;
    let ids = build_cursor_fingerprint_ids();
    logger::log_info(&format!(
        "[Cursor Switch] 多开无感(热写库+实例目录+CDP): email={}, profile={}",
        account.email,
        profile_dir.display()
    ));
    apply_multi_profile_seamless_files(
        profile_dir,
        &account.email,
        &access_token,
        &refresh_token,
        &ids,
    )?;
    switch_tokens_in_profile_db_live(profile_dir, account_id, manual_user_pick)?;
    let storage_json = profile_dir
        .join("User")
        .join("globalStorage")
        .join("storage.json");
    upsert_storage_json_ids(&storage_json, &ids)?;
    match push_multi_auth_via_cdp(
        profile_dir,
        &account.email,
        &access_token,
        &refresh_token,
        &ids,
    ) {
        Ok(_) => {}
        Err(err) => {
            // 窗未开时只落盘；开窗后启动链会再切并可 CDP。此处不硬失败整条切号。
            logger::log_warn(&format!(
                "[Cursor Switch] 多开无感首次 CDP 热推未成（库与实例目录已写）: {}",
                err
            ));
        }
    }
    spawn_multi_cdp_auth_keeper(profile_dir.to_path_buf(), account.email.clone());
    logger::log_info(&format!(
        "[Cursor Switch] 多开无感完成: email={}, profile={}",
        account.email,
        profile_dir.display()
    ));
    Ok(())
}

/// 冷启动后 CDP 才就绪：从实例热换目录读 state，延迟重推 window.store / 侧栏邮箱。
pub fn spawn_multi_cdp_auth_resync_after_launch(profile_dir: PathBuf) {
    let _ = std::thread::Builder::new()
        .name("multi-cdp-auth-resync".into())
        .spawn(move || {
            let dir_s = profile_dir.to_string_lossy().to_string();
            let state_path = multi_profile_seamless_dir(&profile_dir).join("state.json");
            for round in 1..=12u32 {
                std::thread::sleep(Duration::from_secs(2));
                let Some(port) =
                    crate::modules::cursor_instance::resolve_cdp_port_for_user_data_dir(&dir_s)
                else {
                    logger::log_warn(&format!(
                        "[Cursor Switch] 多开 CDP 重推跳过 round={}: 无 CDP 端口",
                        round
                    ));
                    continue;
                };
                let Ok(raw) = fs::read_to_string(&state_path) else {
                    logger::log_warn(&format!(
                        "[Cursor Switch] 多开 CDP 重推跳过 round={}: 无 state.json",
                        round
                    ));
                    continue;
                };
                let Ok(st) = serde_json::from_str::<Value>(&raw) else {
                    continue;
                };
                let email = st
                    .get("email")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim();
                let access = st
                    .get("accessToken")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim();
                let refresh = st
                    .get("refreshToken")
                    .and_then(|v| v.as_str())
                    .unwrap_or(access)
                    .trim();
                if email.is_empty() || access.is_empty() {
                    continue;
                }
                let mut ids: HashMap<&'static str, String> = HashMap::new();
                if let Some(m) = st.get("machineIds").and_then(|v| v.as_object()) {
                    for (k, field) in [
                        ("machineId", "telemetry.machineId"),
                        ("macMachineId", "telemetry.macMachineId"),
                        ("devDeviceId", "telemetry.devDeviceId"),
                        ("sqmId", "telemetry.sqmId"),
                    ] {
                        if let Some(v) = m.get(k).and_then(|x| x.as_str()) {
                            ids.insert(field, v.to_string());
                        }
                    }
                }
                if ids.len() < 4 {
                    ids = build_cursor_fingerprint_ids();
                }
                match push_multi_auth_via_cdp(&profile_dir, email, access, refresh, &ids) {
                    Ok(v) => {
                        let got = v
                            .get("value")
                            .and_then(|x| x.get("email"))
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .trim()
                            .to_lowercase();
                        if got == email.to_lowercase() {
                            logger::log_info(&format!(
                                "[Cursor Switch] 多开 CDP 启动后重推已对齐 round={}, email={}",
                                round, email
                            ));
                            break;
                        }
                        logger::log_warn(&format!(
                            "[Cursor Switch] 多开 CDP 启动后重推未对齐 round={}, want={}, got={}",
                            round, email, got
                        ));
                    }
                    Err(err) => logger::log_warn(&format!(
                        "[Cursor Switch] 多开 CDP 启动后重推失败 round={}: {}",
                        round, err
                    )),
                }
            }
        });
}

/// 显式走无忧传统关窗切号（保留；默认入口已改无感）。
#[allow(dead_code)]
pub fn switch_cursor_account_traditional_default(
    account_id: &str,
    manual_user_pick: bool,
) -> Result<(), String> {
    nirvana_traditional_switch_steps(account_id, manual_user_pick)
}

/// 无忧 `switchTokensInDb`（Kh）：`accessToken`/`refreshToken` 原样裸 JWT（与默认 profile 一致，不做 `user_id::jwt` 转换）。
fn nirvana_kh_auth_tokens(account: &CursorAccount) -> Result<(String, String), String> {
    // 1. 优先从 cursor_auth_raw 获取
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

    // 2. 从 access_token 字段获取
    let access = account.access_token.trim();
    if !access.is_empty() {
        let refresh = account
            .refresh_token
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(access);
        return Ok((access.to_string(), refresh.to_string()));
    }

    Err(format!("账号 {} 无可用 token", account.email))
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
    switch_tokens_in_profile_db_inner(profile_dir, account_id, manual_user_pick, false)
}

/// 默认无感：Cursor 仍占用库时写 token（对齐虚备 `patch_vscdb_auth`：busy 重试、不切 journal、不删 -wal）。
pub fn switch_tokens_in_profile_db_live(
    profile_dir: &Path,
    account_id: &str,
    manual_user_pick: bool,
) -> Result<(), String> {
    switch_tokens_in_profile_db_inner(profile_dir, account_id, manual_user_pick, true)
}

fn is_sqlite_busy_error(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("database is locked")
        || lower.contains("database busy")
        || lower.contains("sqlite_busy")
        || lower.contains("locked")
}

fn switch_tokens_in_profile_db_inner(
    profile_dir: &Path,
    account_id: &str,
    manual_user_pick: bool,
    live_cursor: bool,
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

    let mut last_err = String::new();
    for attempt in 1..=8 {
        match switch_tokens_in_profile_db_once(
            &db_path,
            &account,
            &access_token,
            &refresh_token,
            live_cursor,
        ) {
            Ok(()) => {
                verify_switch_tokens_written(&db_path)?;
                logger::log_info(&format!(
                    "[Cursor Switch] switchTokensInDb 完成: email={}, db={}, token=nirvana_raw, live={}, attempt={}",
                    account.email,
                    db_path.display(),
                    live_cursor,
                    attempt
                ));
                return Ok(());
            }
            Err(e) => {
                last_err = e;
                if is_sqlite_busy_error(&last_err) && attempt < 8 {
                    logger::log_warn(&format!(
                        "[Cursor Switch] 写库忙重试 {}/8 (live={}): {}",
                        attempt, live_cursor, last_err
                    ));
                    // WP-4：线性退避加上限，避免 250ms*attempt 在多次重试时无界拉长
                    let backoff_ms = (250u64 * attempt as u64).min(800);
                    std::thread::sleep(Duration::from_millis(backoff_ms));
                    continue;
                }
                break;
            }
        }
    }
    Err(last_err)
}

fn switch_tokens_in_profile_db_once(
    db_path: &Path,
    account: &CursorAccount,
    access_token: &str,
    refresh_token: &str,
    live_cursor: bool,
) -> Result<(), String> {
    if !live_cursor {
        remove_vscdb_sidecars(db_path);
    }

    let conn = Connection::open(db_path)
        .map_err(|e| format!("打开 Cursor state.vscdb 失败({}): {}", db_path.display(), e))?;
    // WP-4：live busy_timeout 15s → 8s，缩短最坏阻塞
    conn.busy_timeout(Duration::from_secs(if live_cursor { 8 } else { 5 }))
        .map_err(|e| format!("设置 busy_timeout 失败: {}", e))?;

    let mut switched_to_delete = false;
    let mut wal_only = live_cursor;
    if !live_cursor {
        // 切换到 DELETE journal mode：13GB+ 的 DB 在 WAL 模式下 wal_checkpoint 不可靠，
        // DELETE mode 的写入直接进主 DB 文件，不依赖 checkpoint flush。
        match conn.execute_batch("PRAGMA journal_mode=DELETE;") {
            Ok(_) => switched_to_delete = true,
            Err(e) => {
                let msg = format!("切换 journal_mode=DELETE 失败: {}", e);
                if is_sqlite_busy_error(&msg) {
                    logger::log_warn(&format!(
                        "[Cursor Switch] Cursor 仍持锁，跳过 DELETE 改 WAL 直写: {}",
                        msg
                    ));
                    wal_only = true;
                    conn.busy_timeout(Duration::from_secs(8))
                        .map_err(|e| format!("设置 busy_timeout 失败: {}", e))?;
                    let _ = conn.execute_batch("PRAGMA journal_mode;");
                } else {
                    return Err(msg);
                }
            }
        }
    }
    if wal_only && !switched_to_delete {
        // 虚备式：Cursor 持锁时切 journal 会 database is locked；保持现有 WAL 直接写。
        let _ = conn.execute_batch("PRAGMA journal_mode;");
    }

    conn.execute_batch("BEGIN IMMEDIATE;")
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

        upsert_vscdb_item(&conn, "cursorAuth/accessToken", access_token)?;
        upsert_vscdb_item(&conn, "cursorAuth/refreshToken", refresh_token)?;
        upsert_vscdb_item(&conn, "cursorAuth/cachedEmail", &account.email)?;
        upsert_vscdb_item(&conn, "cursorAuth/cachedSignUpType", "Auth_0")?;

        if let Some(ref auth_id) = resolve_quota_pool_id(account) {
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
            if switched_to_delete {
                let _ = conn.execute_batch("PRAGMA journal_mode=WAL;");
            }
            return Err(e);
        }
    }

    if switched_to_delete {
        conn.execute_batch("PRAGMA journal_mode=WAL;")
            .map_err(|e| format!("恢复 journal_mode=WAL 失败: {}", e))?;
    }
    drop(conn);
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

// ---------------------------------------------------------------------------
// Auto-switch (TokenKeeper 保活用)
// ---------------------------------------------------------------------------

const AUTO_SWITCH_COOLDOWN: Duration = Duration::from_secs(60);

/// 是否启用自动换号（读本机配置，不再写死关闭）
pub fn is_auto_switch_enabled() -> bool {
    crate::modules::config::get_user_config().auto_switch_enabled
}

/// 当前绑定号额度（或对话限流）是否已到该换的程度
pub fn should_auto_switch(current_account_id: &str) -> bool {
    let id = current_account_id.trim();
    if id.is_empty() {
        return false;
    }
    let Some(account) = load_account(id) else {
        return false;
    };
    if account
        .chat_probe
        .as_ref()
        .is_some_and(|p| p.outcome == "rate_limited")
    {
        return true;
    }
    let cfg = crate::modules::config::get_user_config();
    if cfg.auto_switch_account_scope_mode == "selected_accounts" {
        let selected = &cfg.auto_switch_selected_account_ids;
        if !selected.is_empty() && !selected.iter().any(|item| item == id) {
            return false;
        }
    }
    let threshold = cfg.auto_switch_threshold.clamp(0, 100);
    // 同 refresh_account_async_once 的既有语义：先认「实时刷新成功」这一事实。
    let refreshed_ok = account.quota_query_last_error.is_none() && account.usage_updated_at.is_some();
    match cursor_overview_remaining_percent(&account) {
        Some(remaining) if remaining <= threshold => true,
        // 与耗尽判定对齐：配额查失败时 remaining=None，也必须触发换号，否则开关形同虚设
        None if has_quota_query_failed(&account) => true,
        // 刷新成功但没有 quota 数据（例如无套餐字段的老数据）：不得靠假 remaining 永远 skip
        None if refreshed_ok => true,
        _ => false,
    }
}

/// 该实例距上次自动换号是否已过冷却
pub fn auto_switch_cooldown_ok(instance_id: &str) -> bool {
    let key = instance_id.trim();
    if key.is_empty() {
        return false;
    }
    let Ok(map) = AUTO_SWITCH_LAST_AT.lock() else {
        return false;
    };
    match map.get(key) {
        None => true,
        Some(at) => at.elapsed() >= AUTO_SWITCH_COOLDOWN,
    }
}

pub fn mark_auto_switch_attempt(instance_id: &str) {
    let key = instance_id.trim();
    if key.is_empty() {
        return;
    }
    if let Ok(mut map) = AUTO_SWITCH_LAST_AT.lock() {
        map.insert(key.to_string(), Instant::now());
    }
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

    // 第一次：Cookie 方式请求
    let response = client
        .get(CURSOR_USAGE_SUMMARY_URL)
        .header("Accept", "application/json")
        .header("Cookie", &cookie)
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
        )
        .header("Referer", "https://cursor.com/cn/dashboard")
        .send()
        .await
        .map_err(|e| format!("请求 Cursor usage API 失败: {}", e))?;

    // 若 401：第二次 Bearer Token 方式重试（对齐上游 nirvana 原版标准逻辑）
    let response = if response.status().as_u16() == 401 {
        logger::log_info("[Cursor Refresh] Cookie 方式返回 401，改用 Bearer Token 重试 usage-summary...");
        client
            .get(CURSOR_USAGE_SUMMARY_URL)
            .header("Accept", "application/json")
            .header("Authorization", format!("Bearer {}", access_token))
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            )
            .header("Referer", "https://cursor.com/cn/dashboard")
            .send()
            .await
            .map_err(|e| format!("Bearer 重试请求 Cursor usage API 失败: {}", e))?
    } else {
        response
    };

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

async fn refresh_account_async_once(
    account_id: &str,
    defer_index_write: bool,
) -> Result<CursorRefreshResult, String> {
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

    let api_started = std::time::Instant::now();
    let access_token = account.access_token.clone();
    let (meta_result, stripe_result, usage_result) = tokio::join!(
        fetch_user_meta_with_client(&client, &access_token),
        fetch_stripe_profile_with_client(&client, &access_token),
        fetch_usage_summary_with_client(&client, &access_token),
    );
    let mut api_ms = api_started.elapsed().as_millis();

    // 0013：JWT 的 exp 不可信——批量号 exp 未到期（实测到 2026-11），服务端却已 401。
    // 因此 `access_token_needs_refresh` 会判「不需要刷新」，配额查询带着过期 token 直接 401，
    // 再被当成「配额查询失败」触发换号，白白换掉本来还能用的号。
    // 401 才是真信号：这里先刷新 access token，再用新 token 重试一次配额查询。
    let usage_result = match &usage_result {
        Err(err) if is_cursor_auth_quota_error(err) => {
            let retry_started = std::time::Instant::now();
            match refresh_account_access_token_with_client(&client, &mut account).await {
                Ok(true) => {
                    logger::log_info(&format!(
                        "[Cursor Refresh] 配额 401，access token 刷新成功后重试: id={}, email={}",
                        account.id, account.email
                    ));
                    let new_token = account.access_token.clone();
                    let retried = fetch_usage_summary_with_client(&client, &new_token).await;
                    api_ms += retry_started.elapsed().as_millis();
                    retried
                }
                Ok(false) => {
                    logger::log_info(&format!(
                        "[Cursor Refresh] 配额 401，但账号无 refresh_token 可刷新: id={}",
                        account.id
                    ));
                    usage_result
                }
                Err(refresh_err) => {
                    logger::log_warn(&format!(
                        "[Cursor Refresh] 配额 401，refresh token 刷新失败: id={}, error={}",
                        account.id, refresh_err
                    ));
                    usage_result
                }
            }
        }
        _ => usage_result,
    };

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
            // 去掉过度保护：本次是真实拉取成功（Ok），API 回什么就写什么，
            // 包括真的 0%。原 merge_usage_preserving_nonzero_history 会以「防闲置号假 0%」
            // 为由拒绝写入，导致用户刷新后额度永远是旧值（拿不到真实额度）。
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
            let is_transient = is_cursor_transient_quota_error(&err);
            logger::log_warn(&format!(
                "[Cursor Refresh] API 配额拉取失败 (transient={}): id={}, error={}",
                is_transient, account.id, err
            ));
            if !is_transient {
                account.quota_query_last_error = Some(err);
                account.quota_query_last_error_at = Some(chrono::Utc::now().timestamp_millis());
            }
        }
    }

    let refreshed_at = now_ts();
    if usage_refreshed {
        account.usage_updated_at = Some(refreshed_at);
    }
    account.last_used = refreshed_at;
    backfill_quota_pool_auth_id(&mut account);
    let updated = account.clone();
    let persist_started = std::time::Instant::now();
    if defer_index_write {
        upsert_account_record_deferred(account)?;
    } else {
        upsert_account_record(account)?;
    }
    let persist_ms = persist_started.elapsed().as_millis();
    if persist_ms >= 200 || api_ms >= 1000 {
        logger::log_info(&format!(
            "[Cursor Perf] id={}, 三接口={}ms, 写盘={}ms",
            updated.id, api_ms, persist_ms
        ));
    }
    logger::log_info(&format!(
        "[Cursor Refresh] 刷新处理完成 (usage_refreshed={}): id={}, email={}",
        usage_refreshed, updated.id, updated.email
    ));
    Ok(CursorRefreshResult {
        account: updated.clone(),
        persisted: cursor_accounts_differ_for_refresh(&existing, &updated),
        usage_refreshed,
    })
}

pub async fn refresh_account_async(account_id: &str) -> Result<CursorRefreshResult, String> {
    refresh_account_async_with_options(account_id, false).await
}

/// 0012：批量刷新用——不逐账号重写整表索引（索引由批末统一回写）。
pub async fn refresh_account_async_deferred(
    account_id: &str,
) -> Result<CursorRefreshResult, String> {
    refresh_account_async_with_options(account_id, true).await
}

pub async fn refresh_account_async_with_options(
    account_id: &str,
    defer_index_write: bool,
) -> Result<CursorRefreshResult, String> {
    let result = refresh_account_async_once(account_id, defer_index_write).await;
    if let Err(err) = &result {
        if !is_cursor_transient_quota_error(err) {
            persist_quota_query_error(account_id, err);
        }
    }
    if let Ok(refreshed) = &result {
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
                usage_refreshed: false,
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

/// 0012：当前绑定账号的**实时**额度快照。
/// 强制走一次 usage API（与磁盘值无关）：查到才算可用，查不到返回 queried=false，
/// 由前端显示「未刷新/查询失败」，禁止拿磁盘旧值伪装满额度。
pub async fn current_account_quota_realtime() -> CursorCurrentQuotaSnapshot {
    let accounts = list_accounts();
    let Some(account_id) = resolve_current_account_id(&accounts) else {
        return CursorCurrentQuotaSnapshot {
            account_id: None,
            queried: false,
            remaining_percent: None,
            error: Some("当前没有绑定账号".to_string()),
            account: None,
        };
    };

    match refresh_account_async(&account_id).await {
        Ok(refreshed) => {
            let remaining = cursor_overview_remaining_percent(&refreshed.account);
            let error = refreshed.account.quota_query_last_error.clone();
            logger::log_info(&format!(
                "[Cursor Refresh] 当前账号实时额度: id={}, queried={}, remaining={:?}, error={:?}",
                account_id, refreshed.usage_refreshed, remaining, error
            ));
            CursorCurrentQuotaSnapshot {
                account_id: Some(account_id),
                queried: refreshed.usage_refreshed,
                remaining_percent: remaining,
                error,
                account: Some(refreshed.account.for_ui_list()),
            }
        }
        Err(err) => {
            logger::log_warn(&format!(
                "[Cursor Refresh] 当前账号实时额度失败: id={}, error={}",
                account_id, err
            ));
            CursorCurrentQuotaSnapshot {
                account_id: Some(account_id.clone()),
                queried: false,
                remaining_percent: None,
                error: Some(err),
                account: load_account(&account_id).map(|a| a.for_ui_list()),
            }
        }
    }
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
    let last_touch = recent.max(err_at).max(account.last_used);
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
    // 0012：并发刷新 + 批末统一回写索引。
    // 旧实现逐个账号串行，且每个账号都重写一遍 4482 条索引（1.1MB），
    // 单账号被拖到 20~40 秒，8 分钟一轮只刷得完十几个，池子永远扫不完。
    const REFRESH_CONCURRENCY: usize = 8;
    let mut results: Vec<(String, Result<CursorRefreshResult, String>)> =
        Vec::with_capacity(active_accounts.len());
    let mut pending: tokio::task::JoinSet<(String, Result<CursorRefreshResult, String>)> =
        tokio::task::JoinSet::new();
    let mut queue = active_accounts.into_iter();
    let mut stop_launching = false;

    loop {
        while !stop_launching && pending.len() < REFRESH_CONCURRENCY {
            let Some(account) = queue.next() else { break };
            if let Some(max_dur) = max_duration {
                if started.elapsed() >= max_dur {
                    stop_launching = true;
                    break;
                }
            }
            let id = account.id.clone();
            mark_refresh_attempted(&id);
            pending.spawn(async move {
                let result = refresh_account_async_deferred(&id).await;
                (id, result)
            });
        }
        if pending.is_empty() {
            break;
        }
        match pending.join_next().await {
            Some(joined) => match joined {
                Ok(item) => results.push(item),
                Err(err) => logger::log_warn(&format!(
                    "[Cursor Refresh] 刷新任务异常: {}",
                    err
                )),
            },
            None => break,
        }
    }

    if !pending.is_empty() {
        pending.abort_all();
    }

    // 批末统一回写索引（一次读、一次写）
    let flushed: Vec<CursorAccount> = results
        .iter()
        .filter_map(|(_, result)| result.as_ref().ok().map(|item| item.account.clone()))
        .collect();
    flush_account_index_for(&flushed);

    logger::log_info(&format!(
        "[Cursor Refresh] 最旧优先刷新结束: done={}, elapsed={}ms, index_flushed={}",
        results.len(),
        started.elapsed().as_millis(),
        flushed.len()
    ));
    Ok(results)
}

pub async fn refresh_all_tokens(
) -> Result<Vec<(String, Result<CursorRefreshResult, String>)>, String> {
    let accounts = list_accounts();
    let active_accounts: Vec<CursorAccount> = accounts
        .into_iter()
        .filter(|account| !is_banned_account(account))
        .collect();

    let mut results = Vec::with_capacity(active_accounts.len());
    for account in active_accounts {
        let id = account.id.clone();
        let result = refresh_account_async(&id).await;
        results.push((id, result));
    }
    Ok(results)
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

fn usage_raw_total_percent(raw: &serde_json::Value) -> Option<f64> {
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
    pick_number(plan_value, &["totalPercentUsed", "total_percent_used"])
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
        // 只有真正算得出「剩余 <= 0」才算用尽。
        // 查不到（刷新失败/未查/零额度）绝不等同于用尽——上游口径：失败只在
        // quota_query_last_error 上标「配额查询失败」，不把账号打成额度用尽。
        Some(remaining) => remaining <= 0,
        None => false,
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

/// usage-summary 的 `individualUsage.plan` 节点（兼容 snake_case 与旧 planUsage 键）。
fn plan_usage_node<'a>(raw: &'a serde_json::Value) -> Option<&'a serde_json::Value> {
    let raw_obj = raw.as_object()?;
    raw_obj
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
        .or_else(|| raw_obj.get("plan_usage"))
}

/// 真实套餐上限（美元侧）。Cockpit 本地库字段单位为分，取到即回退兼容「元」。
fn plan_limit_value(account: &CursorAccount) -> Option<f64> {
    let raw = account.cursor_usage_raw.as_ref()?;
    pick_number(plan_usage_node(raw), &["limit"])
}

/// 套餐包含额度的真实规模：breakdown.included > limit > breakdown.total。
/// 0013：Cockpit 本地库 free 号实测 `plan.limit=0`，若只看百分比会 100-0=100 得到
/// 「假满额」——真正切到 DSH/Cursor 里根本不能用。额度可用性必须以规模字段为准。
fn plan_budget_scale(account: &CursorAccount) -> Option<f64> {
    let raw = account.cursor_usage_raw.as_ref()?;
    let plan_value = plan_usage_node(raw);
    let included = pick_number(plan_value, &["breakdown"])
        .map(|_| ())
        .and_then(|_| {
            plan_value
                .and_then(|value| value.get("breakdown"))
                .and_then(|value| pick_number(Some(value), &["included"]))
        });
    let limit = pick_number(plan_value, &["limit"]);
    let breakdown_total = plan_value
        .and_then(|value| value.get("breakdown"))
        .and_then(|value| pick_number(Some(value), &["total"]));

    [included, limit, breakdown_total]
        .into_iter()
        .flatten()
        .fold(None::<f64>, |acc, value| {
            Some(acc.map_or(value, |current| current.max(value)))
        })
}

/// 是否有真实可用的套餐预算（scale > 0）。
pub fn has_effective_plan_budget(account: &CursorAccount) -> bool {
    plan_budget_scale(account).is_some_and(|scale| scale > 0.0)
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

/// 从 Cursor 本地登录态解析「当前账号」在账号库里的 id（用户 2026-09-17 指令）。
///
/// 为什么不再用 `list_accounts()`：那条路径会无条件调用 `normalize_account_index()`，
/// 即逐条读 4472 个详情文件 + 扫 9843 个文件的账号目录，并全程持有
/// `CURSOR_ACCOUNT_INDEX_LOCK`（实测等锁 41783 / 48471 / 75870 ms），
/// 期间把同期的列表首屏请求 `list_accounts_page_for_ui` 一并堵死。
///
/// 本函数只做两件轻量事：
/// 1. `read_local_cursor_auth()` 读 Cursor `state.vscdb` 的 `cursorAuth/cachedEmail` 与 `cursorAuth/authId`；
/// 2. 在账号索引 summary（自带 `email` / `auth_id`）里匹配，只读 1.1MB 索引。
/// 不调用 `load_account()`、不扫账号目录，因此不会长时间持锁。
pub fn resolve_current_account_id_from_local_state() -> Option<String> {
    let local_payload = read_local_cursor_auth().ok().flatten();

    let _lock = CURSOR_ACCOUNT_INDEX_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let index = load_account_index();

    if let Some(payload) = local_payload.as_ref() {
        // 1) 以 Cursor 本地 authId 精确匹配
        if let Some(auth_id) = payload
            .auth_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            if let Some(hit) = index
                .accounts
                .iter()
                .find(|summary| summary.auth_id.as_deref().map(str::trim) == Some(auth_id))
            {
                return Some(hit.id.clone());
            }
        }
        // 2) 再以本地邮箱（规范化）匹配
        if let Some(email) = normalize_email_identity(Some(payload.email.as_str())) {
            if let Some(hit) = index.accounts.iter().find(|summary| {
                normalize_email_identity(Some(summary.email.as_str()))
                    .map(|candidate| candidate == email)
                    .unwrap_or(false)
            }) {
                return Some(hit.id.clone());
            }
        }
    }

    // 3) 本地态缺失或未命中：回退到既有「记录的当前 id ∩ 现有账号 id」校验，同样不读详情文件
    crate::modules::provider_current_state::resolve_existing_current_account_id(
        "cursor",
        index.accounts.iter().map(|summary| summary.id.as_str()),
    )
}

pub(crate) fn resolve_current_account_id(accounts: &[CursorAccount]) -> Option<String> {
    if let Ok(Some(local_payload)) = read_local_cursor_auth() {
        let incoming_auth_id = resolve_payload_auth_id(&local_payload);
        let incoming_email = normalize_email_identity(Some(local_payload.email.as_str()));
        let incoming_token = normalize_token_identity(Some(local_payload.access_token.as_str()));

        if let Some(account_id) = accounts
            .iter()
            .find(|account| {
                let existing_auth_id = resolve_account_auth_id(account);
                if let (Some(existing), Some(incoming)) =
                    (existing_auth_id.as_ref(), incoming_auth_id.as_ref())
                {
                    return existing == incoming;
                }
                if existing_auth_id.is_some() || incoming_auth_id.is_some() {
                    return false;
                }

                let existing_email = normalize_email_identity(Some(account.email.as_str()));
                let existing_token = normalize_token_identity(Some(account.access_token.as_str()));
                if let (Some(existing), Some(incoming)) =
                    (existing_email.as_ref(), incoming_email.as_ref())
                {
                    if existing == incoming {
                        return true;
                    }
                }
                if let (Some(existing), Some(incoming)) =
                    (existing_token.as_ref(), incoming_token.as_ref())
                {
                    if existing == incoming {
                        return true;
                    }
                }
                false
            })
            .map(|account| account.id.clone())
        {
            return Some(account_id);
        }
    }

    if let Ok(settings) = crate::modules::cursor_instance::load_default_settings() {
        if let Some(bind_id) = settings.bind_account_id {
            let trimmed = bind_id.trim();
            if !trimmed.is_empty() && accounts.iter().any(|account| account.id == trimmed) {
                return Some(trimmed.to_string());
            }
        }
    }

    crate::modules::provider_current_state::resolve_existing_current_account_id(
        "cursor",
        accounts.iter().map(|account| account.id.as_str()),
    )
    .or_else(|| {
        accounts
            .iter()
            .max_by_key(|account| account.last_used)
            .map(|account| account.id.clone())
    })
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
                        "autoPercentUsed": auto,
                        "limit": 1000
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
    fn paid_zero_used_counts_as_full_pool() {
        // 有真实套餐上限（limit=1000）且 0% 用量 → 100% 满额，可切。
        let account = account_with_usage("b", 0, 0);
        assert!(has_effective_plan_budget(&account));
        assert_eq!(cursor_overview_remaining_percent(&account), Some(100));
        assert!(ensure_cursor_overview_pickable(&account).is_ok());
    }

    /// 0013 回归：这一条正是用户报的「看着满额，DSH 里不能用」。
    /// plan.limit=0 的 Free 号以前算出 100% 满额并通过 pick，切过去无法对话。
    #[test]
    fn zero_limit_account_is_not_full_and_not_pickable() {
        let mut account = account_with_usage("b2", 0, 0);
        account.cursor_usage_raw = Some(serde_json::json!({
            "individualUsage": {
                "plan": {
                    "totalPercentUsed": 0,
                    "autoPercentUsed": 0,
                    "limit": 0
                }
            }
        }));


    /// 0013：free 号真实响应形态（官方 usage-summary）。
    #[test]
    fn free_membership_zero_limit_is_not_pickable() {
        let mut account = account_with_usage("b3", 0, 0);
        account.membership_type = Some("free".into());
        account.cursor_usage_raw = Some(serde_json::json!({
            "membershipType": "free",
            "individualUsage": {
                "plan": {
                    "enabled": true,
                    "used": 0,
                    "limit": 0,
                    "remaining": 0,
                    "breakdown": { "included": 0, "bonus": 0, "total": 0 },
                    "autoPercentUsed": 0,
                    "apiPercentUsed": 0,
                    "totalPercentUsed": 0
                },
                "onDemand": { "enabled": false }
            }
        }));


    /// 0013：breakdown.total（已用量合计）不为 0 时不得误判成零额度。
    #[test]

    /// 0013：完全没有规模字段的老数据不得被当成零额度误杀。
    #[test]

    /// 0013：零额度号即使 chat_probe=ok，也不得被判成「满额可切」。
    #[test]

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
            "autoPercentUsed": auto,
            "limit": 1000
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
        // 0011：0% 用量即为 100% 满额可用，overview 判定有效并可挑
        assert_eq!(cursor_overview_remaining_percent(&account), Some(100));
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


#[cfg(test)]
mod hot_path_auto_switch_defense_tests {
    use super::{hot_path_auto_switch_disk_payload, hot_path_effective_auto_switch};

    #[test]
    fn hot_path_never_writes_auto_switch_false() {
        assert!(hot_path_effective_auto_switch(false, false));
        assert!(hot_path_effective_auto_switch(true, false));
    }

    #[test]
    fn polluted_false_is_upgraded_only() {
        // 读到 false 只升级，禁止原样回写 false（对齐历史硬编码病根）。
        assert_eq!(hot_path_effective_auto_switch(false, false), true);
    }

    #[test]
    fn disk_payload_pref_and_seamless_never_false() {
        for user in [false, true] {
            let (pref, seamless_field) = hot_path_auto_switch_disk_payload(user);
            assert_eq!(
                pref.get("enabled").and_then(|x| x.as_bool()),
                Some(true),
                "auto_switch_pref.enabled must not be false (user={user})"
            );
            assert_eq!(
                seamless_field.get("auto_switch").and_then(|x| x.as_bool()),
                Some(true),
                "seamless config.auto_switch must not be false (user={user})"
            );
            assert_eq!(
                pref["enabled"], seamless_field["auto_switch"],
                "pref/seamless dual-write must stay consistent"
            );
        }
    }

    #[test]
    fn rejects_historical_hardcoded_false_shape() {
        // HEAD 曾写 {"enabled": false} 与 config.auto_switch=false；payload 不得再现。
        let (pref, field) = hot_path_auto_switch_disk_payload(false);
        let pref_s = serde_json::to_string(&pref).unwrap();
        let field_s = serde_json::to_string(&field).unwrap();
        assert!(!pref_s.contains("false"), "pref payload leaked false: {pref_s}");
        assert!(!field_s.contains("false"), "seamless field leaked false: {field_s}");
    }
}
}
}

/// 0017 临时探针：分解 load_account 单条 12.9ms 的去向。测量完成即删除本模块。
#[cfg(test)]
mod zz_perf_probe_0017 {
    use super::*;

    #[test]
    fn probe_0017_load_account_breakdown() {
        let index = load_account_index();
        let ids: Vec<String> = index
            .accounts
            .iter()
            .take(200)
            .map(|s| s.id.clone())
            .collect();

        let t = std::time::Instant::now();
        let mut paths = Vec::new();
        for id in &ids {
            paths.push(resolve_account_file_path(id).ok());
        }
        println!(
            "[PROBE17] resolve_account_file_path x{}: {}ms",
            ids.len(),
            t.elapsed().as_millis()
        );

        let t = std::time::Instant::now();
        let mut contents = Vec::new();
        for p in paths.iter().flatten() {
            contents.push(std::fs::read_to_string(p).ok());
        }
        println!(
            "[PROBE17] read_to_string x{}: {}ms",
            ids.len(),
            t.elapsed().as_millis()
        );

        let t = std::time::Instant::now();
        for (p, c) in paths.iter().flatten().zip(contents.iter().flatten()) {
            let _ = crate::modules::secure_account_storage::deserialize_account_file::<
                CursorAccount,
            >(p, c);
        }
        println!(
            "[PROBE17] deserialize(decrypt+parse) x{}: {}ms",
            ids.len(),
            t.elapsed().as_millis()
        );

        let t = std::time::Instant::now();
        let mut ok = 0usize;
        for id in &ids {
            if load_account(id).is_some() {
                ok += 1;
            }
        }
        println!(
            "[PROBE17] load_account x{}: {}ms (ok={})",
            ids.len(),
            t.elapsed().as_millis(),
            ok
        );
    }
}
