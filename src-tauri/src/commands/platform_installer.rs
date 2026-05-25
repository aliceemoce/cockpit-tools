use crate::modules::platform_installer::{
    self, PlatformInstallResult, ResolvedPlatformInstaller,
};
use tauri::AppHandle;

#[tauri::command]
pub fn resolve_platform_installer(platform_id: String) -> Result<ResolvedPlatformInstaller, String> {
    platform_installer::resolve_platform_installer(platform_id)
}

#[tauri::command]
pub async fn download_platform_installer(
    app: AppHandle,
    platform_id: String,
) -> Result<String, String> {
    platform_installer::download_platform_installer(app, platform_id).await
}

#[tauri::command]
pub async fn install_platform_installer(
    app: AppHandle,
    platform_id: String,
    installer_path: String,
) -> Result<PlatformInstallResult, String> {
    platform_installer::install_platform_installer(app, platform_id, installer_path).await
}

#[tauri::command]
pub async fn install_missing_platform(
    app: AppHandle,
    platform_id: String,
) -> Result<PlatformInstallResult, String> {
    platform_installer::install_missing_platform(app, platform_id).await
}

#[tauri::command]
pub fn is_platform_install_supported(platform_id: String) -> Result<bool, String> {
    Ok(platform_installer::is_install_supported(&platform_id))
}
