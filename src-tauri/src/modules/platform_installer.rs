use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use url::Url;

const MANIFEST_JSON: &str = include_str!("../../platformInstallers.json");
const PROGRESS_EVENT: &str = "platform-install://progress";
const INSTALL_CACHE_DIR: &str = "platform-installers";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformInstallProgress {
    pub platform_id: String,
    pub phase: String,
    pub progress: Option<u8>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedPlatformInstaller {
    pub platform_id: String,
    pub os_key: String,
    pub download_url: Option<String>,
    pub download_page: Option<String>,
    pub filename: String,
    pub installer_kind: String,
    pub silent_supported: bool,
    pub silent_install_args: Vec<String>,
    pub can_auto_install: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformInstallResult {
    pub platform_id: String,
    pub installed_path: Option<String>,
    pub used_manual_fallback: bool,
    pub message: String,
}

#[derive(Debug, Deserialize)]
struct InstallerManifest {
    #[serde(flatten)]
    platforms: HashMap<String, HashMap<String, OsInstallerSpec>>,
}

#[derive(Debug, Deserialize, Clone)]
struct OsInstallerSpec {
    #[serde(default)]
    download_page: Option<String>,
    #[serde(default)]
    direct_url: Option<String>,
    #[serde(default)]
    github_repo: Option<String>,
    #[serde(default)]
    asset_name_contains: Vec<String>,
    installer_kind: String,
    #[serde(default)]
    silent_install_args: Vec<String>,
}

fn current_os_key() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "windows"
    }
    #[cfg(target_os = "macos")]
    {
        "macos"
    }
    #[cfg(target_os = "linux")]
    {
        "linux"
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        "unknown"
    }
}

fn load_manifest() -> Result<InstallerManifest, String> {
    serde_json::from_str(MANIFEST_JSON)
        .map_err(|error| format!("Failed to parse platform installers manifest: {}", error))
}

fn normalize_platform_id(platform_id: &str) -> Result<String, String> {
    let normalized = platform_id.trim().to_lowercase();
    match normalized.as_str() {
        "cursor" | "vscode" | "windsurf" | "kiro" | "antigravity" | "codex" | "github-copilot" => {
            Ok(if normalized == "github-copilot" {
                "vscode".to_string()
            } else {
                normalized
            })
        }
        _ => Err(format!("Unsupported platform for auto-install: {}", platform_id)),
    }
}

fn lookup_spec(platform_id: &str) -> Result<OsInstallerSpec, String> {
    let manifest = load_manifest()?;
    let os_key = current_os_key();
    manifest
        .platforms
        .get(platform_id)
        .and_then(|by_os| by_os.get(os_key))
        .cloned()
        .ok_or_else(|| {
            format!(
                "No installer spec for platform '{}' on {}",
                platform_id, os_key
            )
        })
}

fn guess_filename_from_url(url: &str, platform_id: &str) -> String {
    if let Ok(parsed) = Url::parse(url) {
        if let Some(segments) = parsed.path_segments() {
            let last = segments.last().unwrap_or_default();
            if !last.is_empty() && last.contains('.') {
                return last.to_string();
            }
        }
    }
    format!("{}-installer.bin", platform_id)
}

fn silent_supported(spec: &OsInstallerSpec) -> bool {
    !spec.silent_install_args.is_empty()
        && !matches!(
            spec.installer_kind.as_str(),
            "store" | "unknown" | "dmg" | "zip" | "tar" | "deb" | "rpm"
        )
}

fn can_auto_install(spec: &OsInstallerSpec) -> bool {
    if matches!(spec.installer_kind.as_str(), "store" | "unknown") {
        return spec.direct_url.is_some() || spec.github_repo.is_some();
    }
    spec.direct_url.is_some() || spec.github_repo.is_some()
}

fn emit_progress(
    app: &AppHandle,
    platform_id: &str,
    phase: &str,
    progress: Option<u8>,
    message: Option<String>,
) {
    let payload = PlatformInstallProgress {
        platform_id: platform_id.to_string(),
        phase: phase.to_string(),
        progress,
        message,
    };
    let _ = app.emit(PROGRESS_EVENT, payload);
}

async fn resolve_download_url(platform_id: &str, spec: &OsInstallerSpec) -> Result<(String, String), String> {
    if let Some(direct) = spec
        .direct_url
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        let filename = guess_filename_from_url(&direct, platform_id);
        return Ok((direct, filename));
    }

    if let Some(repo) = spec
        .github_repo
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        return resolve_github_release_asset(&repo, &spec.asset_name_contains).await;
    }

    Err("No downloadable URL configured for this platform".to_string())
}

async fn resolve_github_release_asset(
    repo: &str,
    asset_name_contains: &[String],
) -> Result<(String, String), String> {
    let endpoint = format!("https://api.github.com/repos/{}/releases/latest", repo);
    let client = reqwest::Client::builder()
        .user_agent("cockpit-tools-platform-installer")
        .build()
        .map_err(|error| format!("Failed to create HTTP client: {}", error))?;
    let response = client
        .get(&endpoint)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|error| format!("Failed to fetch GitHub release: {}", error))?;
    if !response.status().is_success() {
        return Err(format!(
            "Failed to fetch GitHub release: HTTP {}",
            response.status()
        ));
    }
    let payload: serde_json::Value = response
        .json()
        .await
        .map_err(|error| format!("Failed to parse GitHub release JSON: {}", error))?;
    let assets = payload
        .get("assets")
        .and_then(|value| value.as_array())
        .ok_or_else(|| "GitHub release has no assets".to_string())?;

    let filters: Vec<String> = asset_name_contains
        .iter()
        .map(|value| value.to_lowercase())
        .filter(|value| !value.is_empty())
        .collect();

    for asset in assets {
        let name = asset
            .get("name")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        let download_url = asset
            .get("browser_download_url")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        if download_url.is_empty() {
            continue;
        }
        let lower = name.to_lowercase();
        if filters.is_empty() || filters.iter().all(|needle| lower.contains(needle)) {
            return Ok((download_url.to_string(), name.to_string()));
        }
    }

    Err(format!(
        "No matching GitHub release asset found for {}",
        repo
    ))
}

pub fn resolve_platform_installer(platform_id: String) -> Result<ResolvedPlatformInstaller, String> {
    let platform_id = normalize_platform_id(&platform_id)?;
    let spec = lookup_spec(&platform_id)?;
    let os_key = current_os_key().to_string();
    let download_url = spec.direct_url.clone();
    let download_page = spec.download_page.clone();
    let filename = download_url
        .as_ref()
        .map(|url| guess_filename_from_url(url, &platform_id))
        .unwrap_or_else(|| format!("{}-installer", platform_id));

    Ok(ResolvedPlatformInstaller {
        platform_id: platform_id.clone(),
        os_key,
        download_url,
        download_page,
        filename,
        installer_kind: spec.installer_kind.clone(),
        silent_supported: silent_supported(&spec),
        silent_install_args: spec.silent_install_args.clone(),
        can_auto_install: can_auto_install(&spec),
    })
}

fn cache_dir() -> Result<PathBuf, String> {
    let base = dirs::cache_dir().ok_or_else(|| "Failed to resolve cache directory".to_string())?;
    let path = base.join("com.antigravity.cockpit-tools").join(INSTALL_CACHE_DIR);
    std::fs::create_dir_all(&path)
        .map_err(|error| format!("Failed to create installer cache dir: {}", error))?;
    Ok(path)
}

pub async fn download_platform_installer(
    app: AppHandle,
    platform_id: String,
) -> Result<String, String> {
    let platform_id = normalize_platform_id(&platform_id)?;
    let spec = lookup_spec(&platform_id)?;

    emit_progress(
        &app,
        &platform_id,
        "resolving",
        Some(0),
        Some("Resolving download URL".to_string()),
    );

    let (download_url, filename) = resolve_download_url(&platform_id, &spec).await?;
    emit_progress(
        &app,
        &platform_id,
        "downloading",
        Some(0),
        Some(format!("Downloading {}", filename)),
    );

    crate::modules::logger::log_info(&format!(
        "[PlatformInstall] Download start: platform={}, url={}",
        platform_id, download_url
    ));

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|error| format!("Failed to create HTTP client: {}", error))?;
    let response = client
        .get(&download_url)
        .send()
        .await
        .map_err(|error| format!("Failed to download installer: {}", error))?;
    if !response.status().is_success() {
        return Err(format!(
            "Failed to download installer: HTTP {}",
            response.status()
        ));
    }

    let content_length = response.content_length().unwrap_or(0);
    let target_dir = cache_dir()?;
    let target_path = target_dir.join(&filename);

    let mut stream = response.bytes_stream();
    use futures_util::StreamExt;
    let mut file = tokio::fs::File::create(&target_path)
        .await
        .map_err(|error| format!("Failed to create installer file: {}", error))?;
    let mut downloaded: u64 = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| format!("Failed while downloading installer: {}", error))?;
        use tokio::io::AsyncWriteExt;
        file.write_all(&chunk)
            .await
            .map_err(|error| format!("Failed to write installer file: {}", error))?;
        downloaded += chunk.len() as u64;
        if content_length > 0 {
            let pct = ((downloaded * 100) / content_length).min(100) as u8;
            emit_progress(
                &app,
                &platform_id,
                "downloading",
                Some(pct),
                Some(format!("Downloaded {}%", pct)),
            );
        }
    }

    emit_progress(
        &app,
        &platform_id,
        "downloaded",
        Some(100),
        Some(target_path.display().to_string()),
    );
    crate::modules::logger::log_info(&format!(
        "[PlatformInstall] Download complete: platform={}, path={}",
        platform_id,
        target_path.display()
    ));
    Ok(target_path.to_string_lossy().to_string())
}

fn run_windows_silent_install(
    installer_path: &Path,
    spec: &OsInstallerSpec,
) -> Result<(), String> {
    let extension = installer_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_lowercase();

    if extension == "msi" {
        let status = Command::new("msiexec")
            .args([
                "/i",
                &installer_path.to_string_lossy(),
                "/quiet",
                "/norestart",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|error| format!("Failed to launch msiexec: {}", error))?;
        if status.success() {
            return Ok(());
        }
        return Err(format!("msiexec exited with status {:?}", status.code()));
    }

    if extension == "exe" && silent_supported(spec) {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut cmd = Command::new(installer_path);
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        for arg in &spec.silent_install_args {
            cmd.arg(arg);
        }
        let status = cmd
            .status()
            .map_err(|error| format!("Failed to launch installer: {}", error))?;
        if status.success() {
            return Ok(());
        }
        return Err(format!("Installer exited with status {:?}", status.code()));
    }

    Err("Silent install is not supported for this installer type".to_string())
}

#[cfg(target_os = "windows")]
fn open_path_for_manual_install(path: &Path) -> Result<(), String> {
    Command::new("cmd")
        .args(["/C", "start", "", &path.to_string_lossy()])
        .spawn()
        .map_err(|error| format!("Failed to open installer for manual setup: {}", error))?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn open_path_for_manual_install(path: &Path) -> Result<(), String> {
    Command::new("open")
        .arg(path)
        .spawn()
        .map_err(|error| format!("Failed to open installer for manual setup: {}", error))?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn open_path_for_manual_install(path: &Path) -> Result<(), String> {
    Command::new("xdg-open")
        .arg(path)
        .spawn()
        .map_err(|error| format!("Failed to open installer for manual setup: {}", error))?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn run_macos_install(path: &Path, spec: &OsInstallerSpec) -> Result<(), String> {
    match spec.installer_kind.as_str() {
        "dmg" | "zip" | "pkg" => {
            Command::new("open")
                .arg(path)
                .spawn()
                .map_err(|error| format!("Failed to open installer: {}", error))?;
            Ok(())
        }
        _ => open_path_for_manual_install(path),
    }
}

#[cfg(target_os = "linux")]
fn run_linux_install(path: &Path, spec: &OsInstallerSpec) -> Result<(), String> {
    match spec.installer_kind.as_str() {
        "deb" => {
            let status = Command::new("pkexec")
                .args([
                    "dpkg",
                    "-i",
                    &path.to_string_lossy(),
                ])
                .status()
                .map_err(|error| format!("Failed to launch dpkg install: {}", error))?;
            if status.success() {
                Ok(())
            } else {
                open_path_for_manual_install(path)
            }
        }
        "rpm" => {
            let status = Command::new("pkexec")
                .args([
                    "rpm",
                    "-Uvh",
                    &path.to_string_lossy(),
                ])
                .status()
                .map_err(|error| format!("Failed to launch rpm install: {}", error))?;
            if status.success() {
                Ok(())
            } else {
                open_path_for_manual_install(path)
            }
        }
        "appimage" => {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(path)
                .map_err(|error| format!("Failed to read AppImage permissions: {}", error))?
                .permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(path, perms)
                .map_err(|error| format!("Failed to chmod AppImage: {}", error))?;
            open_path_for_manual_install(path)
        }
        _ => open_path_for_manual_install(path),
    }
}

pub async fn install_platform_installer(
    app: AppHandle,
    platform_id: String,
    installer_path: String,
) -> Result<PlatformInstallResult, String> {
    let platform_id = normalize_platform_id(&platform_id)?;
    let spec = lookup_spec(&platform_id)?;
    let path = PathBuf::from(installer_path.trim());
    if !path.exists() {
        return Err(format!("Installer file not found: {}", path.display()));
    }

    emit_progress(
        &app,
        &platform_id,
        "installing",
        Some(0),
        Some("Running installer".to_string()),
    );

    let install_result = {
        #[cfg(target_os = "windows")]
        {
            run_windows_silent_install(&path, &spec)
        }
        #[cfg(target_os = "macos")]
        {
            run_macos_install(&path, &spec)
        }
        #[cfg(target_os = "linux")]
        {
            run_linux_install(&path, &spec)
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            Err("Unsupported operating system".to_string())
        }
    };

    let mut used_manual_fallback = false;
    if let Err(error) = install_result {
        crate::modules::logger::log_warn(&format!(
            "[PlatformInstall] Silent install failed, opening manual installer: platform={}, error={}",
            platform_id, error
        ));
        open_path_for_manual_install(&path)?;
        used_manual_fallback = true;
    }

    emit_progress(
        &app,
        &platform_id,
        "detecting",
        Some(90),
        Some("Detecting installed application path".to_string()),
    );

    let mut detected = None;
    for attempt in 0..8 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_secs(2)).await;
        } else {
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
        detected = detect_installed_path(&platform_id)?;
        if detected.is_some() {
            break;
        }
    }
    let installed_path = detected.clone();

    emit_progress(
        &app,
        &platform_id,
        if installed_path.is_some() {
            "completed"
        } else {
            "completed_with_manual"
        },
        Some(100),
        installed_path.clone(),
    );

    Ok(PlatformInstallResult {
        platform_id,
        installed_path,
        used_manual_fallback,
        message: if used_manual_fallback {
            "Installer opened for manual completion".to_string()
        } else {
            "Install finished".to_string()
        },
    })
}

pub async fn install_missing_platform(
    app: AppHandle,
    platform_id: String,
) -> Result<PlatformInstallResult, String> {
    let platform_id = normalize_platform_id(&platform_id)?;
    let spec = lookup_spec(&platform_id)?;

    if matches!(spec.installer_kind.as_str(), "store") {
        if let Some(page) = spec.download_page.clone() {
            use tauri_plugin_opener::OpenerExt;
            app.opener()
                .open_url(&page, None::<String>)
                .map_err(|error| format!("Failed to open download page: {}", error))?;
            return Ok(PlatformInstallResult {
                platform_id,
                installed_path: None,
                used_manual_fallback: true,
                message: "Opened store/download page; complete installation manually".to_string(),
            });
        }
        return Err("This platform must be installed from the vendor store".to_string());
    }

    if !can_auto_install(&spec) {
        if let Some(page) = spec.download_page.clone() {
            use tauri_plugin_opener::OpenerExt;
            app.opener()
                .open_url(&page, None::<String>)
                .map_err(|error| format!("Failed to open download page: {}", error))?;
            return Ok(PlatformInstallResult {
                platform_id,
                installed_path: None,
                used_manual_fallback: true,
                message: "Opened download page; complete installation manually".to_string(),
            });
        }
        return Err("Auto-install is not available for this platform".to_string());
    }

    let downloaded = download_platform_installer(app.clone(), platform_id.clone()).await?;
    install_platform_installer(app, platform_id, downloaded).await
}

fn detect_installed_path(platform_id: &str) -> Result<Option<String>, String> {
    let detected = match platform_id {
        "windsurf" => {
            crate::modules::windsurf_instance::detect_and_save_windsurf_launch_path(true)
        }
        "kiro" => crate::modules::kiro_instance::detect_and_save_kiro_launch_path(true),
        "cursor" => crate::modules::cursor_instance::detect_and_save_cursor_launch_path(true),
        _ => crate::modules::process::detect_and_save_app_path(platform_id, true),
    };
    Ok(detected)
}

pub fn is_install_supported(platform_id: &str) -> bool {
    normalize_platform_id(platform_id)
        .ok()
        .and_then(|id| lookup_spec(&id).ok())
        .map(|spec| can_auto_install(&spec) || spec.download_page.is_some())
        .unwrap_or(false)
}
