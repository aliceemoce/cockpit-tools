use std::fs;
use std::path::PathBuf;

use crate::modules::{account, logger};

const BACKUP_ROOT_DIR: &str = "account_backups";

fn data_dir() -> Result<PathBuf, String> {
    account::get_data_dir()
}

fn ensure_dir(path: &PathBuf) -> Result<(), String> {
    if !path.exists() {
        fs::create_dir_all(path).map_err(|e| format!("创建备份目录失败: {} ({})", path.display(), e))?;
    }
    Ok(())
}

fn provider_accounts_dir(provider: &str) -> Result<PathBuf, String> {
    Ok(data_dir()?.join(format!("{}_accounts", provider)))
}

fn provider_index_path(provider: &str) -> Result<PathBuf, String> {
    Ok(data_dir()?.join(format!("{}_accounts.json", provider)))
}

fn backup_provider_accounts_dir(provider: &str) -> Result<PathBuf, String> {
    Ok(data_dir()?
        .join(BACKUP_ROOT_DIR)
        .join(format!("{}_accounts", provider)))
}

fn backup_provider_index_path(provider: &str) -> Result<PathBuf, String> {
    Ok(data_dir()?
        .join(BACKUP_ROOT_DIR)
        .join(format!("{}_accounts_index", provider))
        .join(format!("{}_accounts.json", provider)))
}

pub fn mirror_provider_account_file(provider: &str, account_id: &str) -> Result<(), String> {
    let src = provider_accounts_dir(provider)?.join(format!("{}.json", account_id));
    if !src.exists() {
        return Ok(());
    }
    let dst_dir = backup_provider_accounts_dir(provider)?;
    ensure_dir(&dst_dir)?;
    let dst = dst_dir.join(format!("{}.json", account_id));
    fs::copy(&src, &dst).map_err(|e| {
        format!(
            "镜像备份账号文件失败: provider={}, id={}, src={}, dst={}, error={}",
            provider,
            account_id,
            src.display(),
            dst.display(),
            e
        )
    })?;
    Ok(())
}

pub fn mirror_provider_index_file(provider: &str) -> Result<(), String> {
    let src = provider_index_path(provider)?;
    if !src.exists() {
        return Ok(());
    }
    let dst = backup_provider_index_path(provider)?;
    let dst_parent = dst
        .parent()
        .ok_or_else(|| "索引备份路径无父目录".to_string())?
        .to_path_buf();
    ensure_dir(&dst_parent)?;
    fs::copy(&src, &dst).map_err(|e| {
        format!(
            "镜像备份索引文件失败: provider={}, src={}, dst={}, error={}",
            provider,
            src.display(),
            dst.display(),
            e
        )
    })?;
    Ok(())
}

pub fn mirror_shared_state_file(file_name: &str) -> Result<(), String> {
    let src = data_dir()?.join(file_name);
    if !src.exists() {
        return Ok(());
    }
    let backup_dir = data_dir()?.join(BACKUP_ROOT_DIR).join("shared_state");
    ensure_dir(&backup_dir)?;
    let dst = backup_dir.join(file_name);
    fs::copy(&src, &dst).map_err(|e| {
        format!(
            "镜像备份共享状态文件失败: file={}, src={}, dst={}, error={}",
            file_name,
            src.display(),
            dst.display(),
            e
        )
    })?;
    Ok(())
}

pub fn try_mirror_provider_account_file(provider: &str, account_id: &str) {
    if let Err(err) = mirror_provider_account_file(provider, account_id) {
        logger::log_warn(&format!("[BackupMirror] {}", err));
    }
}

pub fn try_mirror_provider_index_file(provider: &str) {
    if let Err(err) = mirror_provider_index_file(provider) {
        logger::log_warn(&format!("[BackupMirror] {}", err));
    }
}

pub fn try_mirror_shared_state_file(file_name: &str) {
    if let Err(err) = mirror_shared_state_file(file_name) {
        logger::log_warn(&format!("[BackupMirror] {}", err));
    }
}

