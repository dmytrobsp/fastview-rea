# Running the original and ours on Windows

Side-by-side runs are how a behaviour of the original is settled when reading the binary leaves doubt, and how a fix
is checked against it. They run in **throwaway Windows clones**, never on the shared Windows VM (it is the maintainer's
and other projects' machine). The scripts are in `tools/vm/` ([tools/vm/README.md](../../tools/vm/README.md) lists
them and the harness keys).

## A clone

`tools/vm/clone.sh` starts a fresh Windows 11 clone through the claim tool named in `CLAUDE.local.md` and prepares it
in about two minutes: the original from your own copy (`extracted/stage1/` of the main checkout; the English resource
DLL moved aside so its UI is Russian), its VC++ 2013 runtime, ru-RU as system locale and Format (the original narrows
names through the thread code page) with one restart, the sample records, the harness and ours
(`target/x86_64-pc-windows-gnu/release/fastview.exe`).

```sh
FV_CLONE=$HOME/.cache/fastview-rea/clone-<unit> FV_PURPOSE="fastview-rea #<N>: <what>" tools/vm/clone.sh new
FV_CLONE=… FV_PURPOSE=… FV_SHOTS=<scratch>/ tools/vm/clone.sh run o "CLICK 40,12;SHOT a.png" WIN_1999.cfg  # the original
FV_CLONE=… FV_PURPOSE=… tools/vm/clone.sh run u "SHOT b.png" WIN_1999.cfg                                 # ours
FV_CLONE=… FV_PURPOSE=… tools/vm/clone.sh ours        # push a rebuilt ours
FV_CLONE=… FV_PURPOSE=… tools/vm/clone.sh rm          # always, when done — also after a failure
```

- **One clone per unit**: its own `FV_CLONE` state file and `FV_PURPOSE` on every call. The file holds the clone's
  key; it is never printed.
- **Room**: each clone takes a 6.5 GB slot; about three fit at once. Check `<claim tool> slot status` before `new`
  and wait (a bounded wait) rather than fail.
- **Run options**: `FV_FRESH=1` drops the saved state first (a first run); `FV_NOMAX=1` leaves the window as first
  shown (otherwise maximised first); record `-` opens none. Keys: `CLICK` / `RCLICK` / `DCLICK` / `DRAG` / `MOVE` /
  `WHEEL` / `SIZE` / `CLOSE` / `SHOT` / `WINS` / `CURSOR` and SendKeys text, 1.5 s apart.
- **Other files**: `push SRC DST`, `pull SRC DST`, `exec CMD` (no desktop), `desk CMD` (in the desktop session; no
  stdout — write to a file and `pull` it).
- **Loops**: the claim tool reads stdin; close it (`</dev/null`) for every call inside a loop.
- **The overlay** (the clone's disk, a file in the host's RAM; the testbox cap is 4 GB and a clone past it is removed
  by the tool): `clone.sh` prints `overlay: X GB of Y GB (N %)` on stderr after each step, with a warning from 60 %
  of the cap (the size is the allocated bytes of the clone's folder, as the tool counts them; the cap is read from the
  tool's `boxes.json`). What keeps it small, without any setting of the clone changed: `ours` pushes the exe once and
  skips the push while its SHA-256 is unchanged (`FV_FORCE=1` pushes anyway); a file pulled from `out\` or
  `harness\shots\` (`pull`, and the `SHOT`s of `run`) is deleted in the clone right after the pull (`FV_KEEP=1`
  keeps it; other paths are never deleted); OneDrive is stopped at the clone's start and before each run. Print only
  the pages a check needs. Windows Update and Defender are not touched: they are the box's, not ours.

## On a Windows host: Hyper-V

Where there is no claim tool (a Windows PC), `tools/vm/hyperv.ps1` does what `clone.sh` does, on the host's own
Hyper-V: a clone is a Gen 2 VM on a differencing disk of a read-only base, ready in about a minute. It needs Windows
Pro / Enterprise with the Hyper-V feature, and your user in the **Hyper-V Administrators** group (no elevation).

**Setup, once** (about 25 minutes, mostly Windows Setup): build the base from a Windows 11 Enterprise Evaluation ISO
and your own copy of the original. The ISO, the original and the records are parameters and never enter the repository.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools\vm\hyperv-base.ps1 -Iso <Windows ISO> `
    -Orig <main checkout>\extracted\stage1 [-Records <folder with WIN_1999.cfg / .dat ...>] [-Ours <fastview.exe>]
```

It installs unattended (`autounattend.template.xml`: 1280 x 800, no network adapter, en-US display language, ru-RU
system locale and Format, user `tbuser` signed in automatically, a random password), activates the evaluation through
a temporary adapter on the `Default Switch` (a few seconds of network, then removed; needs the host's internet),
provisions the guest as `clone.sh` does (original with `eng\` moved aside, VC++ 2013 x86, records, harness, ours) and
makes the disk read-only. Files in `$env:USERPROFILE\fastview-vm` (`FV_VMROOT`): `fv-base.vhdx` and
`fv-base.cred.xml` (the guest credential, encrypted with your Windows user's DPAPI key: only you on this PC read it).
Never start or change the base after that. To test the script itself, build under another `-Name` and `-Root`.

**The commands** are `clone.sh`'s, with the same environment (`FV_CLONE` = a state file naming the clone, one per
clone, so several run at once; `FV_PURPOSE`, `FV_SHOTS`, `FV_FRESH`, `FV_NOMAX`, `FV_KEEP`, `FV_FORCE`):

```powershell
$h = 'tools\vm\hyperv.ps1'; $env:FV_CLONE = "$env:LOCALAPPDATA\fastview-rea\clone-<unit>"
powershell -NoProfile -ExecutionPolicy Bypass -File $h new
powershell -NoProfile -ExecutionPolicy Bypass -File $h run o "SHOT a.png" WIN_1999.cfg    # the original, maximised
powershell -NoProfile -ExecutionPolicy Bypass -File $h run u "SHOT b.png" none           # ours; record `none` (a lone `-` is eaten by powershell.exe)
powershell -NoProfile -ExecutionPolicy Bypass -File $h push SRC DST   # also pull, exec, desk, shot OUT.png, ours
powershell -NoProfile -ExecutionPolicy Bypass -File $h rm             # always, when done
```

- `new` also refreshes the harness scripts from your checkout and pushes ours
  (`target\x86_64-pc-windows-gnu\release\fastview.exe`; not again when its SHA-256 is unchanged).
- `exec CMD` is a `cmd.exe` command (stdout and exit code come back); `desk CMD` runs in the desktop session, with no
  stdout. `shot OUT.png` is the console thumbnail through Hyper-V's WMI at 1280 x 800: no window opens on the host. It is RGB565
  (5-bit red and blue), so colours read from it are approximate; for exact colours use a `SHOT` in the keys of `run` (the
  guest's own full-colour capture).
  `rm` removes the VM, its disk and folder and the state file.
- A step prints the clone's disk size on stderr (a clone costs about 2 GB of the host's disk after a run or two and 4 GB
  of RAM while it runs; `FV_MEM_GB` changes the RAM). Several clones fit as far as the host's RAM does.
- Never run the commands of one clone from two shells at once; a different clone is independent.
- **A clone with the host's GPU: `FV_GPU=1`** (set for `new`; the clone remembers it in its state file). Without it a clone
  has the basic display adapter only, so ours runs in its software runner; with it ours runs on its GPU runner (eframe /
  OpenGL) on the host GPU's partition (GPU-P). `new` picks the partitionable GPU Hyper-V lists (`Get-VMHostPartitionableGpu`;
  a discrete NVIDIA / AMD one before an integrated one; `FV_GPU_NAME=<part of its name or PCI path>` picks another),
  sets the VM's memory-mapped I/O space (1 GB low, 32 GB high, guest-controlled cache types), adds the partition adapter
  before the first start, then copies the host's display driver into the guest (the DriverStore folder named by the
  adapter's `InstalledDisplayDrivers` to `C:\Windows\System32\HostDriverStore\FileRepository\<same name>`, and the driver's
  DLLs that also live in the host's `System32`, plus the vendor's usual ones: `nv*.dll`, `amd*.dll` / `ati*.dll`, `ig*.dll`),
  restarts the guest and checks that the GPU device is OK there. `run u` then leaves out `-Soft` (`FV_GPU=0` on a call
  keeps it: the software runner) and, after the run, prints the process's OpenGL modules (`gl: GPU path (opengl32 +
  <vendor ICD>)`, or a warning when they are not both loaded: the run was not on the GPU; the list is taken before a
  trailing `CLOSE` step). `FV_GPU=1` on a `run` of a clone without the partition fails at once. When ours' GPU start
  fails its launcher stays alive with a hung window and runs the software runner as a child: `keys.ps1` follows the
  child (so the module list and the warning are the child's) and stops both. It costs: a partitionable host GPU with a driver in the DriverStore, about 5
  minutes for `new` and about 7 GB of overlay (the copied driver is most of it; a plain clone is 2 GB), and a clone still
  has no network. The host driver and the guest's must be the same files: renew the clone after a driver update.
- **Exit codes**, the same in `hyperv.ps1` and `clone.sh`: `exec` and `desk` return the guest command's exit code (`desk`
  gives up after 120 s with 124, the command still running). `run` returns 0 when the keys ran; **3** when the program
  died or showed no window before its first key (`keys.ps1` checks within seconds, 30 s for a program that stays
  windowless, and then sends no key: the maximise keys no longer open Windows Search, whose flyout stayed over later
  shots); 124 when it did not finish in 180 s; 4 for another failure of the driver. The log shows why, and the shots
  that exist are still pulled. Non-ASCII output of `exec` can come back mangled from Git Bash. `clone.sh`: `run` gives
  3 and 124 as they are and 4 for any other failure (testbox's own codes included); `desk` passes on testbox's exit
  status (not run here: no testbox on the Windows PC).
- `new` and every `run` stop the clone's OneDrive (clone.sh does too): its setup window takes the foreground of a fresh clone and the keys of `run` (SIZE, CLICK) would act on it. After a restart inside the clone (a DPI change) it starts again at the next sign-in; the next `run` stops it.

**Calling it** - the two shells each break arguments in their own way, so pass a command with quotes or `/options` as
**one** argument:

- From **Git Bash** (what an agent's Bash tool is on a Windows PC), single quotes around the whole command, `FV_*` paths
  as `C:/...` or `/c/...` (do not set `MSYS_NO_PATHCONV`: it stops that conversion). Separate arguments that start with `/`
  are rewritten by MSYS (`/fi` becomes `C:/Program Files/Git/fi`), so this fails and the one-string form works:
  ```sh
  H="powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools/vm/hyperv.ps1"
  FV_CLONE=C:/tmp/c1.state $H exec 'tasklist /fi "imagename eq explorer.exe" /nh'      # works
  FV_CLONE=C:/tmp/c1.state $H exec tasklist /fi "imagename eq explorer.exe" /nh         # MSYS rewrites /fi: fails
  ```
- From **PowerShell**, `powershell.exe -File` strips embedded double quotes (`exec 'tasklist /fi "imagename eq x"'`
  arrives as three arguments, and `run o 'TYPE "a b";SHOT x.png'` takes `b;SHOT x.png` for the record). Call the script
  in-process instead, which passes every argument unchanged:
  ```powershell
  Set-ExecutionPolicy -Scope Process Bypass -Force; $env:FV_CLONE = "$env:LOCALAPPDATA\fastview-rea\clone-<unit>"
  .\tools\vm\hyperv.ps1 exec 'tasklist /fi "imagename eq explorer.exe" /nh'
  ```
  The `powershell -File` lines above are fine for keys and commands without a double quote.
- "No record" is `none` (or `--`), never `-`: powershell.exe refuses a lone `-` before the script runs (a clone.sh brief's
  `-` becomes `none`).

**Limits.** A clone has no network adapter (nothing leaves it, nothing comes in except through PowerShell Direct, which
the commands use). Like the Windows clones `clone.sh` makes on a Linux host it is 1280 x 800 on the basic display adapter: no OpenGL 2, so ours runs
through its software runner (`run u` passes `-Soft`; `FV_GPU=1` above gives a clone the host's GPU). There are no shared-box claims: it is your machine's VMs; remove
every clone you made.

**The 90-day evaluation.** The base is a Windows evaluation: it works for 90 days from its activation (clones show the
remaining time in the watermark; in a clone `cscript //nologo %windir%\system32\slmgr.vbs /xpr` prints the expiry
date). After that Windows warns and may shut down hourly. Renewal is a **new base**: run `hyperv-base.ps1` again (the
same command; it refuses an existing name, so build under a new `-Name` and set `FV_BASE`, or remove the old base
first; a failed build removes what it made, the VM's empty `Virtual Machines` folder included; `-KeepOnFail` keeps it for a look and prints the commands that remove it), then delete the old base's disk once no clone depends on it (`Get-VHD` of a clone shows its parent).

## What a clone is

Windows 11 Enterprise evaluation, 1280 × 800 on the Basic Display Adapter (no other mode), en-US display language
(the original's MFC strings appear in English there — expected), a fresh registry each time, user `tbuser`.

- **No OpenGL 2 / Direct3D 12**: ours runs through its software runner (`softrun`, our rasteriser into a GDI
  window); the eframe GPU path can't be exercised in a clone (except `FV_GPU=1` on a Hyper-V host, above).
- **The small screen** shows size-dependent behaviour a large one never does (the start-up pane layout, tall dialogs,
  the ribbon's collapse).
- **Display scaling**: set `HKCU\Control Panel\Desktop` `LogPixels` (120 = 125 %, 144 = 150 %) and `Win8DpiScaling` =
  1, then restart inside the clone (`exec` then wait for the desktop). 150 % takes effect at 1280 × 800.

## The Windows test executable

`cargo test --no-run --target x86_64-pc-windows-gnu` builds the test executables; push them and run each in the
clone with stdin closed. CI builds Windows but doesn't run its tests, so the milestone's end runs them in a clone.

## Rules

- Never the shared Windows VM; never shut down, restart or revert a shared box. A clone is yours to change (a
  restart, the registry) and is removed afterwards.
- Captures of the original stay in local scratch; they are never committed or attached to issues.
- Records users send (`extracted/samples/`) may be pushed into a clone for a run, never anywhere public.
