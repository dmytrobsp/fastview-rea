# Completion plan

Where FastView-REA stands against its goal — **behaviour identical to
Fast View 5, proven by reading the original binary** — and what remains, as
workstreams with numbered items and exit criteria. The ledger
(`RECONSTRUCTION_LEDGER.md`) stays the record of *what was found*; this
file is the record of *what is left*. Every commit that closes an item ticks
it here; every checkpoint re-runs the measures and adds a row to the
progress log.

**Priority (the user, 2026-10-24):** the UI must behave and function as
the original's — commands, enable states, dialogs, menus, values, files,
keys and mouse — and still look generally the same. Only small visual
differences are acceptable (font metrics, a pixel here or there): record
those as residuals rather than chase pixel-perfection; visible ones
(layout, missing elements, wrong colours or shapes) are still fixed. Ours'
own «Дополнительно» submenu (our extras) stays.

## Definition of done

1. **Coverage.** Every function of the program's own code (`0x401000` –
   `0x4a8000`) of 50 instructions or more is cited in the notes, or is
   listed in W1's "explained away" list (a library inline, a destructor, a
   thunk) — `tools/coverage.py` reports 0 uncited large and 0 uncited
   medium functions outside that list.
2. **Ledger.** Every entry is ✅, or ⛔ with the reason it cannot be done
   (W6). No 🟡 left.
3. **Unknowns.** Every NOT DETERMINED in the notes is resolved by a static
   read or by a run of the original (W4), or moved to W6 with its reason.
4. **Runtime.** The W4 scenario checklist, run against the original on the
   Windows VM and against ours, matches (differences only from W6).
5. **Gates.** Tests green on Linux and on Windows (the VM run), clippy
   clean on both targets, the publish guard green. CI runs the guard on every
   push and the checks and builds only when something built changed since
   the last commit whose checks passed (notes, tools, indexes and Markdown
   alone skip them); a newer push cancels an older run.

## Measures

```bash
python3 -I tools/coverage.py            # needs dumps/ from tools/regen-dumps.sh
grep '^### ' notes/RECONSTRUCTION_LEDGER.md | grep -c '🟡'
grep -o 'NOT DETERMINED' notes/RECONSTRUCTION_LEDGER.md | wc -l
```

`coverage.py` counts call targets and vtable slots below the library code
as functions and treats a function as covered when the notes cite any
address inside it (`0x…` or `FUN_00…`). It is a floor, not a proof: a cited
function can still hide an unread branch, which W2 and W4 catch. So the
notes cite only what was read: addresses still unread are listed here (the
tool skips this file), never in the ledger or the specs.

## Progress log

| date | commit | own functions cited | instructions in cited functions | large uncited (≥150) | medium uncited (50–149) | ledger ✅ / 🟡 | NOT DETERMINED | Windows tests |
|---|---|---|---|---|---|---|---|---|
| 2026-10-24 | `50472bd` | 523 / 1803 | 60.3 % | 107 / 322 | 474 / 684 | 128 / 36 | 37 | 249 ✓ |
| 2026-10-24 (later) | `36605ec` | 583 / 1803 | 68.1 % | 54 / 322 | 469 / 684 | 137 / 36 | 39 | 256 ✓ |
| 2026-10-24 (night) | `8dadc7d` | 659 / 1803 | 74.9 % | 0 / 322 | 453 / 684 | 163 / 15 | 39 | 256 ✓ |
| 2026-10-24 (W1.8 done) | L9.185 | 1115 / 1803 | 91.8 % | 0 / 322 | 0 / 684 | 170 / 15 | 39 | 260 ✓ |
| 2026-10-24 (handoff) | L9.197 | 1117 / 1803 | 91.8 % | 0 / 322 | 0 / 684 | 182 / 15 | 33 | 260 ✓ (L9.185) |
| 2026-10-24 (W1.7b, W2.19, W2.17 part, W2.10–W2.12) | L9.203 | 1117 / 1803 | 91.8 % | 0 / 322 | 0 / 684 | 188 / 15 | 33 | 275 ✓ |

## W1 — Read the uncited code

Background agents read one batch each (behaviour in prose, addresses as
references), the findings are spot-checked by hand, implemented, tested and
entered in the ledger. A batch is done when every function in it is cited
(or explained away) and its differences are fixed or filed in W2.

Large uncited functions by area (class, or the nearest caller with a known
class; `?` = no labelled caller within four levels), from
`tools/coverage.py` on 2026-10-24:

| area (class or nearest labelled caller) | uncited functions (instructions) |
|---|---|
| ? | `0x4563b0` (677), `0x44dfa0` (461), `0x4897f0` (456), `0x44e920` (420), `0x489de0` (335), `0x424800` (306), `0x47b020` (276), `0x431630` (262), `0x425530` (249), `0x496400` (248), `0x46a350` (228), `0x47ac80` (227), `0x47b7b0` (226), `0x440c30` (215), `0x489510` (213), `0x42ce80` (209), `0x489270` (203), `0x42c580` (202), `0x406c10` (199), `0x416360` (198), `0x456170` (191), `0x490120` (189), `0x4311e0` (186), `0x432d90` (185), `0x42c810` (185), `0x44e680` (178), `0x46ab90` (177), `0x4411c0` (177), `0x440ea0` (177), `0x432a60` (177), `0x410ec0` (177), `0x4a2020` (174), `0x458390` (174), `0x42c9e0` (169), `0x469f80` (168), `0x433340` (165), `0x479040` (164), `0x47b3d0` (160), `0x420410` (160), `0x4a1840` (157), `0x4766e0` (157), `0x478e90` (154), `0x4574f0` (154), `0x455040` (151), `0x410930` (151) |
| ChannelsDlg | `0x433db0` (845), `0x435080` (740), `0x455df0` (310), `0x432270` (228), `0x432020` (221), `0x431970` (203), `0x4593e0` (191), `0x42f020` (190), `0x458a60` (173), `0x4330d0` (173), `0x4588c0` (151) |
| CQAnDiffCursPg | `0x481d60` (1179), `0x485040` (403), `0x485550` (189), `0x484cb0` (166), `0x484920` (166), `0x484730` (163), `0x483f40` (155) |
| map_0x4d1434 | `0x4a38c0` (996), `0x4a2740` (655), `0x411360` (221) |
| DiffDiagramWnd | `0x444630` (882), `0x4441b0` (338), `0x4453e0` (302), `0x42b0c0` (293) |
| CircleDiagramWnd | `0x410140` (228), `0x4103a0` (220), `0x42d460` (183), `0x48ebe0` (182), `0x43d750` (168), `0x4094c0` (152), `0x48e910` (151), `0x42d220` (151) |
| CFastViewDoc | `0x44b300` (590), `0x457810` (242) |
| CFastViewView | `0x45caa0` (252), `0x45ccc0` (241), `0x43f8d0` (162), `0x440a50` (151) |
| CQAnSymmSostPg | `0x48ce30` (331), `0x48d290` (251), `0x48c590` (200) |
| CQAnPhaseSoprotPg | `0x48ac90` (299), `0x48b060` (241), `0x418250` (173) |
| CQAnMezphaseSoedPg | `0x487f50` (271), `0x4882c0` (204) |
| Section_WrongPlaceLocatorData | `0x4a57f0` (287), `0x4a6350` (154) |
| FileFinder22Dlg | `0x46d010` (231), `0x46bae0` (181) |
| AboutWnd | `0x4471e0` (384) |
| AreaItem | `0x403b60` (356) |
| WrongPlaceLocatorDlg | `0x4a7e40` (188), `0x4a7c50` (168) |
| CQAnFormulaEditorPg | `0x432730` (178), `0x4860a0` (172) |
| CParamOscBar | `0x47ed30` (349) |
| FileInfoDlg | `0x496c00` (305) |
| ChanPairDlg | `0x4217d0` (196) |
| Station_WrongPlaceLocatorData | `0x4a5330` (191) |
| DiffDiagPane | `0x443720` (154) |

Batches, in this order (the size of the unread code, user visibility):

- [x] **W1.1** ChannelsDlg area — 11 functions: 4 with behaviour, 7 library code explained away (L9.166).
- [x] **W1.2** Quick-analysis pages — the sixteen functions cited (L9.167); ↑ / ↓ icons fixed.
- [x] **W1.2b** The rest of the quick-analysis pages — read, ours matches (L9.178).
- [x] **W1.3** Circle / differential diagrams — the thirteen cited (L9.170); channel colours fixed.
- [x] **W1.3b** W1.3's remaining differences: the scroll bars after zooming back out are hidden, as ours (VM run, L9.204); the differential scale over [min(c1, c2), …] (L9.205); the pan limit and the scale box's pen (L9.174).
- [x] **W1.4** ОМП — the eight functions (L9.169): the `.omp` format fixed, the toolbar.
- [x] **W1.5** Document and view — the seven cited (L9.175); the level edit and the ruler picks fixed.
- [x] **W1.5b** (L9.227: confirmed on the VM, ours rebound; the original's blank view on an unresolved level is W6) Comments bound by display name at every paint (a rename hides a level / area; the `.set` keeps the old name) — confirm on the VM (put a level on Ia, rename Ia in «Каналы», save, look), then rebind ours.
- [x] **W1.6** Small windows — read (L9.178): About in the original's layout, the finder's reselect, the bar's labels.
- [x] **W1.7** The unlabelled large functions — labelled and read (L9.176); the coefficient dialog fixed.
- [x] **W1.7b** From W1.7 — done in L9.198: the pasted channel's vector phasor, OSC2 channel lines, formula channels as group members, the member paint order, the dissolve after an evaluation. The `.sto` open flow's messages and `-Dst` as a file: L9.207 (the converter stays ⛔).
- [x] **W1.8** The medium functions (50–149 instructions; 453 uncited on 2026-10-24 night) — the same way, in ten batches (`coverage.py --min 50`, split by area; the unlabelled ones by address):
  - [x] B1 unlabelled `0x4035d0`–`0x42cdb0` (58) — L9.181
  - [x] B2 unlabelled `0x42d0f0`–`0x45a720` (58) — L9.182
  - [x] B3 unlabelled `0x45a980`–`0x4a7510` (56) — L9.185
  - [x] B4 circle diagram and its shapes (51) — L9.182
  - [x] B5 differential / frequency / vector diagrams, the property grid, «Пары каналов» (43) — L9.182
  - [x] B6 quick-analysis pages, «Каналы», «Добавить формулу» (42) — L9.181
  - [x] B7 the ОМП engine (`FileInfoDlg` area) and its dialog (30) — L9.181
  - [x] B8 the ОМП data: lines, stations, sections (39) — L9.182
  - [x] B9 document, view, app, frame, merge bar, comment items (51) — L9.182
  - [x] B10 the small windows and library classes (25) — L9.185
- [x] **W1.9** The "explained away" list: functions that need no note, each with a one-line reason — kept per batch in the ledger ("Library code: …" in L9.166–L9.185).

## W2 — Close the 🟡 ledger entries

Reviewed 2026-10-24 (a background read of every 🟡 entry against later
entries and the code): 21 closed (L9.3, L9.4, L9.6, L9.8, L9.9, L9.10,
L9.13, L9.15, L9.22, L9.27, L9.29, L9.40, L9.51, L9.84, L9.89, L9.90, L9.91,
L9.95, L9.98, L9.100, L9.101 — their headings now say so). What is left, as
items (VM ones also in W4.4):

- [x] W2.1 L9.12 — the grid / plane splitter's look (L9.230); its cursor
  cannot be captured (W6).
- [ ] W2.2 L9.14 — the pre-2007 looks 210–214: read the older visual
  managers in mfc120u, or capture each Стиль on the VM (captured
  2026-10-24: done in L9.223 — Windows 11's system colours, the flat
  round button, INFOBK strip, flat status bar, captions; residual there:
  hot / pressed, sunken / gradient checked buttons, panel borders,
  splitter and toolbar strip).
- [ ] W2.3 L9.20 — analog paste with no analog channel selected (with
  W2.16); the paste's start date parsed with the target's revision rule
  (`merge_ui.rs`) — re-read; the clipboard XML's `time_multiplier` /
  `duration_milli_sec` (settled, L9.212) and the merged comment's CR LF (VM:
  done, L9.266 — with the « (вст.)» name and the `%g` ratios).
- [x] W2.4 L9.30 — EMR_STRETCHDIBITS vs EMR_BITBLT in copy's EMF: matched,
  one BitBlt from a memory DC (L9.269, VM-tested).
- [ ] W2.5 L9.31 — phase 2 of loading on the UI thread (derived arrays,
  `.set` formula channels outside the bar); the `.cff` cancelled during its
  split shows «Неправильный формат файла» (done, L9.268, with the ASCII
  reader's byte scan).
- [x] W2.6 L9.34 — MakeUpper folds Cyrillic units and a «ном» channel is not
  vector-eligible (VM runs, L9.210); binary32 with «кВ» (L9.182); string
  3105 is «°» (L9.207).
- [x] W2.7 (L9.231: the preview in the document's window, the panes,
  strips, application button and QAT staying — done) L9.37 — the preview buttons' sizes (static: done, L9.202 — the
  page buttons small); the preview's keytips (P, QI, QO, 2, X, V, C — VM;
  panels ZP, ZZ, ZV when collapsed) and «Стиль» / Help staying: L9.261.
- [x] W2.8 L9.41 — the report's «Осциллограмма:» line: the full path; the
  whole report byte-identical (L9.219).
- [x] W2.9 L9.64 — «Окна...»: not reachable in the original (VM: the
  tabs' ▾ list has no such item, sorted titles with window images) —
  removed from ours (L9.220); the dialog's own questions are moot.
- [ ] W2.10 L9.66 — the print cell ⌊96·s/5⌋ (done, L9.202), the print
  resetting the live zoom (as ours), the differential push consumed by a
  print (done, L9.203), the GDI+ fill's band at s = 1 (`0x43a4a0`, with W5.1).
- [x] W2.11 L9.92 — Ctrl + pin and the plain pin's slide: done, L9.202;
  the animation-off switch is a Remote Desktop session only (W6).
- [x] W2.12 L9.94 — page setup from the default printer, which OK makes the
  application's (L9.203); the «Принтер...» button: absent on Vista+ comdlg
  (a VM look, W4.4).
- [x] W2.13 L9.97 — disabled keytips' ink (VM: F10 with no document): the
  whole keytip at half alpha, and «Г» on «&Главная» (L9.218).
- [x] W2.14 L9.102 — where a tab dragged out lands (VM, mouse driving):
  a pane tab floats with its corner under the pointer (L9.262); a
  document tab makes, joins and empties MDI tab groups (L9.263).
- [x] W2.15 L9.122 — «Свойства...» on the VM, the owner window, the
  orientation merge (with W2.12): no merge (VM); the choice lives with the
  print dialog; ours exercised on the VM (L9.264).
- [x] W2.16 What `doc+0x1d8` counts — settled 2026-10-24 (background
  read, by hand where L9.188 says): every kind-0 record — file analog,
  formula and pasted analog —, `doc+0x1dc` the kind-1 ones; the vector is
  always [kind 0][kind 1] (the readers count by kind; the analog adders
  insert at `rec+0x58` and raise it; the digital adder inserts at
  `rec+0x58` and raises `rec+0x5c`; the paste's insert `0x42e5c0` and the
  remove `0x42ecf0` count by kind). The earlier "file channels only" read
  was wrong. Ours' differences filed in W2.19.
- [x] W2.19 From W2.16 (L9.189, L9.191, L9.192; the rest in L9.199: one
  order over the analog-kind records, a formula channel as the paste
  point, the merged channel's place, the coefficient grid's pasted rows,
  «Канал 1», renaming a pasted lane). Left as residuals: a pasted lane's k
  on its own samples; the rename's row index with pair rows.
- [ ] W2.17 The open residuals of ✅ entries since L9.128 (each entry's
  "Residual:" lines): quick analysis's rollback of its channels on a
  failed recompute (done, L9.201), drag-scroll left / right (done, L9.208), the grid's
  keyboard (done, L9.267), an unknown saved block name in «Тип блока» (done, L9.201),
  the refresh's other callers (most, L9.208), About's text and links (W1.6), the
  frequency dialog's initial enable state (done, L9.201), the ОМП dialog's tree
  icons / geometry (done, L9.209; reader gaps: as ours), a computed channel's colour
  recomputed by place (done, L9.200), an OSC2 record saved as COMTRADE (done, L9.197).

- [ ] W2.18 W1.8's open differences (L9.181 / L9.182). Done in
  L9.183–L9.185: the zoom gate both ways, per-key chart writes, the
  tokenizer's integer, `.omp` gaps, «Каналы»'s rename edit, the merge
  bar's layout and fill, the ОМП tree after a delete, the finder's sort
  and `SortMode`, the settings link, the group stats width, the report's
  «Ктр»; the frequency «Показать все» (no digital, VM run). Left: groups
  by name ignoring case (the dissolve's lookup: L9.198); the ОМП re-run's
  lookup ignoring case (as ours, L9.199's read); the zoom gate's rectangle:
  done, L9.200;
  the splash (done, L9.229: ours, 630 × 400, 2000 ms, topmost); the view scroll bar's
  system menu (done, L9.206; «Канал 1» with formula channels: L9.199; «Скрыть все» and
  «Отсчет углов» over `doc+0x1d8`: already as ours, L9.199's read).

## W3 — Resolve the unknowns

- [x] W3.1 The table (2026-10-24, a background read of every NOT
  DETERMINED / INFERRED / ND in the ledger and the specs): 115 ledger
  marks + 8 in UI_BEHAVIOUR_SPEC; 23 + 5 already settled by later entries
  (their wording to tidy: L4's Ordinal_2948 — case-sensitive, ours folds
  case: a divergence to fix —, L5 ×2, L7, L8 ×2, L3, L2's `arg:`, L9.52,
  L9.79, L9.80, L9.85, L9.97, L9.115, L9.129, L9.134, L9.135b, L9.148b;
  UI spec §10). About 80 distinct open questions remain:
  - **STATIC** (re-read): rec+0x278's default for COMTRADE (L6); `zk:`
    (L6); the Takagi θ with taps `0x49a670` (L7); bitmap 464's indices
    (L9.13); lane clipping when printing / EMF (L9.33); the group stats
    line with unit "none" `0x468b00` / `0x4695c5` (L9.50); computed
    channels' k `ch+0xf0` (L9.69); the unit walk on zero samples `0x425d70`
    (L9.70); the vector magnitude's width (L9.72); the pasted channel's
    ph / ccbm `0x424cb0` (L9.86); #12964 = SelectPrinter? (L9.94); an area
    on an unknown channel `0x4040c0` (L9.96); the flag refresh's other
    callers `0x428490` (L9.103); Repos stretching large buttons
    `0x101227d5` (L9.114); the splitter strip's brush `0x438110`
    (L9.118); pasted samples vs `0x426020` (L9.126); MFC string 61702 and
    the catch handlers' types (L9.131); the ADC flag on computed channels
    (L9.135); DC mean over NaN `0x428795` (L9.136); the merge's padding
    `0x47d990` (L9.137); the "no lane at y" setter path (L9.147); the
    Save As title helper (L9.158); an OSC2 record saved as COMTRADE and
    the binary writer with pending channels (L9.172); the legacy `.omp`
    reader `0x4a45f0` (spec §8.5).
  - **RUN** (VM scenarios, W4.4): ~~`.set` comments by alias~~ (run, L9.227); a
    ~~disabled V scroll bar when the lanes fit~~ (run, L9.196); ~~«МВт» / «ном»
    folding~~ (run, L9.210); the ГВ label's look (L9.42); the 4 × 4
    marks' fill (L9.45); quick-analysis edits and the modified flag
    (L9.55); ~~«Окна...» arrangements under MDI tabs~~ (out of reach, L9.220); ~~the
    `.set` after an ОМП run~~ (run, L9.217); plane labels' background (L9.75); the
    differential table's row order (L9.81); the app menu's keytips
    (L9.90); the ribbon's reduction steps (L9.91); the printed vector
    legend (L9.93); the preview on non-A4 paper (L9.94); GDI's Ellipse box
    (L9.99); the diagram tab order in the workspace (L9.105); the splitter
    cursor (L9.106); the docking divider's limits (L9.109); a paste's
    selection (L9.111); the print dialog's fields / status (L9.113); the
    preview's per-page DC (L9.121); «Свойства»' DEVMODE (L9.122); ~~the
    older looks' app button~~ (run, L9.223); Ctrl over a hot marker (L9.128);
    MDI keys / order (L9.130); a 0 / negative rate in 392 (L9.131,
    L9.148); the `.set` with formula + digital channels (L9.143); menu
    check column, combine with a computed channel, popup keyboard
    (L9.144, L9.146); status bar prompts (L9.147); a group tab strip's
    right-click (L9.149); ~~Aqua's title~~ (run: left, as ours); Enter in quick analysis
    (L9.151); 32806 with no document (L9.152); «Удалить файлы» with
    nothing selected (L9.154); ~~Save As caption~~ (run: the system's own); ~~opening an open
    file~~ (run, L9.190), a drop on the frame (L9.159); the toolbar wheel (L9.161); drag
    auto-scroll at the left / right insets (L9.164); the level edit
    scrolled (L9.175); the caption bars' order (spec §1.3); the ОМП
    channel drop-down with formula channels (spec §8.4).
  - **W6**: landscape margins (our CUPS path), drivers resetting the DC,
    short-row files (no sample), the `.zip` writer (unreachable).
- [x] W3.2 Settle the static ones — part A (rec+0x278, `zk:`, θ, bitmap 464, the print clip → RUN, «none», k, the zero-sample walk, the vector width, #12964, an area on an unknown channel, ph / ccbm): L9.194; part B (`0x428490`'s callers, Repos, the splitter strip, `0x426020` on pending lanes, 61702 / the catches, ADC, NaN DC, the merge's own frequency, the off-lane H pane, the Save As title, OSC2 as COMTRADE, the legacy `.omp`): L9.197. Left: the print clip (→ RUN).
- [ ] W3.3 Run the RUN ones (W4.4) and tidy the settled rows' wording (tidied 2026-10-24: L4, L5 ×2, L7, L8 ×2, L3, L2's `arg:`).

## W4 — Run the original on the Windows VM

The original is installed (2026-10-24): `C:\Users\devuser\fv-test\FastView5`
(the files of `extracted/stage1/`, the English resource DLL moved to
`eng\` so the UI is Russian), VC++ 2013 x86 runtime installed. It runs in devuser's desktop session through `tools/vm/insession.ps1`.

- [x] W4.1 A desktop session and a capture harness (2026-10-24): automatic
  sign-in of devuser (approved by the user), `tools/vm/` (`insession.ps1`
  runs a program in the desktop session, `capture.ps1` / `shot.ps1` grab
  the screen). Captures stay on the VM / local scratch, never committed.
  First capture: the original with no saved workspace — its default pane
  layout (frequency under the view, the differential diagram in the middle,
  vector / circle tabbed at the right), the differential diagram on «Нет»,
  a close button on the record-header strip.
- [x] W4.1a Input driving: `tools/vm/drive.ps1` (SendKeys groups in the
  session) opens records and presses commands in the original.
- [x] W4.1c Ours on the VM: egui_glow needs OpenGL 2 and wgpu a Direct3D 12
  adapter; the VM's basic display driver offers neither (L9.229). The
  software runner (`softrun`: our rasteriser into a GDI window) runs ours
  there and on any GPU-less Windows; the harness drives it (`-Soft`), L9.271.
- [x] W4.1b The VM's system locale and devuser's Format set to ru-RU (user
  approved, 2026-10-24) — the original stores names through the thread
  code page, so both are needed for its Cyrillic.
- [ ] W4.2 File-level comparisons (Save As of WIN_1999, DOC_1999, BINARY:
  `.cfg` / `.dat` / `.set` byte-identical since L9.172; the clipboard document
  (L9.212), the `.set` after channel-menu edits (L9.213) and after a quick
  analysis (L9.214) too, and after an ОМП run, Inom 1000 and 1 (L9.217);
  DOC_1999 / BINARY raw and re-saved, a revision-2013 record as `.cfg`
  and `.cff` (L9.221); left: the `.osc` Save As (no OSC2 sample)) (started 2026-10-24: WIN_1999 saved by
  both through Save As — `FV_SAVE_AS` makes ours; differences found: Ctrl+S
  always asks with a lowercase name, the same-name save rewrites the
  record, and the `.cfg` / `.dat` / `.set` formats, see the save-path read
  in progress) — the strongest checks, no pixels: the
  same record saved by both (Save As `.cfg` / `.cff` / `.osc`, the `.set`
  after the same edits, the clipboard document, the quick-analysis and ОМП
  channels) compared byte for byte or value for value.
- [ ] W4.3 Scenario checklist (each: the original's result recorded, ours
  compared — both on the VM since L9.271; done: the ribbon's states with
  and without a document, the application menu, the lane menu, L9.272;
  the plot's right-click, the ОМП tab, the pane caption menu, L9.273;
  «Каналы», «Коэффициенты», «Быстрый анализ», L9.274; the application
  menu's dialogs and the finder's tree, L9.275; a block picked in the
  differential grid, L9.276; «Печать» and the print preview, L9.277; the workspace after a
  restart, L9.278; the визир by mouse and keys, L9.279; the circle
  diagram with a block, L9.280; an ОМП run, L9.281; copy, paste and merge, L9.282; synchronising; every circle
  block, L9.283–L9.285; the ribbon's value commands and the wheel,
  L9.286):
  start-up window and panes; opening each sample record;
  every ribbon command and its enable state with and without a document;
  every dialog (fields, defaults, Enter / Esc, refusals); context menus
  of the view, lanes, diagrams, captions; keys and the wheel; визиры and
  comments; the four diagrams with each block; merge; ОМП; quick
  analysis; print preview; the workspace after a restart.
- [ ] W4.4 Each W3 runtime unknown as a scenario, and W2's VM checks
  (W2.1, W2.3, W2.6, W2.7, W2.8, W2.9, W2.13, W2.14, W2.15).
- [ ] W4.5 Visual comparison of the main window and the dialogs
  (layout, sizes, colours per skin) — differences either fixed or W6.
  Seen in the first capture: the start-up pane layout (fixed, L9.165);
  the record-header strip's close button (fixed, L9.168). The main window
  with WIN_1999 against the original's (L9.177): stats panel, channel 0
  selected, title, stat rows, grid height fixed. The frame layout (tab
  strip inside the MDI area under the parameters bar) and the status bar
  (153 / 218 px panes): fixed, L9.179; 1 px strokes on pixel centres and
  the waveform texture: fixed, L9.180. The pane captions (20 px, flat glyphs): fixed, L9.186. The ribbon's packing, checked gradient and height: fixed, L9.187.
  The «*K» icon and the diagrams' 11 px font: fixed, L9.193. The tab row:
  The frequency bars' value labels: L9.235.
  L9.195. The QAT's customise arrow, its print split button and the
  ribbon's one context menu: L9.216. The parameter grid: L9.222. The
  plot's fills (stacked GDI+ alpha) and the check box: L9.224 (the scale
  differed only by the pane's height). The ribbon's rows: L9.225.
  The dialogs' Windows frame, their CenterWindow placement and DIALOGs
  463 / 477 on their templates: L9.240–L9.241; 392 and the controls'
  Windows looks (labels, combos): L9.242; 510 and 332 (group boxes,
  radios, the multi-line edit): L9.243; the message boxes: L9.244; 353
  and the single-line edit: L9.245; 336 (ОМП): L9.246; 347 (Печать) and
  the check box: L9.247; the system's Page Setup: L9.248; «Быстрый
  анализ»'s frame: L9.249, its formula page: L9.250, its other pages:
  L9.251; 359 and the TreeView, owner-centred placement: L9.252, its
  hosted forms L9.253 and L9.260; the
  finder (558): L9.255–L9.256; 554, 492: L9.257; MFC's colour dialog:
  L9.258; the 26 individually laid-out forms: L9.260. Every reachable
  dialog now sits on its template (335, 357, 480, 509 and «Окна» are
  never shown by the original: L9.123, L9.220, L9.257). Residual: the
  glyphs (our font, W6 — wider text overruns or wraps where Tahoma does
  not), the colour dialog's «Выбор...» (screen read) and its «Спектр»
  page drawn by us, the shell tree's own scroll bars and icons, the
  disabled controls' looks and the progress bar's colours (approximated).
  Open: the main window's remaining pixel residuals (the plane centre,
  the stats panel width — fonts).

## W5 — Windows-only paths

- [ ] W5.1 Real printing through GDI (a printer that writes a file, e.g.
  Microsoft Print to PDF with a fixed output name) — ours against the
  original's pages.
- [x] W5.2 The clipboard formats both put on the clipboard: the same two, and
  the copied document byte-identical but for the original's uninitialised
  `normal_state` (VM run, L9.212).
- [x] W5.3 Registry / settings persistence across runs: the original's key
  read after a clean exit on the VM — the same settings and defaults as ours
  (L9.211); a settings file instead of the registry, our own layout record
  instead of MFC's docking blobs.

## W6 — Out of reach (with the reason)

- `.sto` (Configurator-MT) records — no sample ships; the reader stays ⛔.
- The proprietary bitmaps (logo, splash, About picture) — assets of the
  original; ours draws its own.
- Where a dropped file lands — egui reports no drop position (L9.159).
- Glyph metrics: settled — Wine's LGPL Tahoma, chosen by the user, sized as
  GDI's em (L9.270); left: Segoe UI where the original uses it (ours
  Tahoma, about 4 % narrower).
- A level or area whose channel name no longer resolves blanks the
  original's oscillogram view (VM, L9.227) — ours leaves that element out
  instead of reproducing the paint failure.
- Exact glyph rasterisation — egui is not GDI; layout and metrics are
  matched, pixels are not.
- The comment bubble bitmap (L9.6), the docking markers 17117–17127 (MFC's
  own bitmaps, L9.101), the ribbon / menu images, the help (Помощь — the CHM
  is not shipped, L9.14, L9.115).
- The scroll-bar repeat timing (system settings, L9.40).
