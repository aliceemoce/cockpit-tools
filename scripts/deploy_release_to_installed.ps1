# 将 S: release 覆盖到安装目录，并跑验收脚本
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\cockpit-build-paths.ps1"
Set-CockpitBuildEnvironment

$TargetDir = Join-Path $env:LOCALAPPDATA "Cockpit Tools"
$Installed = Join-Path $TargetDir "cockpit-tools.exe"

Write-Host "==> stop running Cockpit Tools"
Get-Process -Name "cockpit-tools" -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 2

Set-Location $CockpitRepo
Write-Host "==> verify_cursor_switch_paths.py"
python (Join-Path $CockpitRepo "scripts\verify_cursor_switch_paths.py")
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "==> build-release.ps1"
& (Join-Path $CockpitRepo "scripts\build-release.ps1")
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

if (-not (Test-Path $CockpitReleaseExe)) {
    throw "Release exe not found: $CockpitReleaseExe"
}

New-Item -ItemType Directory -Force -Path $TargetDir | Out-Null
Copy-Item $CockpitReleaseExe $Installed -Force
$info = Get-Item $Installed
Write-Host "Deployed to: $Installed"
Write-Host "  Length=$($info.Length) LastWriteTime=$($info.LastWriteTime)"

Write-Host "==> verify_acceptance_minimized.py"
python (Join-Path $CockpitRepo "scripts\verify_acceptance_minimized.py")
exit $LASTEXITCODE
