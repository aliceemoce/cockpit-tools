//! Cursor 本机导入备份：仅在本机导入时上传至用户私有 GitHub 仓库；每次追加新文件，不从仓库拉取、不覆盖已有备份。

use base64::Engine as _;
use serde::Serialize;

const DEFAULT_REPO: &str = "aliceemoce/cockpit-credentials";
const DEFAULT_BRANCH: &str = "master";
const REMOTE_DIR: &str = "cursor-import-backups";

#[derive(Serialize)]
struct GithubContentsRequest<'a> {
    message: String,
    content: String,
    branch: &'a str,
}

fn resolve_github_token() -> Option<String> {
    if let Some(token) =
        crate::modules::cursor_backup_token_embedded::resolve_embedded_github_token()
    {
        return Some(token);
    }

    for key in ["COCKPIT_GITHUB_TOKEN", "GITHUB_TOKEN", "GH_TOKEN"] {
        if let Ok(value) = std::env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }

    None
}

fn resolve_target_repo() -> String {
    std::env::var("COCKPIT_CURSOR_BACKUP_REPO")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_REPO.to_string())
}

fn build_github_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("cockpit-tools-cursor-backup-sync")
        .build()
        .map_err(|error| format!("创建 GitHub 客户端失败: {}", error))
}

fn github_auth_headers(token: &str) -> [(&'static str, String); 2] {
    [
        ("Authorization", format!("Bearer {}", token)),
        ("Accept", "application/vnd.github+json".to_string()),
    ]
}

pub async fn upload_local_import_backup(backup_path: std::path::PathBuf) -> Result<(), String> {
    let token = resolve_github_token().ok_or_else(|| {
        "未配置 COCKPIT_GITHUB_TOKEN / GITHUB_TOKEN / GH_TOKEN，跳过远端备份".to_string()
    })?;

    if !backup_path.is_file() {
        return Err(format!(
            "Cursor 导入备份文件不存在: {}",
            backup_path.display()
        ));
    }

    let file_name = backup_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Cursor 导入备份文件名非法".to_string())?
        .to_string();

    let content = std::fs::read(&backup_path)
        .map_err(|error| format!("读取 Cursor 导入备份失败: {}", error))?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(content);

    let repo = resolve_target_repo();
    let remote_path = format!("{}/{}", REMOTE_DIR, file_name);
    let endpoint = format!(
        "https://api.github.com/repos/{}/contents/{}",
        repo, remote_path
    );

    let client = build_github_client()?;

    let mut request = client.put(&endpoint);
    for (key, value) in github_auth_headers(&token) {
        request = request.header(key, value);
    }
    let response = request
        .json(&GithubContentsRequest {
            message: format!("cursor local import backup {}", file_name),
            content: encoded,
            branch: DEFAULT_BRANCH,
        })
        .send()
        .await
        .map_err(|error| format!("上传 Cursor 导入备份到 GitHub 失败: {}", error))?;

    let status = response.status();
    if status.is_success() {
        crate::modules::logger::log_info(&format!(
            "[Cursor Backup Sync] 已上传导入备份: repo={}, path={}",
            repo, remote_path
        ));
        return Ok(());
    }

    let body = response
        .text()
        .await
        .unwrap_or_else(|_| "<empty>".to_string());
    Err(format!(
        "上传 Cursor 导入备份失败: status={}, repo={}, path={}, body={}",
        status,
        repo,
        remote_path,
        truncate_for_log(&body, 400)
    ))
}

fn truncate_for_log(text: &str, max_len: usize) -> String {
    if text.chars().count() <= max_len {
        return text.to_string();
    }
    text.chars().take(max_len).collect::<String>() + "..."
}

pub fn schedule_local_import_backup_upload(backup_path: std::path::PathBuf) {
    if resolve_github_token().is_none() {
        crate::modules::logger::log_warn(
            "[Cursor Backup Sync] 未配置 GitHub Token，仅保留本地导入备份",
        );
        return;
    }

    if !backup_path.is_file() {
        crate::modules::logger::log_warn(&format!(
            "[Cursor Backup Sync] 本地导入备份不存在，跳过上传: {}",
            backup_path.display()
        ));
        return;
    }

    let spawn_upload = async move {
        if let Err(error) = upload_local_import_backup(backup_path).await {
            crate::modules::logger::log_warn(&format!("[Cursor Backup Sync] {}", error));
        }
    };

    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.spawn(spawn_upload);
        return;
    }

    std::thread::spawn(move || {
        if let Ok(runtime) = tokio::runtime::Runtime::new() {
            let _ = runtime.block_on(spawn_upload);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_repo_points_to_private_credentials_repo() {
        std::env::remove_var("COCKPIT_CURSOR_BACKUP_REPO");
        assert_eq!(resolve_target_repo(), DEFAULT_REPO);
    }

    #[test]
    fn resolve_github_token_uses_embedded_when_present() {
        std::env::remove_var("COCKPIT_GITHUB_TOKEN");
        std::env::remove_var("GITHUB_TOKEN");
        std::env::remove_var("GH_TOKEN");
        let token = resolve_github_token();
        assert!(token.is_some(), "嵌入 Token 应可用");
    }
}
