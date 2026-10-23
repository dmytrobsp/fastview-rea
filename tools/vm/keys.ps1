# keys.ps1 -Exe <program> [-Open <file>] -Keys "<SendKeys>;<SendKeys>…" [-WaitSec n] [-Shot name.png] [-Leave]
# In the desktop session: start the program (with a file), bring it to the
# front, send each key group (';'-separated, 1.5 s apart), optionally capture
# the screen, then close it (unless -Leave). Writes a log next to itself.
param([Parameter(Mandatory)][string]$Exe, [string]$Open = '', [string]$Keys = '', [int]$WaitSec = 6, [string]$Shot = '', [switch]$Leave)
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$log = Join-Path $here 'keys.log'
"start $(Get-Date -f s) $Exe $Open" | Out-File $log -Encoding utf8
$wsh = New-Object -ComObject WScript.Shell
$p = if ($Open) { Start-Process -FilePath $Exe -ArgumentList "`"$Open`"" -PassThru } else { Start-Process -FilePath $Exe -PassThru }
Start-Sleep -Seconds $WaitSec
$ok = $wsh.AppActivate($p.Id)
"activated $ok pid $($p.Id)" | Out-File $log -Append -Encoding utf8
foreach ($k in ($Keys -split ';')) {
    if ($k) { $wsh.SendKeys($k); "sent $k" | Out-File $log -Append -Encoding utf8; Start-Sleep -Milliseconds 1500 }
}
if ($Shot) { & (Join-Path $here 'shot.ps1') -Out (Join-Path (Join-Path $here 'shots') $Shot) }
if (-not $Leave) { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue }
"done $(Get-Date -f s)" | Out-File $log -Append -Encoding utf8
