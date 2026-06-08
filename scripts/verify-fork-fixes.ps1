$ErrorActionPreference = 'Stop'
$RepoRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $RepoRoot

$TargetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { 'C:\Users\aliceemoce\dev\cargo-target\cockpit-tools' }
$env:CARGO_TARGET_DIR = $TargetDir
New-Item -ItemType Directory -Force -Path $TargetDir | Out-Null

Write-Host '==> TypeScript typecheck'
npm run typecheck
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host '==> Rust unit tests (platform installer + cursor backup sync)'
Push-Location src-tauri
cargo test platform_installer::tests -- --nocapture
if ($LASTEXITCODE -ne 0) { Pop-Location; exit $LASTEXITCODE }
cargo test cursor_import_backup_sync::tests -- --nocapture
if ($LASTEXITCODE -ne 0) { Pop-Location; exit $LASTEXITCODE }
cargo check
$checkCode = $LASTEXITCODE
Pop-Location
if ($checkCode -ne 0) { exit $checkCode }

Write-Host '==> Static grep guards'
$cursorAccount = Join-Path $RepoRoot 'src-tauri\src\modules\cursor_account.rs'
$grepRestore = Select-String -Path $cursorAccount -Pattern 'restore_missing_accounts_from_backups' -SimpleMatch
if ($grepRestore) {
  Write-Error 'restore_missing_accounts_from_backups still present in cursor_account.rs'
}
$grepKeepLocal = Select-String -Path $cursorAccount -Pattern '保留本地' -SimpleMatch
if ($grepKeepLocal) {
  Write-Error 'refresh still preserves local email in cursor_account.rs'
}
$grepBackupUpload = Select-String -Path $cursorAccount -Pattern 'schedule_local_import_backup_upload' -SimpleMatch
if (-not $grepBackupUpload) {
  Write-Error 'cursor_account.rs missing schedule_local_import_backup_upload after local import'
}

$backupSync = Join-Path $RepoRoot 'src-tauri\src\modules\cursor_import_backup_sync.rs'
if (-not (Test-Path $backupSync)) {
  Write-Error 'cursor_import_backup_sync.rs missing'
}
$grepPrivateRepo = Select-String -Path $backupSync -Pattern 'aliceemoce/cockpit-credentials' -SimpleMatch
if (-not $grepPrivateRepo) {
  Write-Error 'cursor_import_backup_sync.rs missing private repo default'
}

$appTsx = Join-Path $RepoRoot 'src\App.tsx'
$grepAppInstall = Select-String -Path $appTsx -Pattern 'handleInstallMissingAppPath|installMissingPlatform' -SimpleMatch
if ($grepAppInstall) {
  Write-Error 'App.tsx still contains install-missing flow'
}

$settingsPage = Join-Path $RepoRoot 'src\pages\SettingsPage.tsx'
$grepSettingsInstall = Select-String -Path $settingsPage -Pattern 'PlatformInstallButton' -SimpleMatch
if ($grepSettingsInstall) {
  Write-Error 'SettingsPage still contains PlatformInstallButton'
}

$platformInstallBtn = Join-Path $RepoRoot 'src\components\PlatformInstallButton.tsx'
if (Test-Path $platformInstallBtn) {
  Write-Error 'PlatformInstallButton.tsx should be removed'
}

$quickSettings = Join-Path $RepoRoot 'src\components\QuickSettingsPopover.tsx'
$grepQsInstall = Select-String -Path $quickSettings -Pattern 'handleInstallAppPath' -SimpleMatch
if (-not $grepQsInstall) {
  Write-Error 'QuickSettingsPopover missing handleInstallAppPath'
}
$grepQsPathRowInstall = Select-String -Path $quickSettings -Pattern '!getAppPath().trim()' -SimpleMatch
if (-not $grepQsPathRowInstall) {
  Write-Error 'QuickSettingsPopover should install from path row when path is empty'
}
$grepQsHeaderInstallBtn = Select-String -Path $quickSettings -Pattern 'qs-section-header-icon-btn' -SimpleMatch
if ($grepQsHeaderInstallBtn) {
  Write-Error 'QuickSettingsPopover should not install from section header icon'
}

Write-Host '==> Optional strict network probe (set COCKPIT_STRICT_NETWORK_TESTS=1 to fail on CDN block)'
Push-Location src-tauri
cargo test cursor_download_url_is_reachable_when_network_allows -- --nocapture
$networkCode = $LASTEXITCODE
Pop-Location
if ($networkCode -ne 0) { exit $networkCode }

Write-Host 'All verification checks passed.'
