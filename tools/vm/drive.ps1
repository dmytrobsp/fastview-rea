# drive.ps1 <keys.ps1 arguments>: keys.ps1 in the desktop session, waiting for it; prints its log and exits with its
# exit code (3: the program died or showed no window before the first key; 124: it did not finish in 180 s).
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$ps = "$env:WINDIR\System32\WindowsPowerShell\v1.0\powershell.exe"
$a = ($args | ForEach-Object { if ($_ -match '\s') { "`"$_`"" } else { $_ } }) -join ' '
& (Join-Path $here 'insession.ps1') -Exe $ps -Args "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$here\keys.ps1`" $a" -Wait -TimeoutSec 180
$code = $LASTEXITCODE
Get-Content (Join-Path $here 'keys.log')
exit $code
