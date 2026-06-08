# 将 dev release 覆盖到桌面快捷方式指向的安装目录，并跑最小化验收脚本。
$ErrorActionPreference = "Stop"
$Repo = "C:\Users\aliceemoce\dev\cockpit-tools"
$TargetDir = Join-Path $env:LOCALAPPDATA "Cockpit Tools"
$Release = "C:\Users\aliceemoce\dev\cargo-target\cockpit-tools\release\cockpit-tools.exe"
$Installed = Join-Path $TargetDir "cockpit-tools.exe"

Write-Host "==> stop running Cockpit Tools (release + installed paths)"
Get-Process -Name "cockpit-tools" -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 2

Set-Location $Repo
Write-Host "==> verify_cursor_switch_paths.py"
python (Join-Path $Repo "scripts\verify_cursor_switch_paths.py")
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Write-Host "==> npm run build"
npm run build
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$env:CARGO_TARGET_DIR = "C:\Users\aliceemoce\dev\cargo-target\cockpit-tools"
Write-Host "==> cargo build --release"
cargo build --release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

if (-not (Test-Path $Release)) {
    throw "Release exe not found: $Release"
}

New-Item -ItemType Directory -Force -Path $TargetDir | Out-Null
Copy-Item $Release $Installed -Force
$info = Get-Item $Installed
Write-Host "Deployed to: $Installed"
Write-Host "  Length=$($info.Length) LastWriteTime=$($info.LastWriteTime)"

Write-Host "==> verify_acceptance_minimized.py"
python (Join-Path $Repo "scripts\verify_acceptance_minimized.py")
exit $LASTEXITCODE
