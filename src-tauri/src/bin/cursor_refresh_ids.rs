//! 按指定 Cursor account_id 刷新配额（仅用于验收对照，不影响主应用逻辑）
use std::time::Instant;

use antigravity_cockpit_tools_lib::modules::{cursor_account, logger};

#[tokio::main]
async fn main() {
    logger::init_logger();
    let _ = antigravity_cockpit_tools_lib::modules::config::get_user_config();

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: cursor_refresh_ids <account_id> [account_id...]");
        std::process::exit(2);
    }

    let started = Instant::now();
    let mut success = 0usize;
    let mut persisted_any = false;

    for id in args {
        match cursor_account::refresh_account_async(&id).await {
            Ok(refreshed) => {
                success += 1;
                if refreshed.persisted {
                    persisted_any = true;
                }
                logger::log_info(&format!(
                    "[Cursor Refresh IDs] done: id={}, email={}, persisted={}",
                    refreshed.account.id, refreshed.account.email, refreshed.persisted
                ));
            }
            Err(err) => {
                logger::log_warn(&format!(
                    "[Cursor Refresh IDs] failed: id={}, error={}",
                    id, err
                ));
            }
        }
    }

    logger::log_info(&format!(
        "[Cursor Refresh IDs] finished: success={}, total={}, persisted_any={}, elapsed={}ms",
        success,
        success.max(1).max(0),
        persisted_any,
        started.elapsed().as_millis()
    ));
}
