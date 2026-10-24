# cleanup.ps1: stop every FastView (ours or the original) and stuck keys.ps1 runs.
Get-Process fastview -ErrorAction SilentlyContinue | Stop-Process -Force
Get-CimInstance Win32_Process -Filter "Name='powershell.exe'" | Where-Object { $_.CommandLine -like '*keys.ps1*' } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force }
"fastview left: " + (Get-Process fastview -ErrorAction SilentlyContinue | Measure-Object).Count
