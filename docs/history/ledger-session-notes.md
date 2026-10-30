# Session notes from the reconstruction ledger (2026-10-20 – 2026-10-30)

The recovery instructions and session handoffs that used to head and close the ledger, kept as history.
Current state: [PROGRESS.md](../PROGRESS.md); the record itself: [original/](../original/README.md).

## The ledger's recovery section


**Mission.** The user wants the original Mekhanotronika **Fast View 5** reversed
and mapped so there is *tangible proof* that this Rust reconstruction is
**behaviourally identical**, not approximated from screenshots. **Proceed
AUTONOMOUSLY until completion.** Update THIS ledger at every checkpoint so any
later cold session can resume from it alone.

**Project.** `~/projects/fastview-rea` — clean-room, cross-platform (egui/Rust)
reconstruction. Two crates: `fvb-core` (engine), `fastview-gui` (egui app).
Keep ALL of these green at every commit:
- `cargo build --workspace`  (0 warnings)
- `cargo test --workspace`   (currently 402 pass)
- `cargo clippy --workspace` (0)
- `cargo build --workspace --target x86_64-pc-windows-gnu` (0 errors)

**State at handoff.** See the "Next in L9 (HANDOFF …)" paragraph at the end
of L9 for the current state. Work commits straight to `main` (no feature
branches), every commit keeps the gates green, and the publish guard
(`tools/publish_guard.py`, pre-commit / pre-push hooks, CI job) must pass —
see `CLAUDE.md`: never commit the original program or raw decompilation /
disassembly / resource dumps; only the repo's configured email.

**Read these first (context you won't have):**
- Memory notes (in `~/.claude/projects/-home-poniraq-projects-fastview-rea/memory/`):
  `ghidra-rea-setup.md`, `fastview-rea-ui-state.md`, `wayland-gui-capture.md`.
- `docs/original/specs/ui.md`, `docs/original/findings.md`, and the local-only RE dumps in
  `notes/ui/` (`menus.txt`, `dialogs.txt` = 99 decoded dialogs, `strings.txt`,
  `accelerators.txt`). `notes/ui/` and `extracted/` are **gitignored proprietary
  material — never commit them**.

**Reverse-engineering tooling (REA + Ghidra 12.1.3).** The original binary is
`extracted/stage1/FastView.exe`
(sha256 `6c503790d13d9d2d0b80711f27b4daba57b209462cfb626cc5a6f719c150d046`,
image base `0x400000`, 9129 procedures, PDB not loaded → names are `FUN_xxxx`,
use RTTI strings + behaviour). To drive it, call the REA MCP tools:
`open_binary` (path above) once, then `search_strings` (param `pattern`),
`xrefs`, `resolve_containing_procedure`, `procedure_pseudo_code` (param
`procedure`), `read_bytes` (`address`,`length`). Tool JSON is verbose — extract
only the essential logic. If REA/Ghidra is unavailable, see `ghidra-rea-setup`
memory (Flatpak overlay + CLI wrapper `~/projects/slack-rea/tools/rea-ghidra.sh`).

**Per-engine working loop (repeat until the table below is all ✅/⛔):**
1. Decompile the original function(s) for the engine (addresses in the table).
2. Distil its exact behaviour (constants, order, rounding, degrees/radians…).
3. Compare to the Rust module; **reconcile** — fix the Rust to match, add a test
   that pins the behaviour.
4. `cargo test` + `cargo clippy` green (+ Windows build for engine changes).
5. Commit (end message with the Co-Authored-By line from the session reminder).
6. **Update this ledger**: the entry's evidence (address), what you verified, and
   the verdict. Append new entries as `L<n>`.

**NEXT TASK (start here) — L9 UI-layer program (2026-10-21).** The user (a
proficient user of the original) reports the UI still *behaves and presents*
functionally differently — not pixel issues — and directed that the code
earlier dismissed as "MFC boilerplate" be reversed. The UI layer is now an
engine like any other: see **L9** below for the inventory method (every MFC
message map → handler addresses, `tools/msgmaps.py`), the scope split, the
spec file `docs/original/specs/ui-behaviour.md` and the per-item reconciliation status.
Work L9 before the older residuals below.

Older residuals (lower impact; triaged 2026-10-23 — the `.osc` Save-As
(`0x44ca80` writes the record's own bytes, ours copies the file), R/X pairs
on the circle diagram (DIALOG 554, `Doc::rx_pairs`) and the parallel-line
entry in the ОМП UI (L9.11 / L9.12, `omp_params`) are done): OSC time
stamps (read in L9.123's report: L9.124); the frequency-correction variant
(`FUN_004267e0`, `rec+0x874` — see the L9 handoff); (the 401–543 dialogs:
L9.123); a GUI screenshot pass (Wayland
capture is NOT usable unasked on this Hyprland desktop — new windows map
on another workspace and grim captures the user's own screen; ask the user
for a screenshot of the reworked views instead). Done: L2 ✅, L3 🟡 (readers ✅),
L4 ✅, L5 ✅, L6 ✅, L7 ✅, L8 ✅.

**Scope reality.** "Every byte" literally = 9129 funcs of mostly MFC boilerplate;
the *provable* target is the behavioural engines (this ledger's table). `.sto`
reading is ⛔ (no sample file ships — map the header from `GetInfoFromStoHeader`
but it can't be byte-verified).

## Session closes and handoffs (newest first)

Session close 2026-10-30 (L9.392–L9.403, all pushed): VM runs moved to
throwaway testbox clones (`tools/vm/clone.sh`, 1280 × 800); the small
screen surfaced the size-dependent items above. User feedback (GitHub #1–#3,
on v0.3.0) split into issues #4–#7, a new record format filed as #8 (a
Siemens 7KE85 COMTRADE 2013 export; the sample is local only, under
`extracted/samples/7ke85/`). Next: one sub-agent per issue (worktree, its
own clone), each verified, fixed, reviewed and opened as a PR into main.
Known small residuals: ours' first-run frame 2–3 px larger than the
original's at 1280 × 800 (1 px pane offsets when maximised); the finder's
shell namespace; Office 2003 panel / checked-button details.

Session close 2026-10-25 (L9.294–L9.331, all pushed): W4.4's RUN list is
down to the harness-unreachable items (non-A4 preview, splitter cursor,
per-page DC, DEVMODE, a drop on the frame); W2.3 closed (L9.326). Next:
W4.3's remaining dialogs / context menus, W4.5's per-skin look, W2.2 /
W2.5 residuals. Known small looks left: file dialogs unowned (no title
icon, L9.328), «Показать все» / «Скрыть все» without images, egui's
selection over a newline, «Помощь» disabled (no CHM).

Next in L9 (HANDOFF 2026-10-24, night 6; continued to L9.197 the same day — status and what is left: docs/history/completion-plan.md): **public at
github.com/dmytrobsp/fastview-rea** (release v0.1.0 = the 2026-10-22 state;
CI artifacts since then: linux-x86_64, windows-x86_64, macos-universal,
each one flat archive). `main` = HEAD, 405 tests.
**Night 6 (L9.128–L9.197)**, audits as background agents (message boxes,
CMDUI, keys / wheel, context menus, dialog DDX, status bar, docking-pane
menus, the caption) and follow-up reads (frequency track, flags on
non-file channels, the ОМП tracked phasor, `.set` per channel, N): L9.128
Ctrl held → no docking; L9.129 CMDUI rules; L9.130 **Ctrl+C / Ctrl+V
never fired in the real app** (egui-winit sends `Event::Copy` /
`Event::Paste`) + MDI / preview / визир keys, per-message wheel; L9.131
posted exceptions as boxes, an alert queue; L9.132 the nominal-frequency
track; L9.133 DIALOG 353's DDV; L9.134–L9.136 invert / DC / modes on
digital, computed and pending channels; L9.137 own frequency vectors and
the ОМП tracked phasor; L9.138 ОМП lookup ignores case, the merge's beep;
L9.139 ↑ / ↓ place a never-placed ГВ; L9.140 the merge's padding via the
accessor; L9.141 **record times from the readers** (ASCII timestamps,
binary k·1e6/R; a trailing ^Z line is no row — DOC_1999 is 11000
samples); L9.142 **N from rec+0x228**; L9.143 `.set` per channel (whole
ShowFlags, NetFreq); L9.144 / L9.146 context menus (MFC separators, check
column, keyboard); L9.145 formulas name digital channels; L9.147 status
bar formats / persistence; L9.148 DDX as MFC 12 (scanf "%d"), Enter /
Esc in dialogs; L9.149 MFC's pane menu on captions; L9.150 **the caption
row: frameless window, our own caption (FV_SYSTEM_TITLEBAR restores the
system frame)**; L9.151 quick-analysis pages fresh on every open, defaults
by name; L9.152 pair rows in «Каналы», stale pairs purged, `Properties
Height` per diagram; L9.153 **the circle / differential parameter grid as
the original's property grid** (tick box per characteristic); then
vtable-override sweeps over every own class: L9.154 dialog keys / defaults
(«Каналы» has no OK, the finder's default button, Enter in a formula
form); L9.155 **Alt never reaches the ribbon** (`0x448e80`); L9.156 the
grid's clicks; L9.157 deleting channels sweeps the comments; L9.158 the
open / Save As filters; L9.159 an open file is brought forward, tab
right-click; L9.160 DIALOG 353 order; L9.161 wheel over a pane toolbar;
L9.162 **the block state is the record's `.set`, «Нет» first**; L9.163 W /
var leave the circle diagram on a refresh; L9.164 drag auto-scroll; then
COMPLETION_PLAN (W1 batches, W4 runs of the original on the VM —
`tools/vm/`, auto sign-in, ru-RU locale and Format): L9.165 the start-up
pane layout; L9.166 / L9.167 «Каналы» and the quick-analysis pages read to
the end; L9.168 the parameters bar's ✕; L9.169 the `.omp` file is a
settings document; L9.170 channel colours by phase / name / place; L9.171
channels start on no diagram, `Codepage` 1 = ANSI; L9.172 **the `.cfg` /
`.dat` / `.set` byte-identical to the original's on three records**;
L9.173 the save path; L9.174 the planes' pan limit; L9.175 «Изменить
уровень» as read; L9.176 the coefficient dialog's selection / commit;
L9.177 **the main window against the original's capture** (stats panel,
channel 0 selected, title, stat rows); L9.178 «О программе» in the
original's layout with our own texts, W1.6 / W1.2b read; L9.179 the
document tabs inside the MDI area, the status bar's panes; L9.180
aliased waveforms, 1 px lines on whole pixels; L9.181 W1.8 B1 / B6 / B7
(a missing `)`, the boxes' order, the digital fill, the ОМП flags on top);
L9.182 W1.8 B2 / B4 / B5 / B8 / B9 (binary32 units, 32805 edits
`Comments/UserData`, scrolling, merged settings categories); L9.183
W2.18 part (zoom gate, per-key chart writes, the tokenizer's integer);
L9.184 the merge bar's layout, the ОМП tree's labels after a delete;
L9.185 W1.8 B3 / B10 — W1.8 done (the finder's sort, the merge bar's
fill, the group stats width); L9.186 the panes' captions; L9.187 the ribbon's packing / checked
gradient / height; L9.188 `doc+0x1d8` = every analog-kind record (W2.16),
the unknowns table (W3.1); L9.189 formula channels on the frequency
diagram; L9.190 VM runs (no document, single instance); L9.191 «Каналы» rows in
vector order; L9.192 the ОМП flags as computed channels; L9.193 the lanes' icons; L9.194 W3.2 part A; L9.195 the ribbon's tab; L9.196 the view's scroll bars (VM run); L9.197 W3.2 part B.
Lessons: tests that inject `Key::C` passed while the
real app never saw a key — check what egui-winit emits; never chain the
gates and a commit in one command (a commit with clippy warnings slipped
through once, fixed at once); a message box raised by a key must not
take that same key (L9.148).
**Night 5 (L9.107–L9.127), worked continuously with parallel
background research agents (MFC docking / tabs, merge, print, ribbon,
finder, style data, the 401–543 dialogs, engine residuals):** L9.107 the
four diagram panes as docking units of their own and MFC's tab show /
hide rules; L9.108 document tabs reorder (MDI tab groups: L9.263);
L9.109 edge blocks in the docking manager's order; L9.110 printing in the
printer's hardware margins (CUPS PPD, preview); L9.111 merge method 2 as
read, N = own rate ÷ 50 / R div trunc(f), flags baked in; L9.112 print
details (device-px offsets, header name −4 chars, preview at the
printer's scale); L9.113 print dialog fields from the spooler (VM-run);
L9.114 ribbon sizes and the original's labels («Дифферен циальная»,
«Осцилло грамма»); L9.115 keytips (recent docs, disabled inert, «A»
Help, Alt-held); L9.116 the finder's shell folder tree / «Выбрать» / My
Documents / date pickers; L9.117 / L9.119 Office 2007 palettes and ribbon
body / panel bands from MFC's style data (Black = light ribbon in a dark
frame); L9.118 the print look keyed to s ≠ 1; L9.120 the merge's zero
padding and sub-sample shift; L9.121 header T from the printer's stock
font (VM: 16 px); L9.122 «Свойства...» on Windows (not exercised);
L9.123 DIALOG 401–543 all mapped + formula tree icons; L9.124 / L9.125
`.osc` stamps, the trigger sample, the dotted trigger line, labels /
readouts / «Абсолютное время» from the trigger; L9.126 merge sync under
the frequency correction; L9.127 the round 45 × 45 application button.
User-visible: diagram tabs dock anywhere; document tabs reorder; ribbon
labels / sizes / colours / application button; finder tree and
calendars; `.osc` dates and trigger line; Linux print margins; merge
results.
**Earlier sessions:** L9.76–L9.92 (unit text, pending lanes, frequency
candidates, page 539, the circle / differential planes as read — the
swapped differential pairs —, the Ohm split, keytips, ribbon reduction,
docking) and L9.68–L9.75; «P» records SECONDARY by default (L9.57); the
measured-frequency correction on by default with a Гц / Hz channel
(L9.65). The carved MFC DLLs live in `extracted/mfc/`. No user questions
pending.
**How to look at the GUI without running it:** `FV_WINDOW_DUMP=<png>
[FV_WINDOW_RECORD=<cfg>] [FV_WINDOW_STATE=analysis (the sheet, two formula
channels)|comments|group|circle|circletable|circlelegend|diff|difftable|keytips|keytipsmenu|trigger|floating|autohide|dockleft|dockbottom|docktab|docksplit|dockmarkers|omp|ompparams|preview|print|pagesetup|
settings|channels|coef|fileparams|explorer|parambar|vizirs|alert|windows|
paste|vecvalues|skin<k>] [FV_WINDOW_XSCALE=f] [FV_WINDOW_SIZE=w,h] [FV_WINDOW_CLICK=x,y]
[FV_WINDOW_HOVER=x,y] cargo test window_dump`, then Read the PNG (crop with
ImageMagick `convert`); `FV_FRAME_BENCH=<cfg> cargo test --release
frame_bench` times a frame. The user's sample records live outside the
repo (`~/Downloads/*.cfg`; never commit them). Dialog control ids: parse
the PE's RT_DIALOG template locally (python), `notes/ui/dialogs.txt` has
texts only. Local disassembly (git-ignored, never commit): `dumps/fv.asm`
(`tools/regen-dumps.sh`) and `dumps/fv-plain.asm` (`objdump -d -M intel
--no-show-raw-insn`, the form the grep recipes assume: `grep -n "^  <addr>:"`);
a class's virtuals: RTTI `.?AV<Name>@@` → COL → vtable; MFC
ordinals: the export table of `extracted/mfc/mfc120u.dll`, disassemble at
the address (an old listing may still sit in a /tmp scratchpad); MFC
strings / dialogs: `tools/pestr.py` on `extracted/mfc/mfc120rus.dll`.
**Open next** (verify a residual is not already done before starting):
33035's message 239 on channels of different rates
(needs per-channel rates, L9.131 — unreachable in practice: only pending
channels carry another rate and they are excluded); the docking marker
bitmaps (ours drawn); L9.64 MDI arrangements under tabs (WM_MDICASCADE /
TILE — visible effect ND); (the caption row: L9.150); backlog §9 #36 / #37 / #45 leftovers (logo, splash:
proprietary assets). Good recurring pass: render the user's real records
(`~/Downloads/DOC_1999.cfg`, `BINARY.cfg`, `WIN_1999.cfg`) after big
changes.
Not yet run on the Windows VM this night (it was unreachable): the
GUI test exe — run it when the VM is back (windows-vm memory).
(Done: night 6 L9.128–L9.150, night 5 L9.107–L9.127.)
---

**Night 7 (L9.198–L9.235, 2026-10-24, with the VM's desktop session):**
byte-for-byte VM comparisons — Save As of WIN_1999 / DOC_1999 / BINARY and
a revision-2013 record as `.cfg` and `.cff` (the `.cff` data always
binary), the clipboard document, the `.set` after channel edits, a quick
analysis and an ОМП run (values times the shown factor k, flags out of
the `.cfg`, keys moving with their channels), the ОМП report — all
identical; then pixel-by-pixel looks: the ribbon's rows and body, its one
context menu and QAT arrows, keytips, the five pre-2007 looks in Windows
11's system colours, the parameter grid as MFC's property grid, the
characteristic fills as GDI+ blends them (`theme::alpha`, gamma-space),
the frame's splitter-brush bands, the document's sunken edge and
gradient frame, pane toolbars and tab strips, Windows 11 scroll bars,
the print preview inside the document window, levels resolved by shown
name, the splash. Lessons: the VM's Windows display language is English,
so MFC's own strings there are English (ours keeps mfc120rus); ours
cannot run on the VM (OpenGL 1.1, no D3D12 adapter — W4.1c); most
remaining visual gaps are font metrics (egui's DejaVu against GDI's
Tahoma / Segoe UI). Test hooks: `FV_OMP_DUMP` / `FV_OMP_INOM` /
`FV_OMP_REPORT`, `FV_WINDOW_SKIN` / `_BLOCK` / `_LATE`. Tests 460.
