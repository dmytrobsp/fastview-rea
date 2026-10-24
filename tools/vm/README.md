# Running the original on a Windows VM

Helpers for COMPLETION_PLAN W4 / W5: run the original Fast View (installed
from your own copy) and ours side by side on a Windows machine reached by
`ssh`, and capture what they show. Nothing here contains any part of the
original.

| script | what |
|---|---|
| `autologon.ps1` | turns on automatic sign-in of the test user (asks for the password at a hidden prompt, checks it first; undo commands at its top) — a desktop session is needed for anything visible |
| `insession.ps1 -Exe <path> [-Args …] [-Wait]` | runs a program in that user's desktop session (an `ssh` session has none) through a one-off scheduled task |
| `shot.ps1 -Out <png>` | captures the whole desktop (DPI-aware) |
| `capture.ps1 -Name <png>` | `shot.ps1` through `insession.ps1`, into `shots\` next to the scripts |
| `drive.ps1 -Exe <program> [-Open <file> \| -ArgLine "<args>"] [-Keys "<s1>;<s2>…"] [-Shot <png>] [-Leave]` | `keys.ps1` in the session: start the program (with a file), run the steps 1.5 s apart — SendKeys text, `CLICK x,y` / `RCLICK x,y` / `DCLICK x,y`, `DRAG x1,y1,x2,y2` (or `HOLD …` then `UP`), `WHEEL x,y,n`, `SHOT name.png` —, capture, close; prints its log |

Install of the original used here (file copy — its own setup wizard needs
clicks): the files of `extracted/stage1/` in a folder, the English resource
DLL moved aside (its presence switches the UI to English), and the VC++ 2013
x86 runtime (`vcredist_vs13_x86.exe /install /quiet /norestart`).

Captures and recordings of the original stay on the VM and in local
scratch; they are never committed.
