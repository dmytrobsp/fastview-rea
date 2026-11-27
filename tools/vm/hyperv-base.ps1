<#
hyperv-base.ps1 - builds the base VM that hyperv.ps1 clones, on Hyper-V (a Windows host; run as a member of the
"Hyper-V Administrators" group, no elevation needed):

  1. installs Windows 11 Enterprise Evaluation unattended (Gen 2, 1280 x 800, no network adapter, en-US display
     language, ru-RU system locale and Format, local administrator `tbuser` signed in automatically);
  2. activates the evaluation (it is TIMEBASED_EVAL: with no network it reports "grace time expired" and may shut down
     hourly) through a temporary adapter on the Default Switch, removed again;
  3. provisions it as clone.sh prepared a clone: the original from YOUR copy (its English resource DLL moved to eng\ so
     the UI is Russian), the VC++ 2013 x86 runtime, the records, the harness scripts of tools\vm, ours;
  4. shuts it down and makes its disk read-only: the parent of the clones' differencing disks.

  hyperv-base.ps1 -Iso <Windows ISO> -Orig <folder with FastView.exe and vcredist_vs13_x86.exe>
                  [-Records <folder>] [-Ours <fastview.exe>] [-Root <folder>] [-Name fv-base]

Nothing of the ISO, the original or the records goes into the repository. The guest's password is random, written to
the answers ISO (deleted at the end) and kept only as <Root>\<Name>.cred.xml, encrypted with this Windows user's DPAPI
key (only this user on this machine reads it). Takes about 25 minutes. A 90-day evaluation: renew by building a new
base (see docs/agents/windows-testing.md). To test this script, build under another -Name and -Root, then remove it.

A failed build removes what it made: the VM, its disk, the credential file and the VM's folder under <Root> (the
"Virtual Machines" folder Remove-VM leaves behind among them), only by the exact paths of this -Name, which did not exist
before the run (the script refuses an existing VM, disk or credential file). -KeepOnFail leaves them for a look at the
half-built VM; it then prints the commands that remove them.
#>
param(
    [Parameter(Mandatory)][string]$Iso,
    [string]$Orig = $env:FV_ORIG,
    [string]$Records = $(if ($env:FV_RECORDS) { $env:FV_RECORDS } else { Join-Path $env:USERPROFILE 'Downloads' }),
    [string]$Ours,
    [string]$Root = $(if ($env:FV_VMROOT) { $env:FV_VMROOT } else { Join-Path $env:USERPROFILE 'fastview-vm' }),
    [string]$Name = 'fv-base',
    [string]$Harness = $PSScriptRoot,
    [string]$ActivationSwitch = 'Default Switch',
    [switch]$NoActivate,
    [switch]$KeepOnFail,
    [int]$Cpus = 4,
    [long]$Memory = 4GB,
    [long]$DiskSize = 64GB,
    [int]$InstallTimeoutMin = 75
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if (-not $Orig -or -not (Test-Path (Join-Path $Orig 'FastView.exe')) -or -not (Test-Path (Join-Path $Orig 'vcredist_vs13_x86.exe'))) {
    throw "-Orig (or FV_ORIG) must name your copy of the original: a folder with FastView.exe and vcredist_vs13_x86.exe"
}
if (Get-VM -Name $Name -ErrorAction SilentlyContinue) { throw "a VM named $Name already exists" }
$Iso = (Resolve-Path $Iso).Path
New-Item -ItemType Directory -Force $Root | Out-Null
$vhd = Join-Path $Root "$Name.vhdx"
$credFile = Join-Path $Root "$Name.cred.xml"
if (Test-Path -LiteralPath $vhd) { throw "$vhd already exists" }
if (Test-Path -LiteralPath $credFile) { throw "$credFile already exists" }
$G = 'C:\Users\tbuser\fv-test'

# 1. The guest credential: a random password, kept only as a DPAPI-encrypted credential file.
$alphabet = 'ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789'.ToCharArray()
$bytes = New-Object byte[] 24
[Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($bytes)
$plain = -join ($bytes | ForEach-Object { $alphabet[$_ % $alphabet.Length] })
$cred = New-Object System.Management.Automation.PSCredential('tbuser', (ConvertTo-SecureString $plain -AsPlainText -Force))
$cred | Export-Clixml $credFile

# From here on a failure must not leave the answers ISO (the password in clear) behind: the finally block at the end
# removes it and its staging folder; the catch block says how to remove the half-built VM.
$stage = Join-Path $Root "cfg-$Name"
$cfgIso = Join-Path $Root "autounattend-$Name.iso"
try {
# 2. autounattend.xml on a small ISO (Windows Setup looks for it at the root of every drive).
New-Item -ItemType Directory -Force $stage | Out-Null
$tpl = Get-Content -Raw -Encoding UTF8 (Join-Path $PSScriptRoot 'autounattend.template.xml')
[IO.File]::WriteAllText((Join-Path $stage 'autounattend.xml'), $tpl.Replace('@@PASSWORD@@', $plain), (New-Object Text.UTF8Encoding($false)))
$plain = $null
if (-not ('IsoWriter' -as [type])) {
    Add-Type -TypeDefinition @'
using System; using System.IO; using System.Runtime.InteropServices; using System.Runtime.InteropServices.ComTypes;
public static class IsoWriter {
    public static void Save(object stream, string path) {
        var s = (IStream)stream; var buf = new byte[65536]; IntPtr pRead = Marshal.AllocHGlobal(4);
        try { using (var f = File.Create(path)) { while (true) { s.Read(buf, buf.Length, pRead); int n = Marshal.ReadInt32(pRead); if (n <= 0) break; f.Write(buf, 0, n); } } }
        finally { Marshal.FreeHGlobal(pRead); }
    }
}
'@
}
$fs = New-Object -ComObject IMAPI2FS.MsftFileSystemImage
$fs.FileSystemsToCreate = 4   # UDF
$fs.VolumeName = 'FVCFG'
$fs.Root.AddTree($stage, $false)
[IsoWriter]::Save($fs.CreateResultImage().ImageStream, $cfgIso)
Remove-Item -LiteralPath (Join-Path $stage 'autounattend.xml')
Remove-Item -LiteralPath $stage

# 3. The VM: Gen 2, Secure Boot with the Windows template, no network adapter, 1280 x 800.
$vm = New-VM -Name $Name -Generation 2 -MemoryStartupBytes $Memory -NewVHDPath $vhd -NewVHDSizeBytes $DiskSize -Path $Root
Get-VMNetworkAdapter -VM $vm | Remove-VMNetworkAdapter
Set-VMProcessor -VM $vm -Count $Cpus
Set-VMMemory -VM $vm -DynamicMemoryEnabled $false
Set-VM -VM $vm -AutomaticCheckpointsEnabled $false -CheckpointType Standard -AutomaticStopAction ShutDown
Set-VMVideo -VM $vm -HorizontalResolution 1280 -VerticalResolution 800 -ResolutionType Single
Enable-VMIntegrationService -VM $vm -Name 'Guest Service Interface'
$dvd = Add-VMDvdDrive -VM $vm -Path $Iso -Passthru
Add-VMDvdDrive -VM $vm -Path $cfgIso
Set-VMFirmware -VM $vm -EnableSecureBoot On -SecureBootTemplate MicrosoftWindows -FirstBootDevice $dvd

# 4. Boot, answering "Press any key to boot from CD or DVD" through the VM's keyboard.
Start-VM -VM $vm
$kbd = Get-CimInstance -Namespace root\virtualization\v2 -ClassName Msvm_Keyboard | Where-Object SystemName -eq $vm.Id.ToString().ToUpper()
if (-not $kbd) { $kbd = Get-CimInstance -Namespace root\virtualization\v2 -ClassName Msvm_Keyboard | Where-Object SystemName -eq $vm.Id.ToString() }
# Only for the prompt's first seconds: a later Enter reaches Setup's own window (its focused Cancel button).
for ($i = 0; $i -lt 8; $i++) {
    Start-Sleep -Milliseconds 500
    if ($kbd) { Invoke-CimMethod -InputObject $kbd -MethodName TypeKey -Arguments @{ keyCode = [uint32]13 } | Out-Null }
}
Write-Host "${Name}: installing (up to $InstallTimeoutMin min)"

# 5. Wait until PowerShell Direct answers as tbuser with a desktop (Setup, its restarts and the first sign-in done).
$deadline = (Get-Date).AddMinutes($InstallTimeoutMin)
while ($true) {
    try {
        $r = Invoke-Command -VMName $Name -Credential $cred -ErrorAction Stop -ScriptBlock {
            [pscustomobject]@{ Build = [Environment]::OSVersion.Version.ToString(); Locale = (Get-WinSystemLocale).Name; Culture = (Get-Culture).Name; Explorer = [bool](Get-Process explorer -ErrorAction SilentlyContinue) -and -not (Get-Process FirstLogonAnim -ErrorAction SilentlyContinue) }
        }
        if ($r.Explorer) { break }
    } catch { }
    if ((Get-Date) -gt $deadline) { throw "${Name}: no PowerShell Direct answer with a desktop after $InstallTimeoutMin min" }
    Start-Sleep -Seconds 20
}
Write-Host "${Name}: installed - Windows $($r.Build), system locale $($r.Locale), format $($r.Culture)"
Get-VMDvdDrive -VM $vm | Set-VMDvdDrive -Path $null
Remove-Item -LiteralPath $cfgIso -Force   # it holds the password in clear

# 6. Activation of the evaluation through a temporary adapter (a few seconds of network).
if (-not $NoActivate) {
    $nic = Add-VMNetworkAdapter -VM $vm -SwitchName $ActivationSwitch -Passthru
    try {
        $status = Invoke-Command -VMName $Name -Credential $cred -ScriptBlock {
            $t0 = Get-Date
            while (((Get-Date) - $t0).TotalSeconds -lt 120 -and -not (Get-NetIPAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue | Where-Object { $_.IPAddress -notlike '169.254.*' -and $_.IPAddress -ne '127.0.0.1' })) { Start-Sleep -Seconds 2 }
            & cscript //nologo "$env:windir\System32\slmgr.vbs" /ato | Out-Null
            Start-Sleep -Seconds 5
            (Get-CimInstance SoftwareLicensingProduct -Filter "PartialProductKey IS NOT NULL AND Name LIKE 'Windows%'" | Select-Object -First 1).LicenseStatus
        }
    } finally { Remove-VMNetworkAdapter -VMNetworkAdapter $nic }
    if ($status -ne 1) { throw "${Name}: the evaluation is not activated (license status $status); is '$ActivationSwitch' connected to the internet? (-NoActivate skips this)" }
    Write-Host "${Name}: evaluation activated (license status 1)"
}

# 7. Provisioning, as clone.sh prepared a clone. Layout: C:\Users\tbuser\fv-test\{FastView5 (eng\ holds the English
#    resource DLL), harness (the *.ps1 of tools\vm, shots\), records, out, ours}.
$s = New-PSSession -VMName $Name -Credential $cred
try {
    Invoke-Command -Session $s -ArgumentList $G -ScriptBlock {
        param($G)
        foreach ($d in 'FastView5\eng', 'records', 'out', 'ours', 'harness\shots') { New-Item -ItemType Directory -Force (Join-Path $G $d) | Out-Null }
    }
    foreach ($f in Get-ChildItem $Orig -File) {
        $dst = if ($f.Name -eq 'FastView_ResEng.dll') { "$G\FastView5\eng\" } else { "$G\FastView5\" }
        Copy-Item -ToSession $s -LiteralPath $f.FullName -Destination $dst
    }
    foreach ($f in Get-ChildItem $Harness -Filter *.ps1 -File | Where-Object { $_.Name -notlike 'hyperv*' }) {
        Copy-Item -ToSession $s -LiteralPath $f.FullName -Destination "$G\harness\"
    }
    $recs = @()
    foreach ($r in 'BINARY', 'DOC_1999', 'WIN_1999') {
        foreach ($e in 'cfg', 'dat') {
            $p = Join-Path $Records "$r.$e"
            if (Test-Path -LiteralPath $p) { Copy-Item -ToSession $s -LiteralPath $p -Destination "$G\records\"; $recs += "$r.$e" }
        }
    }
    if ($Ours) { Copy-Item -ToSession $s -LiteralPath $Ours -Destination "$G\ours\fastview.exe" }
    $r = Invoke-Command -Session $s -ArgumentList $G -ScriptBlock {
        param($G)
        $p = Start-Process -FilePath "$G\FastView5\vcredist_vs13_x86.exe" -ArgumentList '/install', '/quiet', '/norestart' -Wait -PassThru
        [pscustomobject]@{
            VcRedist = $p.ExitCode
            Msvcr120 = Test-Path "$env:windir\SysWOW64\msvcr120.dll"
            Mfc120u  = Test-Path "$env:windir\SysWOW64\mfc120u.dll"
            Locale   = (Get-WinSystemLocale).Name
            Culture  = (Get-Culture).Name
            Files    = (Get-ChildItem -Recurse -File $G | Measure-Object).Count
        }
    }
    Write-Host "${Name}: provisioned - vcredist exit $($r.VcRedist), msvcr120 $($r.Msvcr120), mfc120u $($r.Mfc120u), locale $($r.Locale) / $($r.Culture), $($r.Files) files; records: $($recs -join ', ')"
} finally { Remove-PSSession $s }

# 8. Freeze: shut down and make the disk read-only; clones run on differencing disks of it.
Stop-VM -Name $Name
$vhd = (Get-VMHardDiskDrive -VMName $Name).Path
Set-ItemProperty -LiteralPath $vhd -Name IsReadOnly -Value $true
Write-Host "${Name}: done - $vhd is read-only, the parent of clones (credential: $credFile)"
} catch {
    [Console]::Error.WriteLine("${Name}: build failed: $($_.Exception.Message)")
    $vmDir = Join-Path $Root $Name   # New-VM -Path $Root keeps the VM's own folders here
    if ($KeepOnFail) {
        [Console]::Error.WriteLine("To start over, remove what is left (a rerun refuses existing names):")
        [Console]::Error.WriteLine("  Stop-VM -Name $Name -TurnOff -Force; Remove-VM -Name $Name -Force; Remove-Item -LiteralPath '$vhd', '$credFile'; Remove-Item -LiteralPath '$vmDir' -Recurse")
    } else {
        # Everything under these names is this run's own: the pre-checks above refused a VM, a disk or a credential file
        # that already existed.
        try {
            $half = @(Get-VM | Where-Object { $_.Name -ceq $Name }) | Select-Object -First 1
            if ($half) {
                if ($half.State -ne 'Off') { Stop-VM -VM $half -TurnOff -Force }
                Remove-VM -VM $half -Force
            }
            foreach ($f in @($vhd, $credFile)) { if (Test-Path -LiteralPath $f) { Remove-Item -LiteralPath $f -Force } }
            # Remove-VM leaves the VM's folders (an empty "Virtual Machines" one among them): the known sub-folders, when
            # they hold no file, and the VM's folder itself, when nothing else is in it.
            foreach ($sub in 'Virtual Machines', 'Snapshots') {
                $d = Join-Path $vmDir $sub
                if ((Test-Path -LiteralPath $d -PathType Container) -and -not (Get-ChildItem -LiteralPath $d -Recurse -File -Force)) { Remove-Item -LiteralPath $d -Recurse -Force }
            }
            if ((Test-Path -LiteralPath $vmDir -PathType Container) -and -not (Get-ChildItem -LiteralPath $vmDir -Force)) { Remove-Item -LiteralPath $vmDir -Force }
            [Console]::Error.WriteLine("${Name}: removed what the failed build made (-KeepOnFail keeps it)")
        } catch { [Console]::Error.WriteLine("${Name}: cleanup: $($_.Exception.Message)") }
    }
    throw
} finally {
    Get-VMDvdDrive -VMName $Name -ErrorAction SilentlyContinue | Set-VMDvdDrive -Path $null -ErrorAction SilentlyContinue
    if (Test-Path -LiteralPath $cfgIso) { Remove-Item -LiteralPath $cfgIso -Force }
    if (Test-Path -LiteralPath $stage) { Remove-Item -LiteralPath $stage -Recurse -Force }
}
