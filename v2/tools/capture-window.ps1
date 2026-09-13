<#
.SYNOPSIS
    Capture a window to a PNG, DPI-aware.

.DESCRIPTION
    Screen captures of the launcher have repeatedly been wrong in a way that looks like a UI bug:
    a DPI-UNAWARE process is told a scaled-down desktop, so the image is smaller than reality and the
    launcher's left column gets cropped out of it. That was reported twice as missing buttons, and
    neither was true. See docs/COORDINATES.md for the full model.

    This script makes the capturing THREAD per-monitor-aware-V2 first, which is what makes the
    reported window rect and the screen pixels agree. The context is thread-scoped and the script
    ends, so nothing else is affected.

    The window must be visible and not covered — CopyFromScreen reads the screen, so whatever is on
    top is what lands in the image. Focus the window you want first; this script does not raise it,
    deliberately, because raising it would also take focus from the game.

.EXAMPLE
    .\capture-window.ps1 -Title "Seal Tools v2" -Out docs\images\launcher-cards.png
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Title,
    [Parameter(Mandatory = $true)][string]$Out,
    # Pixels of surrounding screen to include, for a bit of window shadow/context.
    [int]$Pad = 0
)

$ErrorActionPreference = 'Stop'

Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class Cap {
    [DllImport("user32.dll", SetLastError=true)] public static extern IntPtr FindWindow(string cls, string win);
    [DllImport("user32.dll", SetLastError=true)] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    // Thread-scoped: the whole point. PER_MONITOR_AWARE_V2 is the -4 pseudo-handle.
    [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr ctx);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
}
'@

# Before anything is measured or read: make THIS thread DPI-aware, so GetWindowRect returns physical
# pixels and CopyFromScreen samples the same space. Do it first — measuring an unaware rect and then
# capturing is exactly the mismatch that produced the cropped images.
[void][Cap]::SetThreadDpiAwarenessContext([IntPtr](-4))

# [NullString]::Value, not $null: PowerShell marshals a plain $null string parameter as "" (an empty
# class name, which matches nothing) instead of NULL (any class). With $null this never finds the
# window and reports it as missing, which reads as "the app is not open".
$hwnd = [Cap]::FindWindow([NullString]::Value, $Title)
if ($hwnd -eq [IntPtr]::Zero) { throw "No window titled '$Title'. Is it open?" }
if (-not [Cap]::IsWindowVisible($hwnd)) { throw "Window '$Title' exists but is not visible (minimised?)." }

$r = New-Object Cap+RECT
if (-not [Cap]::GetWindowRect($hwnd, [ref]$r)) { throw "GetWindowRect failed for '$Title'." }

$x = $r.Left - $Pad
$y = $r.Top - $Pad
$w = ($r.Right - $r.Left) + 2 * $Pad
$h = ($r.Bottom - $r.Top) + 2 * $Pad
if ($w -le 0 -or $h -le 0) { throw "Window '$Title' has an empty rect ($w x $h)." }

$bmp = New-Object System.Drawing.Bitmap $w, $h
$gfx = [System.Drawing.Graphics]::FromImage($bmp)
try {
    $gfx.CopyFromScreen($x, $y, 0, 0, (New-Object System.Drawing.Size $w, $h))
} finally {
    $gfx.Dispose()
}

$dir = Split-Path -Parent $Out
if ($dir -and -not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }

$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$size = (Get-Item $Out).Length
$bmp.Dispose()

Write-Output "Captured '$Title' at $($r.Left),$($r.Top) ${w}x${h} -> $Out ($([math]::Round($size/1KB,1)) KB)"
