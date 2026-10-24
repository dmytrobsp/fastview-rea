# keys.ps1 -Exe <program> [-Open <file>] -Keys "<k1>;<k2>…" [-WaitSec n] [-Shot name.png] [-Leave]
# In the desktop session: start the program (with a file), bring it to the
# front, run each ';'-separated step 1.5 s apart, optionally capture the
# screen, then close it (unless -Leave). A step is SendKeys text, or
# "CLICK x,y" / "RCLICK x,y" / "DCLICK x,y" (screen pixels), "WHEEL x,y,n"
# (n notches, + up), "SHOT name.png" (a capture now). Logs next to itself.
param([Parameter(Mandatory)][string]$Exe, [string]$Open = '', [string]$Keys = '', [int]$WaitSec = 6, [string]$Shot = '', [switch]$Leave)
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$log = Join-Path $here 'keys.log'
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class In {
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, int data, UIntPtr extra);
}
'@
[In]::SetProcessDPIAware() | Out-Null
function Click([int]$x, [int]$y, [uint32]$down, [uint32]$up, [int]$times = 1) {
    [In]::SetCursorPos($x, $y) | Out-Null; Start-Sleep -Milliseconds 120
    for ($i = 0; $i -lt $times; $i++) { [In]::mouse_event($down, 0, 0, 0, [UIntPtr]::Zero); [In]::mouse_event($up, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 60 }
}
"start $(Get-Date -f s) $Exe $Open" | Out-File $log -Encoding utf8
$wsh = New-Object -ComObject WScript.Shell
$p = if ($Open) { Start-Process -FilePath $Exe -ArgumentList "`"$Open`"" -PassThru } else { Start-Process -FilePath $Exe -PassThru }
Start-Sleep -Seconds $WaitSec
$ok = $wsh.AppActivate($p.Id)
"activated $ok pid $($p.Id)" | Out-File $log -Append -Encoding utf8
foreach ($k in ($Keys -split ';')) {
    if (-not $k) { Start-Sleep -Milliseconds 1500; continue }
    if ($k -match '^(R|D)?CLICK (\d+),(\d+)$') {
        $x = [int]$Matches[2]; $y = [int]$Matches[3]
        switch ($Matches[1]) { 'R' { Click $x $y 0x8 0x10 } 'D' { Click $x $y 0x2 0x4 2 } default { Click $x $y 0x2 0x4 } }
    } elseif ($k -match '^WHEEL (\d+),(\d+),(-?\d+)$') {
        [In]::SetCursorPos([int]$Matches[1], [int]$Matches[2]) | Out-Null; Start-Sleep -Milliseconds 120
        $n = [int]$Matches[3]; $step = if ($n -gt 0) { 120 } else { -120 }
        for ($i = 0; $i -lt [Math]::Abs($n); $i++) { [In]::mouse_event(0x800, 0, 0, $step, [UIntPtr]::Zero); Start-Sleep -Milliseconds 80 }
    } elseif ($k -match '^SHOT (.+)$') {
        & (Join-Path $here 'shot.ps1') -Out (Join-Path (Join-Path $here 'shots') $Matches[1])
    } else {
        $wsh.SendKeys($k)
    }
    "step $k" | Out-File $log -Append -Encoding utf8
    Start-Sleep -Milliseconds 1500
}
if ($Shot) { & (Join-Path $here 'shot.ps1') -Out (Join-Path (Join-Path $here 'shots') $Shot) }
if (-not $Leave) { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue }
"done $(Get-Date -f s)" | Out-File $log -Append -Encoding utf8
