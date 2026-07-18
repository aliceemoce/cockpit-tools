# Cockpit 编译产物与缓存统一放在 S: 盘（节省 C: 空间）
Set-Variable -Name CTBuildRoot -Value 'S:\dev\cockpit-tools-build' -Scope Script -Force
Set-Variable -Name CockpitCargoTargetDir -Value (Join-Path (Get-Variable CTBuildRoot -Scope Script).Value 'cargo-target\cockpit-tools') -Scope Script -Force
Set-Variable -Name CockpitCargoHome -Value (Join-Path (Get-Variable CTBuildRoot -Scope Script).Value 'cargo-home') -Scope Script -Force
Set-Variable -Name CockpitNpmCache -Value (Join-Path (Get-Variable CTBuildRoot -Scope Script).Value 'npm-cache') -Scope Script -Force
Set-Variable -Name CockpitNodeModulesStore -Value (Join-Path (Get-Variable CTBuildRoot -Scope Script).Value 'node_modules\cockpit-tools') -Scope Script -Force
Set-Variable -Name CockpitReleaseExe -Value (Join-Path (Get-Variable CockpitCargoTargetDir -Scope Script).Value 'release\cockpit-tools.exe') -Scope Script -Force
Set-Variable -Name CockpitRepo -Value 'C:\Users\aliceemoce\dev\cockpit-tools' -Scope Script -Force

function Set-CockpitBuildEnvironment {
    foreach ($d in @(
            $script:CTBuildRoot,
            $script:CockpitCargoTargetDir,
            $script:CockpitCargoHome,
            $script:CockpitNpmCache,
            $script:CockpitNodeModulesStore
        )) {
        if (-not (Test-Path $d)) {
            New-Item -ItemType Directory -Force -Path $d | Out-Null
        }
    }
    $env:CARGO_TARGET_DIR = $script:CockpitCargoTargetDir
    # registry 必须在本机盘：放 S: UNC 会导致 MSVC cl.exe 编译 aws-lc-sys 失败
    $env:CARGO_HOME = 'C:\Users\aliceemoce\AppData\Local\cockpit-cargo-home'
    $env:NPM_CONFIG_CACHE = $script:CockpitNpmCache
}
