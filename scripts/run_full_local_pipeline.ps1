# 完整本地流水线：对比 + 构建 + 安装 + 验收。日志写入 scripts/local_exec_log.txt
$ErrorActionPreference = "Continue"
$Repo = "C:\Users\aliceemoce\dev\cockpit-tools"
$Log = Join-Path $Repo "scripts\local_exec_log.txt"
$Summary = Join-Path $Repo "scripts\local_exec_summary.json"

function Log($msg) {
    $line = "$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') | $msg"
    Add-Content -Path $Log -Value $line -Encoding UTF8
    Write-Host $line
}

"" | Set-Content $Log -Encoding UTF8
Log "=== full local pipeline ==="

$errors = @()

Log "STEP: taskkill cockpit-tools"
taskkill /F /IM cockpit-tools.exe 2>$null | Out-Null
Start-Sleep -Seconds 2

Log "STEP: verify_cursor_switch_paths.py"
python (Join-Path $Repo "scripts\verify_cursor_switch_paths.py") 2>&1 | ForEach-Object { Log $_ }
$verifyExit = $LASTEXITCODE

Log "STEP: compare_cursor_switch.py"
python (Join-Path $Repo "scripts\compare_cursor_switch.py") 2>&1 | ForEach-Object { Log $_ }
$compareExit = $LASTEXITCODE

Log "STEP: three_way_compare.py"
python (Join-Path $Repo "scripts\three_way_compare.py") 2>&1 | ForEach-Object { Log $_ }
$threeExit = $LASTEXITCODE

Set-Location $Repo
Log "STEP: npm run build"
npm run build 2>&1 | ForEach-Object { Log $_ }
$npmExit = $LASTEXITCODE
if ($npmExit -ne 0) { $errors += "npm exit $npmExit" }

$env:CARGO_TARGET_DIR = "C:\Users\aliceemoce\dev\cargo-target\cockpit-tools"
Set-Location (Join-Path $Repo "src-tauri")
Log "STEP: cargo build --release"
cargo build --release 2>&1 | ForEach-Object { Log $_ }
$cargoExit = $LASTEXITCODE
if ($cargoExit -ne 0) { $errors += "cargo exit $cargoExit" }

$Release = "C:\Users\aliceemoce\dev\cargo-target\cockpit-tools\release\cockpit-tools.exe"
$Installed = Join-Path $env:LOCALAPPDATA "Cockpit Tools\cockpit-tools.exe"
$copyOk = $false
if (Test-Path $Release) {
    New-Item -ItemType Directory -Force -Path (Split-Path $Installed) | Out-Null
    try {
        Copy-Item $Release $Installed -Force
        $copyOk = $true
        Log "STEP: copy release -> installed OK"
    } catch {
        $errors += "copy failed: $_"
        Log "STEP: copy FAILED $_"
    }
} else {
    $errors += "release exe missing"
}

$rel = Get-Item $Release -ErrorAction SilentlyContinue
$ins = Get-Item $Installed -ErrorAction SilentlyContinue
$installedMatch = $false
if ($rel -and $ins) {
    $installedMatch = ($rel.Length -eq $ins.Length) -and ([math]::Abs(($rel.LastWriteTimeUtc - $ins.LastWriteTimeUtc).TotalSeconds) -lt 3)
}

Set-Location $Repo
Log "STEP: verify_acceptance_minimized.py"
python (Join-Path $Repo "scripts\verify_acceptance_minimized.py") 2>&1 | ForEach-Object { Log $_ }
$acceptExit = $LASTEXITCODE

$acceptPath = Join-Path $Repo "scripts\acceptance_verify_report.json"
$acceptReport = $null
if (Test-Path $acceptPath) {
    try {
        $acceptReport = Get-Content $acceptPath -Raw | ConvertFrom-Json
    } catch {
        $errors += "acceptance json parse failed: $_"
    }
}

$summary = [ordered]@{
    verify_paths_exit = $verifyExit
    compare_exit = $compareExit
    three_way_exit = $threeExit
    npm_exit = $npmExit
    cargo_exit = $cargoExit
    copy_ok = $copyOk
    release_size = if ($rel) { $rel.Length } else { $null }
    release_mtime = if ($rel) { $rel.LastWriteTimeUtc.ToString("o") } else { $null }
    installed_size = if ($ins) { $ins.Length } else { $null }
    installed_mtime = if ($ins) { $ins.LastWriteTimeUtc.ToString("o") } else { $null }
    installed_matches_release = $installedMatch
    acceptance_ok = if ($acceptReport) { [bool]$acceptReport.ok } else { $false }
    acceptance_nav = if ($acceptReport) {
        [bool](@($acceptReport.steps) | Where-Object { $_.navigate_cursor_sidebar -eq $true })
    } else { $false }
    acceptance_installed_matches_release = if ($acceptReport) {
        [bool]$acceptReport.installed_matches_release
    } else { $false }
    any_errors = $errors
}
$summary | ConvertTo-Json -Depth 6 | Set-Content $Summary -Encoding UTF8
Log "SUMMARY written to $Summary"
exit $(if ($summary.acceptance_ok -and $verifyExit -eq 0) { 0 } else { 1 })
