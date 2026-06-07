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
/// Microsoft Store package id for the Codex desktop app (winget msstore source).
const CODEX_MSSTORE_WINGET_ID: &str = "9PLM9XGG6VKS";
const CODEX_LAUNCHER_GITHUB_REPO: &str = "vaportail/codex-windows-updater";
const CODEX_LAUNCHER_ASSET_CONTAINS: &str = "codex-launcher";
const CODEX_LAUNCHER_EXE: &str = "codex-launcher.exe";
const CODEX_LAUNCHER_WORK_SUBDIR: &str = "codex-msix";
const CODEX_LAUNCHER_MSIX_SUBDIR: &str = "test_download";
/// Typical Codex MSIX size for coarse download progress (bytes).
const CODEX_MSIX_EXPECTED_BYTES: u64 = 470_000_000;

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
#[serde(rename_all = "camelCase")]
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
    if matches!(
        spec.installer_kind.as_str(),
        "store" | "unknown" | "msix-launcher"
    ) {
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

#[cfg(target_os = "windows")]
fn is_winget_available() -> bool {
    Command::new("where")
        .args(["winget"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(not(target_os = "windows"))]
fn is_winget_available() -> bool {
    false
}

#[cfg(target_os = "windows")]
fn codex_msix_work_dir() -> Result<PathBuf, String> {
    let dir = cache_dir()?.join(CODEX_LAUNCHER_WORK_SUBDIR);
    std::fs::create_dir_all(&dir)
        .map_err(|error| format!("Failed to create Codex MSIX work dir: {}", error))?;
    Ok(dir)
}

#[cfg(target_os = "windows")]
fn escape_powershell_single_quoted(value: &str) -> String {
    value.replace('\'', "''")
}

#[cfg(target_os = "windows")]
fn find_latest_msix_in_dir(dir: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("msix") {
            continue;
        }
        let modified = entry.metadata().ok()?.modified().ok()?;
        if best.as_ref().map(|(t, _)| modified > *t).unwrap_or(true) {
            best = Some((modified, path));
        }
    }
    best.map(|(_, path)| path)
}

#[cfg(target_os = "windows")]
async fn download_codex_launcher_binary(
    app: &AppHandle,
    platform_id: &str,
    target_path: &Path,
) -> Result<(), String> {
    emit_progress(
        app,
        platform_id,
        "resolving",
        Some(5),
        Some("Resolving codex-launcher release".to_string()),
    );

    let (download_url, _) = resolve_github_release_asset(
        CODEX_LAUNCHER_GITHUB_REPO,
        &[CODEX_LAUNCHER_ASSET_CONTAINS.to_string()],
    )
    .await?;

    emit_progress(
        app,
        platform_id,
        "downloading",
        Some(10),
        Some(format!("Downloading {}", CODEX_LAUNCHER_EXE)),
    );

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|error| format!("Failed to create HTTP client: {}", error))?;
    let response = client
        .get(&download_url)
        .send()
        .await
        .map_err(|error| format!("Failed to download codex-launcher: {}", error))?;
    if !response.status().is_success() {
        return Err(format!(
            "Failed to download codex-launcher: HTTP {}",
            response.status()
        ));
    }

    let content_length = response.content_length().unwrap_or(0);
    let mut stream = response.bytes_stream();
    use futures_util::StreamExt;
    let mut file = tokio::fs::File::create(target_path)
        .await
        .map_err(|error| format!("Failed to create codex-launcher file: {}", error))?;
    let mut downloaded: u64 = 0;
    while let Some(chunk) = stream.next().await {
        let chunk =
            chunk.map_err(|error| format!("Failed while downloading codex-launcher: {}", error))?;
        use tokio::io::AsyncWriteExt;
        file.write_all(&chunk)
            .await
            .map_err(|error| format!("Failed to write codex-launcher file: {}", error))?;
        downloaded += chunk.len() as u64;
        if content_length > 0 {
            let pct = 10 + (((downloaded * 15) / content_length).min(15) as u8);
            emit_progress(
                app,
                platform_id,
                "downloading",
                Some(pct),
                Some(format!("Downloading launcher {}%", (downloaded * 100) / content_length)),
            );
        }
    }

    crate::modules::logger::log_info(&format!(
        "[PlatformInstall] codex-launcher downloaded: path={}",
        target_path.display()
    ));
    Ok(())
}

#[cfg(target_os = "windows")]
async fn run_codex_launcher_msix_download(
    app: &AppHandle,
    platform_id: &str,
    launcher_path: &Path,
    work_dir: &Path,
) -> Result<PathBuf, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let msix_dir = work_dir.join(CODEX_LAUNCHER_MSIX_SUBDIR);
    if msix_dir.exists() {
        let _ = std::fs::remove_dir_all(&msix_dir);
    }
    std::fs::create_dir_all(&msix_dir)
        .map_err(|error| format!("Failed to prepare MSIX download dir: {}", error))?;

    emit_progress(
        app,
        platform_id,
        "downloading",
        Some(20),
        Some("Downloading official Codex MSIX from Microsoft CDN".to_string()),
    );

    let mut child = Command::new(launcher_path)
        .args(["--fetcher", "direct", "--test-download"])
        .current_dir(work_dir)
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("Failed to start codex-launcher: {}", error))?;

    let mut last_pct: u8 = 20;
    loop {
        if let Some(msix_path) = find_latest_msix_in_dir(&msix_dir) {
            if let Ok(meta) = msix_path.metadata() {
                let len = meta.len();
                if len > 0 {
                    let pct = 20 + ((len * 60) / CODEX_MSIX_EXPECTED_BYTES).min(60) as u8;
                    if pct > last_pct {
                        last_pct = pct;
                        emit_progress(
                            app,
                            platform_id,
                            "downloading",
                            Some(pct),
                            Some(format!("Downloading MSIX (~{} MB)", len / 1_048_576)),
                        );
                    }
                }
            }
        }

        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return Err(format!(
                        "codex-launcher MSIX download failed (exit {:?})",
                        status.code()
                    ));
                }
                break;
            }
            Ok(None) => {
                tokio::time::sleep(Duration::from_millis(800)).await;
            }
            Err(error) => {
                return Err(format!("Failed while waiting for codex-launcher: {}", error));
            }
        }
    }

    find_latest_msix_in_dir(&msix_dir).ok_or_else(|| {
        format!(
            "codex-launcher finished but no .msix found under {}",
            msix_dir.display()
        )
    })
}

#[cfg(target_os = "windows")]
fn install_codex_msix_with_appx(msix_path: &Path) -> Result<(), String> {
    let literal = escape_powershell_single_quoted(&msix_path.to_string_lossy());
    let script = format!(
        "Add-AppxPackage -LiteralPath '{literal}' -ForceUpdateFromAnyVersion -ErrorAction Stop"
    );
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("Add-AppxPackage 启动失败: {}", error))?;

    if output.status.success() {
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}\n{}", stdout.trim(), stderr.trim()).trim().to_string();
    if combined.to_ascii_lowercase().contains("already installed")
        || combined.contains("已安装")
        || combined.contains("0x80073D10")
    {
        return Ok(());
    }

    let detail = if combined.is_empty() {
        format!("exit code {:?}", output.status.code())
    } else {
        combined.chars().take(600).collect()
    };
    Err(format!("Add-AppxPackage 安装 Codex 失败: {}", detail))
}

#[cfg(target_os = "windows")]
async fn install_codex_via_msix_launcher(
    app: &AppHandle,
    platform_id: &str,
) -> Result<(), String> {
    let work_dir = codex_msix_work_dir()?;
    let launcher_path = work_dir.join(CODEX_LAUNCHER_EXE);
    if !launcher_path.exists() {
        download_codex_launcher_binary(app, platform_id, &launcher_path).await?;
    }

    let msix_path = run_codex_launcher_msix_download(app, platform_id, &launcher_path, &work_dir).await?;
    crate::modules::logger::log_info(&format!(
        "[PlatformInstall] Codex MSIX ready: {}",
        msix_path.display()
    ));

    emit_progress(
        app,
        platform_id,
        "installing",
        Some(85),
        Some("Installing Codex MSIX (Add-AppxPackage)".to_string()),
    );
    install_codex_msix_with_appx(&msix_path)
}

#[cfg(target_os = "windows")]
fn try_winget_install_codex_store_app() -> Result<(), String> {
    if !is_winget_available() {
        return Err("未找到 winget，无法自动安装 Codex".to_string());
    }

    let output = Command::new("winget")
        .args([
            "install",
            "--id",
            CODEX_MSSTORE_WINGET_ID,
            "-s",
            "msstore",
            "--accept-package-agreements",
            "--accept-source-agreements",
            "--disable-interactivity",
            "-h",
        ])
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("winget 安装命令启动失败: {}", error))?;

    if output.status.success() {
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}\n{}", stdout.trim(), stderr.trim()).trim().to_string();
    if combined.to_ascii_lowercase().contains("already installed")
        || combined.contains("已安装")
    {
        return Ok(());
    }

    let detail = if combined.is_empty() {
        format!("exit code {:?}", output.status.code())
    } else {
        combined.chars().take(500).collect()
    };
    Err(format!("winget 安装 Codex 失败: {}", detail))
}

async fn wait_for_codex_install_detection(
    app: &AppHandle,
    platform_id: &str,
) -> Result<PlatformInstallResult, String> {
    emit_progress(
        app,
        platform_id,
        "detecting",
        Some(90),
        Some("Detecting installed application path".to_string()),
    );

    let mut installed_path = None;
    for attempt in 0..12 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_secs(3)).await;
        } else {
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        installed_path = detect_installed_path(platform_id)?;
        if installed_path.is_some() {
            break;
        }
    }

    let message = if installed_path.is_some() {
        "Codex install finished".to_string()
    } else {
        "Codex install command finished, but launch path was not detected automatically".to_string()
    };

    emit_progress(
        app,
        platform_id,
        if installed_path.is_some() {
            "completed"
        } else {
            "completed_with_manual"
        },
        Some(100),
        installed_path.clone(),
    );

    Ok(PlatformInstallResult {
        platform_id: platform_id.to_string(),
        installed_path,
        used_manual_fallback: false,
        message,
    })
}

async fn install_codex_missing_platform(
    app: AppHandle,
    platform_id: String,
) -> Result<PlatformInstallResult, String> {
    #[cfg(target_os = "windows")]
    {
        if let Err(primary_error) = install_codex_via_msix_launcher(&app, &platform_id).await {
            crate::modules::logger::log_warn(&format!(
                "[PlatformInstall] Codex MSIX launcher install failed, trying winget fallback: {}",
                primary_error
            ));
            emit_progress(
                &app,
                &platform_id,
                "installing",
                Some(10),
                Some("MSIX direct install failed; trying winget fallback".to_string()),
            );
            if let Err(winget_error) = try_winget_install_codex_store_app() {
                return Err(format!(
                    "直连 MSIX 安装失败：{}。winget 回退也失败：{}。不会打开 Microsoft Store 或浏览器；请稍后「重置默认」探测路径，或手动选择 Codex.exe。",
                    primary_error, winget_error
                ));
            }
        }

        return wait_for_codex_install_detection(&app, &platform_id).await;
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        Err(
            "当前系统不支持自动安装 Codex App。请从官方渠道安装后，在设置中配置启动路径。"
                .to_string(),
        )
    }
}

pub async fn install_missing_platform(
    app: AppHandle,
    platform_id: String,
) -> Result<PlatformInstallResult, String> {
    let platform_id = normalize_platform_id(&platform_id)?;
    if platform_id == "codex" {
        return install_codex_missing_platform(app, platform_id).await;
    }
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
    let Ok(id) = normalize_platform_id(platform_id) else {
        return false;
    };
    if id == "codex" {
        #[cfg(target_os = "windows")]
        {
            return true;
        }
        #[cfg(not(target_os = "windows"))]
        {
            return false;
        }
    }
    lookup_spec(&id)
        .ok()
        .map(|spec| can_auto_install(&spec) || spec.download_page.is_some())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_parses_all_installable_platforms() {
        let manifest = load_manifest().expect("manifest should parse");
        for platform in ["cursor", "vscode", "windsurf", "kiro", "antigravity", "codex"] {
            assert!(
                manifest.platforms.contains_key(platform),
                "missing platform entry: {platform}"
            );
            let os_map = manifest
                .platforms
                .get(platform)
                .expect("platform map should exist");
            assert!(
                os_map.contains_key("windows"),
                "missing windows spec for {platform}"
            );
        }
    }

    #[test]
    fn install_support_matches_known_platforms() {
        assert!(is_install_supported("cursor"));
        assert!(is_install_supported("vscode"));
        assert!(is_install_supported("windsurf"));
        assert!(is_install_supported("kiro"));
        assert!(is_install_supported("antigravity"));
        #[cfg(target_os = "windows")]
        assert!(is_install_supported("codex"));
        assert!(!is_install_supported("zed"));
        assert!(!is_install_supported("not-a-platform"));
    }

    #[test]
    fn normalize_platform_id_rejects_empty() {
        assert!(normalize_platform_id("").is_err());
        assert!(normalize_platform_id("   ").is_err());
        assert_eq!(normalize_platform_id(" Cursor ").unwrap(), "cursor");
    }

    #[test]
    fn resolve_cursor_installer_has_direct_download() {
        let resolved =
            resolve_platform_installer("cursor".to_string()).expect("cursor should resolve");
        assert_eq!(resolved.platform_id, "cursor");
        assert_eq!(resolved.os_key, "windows");
        let url = resolved
            .download_url
            .as_ref()
            .expect("cursor windows should have direct_url");
        assert!(url.starts_with("https://"));
        assert!(resolved.can_auto_install);
        assert!(resolved.silent_supported);
    }

    #[tokio::test]
    async fn cursor_download_url_is_reachable_when_network_allows() {
        let resolved =
            resolve_platform_installer("cursor".to_string()).expect("cursor should resolve");
        let url = resolved
            .download_url
            .expect("cursor should expose direct download url");
        assert!(url.starts_with("https://downloader.cursor.sh/"));

        let strict = std::env::var("COCKPIT_STRICT_NETWORK_TESTS")
            .ok()
            .map(|value| {
                matches!(
                    value.trim().to_lowercase().as_str(),
                    "1" | "true" | "yes"
                )
            })
            .unwrap_or(false);

        let client = reqwest::Client::builder()
            .user_agent("cockpit-tools-platform-installer-test")
            .build()
            .expect("http client");
        let response = match client.head(&url).send().await {
            Ok(response) => response,
            Err(error) => {
                let message = format!(
                    "cursor download HEAD skipped (network unavailable): {}",
                    error
                );
                if strict {
                    panic!("{}", message);
                }
                eprintln!("{}", message);
                return;
            }
        };
        assert!(
            response.status().is_success() || response.status().as_u16() == 302,
            "unexpected status for cursor download url: {}",
            response.status()
        );
    }
}
