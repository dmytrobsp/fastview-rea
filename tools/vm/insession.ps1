# Run a command in the logged-on user's desktop session (ssh sessions have no desktop):
#   insession.ps1 -Exe <path> [-Args "<args>"] [-Wait]
# -Wait blocks until it exits (for captures) and exits with its exit code (124 when -TimeoutSec ran out first, the
# program still running); without -Wait the program keeps running and the exit code is 0.
param([Parameter(Mandatory)][string]$Exe, [string]$Args = '', [switch]$Wait, [int]$TimeoutSec = 120)
$ErrorActionPreference = 'Stop'
$name = 'fvrea-' + [guid]::NewGuid().ToString('N').Substring(0, 8)
$a = if ($Args) { New-ScheduledTaskAction -Execute $Exe -Argument $Args -WorkingDirectory (Split-Path $Exe) } else { New-ScheduledTaskAction -Execute $Exe -WorkingDirectory (Split-Path $Exe) }
$p = New-ScheduledTaskPrincipal -UserId "$env:COMPUTERNAME\$env:USERNAME" -LogonType Interactive -RunLevel Limited
$s = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Hours 8)
Register-ScheduledTask -TaskName $name -Action $a -Principal $p -Settings $s | Out-Null
Start-ScheduledTask -TaskName $name
$code = 0
try {
if ($Wait) {
    $t0 = Get-Date
    Start-Sleep -Milliseconds 300
    while ((Get-ScheduledTask -TaskName $name).State -eq 'Running' -and ((Get-Date) - $t0).TotalSeconds -lt $TimeoutSec) { Start-Sleep -Milliseconds 200 }
    if ((Get-ScheduledTask -TaskName $name).State -eq 'Running') {
        $code = 124
        [Console]::Error.WriteLine("insession: still running after $TimeoutSec s")
    } else {
        # The task result is unsigned: a negative exit code or an NTSTATUS (0xC0000005) is above 0x7FFFFFFF.
        $u = [int64](Get-ScheduledTaskInfo -TaskName $name).LastTaskResult
        $code = if ($u -gt 2147483647) { [int]($u - 4294967296) } else { [int]$u }
    }
}
} finally { Unregister-ScheduledTask -TaskName $name -Confirm:$false -ErrorAction SilentlyContinue }
exit $code
