//! 仅多开：总控优先层。不碰默认换号、不删管家注入。
//! 多开 workbench 在管家 i2 前插入一层：get-token 若本窗已有令牌则改回本窗，避免管家全局号盖掉多开。

use std::fs;
use std::path::{Path, PathBuf};

use crate::modules::atomic_write;
use crate::modules::logger;

const SHIM_START: &str = "/*cockpit-pri-s*/";
const SHIM_END: &str = "/*cockpit-pri-e*/";
const I2_MARK: &str = "/*i2s*/";

/// 优先读总控热态 `__cockpitSeamlessAuth`（CDP 注入），再退回 window.store。
/// 使管家 get-token 盖写后仍回到本多开实例刚切的号。
const MULTI_PRIORITY_SHIM: &str = r#"/*cockpit-pri-s*/(function(){try{if(window.__cockpitPri)return;window.__cockpitPri=1;var _of=window.fetch.bind(window);window.fetch=function(input,init){var u=typeof input==="string"?input:(input&&typeof input.url==="string"?input.url:"");var p=_of(input,init);if(u.indexOf("/api/get-token")<0)return p;return p.then(function(r){if(!r||!r.ok)return r;return r.clone().json().then(function(data){try{var a=window.__cockpitSeamlessAuth;var t=(a&&a.accessToken)||(window.store&&window.store.get("cursorAuth/accessToken",-1));var e=(a&&a.email)||(window.store&&window.store.get("cursorAuth/cachedEmail",-1));var rt=(a&&a.refreshToken)||(window.store&&window.store.get("cursorAuth/refreshToken",-1));if(t&&t!==""&&t!=="undefined"){data.accessToken=t;if(rt&&rt!==""&&rt!=="undefined")data.refreshToken=rt;if(e&&e!==""&&e!=="undefined")data.email=e;if(a&&a.machineIds)data.machineIds=a.machineIds;data.is_new=false;}}catch(_e){}return new Response(JSON.stringify(data),{status:200,headers:{"Content-Type":"application/json"}});}).catch(function(){return r;});});};}catch(_e){}})();/*cockpit-pri-e*/"#;

fn workbench_main_js(install_root: &Path) -> PathBuf {
    install_root
        .join("resources")
        .join("app")
        .join("out")
        .join("vs")
        .join("workbench")
        .join("workbench.desktop.main.js")
}

/// 仅多开安装：叠总控优先层；保留管家 `/*i2s*/`。禁止用于默认安装。
pub fn ensure_multi_install_cockpit_priority_shim(cursor_exe: &Path) -> Result<bool, String> {
    let install_root = cursor_exe
        .parent()
        .ok_or_else(|| "多开 Cursor 路径无父目录".to_string())?;
    // 硬闸：禁止写到默认 Local/Program Files 安装
    let root_s = install_root.to_string_lossy().to_lowercase();
    if root_s.contains(r"\appdata\local\programs\cursor")
        || root_s.contains(r"\program files\cursor")
        || root_s.contains(r"\program files (x86)\cursor")
    {
        return Err("禁止对默认 Cursor 安装写入多开优先层".to_string());
    }
    let wb = workbench_main_js(install_root);
    if !wb.exists() {
        return Err(format!("多开 workbench 不存在: {}", wb.display()));
    }
    let mut text = fs::read_to_string(&wb).map_err(|e| format!("读 workbench 失败: {}", e))?;
    let had_xubei = text.contains("__csSeamlessRunning") || text.contains(I2_MARK);
    if text.contains(SHIM_START) && text.contains(SHIM_END) {
        if let Some(start) = text.find(SHIM_START) {
            if let Some(end_rel) = text[start..].find(SHIM_END) {
                let end = start + end_rel + SHIM_END.len();
                if &text[start..end] == MULTI_PRIORITY_SHIM {
                    return Ok(false);
                }
                text.replace_range(start..end, MULTI_PRIORITY_SHIM);
                atomic_write::write_string_atomic(&wb, &text)
                    .map_err(|e| format!("更新多开优先层失败: {}", e))?;
                if had_xubei
                    && !(text.contains("__csSeamlessRunning") || text.contains(I2_MARK))
                {
                    return Err("写入优先层后管家注入标记消失".to_string());
                }
                logger::log_info("[Cursor Priority] 已更新多开总控优先层（未动默认）");
                return Ok(true);
            }
        }
    }
    if let Some(idx) = text.find(I2_MARK) {
        text.insert_str(idx, MULTI_PRIORITY_SHIM);
    } else {
        text.push_str(MULTI_PRIORITY_SHIM);
    }
    if had_xubei && !(text.contains("__csSeamlessRunning") || text.contains(I2_MARK)) {
        return Err("写入优先层后管家注入标记消失".to_string());
    }
    atomic_write::write_string_atomic(&wb, &text)
        .map_err(|e| format!("写入多开优先层失败: {}", e))?;
    logger::log_info(&format!(
        "[Cursor Priority] 已在多开安装写入总控优先层: {}",
        wb.display()
    ));
    Ok(true)
}

pub fn multi_workbench_has_priority_shim(cursor_exe: &Path) -> bool {
    let Some(root) = cursor_exe.parent() else {
        return false;
    };
    let Ok(text) = fs::read_to_string(workbench_main_js(root)) else {
        return false;
    };
    text.contains(SHIM_START) && text.contains(SHIM_END)
}

pub fn multi_workbench_still_has_xubei(cursor_exe: &Path) -> bool {
    let Some(root) = cursor_exe.parent() else {
        return false;
    };
    let Ok(text) = fs::read_to_string(workbench_main_js(root)) else {
        return false;
    };
    text.contains("__csSeamlessRunning") || text.contains(I2_MARK)
}
