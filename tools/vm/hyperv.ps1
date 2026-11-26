<#
hyperv.ps1 - clone.sh on a Windows host: runs of the original and ours in throwaway Hyper-V clones.

  hyperv.ps1 new                     a differencing disk on the base VM (hyperv-base.ps1), boot, wait for the desktop,
                                     refresh the harness scripts and push ours; ready in about a minute
  hyperv.ps1 run o|u "<keys>" [record]
                                     drive.ps1 in the clone: the original (o) or ours (u, in its software runner),
                                     maximised first (not with FV_NOMAX=1), with records\<record> (WIN_1999.cfg by
                                     default, "none" for none; a lone "-" is eaten by powershell.exe -File); FV_FRESH=1 first drops its saved state (a first run);
                                     every SHOT named in the keys lands in $env:FV_SHOTS
  hyperv.ps1 push SRC DST            host file or folder into the clone
  hyperv.ps1 pull SRC DST            clone file or folder to the host (a file in out\ or harness\shots\ is deleted in
                                     the clone after the pull; FV_KEEP=1 keeps it)
  hyperv.ps1 exec CMD...             a cmd.exe command in the clone (no desktop); stdout and the exit code come back
  hyperv.ps1 desk CMD...             the same in the desktop session (no stdout: write to a file and pull it)
  hyperv.ps1 shot OUT.png            the clone's whole screen: the console thumbnail through Hyper-V's WMI at
                                     1280 x 800 (no window on the host; RGB565, so colours are approximate: for exact
                                     ones use a SHOT in the keys of `run`); falls back to the guest's wakeshot.ps1
  hyperv.ps1 ours                    push a rebuilt ours (not again when the SHA-256 is the same; FV_FORCE=1 pushes)
  hyperv.ps1 rm                      remove the clone: VM, differencing disk, folder, state file

Environment (as clone.sh): FV_CLONE = the state file naming the clone (one file per clone: several clones at once;
default %LOCALAPPDATA%\fastview-rea\clone-hyperv); FV_PURPOSE (kept in the VM's notes); FV_SHOTS (default: the current
folder); FV_FRESH; FV_NOMAX; FV_KEEP; FV_FORCE. Hyper-V specific: FV_BASE (default fv-base), FV_VMROOT (default
%USERPROFILE%\fastview-vm: the base disk, its credential file <base>.cred.xml or tbuser.cred.xml, and clones\),
FV_MEM_GB (a clone's memory, default 4). FV_ORIG and FV_RECORDS belong to hyperv-base.ps1: the original and the
records are in the base, a clone inherits them. Needs membership in "Hyper-V Administrators"; no elevation.

Layout in the guest as clone.sh: C:\Users\tbuser\fv-test\{FastView5,harness,records,out,ours}. A clone has no network
adapter. Run it as: powershell -NoProfile -ExecutionPolicy Bypass -File tools\vm\hyperv.ps1 <command> ...
Arguments: give a command with quotes or /options as ONE argument; from Git Bash in single quotes, from PowerShell
without embedded double quotes (see docs/agents/windows-testing.md, "Calling it"). `desk` and `run` exit 0 once the
step was started: read their output / shots to see how it went.
#>
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$Action = if ($args.Count -gt 0) { [string]$args[0] } else { '' }
$Rest = @(if ($args.Count -gt 1) { @($args[1..($args.Count - 1)] | ForEach-Object { [string]$_ }) })

function EnvOr([string]$Name, [string]$Default) {
    $v = [Environment]::GetEnvironmentVariable($Name)
    if ($v) { $v } else { $Default }
}
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$Root = EnvOr 'FV_VMROOT' (Join-Path $env:USERPROFILE 'fastview-vm')
$Base = EnvOr 'FV_BASE' 'fv-base'
$State = EnvOr 'FV_CLONE' (Join-Path $env:LOCALAPPDATA 'fastview-rea\clone-hyperv')
$Shots = EnvOr 'FV_SHOTS' (Get-Location).Path
$Ours = Join-Path $repo 'target\x86_64-pc-windows-gnu\release\fastview.exe'
$G = 'C:\Users\tbuser\fv-test'

function Fail([string]$Message, [int]$Code = 2) { [Console]::Error.WriteLine($Message); exit $Code }

function Get-Cred {
    # tbuser.cred.xml is the live fv-base's credential file (written before files were named after the base).
    foreach ($n in @("$Base.cred.xml") + $(if ($Base -eq 'fv-base') { @('tbuser.cred.xml') } else { @() })) {
        $p = Join-Path $Root $n
        if (Test-Path -LiteralPath $p) { return Import-Clixml $p }
    }
    Fail "no credential file for $Base in $Root (hyperv-base.ps1 writes it; FV_VMROOT, FV_BASE)"
}

# Exactly the form `new` creates: a wildcard or any other name never passes, so no command can reach another VM.
function Test-CloneName([string]$Name) { $Name -cmatch '^fv-clone-[0-9a-f]{6}$' }

function Get-CloneName {
    if (-not (Test-Path -LiteralPath $State)) { Fail 'no clone (run: hyperv.ps1 new)' }
    $n = ((Get-Content -LiteralPath $State -TotalCount 1) -split '\s+')[0]
    if (-not (Test-CloneName $n)) { Fail "$State does not name a clone (expected fv-clone- and six hex digits)" }
    $n
}

# The clone's differencing disk next to the state of the run: its size is what a clone costs the host (stderr).
function Show-Overlay([string]$Name) {
    try {
        $p = (Get-VMHardDiskDrive -VMName $Name).Path
        [Console]::Error.WriteLine(('overlay: {0:N2} GB (differencing disk of {1})' -f ((Get-Item -LiteralPath $p).Length / 1GB), $Name))
    } catch { }
}

function Wait-Desktop([string]$Name, $Cred, [int]$TimeoutSec = 600) {
    $t0 = Get-Date
    $last = ''
    while (((Get-Date) - $t0).TotalSeconds -lt $TimeoutSec) {
        try {
            $ok = Invoke-Command -VMName $Name -Credential $Cred -ErrorAction Stop -ScriptBlock {
                [bool](Get-Process explorer -ErrorAction SilentlyContinue) -and -not (Get-Process FirstLogonAnim -ErrorAction SilentlyContinue)
            }
            if ($ok) { return [int]((Get-Date) - $t0).TotalSeconds }
        } catch {
            $last = $_.Exception.Message
            if ($last -match 'credential is invalid|user name or password is incorrect|logon failure') { throw "${Name}: the guest rejected the credential ($last)" }
        }
        Start-Sleep -Seconds 3
    }
    throw "${Name}: no desktop after $TimeoutSec s (last error: $last)"
}

# A cmd.exe command in the clone: its output is written to the host's stdout, its exit code returned.
function Invoke-Guest([string]$Name, $Cred, [string]$Cmd, [switch]$Desktop) {
    $r = Invoke-Command -VMName $Name -Credential $Cred -ArgumentList $Cmd, $Desktop.IsPresent, $G -ScriptBlock {
        param($Cmd, $Desktop, $G)
        if ($Desktop) {
            Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force
            & "$G\harness\insession.ps1" -Exe "$env:windir\System32\cmd.exe" -Args "/c $Cmd" -Wait
            0
        } else {
            & "$env:windir\System32\cmd.exe" /c $Cmd 2>&1 | ForEach-Object { "$_" }
            $LASTEXITCODE
        }
    }
    $items = @($r)
    if ($items.Count -gt 1) { $items[0..($items.Count - 2)] | ForEach-Object { Write-Host $_ } }
    if ($items.Count -ge 1) { [int]$items[-1] } else { 0 }
}

function Copy-ToGuest($Session, [string]$Src, [string]$Dst) {
    if (-not (Test-Path -LiteralPath $Src)) { throw "no such file or folder: $Src" }
    $isDir = (Get-Item -LiteralPath $Src).PSIsContainer
    Invoke-Command -Session $Session -ArgumentList $Dst, $isDir -ScriptBlock {
        param($Dst, $isDir)
        $parent = if ($Dst.EndsWith('\')) { $Dst } else { Split-Path -Parent $Dst }
        if ($parent) { New-Item -ItemType Directory -Force $parent | Out-Null }
    }
    if ($isDir) { Copy-Item -ToSession $Session -LiteralPath $Src -Destination $Dst -Recurse -Force }
    else { Copy-Item -ToSession $Session -LiteralPath $Src -Destination $Dst -Force }
}

function Copy-FromGuest($Session, [string]$Src, [string]$Dst) {
    $isDir = Invoke-Command -Session $Session -ArgumentList $Src -ScriptBlock { param($Src) Test-Path -LiteralPath $Src -PathType Container }
    $parent = Split-Path -Parent $Dst
    if ($parent -and -not (Test-Path -LiteralPath $parent)) { New-Item -ItemType Directory -Force $parent | Out-Null }
    if ($isDir) { Copy-Item -FromSession $Session -LiteralPath $Src -Destination $Dst -Recurse -Force }
    else { Copy-Item -FromSession $Session -LiteralPath $Src -Destination $Dst -Force }
}

# The decision of tools/vm/cloneutil.py pull_cleanup, ported check for check: only what lies inside out\ or
# harness\shots\ qualifies, never an area itself; a shell metacharacter, a `..`, `*?:` or a component Windows would
# trim to nothing (`.`, a trailing dot or space) never does. Returns the path to delete, or $null.
function Get-PulledDeletable([string]$Src) {
    if (-not $Src) { return $null }
    if ($Src -match '["&|%^<>!\r\n\t]') { return $null }
    if (@($Src.Replace('\', '/').Split('/')) -contains '..') { return $null }
    $s = $Src.Replace('/', '\').TrimEnd('\')
    $g = $G.ToLower() + '\'
    if (-not $s.ToLower().StartsWith($g)) { return $null }
    $rel = $s.ToLower().Substring($g.Length)
    $inArea = $false
    foreach ($a in 'harness\shots', 'out') { if ($rel.StartsWith($a + '\')) { $inArea = $true } }
    if (-not $inArea) { return $null }
    if ($rel -match '[*?:]') { return $null }
    foreach ($part in $rel.Split('\')) {
        if ($part.Trim('. ') -eq '' -or $part.EndsWith('.') -or $part.EndsWith(' ')) { return $null }
    }
    $s
}

function Remove-Pulled($Session, [string]$Src) {
    if ($env:FV_KEEP) { return }
    $s = Get-PulledDeletable $Src
    if (-not $s) { return }
    Invoke-Command -Session $Session -ArgumentList $s -ScriptBlock { param($s) Remove-Item -LiteralPath $s -Recurse -Force -ErrorAction SilentlyContinue }
}

# ours: refreshed in the clone unless its SHA-256 is the same as the host's file.
function Push-Ours($Session) {
    if (-not (Test-Path -LiteralPath $Ours)) { Fail "no $Ours (cargo build -p fastview-gui --release --target x86_64-pc-windows-gnu)" }
    $h = (Get-FileHash -LiteralPath $Ours -Algorithm SHA256).Hash.ToLower()
    if (-not $env:FV_FORCE) {
        $there = Invoke-Command -Session $Session -ArgumentList "$G\ours\fastview.exe" -ScriptBlock {
            param($p) if (Test-Path -LiteralPath $p) { (Get-FileHash -LiteralPath $p -Algorithm SHA256).Hash.ToLower() } else { '' }
        }
        if ($there -eq $h) { [Console]::Error.WriteLine("ours: unchanged (sha256 $($h.Substring(0, 12))), not pushed again"); return }
    }
    Copy-ToGuest $Session $Ours "$G\ours\fastview.exe"
}

# The console thumbnail through Hyper-V's WMI (RGB565), without a console window on the host.
function Save-Thumbnail([string]$Name, [string]$Out, [int]$Width = 1280, [int]$Height = 800) {
    $vm = Get-CimInstance -Namespace root\virtualization\v2 -ClassName Msvm_ComputerSystem -Filter "ElementName='$Name'"
    $vssd = Get-CimAssociatedInstance -InputObject $vm -ResultClassName Msvm_VirtualSystemSettingData | Where-Object VirtualSystemType -eq 'Microsoft:Hyper-V:System:Realized'
    $svc = Get-CimInstance -Namespace root\virtualization\v2 -ClassName Msvm_VirtualSystemManagementService
    $r = Invoke-CimMethod -InputObject $svc -MethodName GetVirtualSystemThumbnailImage -Arguments @{ TargetSystem = $vssd; WidthPixels = [uint16]$Width; HeightPixels = [uint16]$Height }
    if ($r.ReturnValue -ne 0 -or -not $r.ImageData) { throw "thumbnail failed: $($r.ReturnValue)" }
    Add-Type -AssemblyName System.Drawing
    $img = [byte[]]$r.ImageData
    $row = $Width * 2
    if ($img.Length -lt $row * $Height) { throw "thumbnail: $($img.Length) bytes, expected $($row * $Height)" }
    $bmp = New-Object System.Drawing.Bitmap($Width, $Height, [System.Drawing.Imaging.PixelFormat]::Format16bppRgb565)
    $data = $bmp.LockBits((New-Object System.Drawing.Rectangle(0, 0, $Width, $Height)), [System.Drawing.Imaging.ImageLockMode]::WriteOnly, $bmp.PixelFormat)
    for ($y = 0; $y -lt $Height; $y++) {
        [System.Runtime.InteropServices.Marshal]::Copy($img, $y * $row, [IntPtr]($data.Scan0.ToInt64() + $y * $data.Stride), $row)
    }
    $bmp.UnlockBits($data)
    $bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
}

function Remove-Clone([string]$Name) {
    if (-not (Test-CloneName $Name)) { throw "refusing to remove ${Name}: not a clone" }
    # By exact comparison: Get-VM -Name would expand a wildcard.
    $vm = @(Get-VM | Where-Object { $_.Name -ceq $Name }) | Select-Object -First 1
    $disks = @()
    if ($vm) {
        if ($vm.State -ne 'Off') { Stop-VM -VM $vm -TurnOff -Force }
        $disks = @(Get-VMHardDiskDrive -VM $vm | ForEach-Object { $_.Path })
        Remove-VM -VM $vm -Force
    }
    $dir = Join-Path (Join-Path $Root 'clones') $Name
    foreach ($d in $disks) { if ($d -and $d -like "$Root\clones\*") { Remove-Item -LiteralPath $d -Force -ErrorAction SilentlyContinue } }
    $vhd = Join-Path (Join-Path $Root 'clones') "$Name.vhdx"
    if (Test-Path -LiteralPath $vhd) { Remove-Item -LiteralPath $vhd -Force }
    if (Test-Path -LiteralPath $dir) { Remove-Item -LiteralPath $dir -Recurse -Force }
}

function New-Clone {
    if (Test-Path -LiteralPath $State) { Fail "a clone is already recorded in $State (hyperv.ps1 rm first)" }
    $cred = Get-Cred
    $parent = Join-Path $Root "$Base.vhdx"
    if (-not (Test-Path -LiteralPath $parent)) { Fail "no base disk $parent (hyperv-base.ps1 builds it)" }
    $name = 'fv-clone-' + [guid]::NewGuid().ToString('N').Substring(0, 6)
    $dir = Join-Path $Root 'clones'
    New-Item -ItemType Directory -Force $dir | Out-Null
    $vhd = Join-Path $dir "$name.vhdx"
    $mem = [long]([double](EnvOr 'FV_MEM_GB' '4') * 1GB)
    try {
        New-VHD -Differencing -ParentPath $parent -Path $vhd | Out-Null
        $vm = New-VM -Name $name -Generation 2 -MemoryStartupBytes $mem -VHDPath $vhd -Path $dir
        Get-VMNetworkAdapter -VM $vm | Remove-VMNetworkAdapter
        Set-VMProcessor -VM $vm -Count 4
        Set-VMMemory -VM $vm -DynamicMemoryEnabled $false
        Set-VM -VM $vm -AutomaticCheckpointsEnabled $false -AutomaticStopAction TurnOff -Notes (EnvOr 'FV_PURPOSE' 'fastview-rea: runs of the original and ours')
        Set-VMVideo -VM $vm -HorizontalResolution 1280 -VerticalResolution 800 -ResolutionType Single
        Set-VMFirmware -VM $vm -EnableSecureBoot On -SecureBootTemplate MicrosoftWindows
        Enable-VMIntegrationService -VM $vm -Name 'Guest Service Interface'
        New-Item -ItemType Directory -Force (Split-Path -Parent $State) | Out-Null
        Set-Content -LiteralPath $State -Value $name -Encoding ASCII
        Start-VM -VM $vm
        Write-Host "clone ${name}: preparing"
        $sec = Wait-Desktop $name $cred
        # The harness scripts of this checkout (not the base's copies), then ours.
        $s = New-PSSession -VMName $name -Credential $cred
        try {
            foreach ($f in Get-ChildItem $PSScriptRoot -Filter *.ps1 -File | Where-Object { $_.Name -notlike 'hyperv*' }) {
                Copy-ToGuest $s $f.FullName "$G\harness\$($f.Name)"
            }
            if (Test-Path -LiteralPath $Ours) { Push-Ours $s } else { [Console]::Error.WriteLine("ours: no $Ours yet (the base's copy stays)") }
        } finally { Remove-PSSession $s }
        Show-Overlay $name
        Write-Host "clone ${name}: ready ($sec s to the desktop)"
    } catch {
        [Console]::Error.WriteLine("clone: new failed ($($_.Exception.Message)); removing what was made")
        try { Remove-Clone $name } catch { [Console]::Error.WriteLine("clone: cleanup: $($_.Exception.Message)") }
        Remove-Item -LiteralPath $State -Force -ErrorAction SilentlyContinue
        exit 5
    }
}

try {
    switch ($Action) {
        'new' { New-Clone }
        'run' {
            if ($Rest.Count -lt 2 -or $Rest[0] -notin 'o', 'u') { Fail 'usage: hyperv.ps1 run o|u "<keys>" [record]' }
            $name = Get-CloneName; $cred = Get-Cred
            $who = $Rest[0]; $keys = $Rest[1]; $rec = if ($Rest.Count -gt 2) { $Rest[2] } else { 'WIN_1999.cfg' }
            if (-not $env:FV_NOMAX) { $keys = "% ;x;;$keys" }
            $exe = if ($who -eq 'o') { "$G\FastView5\FastView.exe" } else { "$G\ours\fastview.exe" }
            $open = if ($rec -notin '-', '--', 'none') { "$G\records\$rec" } else { '' }
            $out = Invoke-Command -VMName $name -Credential $cred -ArgumentList $G, $exe, $open, ($who -eq 'u'), $keys, $who, [bool]$env:FV_FRESH -ScriptBlock {
                param($G, $exe, $open, $soft, $Keys, $Who, $Fresh)
                Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force
                $H = "$G\harness"
                & powershell -NoProfile -ExecutionPolicy Bypass -File "$H\cleanup.ps1" *> $null
                if ($Fresh) {
                    if ($Who -eq 'o') { Get-ChildItem HKCU:\Software | Get-ChildItem | Where-Object PSChildName -like '*FastView' | Remove-Item -Recurse }
                    else { Remove-Item -Recurse -Force "$env:APPDATA\fastview-rea" -ErrorAction SilentlyContinue }
                }
                # drive.ps1 is called directly with an ARRAY of arguments: it forwards $args as text, so a hashtable
                # (-Soft:True, which keys.ps1 rejects) or a nested `powershell -Command "..."` (which loses the
                # quotes inside -Keys) would break it.
                $a = @('-Exe', $exe)
                if ($open) { $a += @('-Open', $open) }
                if ($soft) { $a += '-Soft' }
                $a += @('-Keys', $Keys)
                & "$H\drive.ps1" @a 2>&1 | Where-Object { "$_" -notmatch '^step' } | Select-Object -Last 3
            }
            @($out) | ForEach-Object { Write-Host $_ }
            $names = @([regex]::Matches($keys, 'SHOT ([^;]+)') | ForEach-Object { $_.Groups[1].Value.Trim() })
            if ($names.Count -gt 0) {
                New-Item -ItemType Directory -Force $Shots | Out-Null
                $s = New-PSSession -VMName $name -Credential $cred
                try {
                    foreach ($f in $names) {
                        Copy-FromGuest $s "$G\harness\shots\$f" (Join-Path $Shots $f)
                        Remove-Pulled $s "$G\harness\shots\$f"
                    }
                } finally { Remove-PSSession $s }
            }
            Show-Overlay $name
        }
        'push' {
            if ($Rest.Count -ne 2) { Fail 'usage: hyperv.ps1 push SRC DST' }
            $name = Get-CloneName; $s = New-PSSession -VMName $name -Credential (Get-Cred)
            try { Copy-ToGuest $s $Rest[0] $Rest[1] } finally { Remove-PSSession $s }
            Show-Overlay $name
        }
        'pull' {
            if ($Rest.Count -ne 2) { Fail 'usage: hyperv.ps1 pull SRC DST' }
            $name = Get-CloneName; $s = New-PSSession -VMName $name -Credential (Get-Cred)
            try { Copy-FromGuest $s $Rest[0] $Rest[1]; Remove-Pulled $s $Rest[0] } finally { Remove-PSSession $s }
            Show-Overlay $name
        }
        'exec' {
            if ($Rest.Count -lt 1) { Fail 'usage: hyperv.ps1 exec CMD...' }
            $name = Get-CloneName
            $code = Invoke-Guest $name (Get-Cred) ($Rest -join ' ')
            Show-Overlay $name
            exit $code
        }
        'desk' {
            if ($Rest.Count -lt 1) { Fail 'usage: hyperv.ps1 desk CMD...' }
            $name = Get-CloneName
            $null = Invoke-Guest $name (Get-Cred) ($Rest -join ' ') -Desktop
            Show-Overlay $name
        }
        'shot' {
            if ($Rest.Count -ne 1) { Fail 'usage: hyperv.ps1 shot OUT.png' }
            $name = Get-CloneName
            $out = [IO.Path]::GetFullPath($(if ([IO.Path]::IsPathRooted($Rest[0])) { $Rest[0] } else { Join-Path (Get-Location).Path $Rest[0] }))
            New-Item -ItemType Directory -Force (Split-Path -Parent $out) | Out-Null
            try { Save-Thumbnail $name $out }
            catch {
                [Console]::Error.WriteLine("shot: thumbnail failed ($($_.Exception.Message)); using the guest's shot.ps1")
                $cred = Get-Cred
                $guestPng = "$G\harness\shots\_shot.png"
                $null = Invoke-Guest $name $cred "powershell -NoProfile -ExecutionPolicy Bypass -File $G\harness\wakeshot.ps1 -Out $guestPng" -Desktop
                $s = New-PSSession -VMName $name -Credential $cred
                try { Copy-FromGuest $s $guestPng $out; Remove-Pulled $s $guestPng } finally { Remove-PSSession $s }
            }
            Write-Host "shot: $out"
        }
        'ours' {
            $name = Get-CloneName; $s = New-PSSession -VMName $name -Credential (Get-Cred)
            try { Push-Ours $s } finally { Remove-PSSession $s }
            Show-Overlay $name
        }
        'rm' {
            $name = Get-CloneName
            Remove-Clone $name
            Remove-Item -LiteralPath $State -Force
            Write-Host "${name}: removed"
        }
        default { (Get-Content -LiteralPath $PSCommandPath -TotalCount 40) -join "`n" | Write-Host; exit 2 }
    }
} catch {
    [Console]::Error.WriteLine("hyperv.ps1 ${Action}: $($_.Exception.Message)")
    exit 1
}
