//! Cursor 配额刷新：启动 10 分钟后才开始；当前账号 20 秒；全量分批次不堵队列。

use std::sync::atomic::{AtomicBool, Ordering};

use crate::modules::{app_startup_defer, cursor_account, logger};

const CURRENT_QUOTA_REFRESH_SECS: u64 = 20;

static SCHEDULER_STARTED: AtomicBool = AtomicBool::new(false);

pub fn ensure_started() {
    if SCHEDULER_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    logger::log_info(&format!(
        "[Cursor Refresh] 已安排后台调度: startup_defer={}s, current_interval={}s",
        app_startup_defer::STARTUP_DEFER_SECS,
        CURRENT_QUOTA_REFRESH_SECS
    ));
    tauri::async_runtime::spawn(async {
        app_startup_defer::wait_startup_defer().await;
        logger::log_info("[Cursor Refresh] 启动延时结束，开始分批次全量刷新");
        if let Err(err) = cursor_account::refresh_all_tokens_batched().await {
            logger::log_warn(&format!("[Cursor Refresh] 首次分批次全量刷新失败: {}", err));
        }
        loop {
            if let Some(account_id) = cursor_account::resolve_current_account_id_for_refresh() {
                if let Err(err) =
                    cursor_account::refresh_account_async(&account_id).await
                {
                    logger::log_warn(&format!(
                        "[Cursor Refresh] 当前账号配额刷新失败: id={}, error={}",
                        account_id, err
                    ));
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(CURRENT_QUOTA_REFRESH_SECS)).await;
        }
    });
}
