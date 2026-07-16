//! 独立触发 Cursor 最旧优先刷新（与 GUI `refresh_all_cursor_tokens` 同逻辑），写盘后可被 Cockpit 列表读取。
//! 仍串行；可选环境变量 `CURSOR_REFRESH_MAX_COUNT` 限制本轮数量。
use antigravity_cockpit_tools_lib::modules::{config, cursor_account, logger};

#[tokio::main]
async fn main() {
    logger::init_logger();
    let _ = config::get_user_config();
    logger::log_info("[Cursor Batch CLI] 最旧优先刷新开始");
    let started = std::time::Instant::now();

    let max_count = std::env::var("CURSOR_REFRESH_MAX_COUNT")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|n| *n > 0);

    let results = match cursor_account::refresh_tokens_stale_first(max_count, None).await {
        Ok(v) => v,
        Err(err) => {
            logger::log_warn(&format!("[Cursor Batch CLI] 刷新失败: {}", err));
            return;
        }
    };

    let total = results.len();
    let mut success = 0usize;
    let mut persisted_any = false;
    for (id, result) in results {
        match result {
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
