# Turns on automatic sign-in of devuser (for FastView-REA's runs of the original).
# Asks for the password at a hidden prompt, checks it, then writes the Winlogon keys.
# Undo: Set-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon' AutoAdminLogon '0'
#       Remove-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon' DefaultPassword
$ErrorActionPreference = 'Stop'
$sec = Read-Host -AsSecureString 'Password of devuser'
$bstr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($sec)
$plain = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($bstr)
[Runtime.InteropServices.Marshal]::ZeroFreeBSTR($bstr)
Add-Type -AssemblyName System.DirectoryServices.AccountManagement
$ctx = New-Object System.DirectoryServices.AccountManagement.PrincipalContext('Machine')
if (-not $ctx.ValidateCredentials('devuser', $plain)) { Write-Host 'Wrong password - nothing changed.'; exit 1 }
$k = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon'
Set-ItemProperty $k 'AutoAdminLogon' '1'
Set-ItemProperty $k 'DefaultUserName' 'devuser'
Set-ItemProperty $k 'DefaultDomainName' $env:COMPUTERNAME
Set-ItemProperty $k 'DefaultPassword' $plain
# Windows Hello's "passwordless only" setting blocks automatic sign-in.
Set-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\PasswordLess\Device' 'DevicePasswordLessBuildVersion' 0 -Type DWord
Write-Host 'Automatic sign-in is on for devuser (takes effect at the next restart).'
