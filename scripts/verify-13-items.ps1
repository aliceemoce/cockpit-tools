$ErrorActionPreference = 'Continue'
$Repo = 'C:\Users\aliceemoce\dev\cockpit-tools'
$Out = Join-Path $Repo 'scripts\verify-13-items-result.txt'
$env:CARGO_TARGET_DIR = 'C:\Users\aliceemoce\dev\cargo-target\cockpit-tools'
Set-Location $Repo

function Log($msg) { Add-Content -Path $Out -Value $msg; Write-Host $msg }

Remove-Item $Out -Force -ErrorAction SilentlyContinue
Log "=== verify-13-items $(Get-Date -Format o) ==="

# Item checks via file grep
$items = [ordered]@{
  '1-email-api' = @{ file='src-tauri\src\modules\cursor_account.rs'; need='account.email = email'; forbid='保留本地' }
  '2-no-restore' = @{ file='src-tauri\src\modules\cursor_account.rs'; forbid='restore_missing_accounts_from_backups' }
  '2-upload-hook' = @{ file='src-tauri\src\modules\cursor_account.rs'; need='schedule_local_import_backup_upload' }
  '2-upload-repo' = @{ file='src-tauri\src\modules\cursor_import_backup_sync.rs'; need='aliceemoce/cockpit-credentials' }
  '7-path-install' = @{ file='src\components\QuickSettingsPopover.tsx'; need='!getAppPath().trim()' }
  '7-no-header-btn' = @{ file='src\components\QuickSettingsPopover.tsx'; forbid='qs-section-header-icon-btn' }
  '9-no-app-install' = @{ file='src\App.tsx'; forbid='handleInstallMissingAppPath' }
  '11-no-pib' = @{ file='src\components\PlatformInstallButton.tsx'; missing=$true }
  '12-platformInstall' = @{ file='src\components\QuickSettingsPopover.tsx'; need='installMissingPlatform' }
  '8-installer' = @{ file='src-tauri\src\modules\platform_installer.rs'; need='resolve_platform_installer' }
  '10-commands' = @{ file='src-tauri\src\lib.rs'; need='install_missing_platform' }
  '4-windsurf-cache' = @{ file='src\stores\useWindsurfAccountStore.ts'; need='enableAccountsCache: true' }
}

foreach ($key in $items.Keys) {
  $c = $items[$key]
  $path = Join-Path $Repo $c.file
  if ($c.missing) {
    $ok = -not (Test-Path $path)
    Log ("[{0}] {1} file_exists={2}" -f $(if($ok){'PASS'}else{'FAIL'}), $key, (Test-Path $path))
    continue
  }
  if (-not (Test-Path $path)) { Log "[FAIL] $key missing $($c.file)"; continue }
  $text = Get-Content $path -Raw
  $ok = $true
  if ($c.need -and ($text -notmatch [regex]::Escape($c.need))) { $ok = $false }
  if ($c.forbid -and ($text -match [regex]::Escape($c.forbid))) { $ok = $false }
  Log ("[{0}] {1}" -f $(if($ok){'PASS'}else{'FAIL'}), $key)
}

Log '--- npm typecheck ---'
npm run typecheck 2>&1 | Tee-Object -FilePath (Join-Path $Repo 'scripts\verify-typecheck.log') | Out-Null
Log ("typecheck exit=$LASTEXITCODE")

Log '--- cargo tests ---'
Push-Location (Join-Path $Repo 'src-tauri')
cargo test platform_installer::tests -- --nocapture 2>&1 | Tee-Object -FilePath (Join-Path $Repo 'scripts\verify-cargo-test.log') | Out-Null
if ($LASTEXITCODE -eq 0) {
  cargo test cursor_import_backup_sync::tests -- --nocapture 2>&1 | Add-Content (Join-Path $Repo 'scripts\verify-cargo-test.log')
}
Log ("cargo-test exit=$LASTEXITCODE")
Pop-Location

Log '--- resolve cursor installer (rust test output grep) ---'
$testLog = Get-Content (Join-Path $Repo 'scripts\verify-cargo-test.log') -Raw -ErrorAction SilentlyContinue
if ($testLog -match 'test result: ok') { Log '[PASS] rust-tests-ok' } else { Log '[FAIL] rust-tests-ok' }

Log '--- github token ---'
$tok = @('COCKPIT_GITHUB_TOKEN','GITHUB_TOKEN','GH_TOKEN') | Where-Object { [string]::IsNullOrEmpty((Get-Item "Env:$_" -ErrorAction SilentlyContinue).Value) -eq $false }
if ($tok) { Log "[INFO] token-set=$($tok -join ',')" } else { Log '[WARN] no-github-token-upload-untested' }

Log '--- app process ---'
$proc = Get-Process cockpit-tools -ErrorAction SilentlyContinue
if ($proc) { Log "[INFO] cockpit-tools running pid=$($proc.Id)" } else { Log '[INFO] cockpit-tools not-running' }

Log '=== DONE ==='
