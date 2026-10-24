# wakeshot.ps1 -Out <png>: wake the display (it turns off when the VM idles and
# the captures then freeze on its last frame), nudge the cursor, capture.
param([Parameter(Mandatory)][string]$Out)
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class W {
  [DllImport("kernel32.dll")] public static extern uint SetThreadExecutionState(uint flags);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
}
'@
[W]::SetProcessDPIAware() | Out-Null
[W]::SetThreadExecutionState(0x80000002) | Out-Null
[W]::SetCursorPos(2550, 700) | Out-Null; Start-Sleep -Milliseconds 300; [W]::SetCursorPos(2555, 705) | Out-Null
Start-Sleep -Seconds 3
& (Join-Path (Split-Path -Parent $MyInvocation.MyCommand.Path) 'shot.ps1') -Out $Out
