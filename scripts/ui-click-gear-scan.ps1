param([string]$OutDir = $env:TEMP)

Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
Add-Type -AssemblyName System.Windows.Forms, System.Drawing

Add-Type @"
using System;
using System.Runtime.InteropServices;
public class ClickWin {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int X, int Y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint a, uint b, uint c, UIntPtr d);
  public static void Click(int x, int y) {
    SetCursorPos(x,y);
    mouse_event(2,0,0,0,UIntPtr.Zero);
    mouse_event(4,0,0,0,UIntPtr.Zero);
  }
}
"@

$proc = Get-Process cockpit-tools -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $proc) { Write-Host 'NO_APP'; exit 1 }

$root = [System.Windows.Automation.AutomationElement]::RootElement
$cond = New-Object System.Windows.Automation.PropertyCondition (
    [System.Windows.Automation.AutomationElement]::ProcessIdProperty, $proc.Id)
$all = $root.FindAll([System.Windows.Automation.TreeScope]::Children, $cond)
$win = $null; $max = 0
foreach ($c in $all) {
  $r = $c.Current.BoundingRectangle
  $a = [int]$r.Width * [int]$r.Height
  if ($a -gt $max) { $max = $a; $win = $c }
}
$rect = $win.Current.BoundingRectangle
[ClickWin]::SetForegroundWindow($win.Current.NativeWindowHandle) | Out-Null
Start-Sleep -Milliseconds 500

$wx = [int]$rect.X; $wy = [int]$rect.Y; $ww = [int]$rect.Width; $wh = [int]$rect.Height
Write-Host "WINDOW ${wx},${wy} ${ww}x${wh}"

function Shot($name) {
  $path = Join-Path $OutDir $name
  $bmp = New-Object Drawing.Bitmap $ww, $wh
  $g = [Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($wx, $wy, 0, 0, (New-Object Drawing.Size($ww,$wh)))
  $bmp.Save($path)
  return $path
}

$before = Shot 'gear-scan-before.png'
Write-Host "before=$before"

# Toolbar right: scan Y 95-160, X from 75% to 98% width
$hits = @()
foreach ($ry in 100,115,125,135,145) {
  foreach ($rx in @(0.82,0.85,0.88,0.91,0.94,0.97) | ForEach-Object { [int]($ww * $_) }) {
    $ax = $wx + $rx; $ay = $wy + $ry
    [ClickWin]::Click($ax, $ay)
    Start-Sleep -Milliseconds 600
    $after = Shot "gear-scan-${rx}x${ry}.png"
    $b1 = [Drawing.Image]::FromFile($before)
    $b2 = [Drawing.Image]::FromFile($after)
    $diff = 0
    for ($y=0; $y -lt [Math]::Min($b1.Height,$b2.Height); $y+=8) {
      for ($x=0; $x -lt [Math]::Min($b1.Width,$b2.Width); $x+=8) {
        $p1 = $b1.GetPixel($x,$y); $p2 = $b2.GetPixel($x,$y)
        if ([Math]::Abs($p1.R-$p2.R)+[Math]::Abs($p1.G-$p2.G)+[Math]::Abs($p1.B-$p2.B) -gt 40) { $diff++ }
      }
    }
    $b1.Dispose(); $b2.Dispose()
    Write-Host "click rel=($rx,$ry) abs=($ax,$ay) diff=$diff"
    if ($diff -gt 50) {
      $hits += "OPENED at rel=($rx,$ry) abs=($ax,$ay) diff=$diff file=$after"
      Write-Host "HIT: rel=($rx,$ry)"
      break 2
    }
    $before = $after
  }
}

if ($hits.Count -eq 0) { Write-Host 'NO_MODAL_OPENED' } else { $hits | ForEach-Object { Write-Host $_ } }
