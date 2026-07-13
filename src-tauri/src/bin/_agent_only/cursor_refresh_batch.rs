//! 独立触发 Cursor 全量 refresh（与 GUI `refresh_all_cursor_tokens` 同逻辑），写盘后可被 Cockpit 列表读取。
use antigravity_cockpit_tools_lib::modules::{config, cursor_account, logger};

#[tokio::main]
async fn main() {
    logger::init_logger();
    let _ = config::get_user_config();
    logger::log_info("[Cursor Batch CLI] 批量刷新开始");
    let started = std::time::Instant::now();

    let accounts = cursor_account::list_accounts();
    let active: Vec<_> = accounts
        .into_iter()
        .filter(|a| !cursor_account::is_banned_account(a))
        .collect();
    let total = active.len();
    let mut success = 0usize;
    let mut persisted_any = false;

    for account in active {
        let id = account.id.clone();
        match cursor_account::refresh_account_async(&id).await {
            Ok(refreshed) => {
                success += 1;
                if refreshed.persisted {
                    persisted_any = true;
                }
            }
            Err(err) => {
                logger::log_warn(&format!(
                    "[Cursor Batch CLI] 刷新失败: id={}, error={}",
                    id, err
                ));
            }
        }
    }

    logger::log_info(&format!(
        "[Cursor Batch CLI] 批量刷新完成: success={}, total={}, persisted_any={}, elapsed={}ms",
        success,
        total,
        persisted_any,
        started.elapsed().as_millis()
    ));
}
