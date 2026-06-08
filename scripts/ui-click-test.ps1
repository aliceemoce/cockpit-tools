param(
    [int]$RelX,
    [int]$RelY,
    [string]$Label = 'click'
)

Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes

Add-Type @"
using System;
using System.Runtime.InteropServices;
public class MouseOps {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int X, int Y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint dwFlags, uint dx, uint dy, uint dwData, UIntPtr dwExtraInfo);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
  public const uint MOUSEEVENTF_LEFTDOWN = 0x0002;
  public const uint MOUSEEVENTF_LEFTUP = 0x0004;
  public static void LeftClick(int x, int y) {
    SetCursorPos(x, y);
    mouse_event(MOUSEEVENTF_LEFTDOWN, 0, 0, 0, UIntPtr.Zero);
    mouse_event(MOUSEEVENTF_LEFTUP, 0, 0, 0, UIntPtr.Zero);
  }
}
"@

$proc = Get-Process -Name cockpit-tools -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $proc) { Write-Host 'FAIL no process'; exit 1 }

$root = [System.Windows.Automation.AutomationElement]::RootElement
$cond = New-Object System.Windows.Automation.PropertyCondition (
    [System.Windows.Automation.AutomationElement]::ProcessIdProperty,
    $proc.Id
)
$all = $root.FindAll([System.Windows.Automation.TreeScope]::Children, $cond)
$win = $null
$maxArea = 0
foreach ($candidate in $all) {
    $r = $candidate.Current.BoundingRectangle
    $area = [int]$r.Width * [int]$r.Height
    if ($area -gt $maxArea) {
        $maxArea = $area
        $win = $candidate
    }
}
if (-not $win) { Write-Host 'FAIL no window'; exit 1 }
$rect = $win.Current.BoundingRectangle
[MouseOps]::SetForegroundWindow($win.Current.NativeWindowHandle) | Out-Null
Start-Sleep -Milliseconds 400

$absX = [int]($rect.X + $RelX)
$absY = [int]($rect.Y + $RelY)
[MouseOps]::LeftClick($absX, $absY)
Write-Host "OK: $Label at ($absX,$absY) rel=($RelX,$RelY) window=($([int]$rect.Width)x$([int]$rect.Height))"
