# capture.ps1 -Name <file.png>: devuser's desktop into fv-test\harness\shots\<file.png>.
param([Parameter(Mandatory)][string]$Name)
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$dir = Join-Path $here 'shots'
New-Item -ItemType Directory -Force $dir | Out-Null
$out = Join-Path $dir $Name
$ps = "$env:WINDIR\System32\WindowsPowerShell\v1.0\powershell.exe"
& (Join-Path $here 'insession.ps1') -Exe $ps -Args "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$here\shot.ps1`" -Out `"$out`"" -Wait
if (Test-Path $out) { "saved $out" } else { "no capture" }
