use std::collections::HashSet;
use std::path::Path;

use crate::models::{DefaultInstanceSettings, InstanceProfileView};
use crate::modules;
use tokio::sync::{Mutex, OnceCell};

const DEFAULT_INSTANCE_ID: &str = "__default__";

static CURSOR_SWITCH_LAUNCH_LOCK: OnceCell<Mutex<()>> = OnceCell::const_new();

async fn cursor_switch_launch_lock() -> &'static Mutex<()> {
    CURSOR_SWITCH_LAUNCH_LOCK
        .get_or_init(|| async { Mutex::new(()) })
        .await
}

fn is_profile_initialized(user_data_dir: &str) -> bool {
    let path = Path::new(user_data_dir);
    if !path.exists() {
        return false;
    }
    match std::fs::read_dir(path) {
        Ok(mut iter) => iter.next().is_some(),
        Err(_) => false,
    }
}

fn collect_reserved_account_ids(instance_id: &str) -> HashSet<String> {
    let mut reserved = HashSet::new();
    let Ok(store) = modules::cursor_instance::load_instance_store() else {
        return reserved;
    };
    for instance in &store.instances {
        if instance.id == instance_id {
            continue;
        }
        if let Some(bind_id) = instance
            .bind_account_id
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            reserved.insert(bind_id.to_string());
        }
    }
    if instance_id != DEFAULT_INSTANCE_ID {
        if let Some(bind_id) = store
            .default_settings
            .bind_account_id
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            reserved.insert(bind_id.to_string());
        }
    }
    reserved
}


fn pick_account_for_auto_switch(exclude: &HashSet<String>) -> Result<(String, &'static str, usize), String> {
    if let Some(full_id) = modules::cursor_account::pick_full_quota_account(exclude) {
        return Ok((full_id, "full", 1usize));
    }
    let id = modules::cursor_account::pick_highest_remaining_credits_account(exclude).ok_or_else(|| {
        "没有可用的满额 Cursor 账号，请稍后重试或手动绑定账号".to_string()
    })?;
    Ok((id, "highest", 1usize))
}

async fn resolve_auto_switch_account_with_probe_retry(
    instance_id: &str,
    bind_account_id: Option<&str>,
    trace: &modules::cursor_switch_audit::CursorSwitchAuditCtx,
) -> Result<String, String> {
    let mut exclude = collect_reserved_account_ids(instance_id);
    if let Some(bind_id) = bind_account_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        exclude.insert(bind_id.to_string());
    }

    loop {
        let (pick_id, pick_pool, pick_candidates) = match pick_account_for_auto_switch(&exclude) {
            Ok(value) => value,
            Err(err) => {
                modules::cursor_switch_audit::write_pick_no_candidates(trace, &err);
                return Err(err);
            }
        };
        let account = modules::cursor_account::load_account(&pick_id)
            .ok_or_else(|| format!("Cursor 账号不存在: {}", pick_id))?;
        modules::cursor_switch_audit::write_pick(trace, &account, pick_pool, pick_candidates);

        let mut probe_ok = false;
        let mut last_probe_err: Option<String> = None;
        for attempt in 0..3 {
            if attempt > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(800)).await;
            }
            match modules::cursor_account::probe_cursor_account_live_auth(&pick_id).await {
                Ok(()) => {
                    probe_ok = true;
                    break;
                }
                Err(err) => {
                    last_probe_err = Some(err);
                }
            }
        }

        if probe_ok {
            modules::cursor_switch_audit::write_probe_pre(trace, &account, "ok", None);
            let from_bind = bind_account_id.unwrap_or("(none)");
            modules::logger::log_info(&format!(
                "[Cursor Switch] 自动轮换账号: instance_id={}, from_bind={}, to={}, pool={}, remaining={}%",
                instance_id,
                from_bind,
                account.email,
                pick_pool, 0
            ));
            return Ok(pick_id);
        }

        let err = last_probe_err.unwrap_or_else(|| "probe failed".to_string());
        let outcome = if err.contains("会话")
            || err.contains("未认证")
            || err.contains("失效")
            || err.contains("过期")
        {
            "auth_fail"
        } else if modules::cursor_account::is_cursor_transient_quota_error(&err) {
            "transient"
        } else {
            "err"
        };
        modules::cursor_switch_audit::write_probe_pre(trace, &account, outcome, Some(&err));
        exclude.insert(pick_id.clone());
    }
}

fn persist_auto_bind_account(instance_id: &str, account_id: &str) -> Result<(), String> {
    if instance_id == DEFAULT_INSTANCE_ID {
        modules::cursor_instance::update_default_settings(
            Some(Some(account_id.to_string())),
            None,
            None,
        )?;
        return Ok(());
    }
    modules::cursor_instance::update_instance(modules::cursor_instance::UpdateInstanceParams {
        instance_id: instance_id.to_string(),
        name: None,
        working_dir: None,
        extra_args: None,
        bind_account_id: Some(Some(account_id.to_string())),
    })?;
    Ok(())
}

fn prepare_instance_for_account(user_data_dir: &str, account_id: &str) -> Result<(), String> {
    modules::cursor_account::switch_cursor_account_to_profile(account_id, Path::new(user_data_dir))
}

struct InstanceLaunchContext {
    user_data_dir: String,
    bind_account_id: Option<String>,
    is_default: bool,
}

fn resolve_instance_launch_context(instance_id: &str) -> Result<InstanceLaunchContext, String> {
    if instance_id == DEFAULT_INSTANCE_ID {
        let default_dir = modules::cursor_instance::get_default_cursor_user_data_dir()?;
        let default_settings = modules::cursor_instance::load_default_settings()?;
        return Ok(InstanceLaunchContext {
            user_data_dir: default_dir.to_string_lossy().to_string(),
            bind_account_id: default_settings.bind_account_id,
            is_default: true,
        });
    }

    let store = modules::cursor_instance::load_instance_store()?;
    let instance = store
        .instances
        .into_iter()
        .find(|item| item.id == instance_id)
        .ok_or("实例不存在")?;
    Ok(InstanceLaunchContext {
        user_data_dir: instance.user_data_dir,
        bind_account_id: instance.bind_account_id,
        is_default: false,
    })
}

/// 账号总览 Play 与多开实例启动的唯一切号入口。
pub async fn start_cursor_instance_with_account_switch(
    instance_id: String,
    forced_account_id: Option<String>,
) -> Result<InstanceProfileView, String> {
    let _guard = cursor_switch_launch_lock().await.lock().await;
    modules::cursor_instance::ensure_cursor_launch_path_configured()?;
    let ctx = resolve_instance_launch_context(&instance_id)?;
    let trace = modules::cursor_switch_audit::CursorSwitchAuditCtx::new(&instance_id);
    modules::cursor_switch_audit::write_switch_start(&trace, forced_account_id.as_deref());

    let update_default_bind = ctx.is_default && forced_account_id.is_some();
    let auto_rotation = forced_account_id.is_none();
    let account_id = match forced_account_id {
        Some(id) => {
            if modules::cursor_account::load_account(&id).is_none() {
                return Err(format!("Cursor 账号不存在: {}", id));
            }
            id
        }
        None => {
            resolve_auto_switch_account_with_probe_retry(
                &instance_id,
                ctx.bind_account_id.as_deref(),
                &trace,
            )
            .await?
        }
    };

    let account = modules::cursor_account::load_account(&account_id)
        .ok_or_else(|| format!("Cursor 账号不存在: {}", account_id))?;

    let needs_auto_bind = ctx
        .bind_account_id
        .as_ref()
        .map(|value| value.trim().is_empty())
        .unwrap_or(true);
    let bind_changed = ctx
        .bind_account_id
        .as_deref()
        .map(|bind_id| bind_id != account_id.as_str())
        .unwrap_or(true);
    if needs_auto_bind || (auto_rotation && bind_changed) {
        if let Err(err) = persist_auto_bind_account(&instance_id, &account_id) {
            modules::logger::log_warn(&format!(
                "自动绑定 Cursor 实例账号失败: instance_id={}, account_id={}, error={}",
                instance_id, account_id, err
            ));
        }
    }

    let user_data_dir = ctx.user_data_dir.clone();
    let account_id_for_switch = account_id.clone();
    let inject_result = tokio::task::spawn_blocking(move || {
        prepare_instance_for_account(&user_data_dir, &account_id_for_switch)
    })
    .await
    .map_err(|err| format!("切号任务异常: {}", err))?;

    match inject_result {
        Ok(()) => modules::cursor_switch_audit::write_inject(&trace, &account, "ok", None),
        Err(err) => {
            modules::cursor_switch_audit::write_inject(&trace, &account, "err", Some(&err));
            return Err(err);
        }
    }

    let _ = modules::provider_current_state::set_current_account_id("cursor", Some(&account_id));

    if update_default_bind {
        if let Err(err) = modules::cursor_instance::update_default_settings(
            Some(Some(account_id.clone())),
            None,
            Some(false),
        ) {
            modules::logger::log_warn(&format!("更新 Cursor 默认实例绑定账号失败: {}", err));
        }
    }

    let launch_result = cursor_start_instance_prepared(instance_id, true).await;
    match &launch_result {
        Ok(_) => {
            modules::cursor_switch_audit::write_launch(&trace, &account, "ok", None);
            spawn_switch_post_audit(trace, account_id);
        }
        Err(err) => modules::cursor_switch_audit::write_launch(&trace, &account, "err", Some(err)),
    }
    launch_result
}

fn spawn_switch_post_audit(
    trace: modules::cursor_switch_audit::CursorSwitchAuditCtx,
    account_id: String,
) {
    std::thread::Builder::new()
        .name("cursor-switch-post-audit".into())
        .spawn(move || {
            let Ok(rt) = tokio::runtime::Runtime::new() else {
                return;
            };
            rt.block_on(async move {
                let Some(account) = modules::cursor_account::load_account(&account_id) else {
                    return;
                };
                match modules::cursor_account::probe_cursor_account_live_auth(&account_id).await {
                    Ok(()) => modules::cursor_switch_audit::write_probe_post(
                        &trace, &account, "ok", None,
                    ),
                    Err(err) => {
                        let outcome = if err.contains("会话") || err.contains("未认证") || err.contains("失效") {
                            "auth_fail"
                        } else {
                            "err"
                        };
                        modules::cursor_switch_audit::write_probe_post(
                            &trace, &account, outcome, Some(&err),
                        );
                    }
                }
                match modules::cursor_account::refresh_account_fast_async(&account_id).await {
                    Ok(refreshed) => {
                        let updated = refreshed.account;
                        let outcome =
                            if modules::cursor_account::account_has_auth_failure_marker(&updated) {
                                "auth_fail"
                            } else {
                                "ok"
                            };
                        modules::cursor_switch_audit::write_refresh_post(
                            &trace.switch_trace_id,
                            &trace.instance_id,
                            &updated,
                            outcome,
                            None,
                        );
                    }
                    Err(err) => {
                        if let Some(acct) = modules::cursor_account::load_account(&account_id) {
                            modules::cursor_switch_audit::write_refresh_post(
                                &trace.switch_trace_id,
                                &trace.instance_id,
                                &acct,
                                "err",
                                Some(&err),
                            );
                        }
                    }
                }
            });
        })
        .ok();
}

#[tauri::command]
pub async fn cursor_get_instance_defaults() -> Result<modules::instance::InstanceDefaults, String> {
    modules::cursor_instance::get_instance_defaults()
}

#[tauri::command]
pub async fn cursor_list_instances() -> Result<Vec<InstanceProfileView>, String> {
    let store = modules::cursor_instance::load_instance_store()?;
    let default_dir = modules::cursor_instance::get_default_cursor_user_data_dir()?;
    let default_dir_str = default_dir.to_string_lossy().to_string();

    let default_settings = store.default_settings.clone();
    let process_entries = modules::cursor_instance::collect_cursor_process_entries();

    let mut result: Vec<InstanceProfileView> = store
        .instances
        .into_iter()
        .map(|instance| {
            let resolved_pid = modules::cursor_instance::resolve_cursor_pid_from_entries(
                instance.last_pid,
                Some(&instance.user_data_dir),
                &process_entries,
            );
            let running = resolved_pid.is_some();
            let initialized = is_profile_initialized(&instance.user_data_dir);
            let mut view = InstanceProfileView::from_profile(instance, running, initialized);
            view.last_pid = resolved_pid;
            view
        })
        .collect();

    let default_pid = modules::cursor_instance::resolve_cursor_pid_from_entries(
        default_settings.last_pid,
        None,
        &process_entries,
    );
    let default_running = default_pid.is_some();
    result.insert(
        0,
        InstanceProfileView {
            id: DEFAULT_INSTANCE_ID.to_string(),
            name: String::new(),
            user_data_dir: default_dir_str,
            working_dir: None,
            extra_args: default_settings.extra_args.clone(),
            bind_account_id: default_settings.bind_account_id.clone(),
            created_at: 0,
            last_launched_at: None,
            last_pid: default_pid,
            running: default_running,
            initialized: is_profile_initialized(&default_dir.to_string_lossy()),
            is_default: true,
            follow_local_account: false,
        },
    );

    Ok(result)
}

#[tauri::command]
pub async fn cursor_create_instance(
    name: String,
    user_data_dir: String,
    extra_args: Option<String>,
    bind_account_id: Option<String>,
    copy_source_instance_id: Option<String>,
    init_mode: Option<String>,
) -> Result<InstanceProfileView, String> {
    let instance = modules::cursor_instance::create_instance(
        modules::cursor_instance::CreateInstanceParams {
            working_dir: None,
            name,
            user_data_dir,
            extra_args: extra_args.unwrap_or_default(),
            bind_account_id,
            copy_source_instance_id,
            init_mode,
        },
    )?;

    let initialized = is_profile_initialized(&instance.user_data_dir);
    Ok(InstanceProfileView::from_profile(
        instance,
        false,
        initialized,
    ))
}

#[tauri::command]
pub async fn cursor_update_instance(
    instance_id: String,
    name: Option<String>,
    extra_args: Option<String>,
    bind_account_id: Option<Option<String>>,
    follow_local_account: Option<bool>,
) -> Result<InstanceProfileView, String> {
    if instance_id == DEFAULT_INSTANCE_ID {
        let default_dir = modules::cursor_instance::get_default_cursor_user_data_dir()?;
        let default_dir_str = default_dir.to_string_lossy().to_string();
        let updated = modules::cursor_instance::update_default_settings(
            bind_account_id,
            extra_args,
            follow_local_account,
        )?;

        let running = updated
            .last_pid
            .and_then(|pid| modules::cursor_instance::resolve_cursor_pid(Some(pid), None))
            .is_some();

        return Ok(InstanceProfileView {
            id: DEFAULT_INSTANCE_ID.to_string(),
            name: String::new(),
            user_data_dir: default_dir_str,
            working_dir: None,
            extra_args: updated.extra_args,
            bind_account_id: updated.bind_account_id,
            created_at: 0,
            last_launched_at: None,
            last_pid: updated.last_pid,
            running,
            initialized: is_profile_initialized(&default_dir.to_string_lossy()),
            is_default: true,
            follow_local_account: false,
        });
    }

    let wants_bind = bind_account_id
        .as_ref()
        .and_then(|next| next.as_ref())
        .is_some();
    if wants_bind {
        let store = modules::cursor_instance::load_instance_store()?;
        if let Some(target) = store.instances.iter().find(|item| item.id == instance_id) {
            if !is_profile_initialized(&target.user_data_dir) {
                return Err(
                    "INSTANCE_NOT_INITIALIZED:请先启动一次实例创建数据后，再进行账号绑定"
                        .to_string(),
                );
            }
        }
    }

    let instance = modules::cursor_instance::update_instance(
        modules::cursor_instance::UpdateInstanceParams {
            working_dir: None,
            instance_id,
            name,
            extra_args,
            bind_account_id,
        },
    )?;

    let running = instance
        .last_pid
        .and_then(|pid| {
            modules::cursor_instance::resolve_cursor_pid(Some(pid), Some(&instance.user_data_dir))
        })
        .is_some();
    let initialized = is_profile_initialized(&instance.user_data_dir);
    Ok(InstanceProfileView::from_profile(
        instance,
        running,
        initialized,
    ))
}

#[tauri::command]
pub async fn cursor_delete_instance(instance_id: String) -> Result<(), String> {
    if instance_id == DEFAULT_INSTANCE_ID {
        return Err("默认实例不可删除".to_string());
    }
    modules::cursor_instance::delete_instance(&instance_id)
}

#[tauri::command]
pub async fn cursor_start_instance(instance_id: String) -> Result<InstanceProfileView, String> {
    modules::logger::log_info(&format!("开始启动 Cursor 实例: {}", instance_id));
    start_cursor_instance_with_account_switch(instance_id, None).await
}

pub async fn cursor_start_instance_prepared(
    instance_id: String,
    skip_prelaunch_close: bool,
) -> Result<InstanceProfileView, String> {
    // 对齐无忧传统切号：换号步骤完成后等待 1.5s 再启动 Cursor。
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

    if instance_id == DEFAULT_INSTANCE_ID {
        let default_dir = modules::cursor_instance::get_default_cursor_user_data_dir()?;
        let default_dir_str = default_dir.to_string_lossy().to_string();
        let default_settings = modules::cursor_instance::load_default_settings()?;

        if !skip_prelaunch_close {
            modules::logger::log_info(&format!(
                "[Cursor Instance Start] scoped default close begin profile={}, last_pid={:?}",
                default_dir_str, default_settings.last_pid
            ));
            modules::cursor_instance::close_cursor_profile_scoped(&default_dir_str, true, 20)?;
            let _ = modules::cursor_instance::update_default_pid(None)?;
            modules::logger::log_info("[Cursor Instance Start] scoped default close done");
        } else {
            modules::logger::log_info("[Cursor Switch] 切号后跳过二次 close，直接启动默认实例");
        }

        let extra_args = modules::process::parse_extra_args(&default_settings.extra_args);
        let workspace = modules::cursor_instance::resolve_launch_workspace(
            default_settings.working_dir.as_deref(),
            Some(&default_dir_str),
        );
        let use_new_window =
            modules::cursor_instance::should_use_new_window_for_profile(&default_dir_str);
        let pid = modules::cursor_instance::start_cursor_with_args_with_new_window(
            &default_dir_str,
            &extra_args,
            use_new_window,
            workspace.as_deref(),
        )?;
        let pid_for_store = pid;
        tokio::task::spawn_blocking(move || {
            let _ = modules::cursor_instance::update_default_pid(Some(pid_for_store));
        });

        let running = modules::cursor_instance::resolve_cursor_pid(Some(pid), None).is_some();
        return Ok(InstanceProfileView {
            id: DEFAULT_INSTANCE_ID.to_string(),
            name: String::new(),
            user_data_dir: default_dir_str,
            working_dir: None,
            extra_args: default_settings.extra_args,
            bind_account_id: default_settings.bind_account_id,
            created_at: 0,
            last_launched_at: None,
            last_pid: Some(pid),
            running,
            initialized: is_profile_initialized(&default_dir.to_string_lossy()),
            is_default: true,
            follow_local_account: false,
        });
    }

    let store = modules::cursor_instance::load_instance_store()?;
    let instance = store
        .instances
        .into_iter()
        .find(|item| item.id == instance_id)
        .ok_or("实例不存在")?;

    modules::logger::log_info(&format!(
        "[Cursor Instance Start] begin id={}, profile={}, last_pid={:?}",
        instance.id, instance.user_data_dir, instance.last_pid
    ));

    if !skip_prelaunch_close {
        if let Some(pid) = modules::cursor_instance::resolve_cursor_pid(
            instance.last_pid,
            Some(&instance.user_data_dir),
        ) {
            modules::logger::log_info(&format!(
                "[Cursor Instance Start] closing resolved last_pid={} for id={}",
                pid, instance.id
            ));
            modules::process::close_pid(pid, 20)?;
            let _ = modules::cursor_instance::update_instance_pid(&instance.id, None)?;
        }

        modules::logger::log_info(&format!(
            "[Cursor Instance Start] strict profile close begin id={}, profile={}",
            instance.id, instance.user_data_dir
        ));
        modules::cursor_instance::close_cursor_profile_strict(&instance.user_data_dir, 20)?;
        modules::logger::log_info(&format!(
            "[Cursor Instance Start] strict profile close done id={}",
            instance.id
        ));
    } else {
        modules::logger::log_info(&format!(
            "[Cursor Switch] 切号后跳过二次 close，直接启动实例: {}",
            instance.id
        ));
    }

    let extra_args = modules::process::parse_extra_args(&instance.extra_args);
    let workspace = modules::cursor_instance::resolve_launch_workspace(
        instance.working_dir.as_deref(),
        Some(&instance.user_data_dir),
    );
    let use_new_window =
        modules::cursor_instance::should_use_new_window_for_profile(&instance.user_data_dir);
    modules::logger::log_info(&format!(
        "[Cursor Instance Start] launching id={}, use_new_window={}, workspace={}",
        instance.id,
        use_new_window,
        workspace
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "-".to_string())
    ));
    let pid = modules::cursor_instance::start_cursor_with_args_with_new_window(
        &instance.user_data_dir,
        &extra_args,
        use_new_window,
        workspace.as_deref(),
    )?;
    modules::logger::log_info(&format!(
        "[Cursor Instance Start] launch returned id={}, pid={}",
        instance.id, pid
    ));
    let instance_id_for_store = instance.id.clone();
    let pid_for_store = pid;
    tokio::task::spawn_blocking(move || {
        let _ = modules::cursor_instance::update_instance_after_start(
            &instance_id_for_store,
            pid_for_store,
        );
    });

    let updated = instance;
    let running =
        modules::cursor_instance::resolve_cursor_pid(Some(pid), Some(&updated.user_data_dir))
            .is_some();
    let initialized = is_profile_initialized(&updated.user_data_dir);
    let mut view = InstanceProfileView::from_profile(updated, running, initialized);
    view.last_pid = Some(pid);
    view.last_launched_at = Some(chrono::Utc::now().timestamp_millis());
    Ok(view)
}

#[tauri::command]
pub async fn cursor_stop_instance(instance_id: String) -> Result<InstanceProfileView, String> {
    if instance_id == DEFAULT_INSTANCE_ID {
        let default_dir = modules::cursor_instance::get_default_cursor_user_data_dir()?;
        let default_dir_str = default_dir.to_string_lossy().to_string();
        let default_settings = modules::cursor_instance::load_default_settings()?;

        if let Some(pid) =
            modules::cursor_instance::resolve_cursor_pid(default_settings.last_pid, None)
        {
            modules::process::close_pid(pid, 20)?;
        }

        let updated_settings = modules::cursor_instance::update_default_pid(None)?;
        let running = updated_settings
            .last_pid
            .and_then(|pid| modules::cursor_instance::resolve_cursor_pid(Some(pid), None))
            .is_some();

        return Ok(InstanceProfileView {
            id: DEFAULT_INSTANCE_ID.to_string(),
            name: String::new(),
            user_data_dir: default_dir_str,
            working_dir: None,
            extra_args: default_settings.extra_args,
            bind_account_id: default_settings.bind_account_id,
            created_at: 0,
            last_launched_at: None,
            last_pid: None,
            running,
            initialized: is_profile_initialized(&default_dir.to_string_lossy()),
            is_default: true,
            follow_local_account: false,
        });
    }

    let store = modules::cursor_instance::load_instance_store()?;
    let instance = store
        .instances
        .into_iter()
        .find(|item| item.id == instance_id)
        .ok_or("实例不存在")?;

    if let Some(pid) = modules::cursor_instance::resolve_cursor_pid(
        instance.last_pid,
        Some(&instance.user_data_dir),
    ) {
        modules::process::close_pid(pid, 20)?;
    }

    let updated = modules::cursor_instance::update_instance_pid(&instance.id, None)?;
    let initialized = is_profile_initialized(&updated.user_data_dir);
    Ok(InstanceProfileView::from_profile(
        updated,
        false,
        initialized,
    ))
}

#[tauri::command]
pub async fn cursor_open_instance_window(instance_id: String) -> Result<(), String> {
    if instance_id == DEFAULT_INSTANCE_ID {
        let default_settings: DefaultInstanceSettings =
            modules::cursor_instance::load_default_settings()?;
        modules::cursor_instance::focus_cursor_instance(default_settings.last_pid, None)
            .map_err(|err| format!("定位 Cursor 默认实例窗口失败: {}", err))?;
        return Ok(());
    }

    let store = modules::cursor_instance::load_instance_store()?;
    let instance = store
        .instances
        .into_iter()
        .find(|item| item.id == instance_id)
        .ok_or("实例不存在")?;

    modules::cursor_instance::focus_cursor_instance(
        instance.last_pid,
        Some(&instance.user_data_dir),
    )
    .map_err(|err| {
        format!(
            "定位 Cursor 实例窗口失败: instance_id={}, err={}",
            instance.id, err
        )
    })?;

    Ok(())
}

#[tauri::command]
pub async fn cursor_close_all_instances() -> Result<(), String> {
    let store = modules::cursor_instance::load_instance_store()?;
    let default_dir = modules::cursor_instance::get_default_cursor_user_data_dir()?;

    let mut target_dirs: Vec<String> = Vec::new();
    target_dirs.push(default_dir.to_string_lossy().to_string());
    for instance in &store.instances {
        let dir = instance.user_data_dir.trim();
        if !dir.is_empty() {
            target_dirs.push(dir.to_string());
        }
    }

    modules::cursor_instance::close_cursor(&target_dirs, 20)?;
    let _ = modules::cursor_instance::clear_all_pids();
    Ok(())
}
