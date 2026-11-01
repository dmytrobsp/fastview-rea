# Running the original on a Windows VM

Helpers for the runs of the original on Windows (formerly COMPLETION_PLAN W4 / W5): run the original Fast View (installed
from your own copy) and ours side by side on a Windows machine reached by
`ssh`, and capture what they show. Nothing here contains any part of the
original.

Use a throwaway clone (`clone.sh`): a fresh Windows 11
throwaway VM (the maintainer's testbox tool, `instance new --desktop windows`), 1280 × 800,
user `tbuser`, prepared by the script (the original from your copy, its VC++
runtime, ru-RU locale and Format with one restart, the records, this folder
and ours) in about two minutes, removed after the runs. A shared VM is for
tests that need that machine itself.

```sh
tools/vm/clone.sh new                       # start + prepare (prints its name)
FV_SHOTS=out/ tools/vm/clone.sh run o "SHOT a.png"   # the original, maximised, WIN_1999
FV_NOMAX=1 tools/vm/clone.sh run u "SIZE 900,600;SHOT b.png"  # ours, as first shown
tools/vm/clone.sh ours                      # push a rebuilt ours
tools/vm/clone.sh rm
```

Parallel agents each keep their own clone: `FV_CLONE=~/.cache/fastview-rea/clone-<name>`
and `FV_PURPOSE="fastview-rea: <what>"` on every call (a worktree finds the
original in the main checkout's `extracted/`). `FV_FRESH=1 run …` starts
from no saved state; record `-` opens none. `tools/gates.sh` is the gate
(clippy both targets, tests, publish guard; GATES-OK).

| script | what |
|---|---|
| `autologon.ps1` | turns on automatic sign-in of the test user (asks for the password at a hidden prompt, checks it first; undo commands at its top) — a desktop session is needed for anything visible |
| `insession.ps1 -Exe <path> [-Args …] [-Wait]` | runs a program in that user's desktop session (an `ssh` session has none) through a one-off scheduled task |
| `shot.ps1 -Out <png>` | captures the whole desktop (DPI-aware) |
| `capture.ps1 -Name <png>` | `shot.ps1` through `insession.ps1`, into `shots\` next to the scripts |
| `wakeshot.ps1 -Out <png>` | `shot.ps1` after waking the display (an idle VM turns it off and captures freeze on its last frame); run it through `insession.ps1` |
| `cleanup.ps1` | stops every FastView (ours or the original) and stuck `keys.ps1` runs |
| `drive.ps1 -Exe <program> [-Open <file> \| -ArgLine "<args>"] [-Keys "<s1>;<s2>…"] [-Shot <png>] [-Leave] [-Soft]` | `keys.ps1` in the session: start the program (with a file), run the steps 1.5 s apart — SendKeys text, `CLICK x,y` / `RCLICK x,y[,ms]` / `DCLICK x,y[,ms]` (ms: the gap between the clicks, each press then held 40 ms), `DRAG x1,y1,x2,y2` (or `HOLD …` then `UP`), `WHEEL x,y,n`, `MOVE x,y`, `SIZE w,h`, `CLOSE`, `SHOT name.png`, `WINS f.txt` (top-level windows), `CURSOR f.txt` (the pointer's shape), `FOCUS f.txt` (the front window's keyboard focus: class, visibility, ancestors), `CLIP f.txt` (the clipboard's formats), `TREE f.txt` (the front window's descendants), `FLOATAT x,y` (moves the first floating pane's mini-frame), `CLIPEMF f.emf` (saves the clipboard's metafile), `SCROLL f.txt` (the scroll bars of the front window and its descendants: range, page, position) —, capture, close; prints its log. `-ArgLine` passes a raw argument line instead of `-Open`; `-Soft` starts ours in its software runner (`FV_SOFTWARE=1`) |

Install of the original used here (file copy — its own setup wizard needs
clicks): the files of `extracted/stage1/` in a folder, the English resource
DLL moved aside (its presence switches the UI to English), and the VC++ 2013
x86 runtime (`vcredist_vs13_x86.exe /install /quiet /norestart`).

Captures and recordings of the original stay on the VM and in local
scratch; they are never committed.
