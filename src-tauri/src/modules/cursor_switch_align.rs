//! 无忧小助手 `cursor:accounts:switch` 传统切号路径对齐（main-FXxcqbQA.js `i()` @20973）：
//! close → switchTokensInDb → resetStorageJsonIds → resetMachineIdFile → patchCursorMachineId
//! → (Win) resetWindowsMachineGuid → 启动前等待 1.5s
//!
//! workbench auth bridge（Xh）仅用于「无感换号 / seamless」安装，不在传统切号里调用。

use std::fs;
use std::path::{Path, PathBuf};

use rand::Rng;
use regex::Regex;
use uuid::Uuid;

use crate::modules::{atomic_write, logger};

const AUTH_BRIDGE_MARKER: &str = "jzzcg-auth-bridge";
const AUTH_BRIDGE_SNIPPET: &str = r#"/*jzzcg-auth-bridge*/globalThis.__cursorAuthBridge={switchAccount:(at,rt,em,st)=>{this.storeAccessRefreshToken(at,rt);em&&(this.storageService.store("cursorAuth/cachedEmail",em,-1,1),this.storageService.store("cursorAuth/cachedSignUpType",st||"Auth_0",-1,1));this.refreshMembership()}},/*jzzcg-auth-bridge-end*/"#;

pub fn resolve_main_js(cursor_exe: &Path) -> Option<PathBuf> {
    let root = cursor_exe.parent()?;
    let main_js = root.join("resources/app/out/main.js");
    if main_js.is_file() {
        Some(main_js)
    } else {
        None
    }
}

pub fn resolve_workbench_main_js(cursor_exe: &Path) -> Option<PathBuf> {
    let root = cursor_exe.parent()?;
    let candidates = [
        root.join("resources/app/out/vs/workbench/workbench.desktop.main.js"),
        root.join("resources/app/out/vs/code/electron-sandbox/workbench/workbench.js"),
    ];
    candidates.into_iter().find(|p| p.is_file())
}

fn cursor_backups_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "无法定位用户主目录".to_string())?;
    let dir = home.join(".cursor-backups");
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| format!("创建 .cursor-backups 失败: {}", e))?;
    }
    Ok(dir)
}

fn random_mac_address() -> String {
    let hex: Vec<char> = "0123456789ABCDEF".chars().collect();
    let mut rng = rand::thread_rng();
    loop {
        let mut parts = Vec::with_capacity(6);
        for _ in 0..6 {
            let a = hex[rng.gen_range(0..16)];
            let b = hex[rng.gen_range(0..16)];
            parts.push(format!("{a}{b}"));
        }
        let mac = parts.join(":");
        if mac != "00:00:00:00:00:00" && mac != "FF:FF:FF:FF:FF:FF" {
            return mac;
        }
    }
}

/// 对齐无忧 `patchCursorMachineId`（Yh）：patch `resources/app/out/main.js`。
pub fn patch_cursor_machine_id(cursor_exe: &Path) -> Result<(), String> {
    let main_js = resolve_main_js(cursor_exe)
        .ok_or_else(|| "未找到 Cursor main.js".to_string())?;

    let content = fs::read_to_string(&main_js)
        .map_err(|e| format!("读取 main.js 失败({}): {}", main_js.display(), e))?;

    if content.contains("/*csp1*/") || content.contains("/*csp2*/") {
        logger::log_info("[Cursor Switch] main.js 已 patch，跳过");
        return Ok(());
    }

    let already_patched = Regex::new(r"async getMachineId\(\)\{return [a-zA-Z_$][a-zA-Z0-9_$]*\}")
        .map_err(|e| e.to_string())?
        .is_match(&content)
        && !Regex::new(r"async getMachineId\(\)\{return [^??]+\?\?")
            .map_err(|e| e.to_string())?
            .is_match(&content);
    if already_patched {
        logger::log_info("[Cursor Switch] main.js getMachineId 已处理，跳过");
        return Ok(());
    }

    let backup_dir = cursor_backups_dir()?;
    let backup = backup_dir.join("main.js.bak");
    if !backup.exists() {
        fs::copy(&main_js, &backup).map_err(|e| format!("备份 main.js 失败: {}", e))?;
        logger::log_info(&format!(
            "[Cursor Switch] 已备份 main.js: {}",
            backup.display()
        ));
    }

    let mut patched = content.clone();
    let mut touched = Vec::<&str>::new();
    let csp1_id = Uuid::new_v4().to_string();
    let mac = random_mac_address();
    let device_id = Uuid::new_v4().to_string();

    if let Ok(re) = Regex::new(r"=.{0,50}timeout.{0,10}5e3.*?,") {
        if re.is_match(&patched) {
            patched = re
                .replace(&patched, format!(r#"=/*csp1*/"{csp1_id}"/*1csp*/,"#))
                .into_owned();
            touched.push("MachineId");
        }
    }

    if let Ok(re) =
        Regex::new(r"(function .{0,50}\{).{0,300}Unable to retrieve mac address.*?(\})")
    {
        if re.is_match(&patched) {
            patched = re
                .replace(&patched, format!(r#"$1return/*csp2*/"{mac}"/*2csp*/;$2"#))
                .into_owned();
            touched.push("MacAddress");
        }
    }

    if let Ok(re) = Regex::new(r"return.{0,50}vscode/deviceid.*?getDeviceId\(\)") {
        if re.is_match(&patched) {
            patched = re
                .replace(&patched, format!(r#"return/*csp4*/"{device_id}"/*4csp*/"#))
                .into_owned();
            touched.push("DeviceId");
        }
    }

    for (pattern, replacement, name) in [
        (
            r"async getMachineId\(\)\{return[^}]*?\?\?([^}]+)\}",
            "async getMachineId(){return $1}",
            "getMachineId",
        ),
        (
            r"async getMacMachineId\(\)\{return[^}]*?\?\?([^}]+)\}",
            "async getMacMachineId(){return $1}",
            "getMacMachineId",
        ),
    ] {
        if let Ok(re) = Regex::new(pattern) {
            let next = re.replace_all(&patched, replacement).into_owned();
            if next != patched {
                touched.push(name);
                patched = next;
            }
        }
    }

    if touched.is_empty() {
        if patched.contains("async getMachineId(){return this._telemetryService.machineId}") {
            logger::log_info("[Cursor Switch] 新版 Cursor 无需 patch main.js");
            return Ok(());
        }
        logger::log_info("[Cursor Switch] main.js 未找到可 patch 模式，跳过（不影响换号）");
        return Ok(());
    }

    atomic_write::write_string_atomic(&main_js, &patched)
        .map_err(|e| format!("写入 main.js patch 失败: {}", e))?;
    logger::log_info(&format!(
        "[Cursor Switch] main.js patch 完成: {}",
        touched.join(", ")
    ));
    Ok(())
}

/// 无感换号 / seamless 安装用，传统切号不调用。
pub fn patch_cursor_workbench_auth_bridge(cursor_exe: &Path) -> Result<(), String> {
    let workbench = resolve_workbench_main_js(cursor_exe)
        .ok_or_else(|| "未找到 Cursor workbench.desktop.main.js".to_string())?;

    let content = fs::read_to_string(&workbench)
        .map_err(|e| format!("读取 workbench 失败({}): {}", workbench.display(), e))?;

    if content.contains(AUTH_BRIDGE_MARKER) {
        logger::log_info("[Cursor Switch] workbench auth bridge 已存在，跳过 patch");
        return Ok(());
    }

    let anchor = "this.logout=";
    let pos = content
        .find(anchor)
        .ok_or_else(|| "未找到 workbench 注入点 this.logout=，可能 Cursor 版本不兼容".to_string())?;

    let backup_dir = cursor_backups_dir()?;
    let backup = backup_dir.join("workbench.desktop.main.js.bak");
    if !backup.exists() {
        fs::copy(&workbench, &backup).map_err(|e| format!("备份 workbench 失败: {}", e))?;
        logger::log_info(&format!(
            "[Cursor Switch] 已备份 workbench: {}",
            backup.display()
        ));
    }

    let patched = format!(
        "{}{}{}",
        &content[..pos],
        AUTH_BRIDGE_SNIPPET,
        &content[pos..]
    );
    atomic_write::write_string_atomic(&workbench, &patched)
        .map_err(|e| format!("写入 workbench patch 失败: {}", e))?;
    logger::log_info("[Cursor Switch] workbench auth bridge patch 完成");
    Ok(())
}

#[cfg(target_os = "windows")]
fn backup_windows_machine_guid_once(old_guid: &str) -> Result<(), String> {
    if old_guid.trim().is_empty() {
        return Ok(());
    }
    let home = dirs::home_dir().ok_or_else(|| "无法定位用户主目录".to_string())?;
    let backup_dir = home.join("MachineGuid_Backups");
    if !backup_dir.exists() {
        fs::create_dir_all(&backup_dir)
            .map_err(|e| format!("创建 MachineGuid_Backups 失败: {}", e))?;
    }

    let has_backup = fs::read_dir(&backup_dir)
        .map_err(|e| format!("读取 MachineGuid_Backups 失败: {}", e))?
        .filter_map(Result::ok)
        .any(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.starts_with("MachineGuid_") && name.ends_with(".txt")
        });
    if has_backup {
        return Ok(());
    }

    let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H-%M-%S").to_string();
    let backup_file = backup_dir.join(format!("MachineGuid_{timestamp}.txt"));
    atomic_write::write_string_atomic(&backup_file, old_guid)
        .map_err(|e| format!("备份 MachineGuid 失败: {}", e))?;
    logger::log_info(&format!(
        "[Cursor Switch] 原始 MachineGuid 已备份: {}",
        backup_file.display()
    ));
    Ok(())
}

/// 对齐无忧 `resetWindowsMachineGuid`（Nc）：备份 + 写入新 GUID（进程内 API，不 spawn reg.exe）。
#[cfg(target_os = "windows")]
pub fn reset_windows_machine_guid() -> Result<(), String> {
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE};
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let key = hklm
        .open_subkey_with_flags(r"SOFTWARE\Microsoft\Cryptography", KEY_READ | KEY_WRITE)
        .map_err(|e| format!("无法打开注册表 Cryptography（可能需要管理员权限）: {}", e))?;

    let old: String = key
        .get_value("MachineGuid")
        .map_err(|e| format!("无法读取 MachineGuid: {}", e))?;

    backup_windows_machine_guid_once(&old)?;

    let new_guid = Uuid::new_v4().to_string().to_uppercase();
    key.set_value("MachineGuid", &new_guid)
        .map_err(|e| format!("重置 MachineGuid 失败（需管理员）: {}", e))?;

    let verify: String = key
        .get_value("MachineGuid")
        .map_err(|e| format!("验证 MachineGuid 失败: {}", e))?;
    if verify != new_guid {
        return Err(format!(
            "验证失败：当前值 {verify} 与预期 {new_guid} 不匹配"
        ));
    }

    logger::log_info(&format!(
        "[Cursor Switch] Windows MachineGuid 已重置 (old: {old}, new: {new_guid})"
    ));
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn reset_windows_machine_guid() -> Result<(), String> {
    Ok(())
}

/// 无忧传统切号路径：`patchCursorMachineId` → `resetWindowsMachineGuid`（失败可跳过，与无忧 Nc 一致）。
pub fn apply_nirvana_traditional_switch_patches(cursor_exe: &Path) {
    if let Err(err) = patch_cursor_machine_id(cursor_exe) {
        logger::log_warn(&format!("[Cursor Switch] main.js patch 跳过: {}", err));
    }
    if let Err(err) = reset_windows_machine_guid() {
        logger::log_warn(&format!("[Cursor Switch] MachineGuid 重置跳过: {}", err));
    }
}
