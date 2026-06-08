$ErrorActionPreference = 'Stop'

Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Windows.Forms, System.Drawing

$proc = Get-Process -Name cockpit-tools -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $proc) {
    Write-Host 'FAIL: cockpit-tools process not running'
    exit 1
}

$root = [System.Windows.Automation.AutomationElement]::RootElement
$cond = New-Object System.Windows.Automation.PropertyCondition (
    [System.Windows.Automation.AutomationElement]::ProcessIdProperty,
    $proc.Id
)
$win = $root.FindFirst([System.Windows.Automation.TreeScope]::Children, $cond)
if (-not $win) {
    Write-Host 'FAIL: no automation window for cockpit-tools'
    exit 1
}

Write-Host "OK: window='$($win.Current.Name)' pid=$($proc.Id)"

function Walk-Element {
    param(
        [System.Windows.Automation.AutomationElement]$Element,
        [int]$Depth,
        [int]$MaxDepth
    )
    if ($Depth -gt $MaxDepth) { return }

    $pad = ' ' * ($Depth * 2)
    $name = $Element.Current.Name
    $ctype = $Element.Current.ControlType.ProgrammaticName
    $aid = $Element.Current.AutomationId
    $cls = $Element.Current.ClassName
    if ($name -or $aid -or $cls) {
        Write-Host "${pad}${ctype} name='${name}' id='${aid}' class='${cls}'"
    }

    $children = $Element.FindAll(
        [System.Windows.Automation.TreeScope]::Children,
        [System.Windows.Automation.Condition]::TrueCondition
    )
    foreach ($child in $children) {
        Walk-Element -Element $child -Depth ($Depth + 1) -MaxDepth $MaxDepth
    }
}

Write-Host '--- UIA tree (depth 8) ---'
Walk-Element -Element $win -Depth 0 -MaxDepth 8

$rect = $win.Current.BoundingRectangle
$out = Join-Path $env:TEMP 'cockpit-window-only.png'
$bmp = New-Object System.Drawing.Bitmap ([int]$rect.Width), ([int]$rect.Height)
$graphics = [System.Drawing.Graphics]::FromImage($bmp)
$graphics.CopyFromScreen([int]$rect.X, [int]$rect.Y, 0, 0, $bmp.Size)
$bmp.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
Write-Host "OK: window screenshot=$out bytes=$((Get-Item $out).Length)"
