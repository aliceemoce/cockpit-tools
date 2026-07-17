//! Cursor 真实对话验活 CLI（验收用）。
//! usage: cursor_chat_probe <account_id> [account_id...]

use antigravity_cockpit_tools_lib::modules::{cursor_chat_probe, logger};

fn main() {
    logger::init_logger();
    let _ = antigravity_cockpit_tools_lib::modules::config::get_user_config();

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: cursor_chat_probe <account_id> [account_id...]");
        std::process::exit(2);
    }

    let mut ok = 0usize;
    let mut fail = 0usize;
    for id in args {
        match cursor_chat_probe::probe_account_chat(&id) {
            Ok(account) => {
                let outcome = account
                    .chat_probe
                    .as_ref()
                    .map(|p| p.outcome.as_str())
                    .unwrap_or("missing");
                if outcome == "ok" {
                    ok += 1;
                } else {
                    fail += 1;
                }
                println!(
                    "id={} email={} outcome={} status_email={:?} detail={:?}",
                    account.id,
                    account.email,
                    outcome,
                    account.chat_probe.as_ref().and_then(|p| p.status_email.clone()),
                    account.chat_probe.as_ref().and_then(|p| p.detail.clone()),
                );
            }
            Err(err) => {
                fail += 1;
                eprintln!("id={} error={}", id, err);
            }
        }
    }
    println!("summary ok={} fail={}", ok, fail);
    if ok == 0 {
        std::process::exit(1);
    }
}
