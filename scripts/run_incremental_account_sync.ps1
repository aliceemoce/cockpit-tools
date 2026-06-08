$ErrorActionPreference = 'Stop'
$env:COCKPIT_DATA_DIR = "$env:USERPROFILE\.antigravity_cockpit"
$env:COCKPIT_CREDENTIALS_DIR = "$env:USERPROFILE\dev\cockpit-credentials"
python "$PSScriptRoot\incremental_account_sync.py"
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
