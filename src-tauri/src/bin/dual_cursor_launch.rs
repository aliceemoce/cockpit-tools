//! 验收用：启动多开实例（默认实例已在线时跳过，避免关 Cos1）。
use std::time::Duration;

use antigravity_cockpit_tools_lib::commands::cursor_instance;
use antigravity_cockpit_tools_lib::modules::{config, cursor_instance as cursor_inst, logger};

#[tokio::main]
async fn main() {
    logger::init_logger();
    let _ = config::get_user_config();

    let args: Vec<String> = std::env::args().collect();
    let multi_only = args.iter().any(|a| a == "--multi-only");
    let forced_account = args
        .iter()
        .position(|a| a == "--account")
        .and_then(|idx| args.get(idx + 1))
        .cloned();
    let instance_id = args
        .iter()
        .skip(1)
        .find(|a| !a.starts_with('-') && *a != "--account" && forced_account.as_deref() != Some(a.as_str()))
        .cloned()
        .or_else(read_first_instance_id)
        .unwrap_or_else(|| {
            eprintln!("用法: dual-cursor-launch [--multi-only] [instance_uuid]");
            std::process::exit(2);
        });

    let default_running = cursor_inst::resolve_cursor_pid(None, None).is_some();

    if multi_only || default_running {
        if default_running {
            eprintln!("[dual-cursor-launch] 默认 Cursor 已在运行，跳过默认实例切号/重启");
        } else {
            eprintln!("[dual-cursor-launch] --multi-only：跳过默认实例");
        }
    } else {
        eprintln!("[dual-cursor-launch] 1/2 默认实例 …");
        if let Err(err) = cursor_instance::start_cursor_instance_with_account_switch(
            "__default__".to_string(),
            None,
        )
        .await
        {
            eprintln!("[dual-cursor-launch] 默认实例失败: {err}");
            std::process::exit(1);
        }
        tokio::time::sleep(Duration::from_secs(4)).await;
    }

    eprintln!("[dual-cursor-launch] 2/2 多开实例 {instance_id} …");
    if let Some(ref account_id) = forced_account {
        eprintln!("[dual-cursor-launch] 强制账号: {account_id}");
    }
    if let Err(err) = cursor_instance::start_cursor_instance_with_account_switch(
        instance_id.clone(),
        forced_account,
    )
    .await
    {
        eprintln!("[dual-cursor-launch] 多开实例失败: {err}");
        std::process::exit(1);
    }

    eprintln!("[dual-cursor-launch] 完成");
}

fn read_first_instance_id() -> Option<String> {
    let store = cursor_inst::load_instance_store().ok()?;
    store.instances.first().map(|i| i.id.clone())
}
