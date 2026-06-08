//! Cursor 切号前置步骤（关进程后的补丁与系统指纹），与主流续杯工具流程一致：
//! close → machineId/storage/vscdb 指纹 → (Win) MachineGuid → workbench auth bridge patch → 写 token → 启动

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::modules::{atomic_write, logger};

const AUTH_BRIDGE_MARKER: &str = "jzzcg-auth-bridge";
const AUTH_BRIDGE_SNIPPET: &str = r#"/*jzzcg-auth-bridge*/globalThis.__cursorAuthBridge={switchAccount:(at,rt,em,st)=>{this.storeAccessRefreshToken(at,rt);em&&(this.storageService.store("cursorAuth/cachedEmail",em,-1,1),this.storageService.store("cursorAuth/cachedSignUpType",st||"Auth_0",-1,1));this.refreshMembership()}},/*jzzcg-auth-bridge-end*/"#;

pub fn resolve_workbench_main_js(cursor_exe: &Path) -> Option<PathBuf> {
    let root = cursor_exe.parent()?;
    let candidates = [
        root.join("resources/app/out/vs/workbench/workbench.desktop.main.js"),
        root.join("resources/app/out/vs/code/electron-sandbox/workbench/workbench.js"),
    ];
    candidates.into_iter().find(|p| p.is_file())
}

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

    let backup = workbench.with_extension("js.bak");
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
pub fn reset_windows_machine_guid() -> Result<(), String> {
    use uuid::Uuid;

    let output = Command::new("reg")
        .args([
            "query",
            r"HKLM\SOFTWARE\Microsoft\Cryptography",
            "/v",
            "MachineGuid",
        ])
        .output()
        .map_err(|e| format!("查询 MachineGuid 失败: {}", e))?;

    if !output.status.success() {
        return Err("无法读取注册表 MachineGuid（可能需要管理员权限）".to_string());
    }

    let old = String::from_utf8_lossy(&output.stdout);
    let new_guid = Uuid::new_v4().to_string();
    let set = Command::new("reg")
        .args([
            "add",
            r"HKLM\SOFTWARE\Microsoft\Cryptography",
            "/v",
            "MachineGuid",
            "/t",
            "REG_SZ",
            "/d",
            &new_guid,
            "/f",
        ])
        .output()
        .map_err(|e| format!("写入 MachineGuid 失败: {}", e))?;

    if !set.status.success() {
        let stderr = String::from_utf8_lossy(&set.stderr);
        return Err(format!(
            "重置 MachineGuid 失败（需管理员）: {}",
            stderr.trim()
        ));
    }

    logger::log_info(&format!(
        "[Cursor Switch] Windows MachineGuid 已重置 (old snippet: {})",
        old.lines().find(|l| l.contains("MachineGuid")).unwrap_or("")
    ));
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn reset_windows_machine_guid() -> Result<(), String> {
    Ok(())
}

pub fn apply_pre_inject_cursor_patches(cursor_exe: &Path) {
    if let Err(err) = reset_windows_machine_guid() {
        logger::log_warn(&format!("[Cursor Switch] MachineGuid 重置跳过: {}", err));
    }
    if let Err(err) = patch_cursor_workbench_auth_bridge(cursor_exe) {
        logger::log_warn(&format!("[Cursor Switch] workbench patch 跳过: {}", err));
    }
}
