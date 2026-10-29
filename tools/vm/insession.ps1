# Run a command in the logged-on user's desktop session (ssh sessions have no desktop):
#   insession.ps1 -Exe <path> [-Args "<args>"] [-Wait]
# -Wait blocks until it exits (for captures); without it, the program keeps running.
param([Parameter(Mandatory)][string]$Exe, [string]$Args = '', [switch]$Wait, [int]$TimeoutSec = 120)
$ErrorActionPreference = 'Stop'
$name = 'fvrea-' + [guid]::NewGuid().ToString('N').Substring(0, 8)
$a = if ($Args) { New-ScheduledTaskAction -Execute $Exe -Argument $Args -WorkingDirectory (Split-Path $Exe) } else { New-ScheduledTaskAction -Execute $Exe -WorkingDirectory (Split-Path $Exe) }
$p = New-ScheduledTaskPrincipal -UserId "$env:COMPUTERNAME\$env:USERNAME" -LogonType Interactive -RunLevel Limited
$s = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Hours 8)
Register-ScheduledTask -TaskName $name -Action $a -Principal $p -Settings $s | Out-Null
Start-ScheduledTask -TaskName $name
if ($Wait) {
    $t0 = Get-Date
    Start-Sleep -Milliseconds 300
    while ((Get-ScheduledTask -TaskName $name).State -eq 'Running' -and ((Get-Date) - $t0).TotalSeconds -lt $TimeoutSec) { Start-Sleep -Milliseconds 200 }
}
Unregister-ScheduledTask -TaskName $name -Confirm:$false
