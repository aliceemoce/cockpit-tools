use std::collections::HashMap;

use base64::Engine as _;
use serde::{Deserialize, Serialize};

use crate::models::cursor::CursorImportPayload;

const DEFAULT_REPO: &str = "aliceemoce/cockpit-credentials";
const DEFAULT_BRANCH: &str = "master";
const REMOTE_DIR: &str = "cursor-import-backups";
const REMOTE_LIST_PAGE_SIZE: usize = 100;

#[derive(Serialize)]
struct GithubContentsRequest<'a> {
    message: String,
    content: String,
    branch: &'a str,
}

fn resolve_github_token() -> Option<String> {
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

#[derive(Debug, Deserialize)]
struct GithubContentEntry {
    name: String,
    path: String,
    #[serde(rename = "type")]
    entry_type: String,
    content: Option<String>,
    encoding: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RemoteImportBackupSnapshot {
    created_at_ms: i64,
    email: String,
    payload: CursorImportPayload,
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

fn parse_link_next(link_header: &str) -> Option<String> {
    for segment in link_header.split(',') {
        let segment = segment.trim();
        if segment.contains("rel=\"next\"") {
            let start = segment.find('<')? + 1;
            let end = segment.find('>')?;
            return Some(segment[start..end].to_string());
        }
    }
    None
}

fn normalize_snapshot_email(snapshot: &RemoteImportBackupSnapshot) -> Option<String> {
    let email = snapshot.email.trim();
    if email.is_empty() {
        return None;
    }
    let lowered = email.to_lowercase();
    if lowered.contains('@') {
        Some(lowered)
    } else {
        None
    }
}

fn decode_github_content(entry: &GithubContentEntry) -> Result<Vec<u8>, String> {
    let encoded = entry
        .content
        .as_deref()
        .ok_or_else(|| format!("GitHub 内容缺少 payload: {}", entry.path))?;
    base64::engine::general_purpose::STANDARD
        .decode(encoded.replace('\n', ""))
        .map_err(|error| format!("解码 GitHub 内容失败: path={}, error={}", entry.path, error))
}

async fn list_remote_backup_entries(
    client: &reqwest::Client,
    token: &str,
    repo: &str,
) -> Result<Vec<GithubContentEntry>, String> {
    let mut entries = Vec::new();
    let mut page = 1usize;
    let mut next_url: Option<String> = None;

    loop {
        let url = next_url.clone().unwrap_or_else(|| {
            format!(
                "https://api.github.com/repos/{}/contents/{}?ref={}&per_page={}&page={}",
                repo, REMOTE_DIR, DEFAULT_BRANCH, REMOTE_LIST_PAGE_SIZE, page
            )
        });

        let mut request = client.get(&url);
        for (key, value) in github_auth_headers(token) {
            request = request.header(key, value);
        }

        let response = request
            .send()
            .await
            .map_err(|error| format!("列出远端 Cursor 导入备份失败: {}", error))?;

        let status = response.status();
        if status.as_u16() == 404 {
            return Ok(Vec::new());
        }
        if !status.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<empty>".to_string());
            return Err(format!(
                "列出远端 Cursor 导入备份失败: status={}, body={}",
                status,
                truncate_for_log(&body, 400)
            ));
        }

        let link_header = response
            .headers()
            .get("link")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);

        let body = response
            .text()
            .await
            .map_err(|error| format!("读取远端 Cursor 导入备份列表失败: {}", error))?;

        let page_entries: Vec<GithubContentEntry> =
            serde_json::from_str(&body).map_err(|error| {
                format!(
                    "解析远端 Cursor 导入备份列表失败: page={}, error={}",
                    page, error
                )
            })?;

        let page_len = page_entries.len();
        entries.extend(
            page_entries
                .into_iter()
                .filter(|entry| entry.entry_type == "file" && entry.name.ends_with(".json")),
        );

        next_url = link_header.as_deref().and_then(parse_link_next);
        if next_url.is_some() {
            continue;
        }
        if page_len < REMOTE_LIST_PAGE_SIZE {
            break;
        }
        page += 1;
    }

    Ok(entries)
}

async fn fetch_remote_backup_snapshot(
    client: &reqwest::Client,
    token: &str,
    repo: &str,
    path: &str,
) -> Result<RemoteImportBackupSnapshot, String> {
    let url = format!("https://api.github.com/repos/{}/contents/{}?ref={}", repo, path, DEFAULT_BRANCH);
    let mut request = client.get(&url);
    for (key, value) in github_auth_headers(token) {
        request = request.header(key, value);
    }

    let response = request
        .send()
        .await
        .map_err(|error| format!("下载远端 Cursor 导入备份失败: path={}, error={}", path, error))?;

    let status = response.status();
    if !status.is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<empty>".to_string());
        return Err(format!(
            "下载远端 Cursor 导入备份失败: path={}, status={}, body={}",
            path,
            status,
            truncate_for_log(&body, 400)
        ));
    }

    let entry = response
        .json::<GithubContentEntry>()
        .await
        .map_err(|error| format!("解析远端 Cursor 导入备份元数据失败: path={}, error={}", path, error))?;

    let bytes = decode_github_content(&entry)?;
    serde_json::from_slice::<RemoteImportBackupSnapshot>(&bytes).map_err(|error| {
        format!(
            "解析远端 Cursor 导入备份 JSON 失败: path={}, error={}",
            path, error
        )
    })
}

pub async fn pull_remote_import_backups() -> Result<usize, String> {
    let token = match resolve_github_token() {
        Some(token) => token,
        None => {
            crate::modules::logger::log_warn(
                "[Cursor Backup Sync] 未配置 GitHub Token，跳过远端账号拉取",
            );
            return Ok(0);
        }
    };

    let repo = resolve_target_repo();
    let client = build_github_client()?;
    let entries = list_remote_backup_entries(&client, &token, &repo).await?;
    if entries.is_empty() {
        return Ok(0);
    }

    let mut latest_by_email = HashMap::<String, RemoteImportBackupSnapshot>::new();
    for entry in entries {
        match fetch_remote_backup_snapshot(&client, &token, &repo, entry.path.as_str()).await {
            Ok(snapshot) => {
                let Some(email) = normalize_snapshot_email(&snapshot) else {
                    continue;
                };
                let replace = latest_by_email
                    .get(&email)
                    .map(|existing| snapshot.created_at_ms >= existing.created_at_ms)
                    .unwrap_or(true);
                if replace {
                    latest_by_email.insert(email, snapshot);
                }
            }
            Err(error) => {
                crate::modules::logger::log_warn(&format!(
                    "[Cursor Backup Sync] 跳过损坏的远端备份: path={}, error={}",
                    entry.path, error
                ));
            }
        }
    }

    if latest_by_email.is_empty() {
        return Ok(0);
    }

    let mut live_emails = crate::modules::cursor_account::collect_live_account_emails();
    let mut merged = 0usize;
    let mut merged_emails = Vec::new();
    let mut snapshots = latest_by_email.into_values().collect::<Vec<_>>();
    snapshots.sort_by_key(|snapshot| snapshot.created_at_ms);

    for snapshot in snapshots {
        let Some(email) = normalize_snapshot_email(&snapshot) else {
            continue;
        };
        if live_emails.contains(&email) {
            continue;
        }
        let display_email = snapshot.email.clone();
        match crate::modules::cursor_account::upsert_import_payload_if_missing_email(
            snapshot.payload,
            &live_emails,
        ) {
            Ok(true) => {
                live_emails.insert(email);
                merged += 1;
                merged_emails.push(display_email);
            }
            Ok(false) => {}
            Err(error) => {
                crate::modules::logger::log_warn(&format!(
                    "[Cursor Backup Sync] 合并远端账号失败: email={}, error={}",
                    display_email, error
                ));
            }
        }
    }

    if merged > 0 {
        crate::modules::logger::log_info(&format!(
            "[Cursor Backup Sync] 已从私有仓库拉取并合并 {} 个 Cursor 账号: {}",
            merged,
            merged_emails.join(",")
        ));
    }

    Ok(merged)
}

pub async fn upload_local_import_backup(backup_path: std::path::PathBuf) -> Result<(), String> {
    let token = resolve_github_token()
        .ok_or_else(|| "未配置 COCKPIT_GITHUB_TOKEN / GITHUB_TOKEN / GH_TOKEN，跳过远端备份".to_string())?;

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
    fn parse_link_next_reads_github_pagination() {
        let header = r#"<https://api.github.com/repos/o/r/contents/cursor-import-backups?per_page=100&page=2>; rel="next", <https://api.github.com/repos/o/r/contents/cursor-import-backups?per_page=100&page=5>; rel="last""#;
        assert_eq!(
            parse_link_next(header),
            Some("https://api.github.com/repos/o/r/contents/cursor-import-backups?per_page=100&page=2".to_string())
        );
    }

    #[test]
    fn default_repo_points_to_private_credentials_repo() {
        std::env::remove_var("COCKPIT_CURSOR_BACKUP_REPO");
        assert_eq!(resolve_target_repo(), DEFAULT_REPO);
    }

    #[test]
    fn upload_requires_token_when_missing() {
        let saved = [
            "COCKPIT_GITHUB_TOKEN",
            "GITHUB_TOKEN",
            "GH_TOKEN",
        ]
        .into_iter()
        .map(|key| (key, std::env::var(key).ok()))
        .collect::<Vec<_>>();
        for (key, _) in &saved {
            std::env::remove_var(key);
        }

        let backup_path = std::env::temp_dir().join(format!(
            "cockpit-cursor-backup-test-{}.json",
            std::process::id()
        ));
        std::fs::write(&backup_path, br#"{"ok":true}"#).expect("temp backup");

        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let result = runtime.block_on(upload_local_import_backup(backup_path.clone()));
        let _ = std::fs::remove_file(&backup_path);

        for (key, value) in saved {
            if let Some(value) = value {
                std::env::set_var(key, value);
            }
        }

        assert!(result.is_err());
        let message = result.err().unwrap_or_default();
        assert!(
            message.contains("TOKEN") || message.contains("未配置"),
            "expected missing token error, got: {}",
            message
        );
    }
}
