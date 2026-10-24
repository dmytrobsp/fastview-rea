# Completion plan

Where FastView-REA stands against its goal — **behaviour identical to
Fast View 5, proven by reading the original binary** — and what remains, as
workstreams with numbered items and exit criteria. The ledger
(`RECONSTRUCTION_LEDGER.md`) stays the record of *what was found*; this
file is the record of *what is left*. Every commit that closes an item ticks
it here; every checkpoint re-runs the measures and adds a row to the
progress log.

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
   clean on both targets, the publish guard green.

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
- [ ] **W1.3b** W1.3's remaining differences: scroll bars disabled (not hidden) after zooming back out (check on the VM first), the differential scale over [min(c1, c2), …] (the pan limit and the scale box's pen: done, L9.174).
- [x] **W1.4** ОМП — the eight functions (L9.169): the `.omp` format fixed, the toolbar.
- [x] **W1.5** Document and view — the seven cited (L9.175); the level edit and the ruler picks fixed.
- [ ] **W1.5b** Comments bound by display name at every paint (a rename hides a level / area; the `.set` keeps the old name) — confirm on the VM (put a level on Ia, rename Ia in «Каналы», save, look), then rebind ours.
- [x] **W1.6** Small windows — read (L9.178): About in the original's layout, the finder's reselect, the bar's labels.
- [x] **W1.7** The unlabelled large functions — labelled and read (L9.176); the coefficient dialog fixed.
- [ ] **W1.7b** From W1.7: groups pruned after every formula evaluation (`0x455c70`) and computed channels as group members (with L9.144); a pasted channel's vector phasor from its own cache (`0x490120`); OSC2 channel lines (field counts, digital normal state, no phase, the empty-name quirk); the `.sto` open flow's messages and `-Dst` as a file (reader stays ⛔); group members painted in name order.
- [ ] **W1.8** The medium functions (50–149 instructions, 474 uncited) — the same way, batch by area once W1.1–W1.7 are done.
- [ ] **W1.9** The "explained away" list: functions that need no note, each with a one-line reason.

## W2 — Close the 🟡 ledger entries

Reviewed 2026-10-24 (a background read of every 🟡 entry against later
entries and the code): 21 closed (L9.3, L9.4, L9.6, L9.8, L9.9, L9.10,
L9.13, L9.15, L9.22, L9.27, L9.29, L9.40, L9.51, L9.84, L9.89, L9.90, L9.91,
L9.95, L9.98, L9.100, L9.101 — their headings now say so). What is left, as
items (VM ones also in W4.4):

- [ ] W2.1 L9.12 — the grid / plane splitter's look and cursor (VM).
- [ ] W2.2 L9.14 — the pre-2007 looks 210–214: read the older visual
  managers in mfc120u, or capture each Стиль on the VM.
- [ ] W2.3 L9.20 — analog paste with no analog channel selected (with
  W2.16); the paste's start date parsed with the target's revision rule
  (`merge_ui.rs`) — re-read; the clipboard XML's `time_multiplier` /
  `duration_milli_sec` and the merged comment's CR LF (VM).
- [ ] W2.4 L9.30 — EMR_STRETCHDIBITS vs EMR_BITBLT in copy's EMF (decide:
  match or accept).
- [ ] W2.5 L9.31 — phase 2 of loading on the UI thread (derived arrays,
  `.set` formula channels outside the bar); the `.cff` cancelled during its
  split shows «Неправильный формат файла» (ours silent).
- [ ] W2.6 L9.34 — MakeUpper on Cyrillic units («МВт»), binary32 with
  «кВ», «ном» vector-eligibility (VM); string 3105's «гр» spelling (static).
- [ ] W2.7 L9.37 — the preview buttons' sizes (static, mfc120u
  `0x100fcfc2`); QAT / app button / MDI tabs during preview (VM).
- [ ] W2.8 L9.41 — the report's «Осциллограмма:» line: path or title (VM).
- [ ] W2.9 L9.64 — «Окна...»: Каскадом / Горизонтально / Вертикально /
  Свернуть under MDI tabs (VM), OK's slot, LBS_SORT collation, the size from
  the 316 × 169 DLU template.
- [ ] W2.10 L9.66 — the print cell ⌊96·s/5⌋ (ours a fixed 19), the print
  resetting the live zoom, the differential push consumed by a print, the
  GDI+ fill's offset band.
- [ ] W2.11 L9.92 — Ctrl + pin (`0x100871c7`'s Ctrl branch); the
  animation-off switch.
- [ ] W2.12 L9.94 — page setup's orientation into the printer's DEVMODE;
  the «Принтер...» button (flags 0x260a).
- [ ] W2.13 L9.97 — disabled keytips' ink (VM: F10 with no document).
- [ ] W2.14 L9.102 — where a tab dragged out lands (VM, mouse driving).
- [ ] W2.15 L9.122 — «Свойства...» on the VM, the owner window, the
  orientation merge (with W2.12).
- [ ] W2.16 Whether `doc+0x1d8` counts computed channels (shared by
  L9.20, L9.151, L9.154, L9.166). Read 2026-10-24: only the readers raise it
  (`.cfg` `0x451181` / `0x451250`, `.osc` `0x4529f8` / `0x452b65`, with
  `doc+0x1dc`; zeroed `0x44b95a`, `0x4966f8`) — it counts the file's
  channels (analog + digital); added formula / pasted channels do not raise
  it. With the vector analog → computed → digital (L9.145) a loop over the
  first `doc+0x1d8` entries takes the computed channels only up to the
  number of digital ones. To do: confirm where formula channels are
  inserted, then apply to the pair combos, the circle refresh and the paste.
- [ ] W2.17 The open residuals of ✅ entries since L9.128 (each entry's
  "Residual:" lines): quick analysis's rollback of its channels on a
  failed recompute (L9.157), drag-scroll left / right (L9.164), the grid's
  keyboard (L9.156), an unknown saved block name in «Тип блока» (L9.162),
  the refresh's other callers (L9.162), About's text and links (W1.6), the
  frequency dialog's initial enable state (L9.160), the ОМП dialog's tree
  icons / geometry / reader gaps (L9.169), a computed channel's colour
  recomputed by place (L9.170), an OSC2 record saved as COMTRADE (L9.172).

## W3 — Resolve the unknowns

- [ ] W3.1 List every NOT DETERMINED (37) and INFERRED (44) of the ledger
  with its entry, in a table here: how it will be settled (a static
  re-read, a W4 run) or why it cannot be (W6).
- [ ] W3.2 Settle the static ones.
- [ ] W3.3 Hand the runtime ones to W4 as scenarios.

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
- [x] W4.1b The VM's system locale and devuser's Format set to ru-RU (user
  approved, 2026-10-24) — the original stores names through the thread
  code page, so both are needed for its Cyrillic.
- [ ] W4.2 File-level comparisons (Save As of WIN_1999, DOC_1999, BINARY:
  `.cfg` / `.dat` / `.set` byte-identical since L9.172; next: the clipboard
  document, quick-analysis / ОМП channels, a `.set` after edits) (started 2026-10-24: WIN_1999 saved by
  both through Save As — `FV_SAVE_AS` makes ours; differences found: Ctrl+S
  always asks with a lowercase name, the same-name save rewrites the
  record, and the `.cfg` / `.dat` / `.set` formats, see the save-path read
  in progress) — the strongest checks, no pixels: the
  same record saved by both (Save As `.cfg` / `.cff` / `.osc`, the `.set`
  after the same edits, the clipboard document, the quick-analysis and ОМП
  channels) compared byte for byte or value for value.
- [ ] W4.3 Scenario checklist (each: the original's result recorded, ours
  compared): start-up window and panes; opening each sample record;
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
  selected, title, stat rows, grid height fixed. Still to do, in order:
  the frame layout (tab strip inside the MDI area under the parameters
  bar), 1 px strokes on pixel centres (lines and waveform texture), the
  status bar (left-aligned, 153 / 218 px panes), the ribbon (small-button
  pitch 22, large buttons ~55, the QAT arrow), the pane captions (~17 px,
  flat glyphs), the diagram / bar font scale (11), the «*K» icon.

## W5 — Windows-only paths

- [ ] W5.1 Real printing through GDI (a printer that writes a file, e.g.
  Microsoft Print to PDF with a fixed output name) — ours against the
  original's pages.
- [ ] W5.2 The clipboard formats both put on the clipboard.
- [ ] W5.3 Registry / settings persistence across runs (what the original
  writes under its key vs ours).

## W6 — Out of reach (with the reason)

- `.sto` (Configurator-MT) records — no sample ships; the reader stays ⛔.
- The proprietary bitmaps (logo, splash, About picture) — assets of the
  original; ours draws its own.
- Where a dropped file lands — egui reports no drop position (L9.159).
- Exact glyph rasterisation — egui is not GDI; layout and metrics are
  matched, pixels are not.
- The comment bubble bitmap (L9.6), the docking markers 17117–17127 (MFC's
  own bitmaps, L9.101), the ribbon / menu images, the help (Помощь — the CHM
  is not shipped, L9.14, L9.115).
- The scroll-bar repeat timing (system settings, L9.40).
