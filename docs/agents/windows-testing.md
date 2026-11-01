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

## What a clone is

Windows 11 Enterprise evaluation, 1280 × 800 on the Basic Display Adapter (no other mode), en-US display language
(the original's MFC strings appear in English there — expected), a fresh registry each time, user `tbuser`.

- **No OpenGL 2 / Direct3D 12**: ours runs through its software runner (`softrun`, our rasteriser into a GDI
  window); the eframe GPU path can't be exercised in a clone.
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
