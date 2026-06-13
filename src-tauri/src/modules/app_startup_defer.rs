use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const STARTUP_DEFER_SECS: u64 = 10 * 60;

static APP_START_TS_SECS: AtomicI64 = AtomicI64::new(0);

pub fn mark_app_started() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or(0);
    APP_START_TS_SECS.store(now, Ordering::Release);
}

pub fn startup_defer_elapsed() -> bool {
    let started = APP_START_TS_SECS.load(Ordering::Acquire);
    if started <= 0 {
        return false;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or(0);
    now.saturating_sub(started) >= STARTUP_DEFER_SECS as i64
}

pub fn startup_defer_remaining() -> Duration {
    let started = APP_START_TS_SECS.load(Ordering::Acquire);
    if started <= 0 {
        return Duration::from_secs(STARTUP_DEFER_SECS);
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or(0);
    let elapsed = now.saturating_sub(started).max(0) as u64;
    Duration::from_secs(STARTUP_DEFER_SECS.saturating_sub(elapsed))
}

pub async fn wait_startup_defer() {
    let remaining = startup_defer_remaining();
    if remaining.is_zero() {
        return;
    }
    tokio::time::sleep(remaining).await;
}
