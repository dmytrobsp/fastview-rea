# Deliberate differences from the original

FastView-REA reproduces Fast View 5's behaviour; where it behaves
differently on purpose — an improvement — the difference is listed here.
Each entry says what the original does (addresses in `FastView.exe` /
`mfc120u.dll` as references), what ours does, and why, with the issue or
pull request it came from. Differences we know of and don't fix are a
separate list: [ACCEPTED-DIFFERENCES.md](ACCEPTED-DIFFERENCES.md). A difference listed in neither
is a bug.

Ours improves on the original only where the original visibly fails or
misbehaves and the better behaviour is obvious (the maintainer,
2026-10-30). The pull request that makes an improvement adds its entry.

## Extras

- **«Дополнительно (FastView-REA)»** in the application menu: our own
  commands (demo record, CSV / PNG export, `.fvb` loading). The original has
  no such submenu. Reason: tools for the reconstruction and for users, kept
  apart from the original's menu so its items stay where they are.

## Records (COMTRADE, #8)

- **A UTF-8 `.cfg`.** The original widens every `.cfg` token through the
  program's `Display Encoding` (`0x422300`, default 1251) and knows no BOM:
  a C37.111-2013 export in UTF-8 shows mojibake, the BOM as «п»ї» before
  the station, unless the whole program is switched to UTF-8 (VM). Ours
  reads a `.cfg` / `.cff` that starts with the UTF-8 BOM, or is of revision
  2013 and valid UTF-8 with multi-byte Latin / Greek / Cyrillic text, as
  UTF-8 whatever the setting, the BOM stripped. Reason: the standard
  prescribes UTF-8 for 2013; the original visibly fails on it.
- **A U+FFFD in a name** (an exporter that cut a name mid-character). The
  original shows «?» (the ANSI narrowing of `0x422300`, under UTF-8 too);
  ours keeps the character. Reason: it is what the file says — lossless,
  saved back byte for byte, and it marks the cut.
- **A `.hdr` beside a UTF-8 `.cfg`.** Ours reads it as UTF-8 too when it is
  valid UTF-8, whatever the program encoding (otherwise it guesses as before). The original reads the `.hdr` in the program
  encoding or guesses 866 / 1251, so under 1251 a UTF-8 export's comment
  (32805) is mojibake there. Reason: the comment of a UTF-8 record is
  UTF-8 text, as its names are.
- **Saving a record read as UTF-8.** The original never reads a record as
  UTF-8 by itself. Ours writes such a record — while its names go out in
  UTF-8 — all in UTF-8 (the `.cff` sections and markers too), with a BOM
  when it had one or when it would not read back as UTF-8 without.
  Reason: the saved file reads back as it was, with no 1251 / UTF-8 mix.
- **«Кодировка» on a record read in UTF-8 / KOI8-R.** The original offers
  OEM / ANSI only (`doc+0x6e8`, `0x44a112`, `0x44a330` → `0x4543e0`); its
  names exist only as ANSI strings — narrowed with Windows' best fit, «?»
  where 1251 has none («√ ─ ä ∆» show «v - a ?», VM) — so that is all the
  way back from 866 gives. Ours gives the same two entries the same
  results — ANSI those narrowed names, OEM their bytes read as 866 — and
  lists the record's own encoding as a
  third entry, current at open (a stored `Codepage` other than 0 selects
  it), which gives the names back as read, nothing narrowed or
  best-fitted («√», box-drawing, U+FFFD kept). Reason: the combo shows the
  record's real encoding, and coming back from 866 / 1251 loses nothing of
  the record's own names, which come back as read. A stored `Codepage`
  other than 0 / 1 on such a record is not kept raw (the third entry is
  current for it); on a 1251 / 866 record the original's rule applies (ANSI,
  written back).
- **A name pasted or merged into a record that cannot hold its letters is
  kept as typed (#81, the maintainer's decision 2026-10-31).** The original
  keeps a pasted or merged name as the bytes of the record's code page only.
  Ours keeps the name as an alias, shown as typed across every «Кодировка»
  switch, when its text does not come back from those bytes:
  - *Record read in KOI8-R* (no і ї є ґ; the original has no KOI8-R record
    mode, ours before kept such a name as KOI8-R bytes, so the letters came
    back as «?» after a switch away and back): ours keeps the name as typed.
    A name KOI8-R holds stays a plain stored name, re-decoded like the
    record's own, except while 866 is shown: there the names are the OEM
    view of the bytes, which KOI8-R cannot give back, so every Cyrillic name
    pasted under 866 is kept as typed.
  - *Record read in 1251 / 866, shown in the other one at the paste*: the
    original keeps the name as the shown code page's bytes, so back in the
    first it reads as those bytes (every pasted name is garbled, «(вст.)»
    becomes «(ўбв.)», L9.404, clone). Ours does the same for a name the
    shown code page can hold; a name it cannot hold (і under 866) is kept as
    typed instead of turning into «?».
  - *Record read in 1251 / 866, shown in its own code page*: a name with
    letters that page lacks (copied from a UTF-8 / KOI8-R record: «√», «ä»,
    і under 866) is kept as typed. The original cannot hold those letters
    at all: it has no UTF-8 view, its copy of such a name is already
    best-fitted («a», «v»), and only і under 866 would show «?».
  - *Record read in UTF-8*: the bytes hold every letter, so a name is an
    alias only while 866 is shown and the name has a letter 866 cannot hold
    (і); otherwise it follows «Кодировка» as the record's own names do (1251
    its ANSI narrowing, 866 its OEM view).
  Reason: a name that cannot survive its own bytes is lost for good in the
  original; keeping it as typed costs nothing.
- **ADC codes beyond ±2³¹** (float32-range samples with a tiny `a`). The
  original truncates the code to a 32-bit int (`0x422250` → `0x411bf0`),
  every such code −2147483648; ours keeps it whole. Reason: the true raw
  sample; ours' ASCII `.dat` writer already writes such codes whole,
  where the original's saved file holds −2147483648 throughout. A binary
  `.dat` or a `.cff` (always binary) stores 32-bit ints: there such codes
  are lost in ours as in the original — ours saturates to the nearest
  bound (±2147483647 / −2147483648), the original writes −2147483648.
- **One compute at open.** The original recomputes every channel when
  applying the `.set` (`0x44f5b0`); ours spares that pass at an open when
  the settings change nothing it reads (the result is the same, tested).
  Reason: a shorter open for a big record (the `.set` apply runs on the loader's thread, so the gain is the
  progress dialog's wait).

- **ASCII times past 2³¹ µs (#212).** The original's ASCII loader converts a row's rounded µs time to 32 bits
  (`0x4518b7` via `0x411bf0`): in a record longer than about 35.8 minutes every later row's time saturates at the
  limit. Ours keeps the full value. Reason: a time axis stuck at one value is a visible failure; the decision of the
  2026-11-27 triage.
- **The time axis of a record whose steps disagree with its rate (#234).** On a malformed record whose time column
  disagrees with its sampling rate (a synthetic 5-row record on a clone) the original labels every tick of the time
  axis 0; ours labels the ticks by its rule (0 / 3 / 7 / 10 ms there). Normal records' axes are the same. Reason: a
  row of zeros says nothing; the decision of the 2026-11-27 triage.
- **A start time without «:» (#43).** The absolute-time labels and the date / time bands read the record's start
  time token by `_wtoi` of the hours and minutes and `_wtof` of the seconds (`0x464350`, `0x4222d0`); on a token
  without a «:» the original takes the later fields from uninitialised stack, so the labels depend on whatever the
  stack held. Ours counts a missing field as 0, so a time
  «12» is 12:00:00. Reason: a label built from uninitialised memory is undefined; a stable, defined reading
  beats it (the record: L9.18).

## Behaviour

- **Channels that share a name (#58).** The original finds a channel's
  group by its name (`0x455b30` compares names exactly, and so do the
  group lookups in its 20 callers, ledger L9.395): in a record with two
  channels of the same exact name, or a digital channel named like an
  analog group member, the namesake is taken into the group — its lane
  paints the group's members instead of its own samples, and hiding or
  flagging it acts on them. Ours keeps groups per channel, so each lane
  shows its own channel. Reason: a lane that shows another channel's data
  is wrong whatever the record; only records with duplicate names (which
  COMTRADE discourages but allows) are affected. The maintainer,
  2026-10-30.

- **A level or area whose channel name no longer resolves.** The original
  blanks its whole oscillogram view (VM run, ledger L9.227); ours leaves
  that one element out. Reason: the blank view is a paint failure, not a
  feature.

- **Names after «Кодировка» (#7).** The original re-decodes every channel
  name in place (`0x4543e0` → `0x425960`) and keeps every name-keyed
  reference as text: a formula on the old names then fails («Имя канала
  задано неправильно.», the whole quick-analysis batch with it; VM), the
  levels and areas stop drawing (ledger L9.227) and the quick-analysis
  pages forget their picks. Ours rewrites what named a channel to its new
  name — the formula channels' references (digital channels' included),
  the differential / R-X pairs, the quick-analysis pages' stored picks, the
  ОМП stations' channels and the vector diagram's angle reference — and
  keeps the levels and areas on their channel. A name that stands for two
  channels on either side (866 reads «В» and «С» alike, as «T») is not
  followed: that reference fails as in the original instead of reading the
  wrong channel. Reason: the channel is the same, only its decoding
  changed; the user's work on it should not break.
- **Display scaling on a monitor other than the primary one, and after a
  live scale change (#5, #6).** The original is system-DPI aware (its
  manifest's `dpiAware`): its dpi is fixed at sign-in to the primary
  monitor's, and it paints in device pixels at that dpi. On a monitor at
  another scale, or after the scale is changed without signing out,
  Windows bitmap-stretches its window by the ratio of the two scales —
  blurry, and larger or smaller than 1:1. Ours is per-monitor DPI aware
  and keeps one point per device pixel on whichever monitor its window is
  (from the window's first frame, and again each time its monitor's scale
  changes): crisp and 1:1 there, so smaller than the original's stretched
  picture on a 150 % monitor beside a 100 % primary, larger the other way;
  on a scale change Windows resizes the window while its content stays
  1:1. On the primary monitor at the sign-in scale both are the same
  (device pixels; see `device_zoom` in `crates/fastview-gui/src/main.rs`).
  MFC's chrome — ribbon, captions, tab strips, status bar, menus,
  tooltips, dialogs — is laid out for the system dpi in both (#12): there
  ours shows it unstretched like the rest, at its system-dpi size (the
  ribbon of a 100 % sign-in stays 100 %-sized on a 150 % monitor, where
  the original's is stretched to 1.5 ×; `crates/fastview-gui/src/dpi.rs`).
  Reason: the stretched picture is the blur #5 / #6 report.
- **The lanes' names after «Кодировка» (#7).** The original's lanes keep
  the old names until their next layout (a missing repaint, ledger
  L9.350); ours shows the new names at once. Reason: a stale repaint.
- **The fundamental's own label in «% of the fundamental» (#60).** The
  original prints the label as `(v · 100) / v`, with the bar's value and
  the norm both taken from the same spectrum call (the label at
  `0x4704e7`–`0x4704f3`, the format by `0x46f5a0`). In doubles that is
  exactly 100, or 100 plus an ulp (printed «100»), or 100 minus an ulp,
  which its "%2.1f below 100" branch prints as «100.0» — wider, so it can
  also rotate differently. Ours always prints «100» for the fundamental's own
  bar (the bar at the number of periods, when the norm is the fundamental,
  not the RMS); the other bars are computed as the original does. Reason:
  the fundamental is 100 % of itself by definition; «100.0» is a rounding
  accident of the format's branch, not a choice.
- **The caption drag of a docked pane shows the move cursor (#90).** The
  original floats a docked pane once the pointer is more than the system
  drag distance (SM_CXDRAG / SM_CYDRAG, read once at start-up,
  `0x1005a5fb`) from the press along an axis, wherever the pointer is
  (`0x10086f3a`; clone at 100 %: 4 px stays docked, 5 px floats the pane
  with the pointer still on it), and keeps the arrow cursor: the four-way
  move cursor in its docking code (`0x100d66dd`, `0x100d75a8`) is set only in the
  immediate, non-smart docking mode, which FastView does not use; the
  clone showed the arrow at 0 / 3 / 4 / 5 px and, with the button held,
  at 8 px (a reading under mouse capture, so the binary is the firmer
  evidence). Ours floats at the same distance and shows the four-way move
  cursor from the press on the caption (not on its pin or close buttons)
  and while the pane is dragged, for a docked or a floating pane (an
  auto-hidden pane's caption does nothing, so keeps the arrow); the left
  button only. Reason: the maintainer's request (2026-10-30): the arrow
  gave no hint that dragging on would pull the pane out of its dock.
  The distance: ours grows it with the display scale (5 px at 125 %, 6 at
  150 %), by the maintainer's decision; the original reads the system
  metric once at start-up, and whether Windows scales that metric at a
  scaled display was not checked, so this may be parity. (An earlier build
  of ours also asked the pointer to leave the pane: a fix to match the
  original, not an improvement.)
- **A slid-out side or top pane starts below the caption row (#110).** The
  original lays a slid-out auto-hidden pane over the window's whole height
  (a side pane) or width (a top one) from the window's top, its caption
  under the window's caption glass (L9.410); a pin or
  close pressed there still acts (clone, a left pane: the pin docks it
  back, the close hides it, the ribbon button then unchecked), but the
  buttons are invisible, and on a right or top pane they lie under the
  window's own buttons, so they can't be reached at all. Ours starts the
  pane below the caption row, so its caption shows and its pin and close
  work on every edge; the pane is that much shorter. Reason: the
  maintainer's decision (2026-10-31): a pane's pin and close should be in
  reach. The hide check still covers the band above the pane up to the
  window's top, as the original's pane does, so the pointer resting
  between a top strip and its pane keeps it out. A strip button under the glass (a left strip's first stub) is
  hovered as in the original, which is parity, not an improvement.
- **A pane's tab dragged out of its strip shows the move cursor (#111).**
  The original keeps the arrow while a tab is pressed and pulled out of
  the strip (`0x100690b4`, `0x10068657`; clone, 100 %: the arrow before
  and after the tab leaves the strip). Ours shows the four-way move
  cursor from the press on a tab of a strip with several tabs (left
  button) and while it is dragged, as for a caption drag (the entry
  above). The distance is the caption's, grown with the display scale.
  Reason: as for the caption, it hints that dragging on pulls the pane out.
- **A channel drag dropped on another record's view or diagram does nothing (#74).** With two tab groups, the original
  runs the drop on the *active* document (`0x4415b0`; the lane is found in the drop window): channels dragged out of
  record A and let go on record B's view: the lane under the pointer in B's view is added, by its number, to A's
  selection without repainting A; then the combine runs on B's own current selection, which joins or is refused with message 1086 (clone). The drag carries only channel
  numbers (the private format, `0x42be30`), no record identity, so the result is arbitrary. Ours remembers the record
  the drag began in and treats a drop on another record's view as a drop outside: no change, no message, and the
  "no-drop" cursor over that view (the drag keeps auto-scrolling it, as the original's drop target does). A diagram
  pane shows the active record only, so a drag begun in a record that is no longer the active one drops nowhere
  there either. Reason: the maintainer's decision (2026-10-31); Copy / Paste remains the way to move channels
  between records.
- **The frequency diagram's «%» labels of a constant channel read 0 (#93).** A channel stuck at one non-zero value
  has no harmonics and no fundamental, but computed in floating point each bin holds rounding noise (around 1e-14 of
  the level). In «% of the fundamental» mode the original divides that noise by the noise (`0x4704e7`) and prints
  meaningless percentages and a huge DC figure (in absolute mode the harmonics print 0, the DC bar the level).
  Ours prints «0» on every bar when the fundamental is below 1e-9 of the window's largest |sample|; the threshold sits five orders above
  the noise and three below a ripple of 1e-6 of the level, so real signals, near-constant ones included, keep their
  labels. Reason: the maintainer's decision (2026-10-31). The «% of RMS» mode is untouched (its norm is the level).
- **A BINARY or binary32 record of three rows numbered 1, 2, 3 or fewer opens (#171).** The original checks the record
  size by reading the first three sample numbers (and the fourth after 1, 2, 3) unconditionally (`0x451be0` for BINARY,
  `0x4521bd` for binary32); with too few records the read is a null read and the program dies with an access
  violation (exception 0xc0000005 on a clone, fault offsets at `0x451c9c`–`0x451cd1` and `0x452198`–`0x4521d9`; no
  window, no message; synthetic records of 1, 2 and 3 rows numbered 1, 2, 3 in both formats, all six crash; four
  rows and more load; other numberings take BINARY's fallback size or binary32's error 330, in both programs). Ours
  loads such a record at the computed record size. Reason: a crash in the original is a visible failure; the
  controller's ruling (2026-11-01).
- **A `.cff` saved after a lane paste is consistent (#192).** With a pasted lane in the record the original's `.cff`
  counts the written channels in its CFG part (`3A`, `2D` on the clone record) but its DAT part (BINARY, int32)
  carries a column for the lane in front of the file channels (a clone run: 26-byte rows where 22 are declared).
  Such a file cannot be read back correctly — inferred for a record labelled BINARY; an ASCII-labelled one reopens
  as garbage in both programs. Ours writes the DAT part of the channels the CFG part declares. Reason: the
  original's file is internally inconsistent; the controller's ruling (2026-11-01).
- **Quick analysis on crafted input: a record without samples with a formula, and page 539 with two stored reduction methods (#193).**
  (1) The original's `.osc` reader (`0x4524b0`; we no longer read `.osc`, #59) has no zero-sample check (only the `.cfg` / `.cff` loader has one,
  `0x450bc1`), so a header-only `.osc` loads with no samples; ours has no such record from a file, but a record built in
  memory can be one. A formula over it runs the batch (`0x415780`) with zero
  samples: the evaluator never runs, and the unit walk (`0x415bd0`) reads the token at the position an earlier
  evaluation left (only the per-sample evaluate resets it, `0x419997`; the tokenizer does not). The token getter
  (`0x419800`) is bounds-checked: a position past the formula's tokens throws string 343 «Синтаксическая ошибка в
  формуле», an in-range bracket token becomes the unit. So the result depends on the definition evaluated before it
  in the session: a spurious syntax-error box or a wrong unit (the manager's initial position was not read).
  Ours always walks the formula from its start: empty channels with the formula's own unit, no spurious error.
  (2) A `.set` that stores two 539 reduction methods makes the formulas outnumber the names list (one set per arm);
  `0x4167f0` pairs them by index with no bounds check (`0x416c61`-`0x416c9d`) and reads past the names vector:
  undefined (garbage or a crash). Ours names the extra lines «_2», «_3»... Reason: both are reachable only through a
  hand-made `.osc` / `.set` (in ours, a hand-made `.set`); a defined, error-free result beats a session-dependent fault or undefined behaviour.
- **The horizontal bar's first sample and position on long records (#196).** The original's Shift+wheel
  (`0x45d990`) and the bar's thumb drag (`0x45d6d0`) compute the first sample as (N − V) × position in 32-bit
  arithmetic before dividing, and the bar update (`0x45d4a0`) computes the position from first × 10000 the same way.
  Past about 429 000 samples the products wrap modulo 2³²: just past the limit a wheel notch or a thumb position near
  the end puts the first sample near the record's start (a visible jump), further on somewhere else wrong in the
  record, and the update's wrapped position is a wrong starting point for the next notch. Ours computes in 64 bits.
  Reason: a first sample far from where it belongs on a long record is a visible failure; the controller's ruling
  (2026-11-01).
- **The merge bar's «Канал 1» after «Кодировка» (#48).** With a pending paste, after an encoding switch the
  original's merge bar keeps showing the old name in «Канал 1» while every other place shows the re-decoded one (a
  missing refresh). Ours shows the new name there too. Reason: one name in one place and another elsewhere is a
  visible inconsistency; the decision of the 2026-11-27 triage.
- **The oscillogram's printed page has no black areas (#180).** On some setups (a Windows 11 clone with Microsoft Print
  to PDF, Letter and A4) the original's printed and previewed oscillogram page fills only the upper-left part of the
  margin rectangle and the rest is black — its memory bitmap's unpainted area stretched in (`0x405970`, MM_ISOTROPIC).
  Ours always fits the view to the margin rectangle. Reason: black areas on paper waste toner and hide nothing useful;
  the decision of the 2026-11-27 triage (the rule behind the black area was not decoded).
- **A визир switched on beyond the end of a short record (#251).** The original's ribbon toggle (`0x45c120`) clamps the
  placed sample only at 0. On a record shorter than three quarters of the plot, В2 / В3 land past the last sample. The
  status bar and the difference band then give times beyond the record's end, while the lines (and their labels) are
  drawn one label-column width left of that position, over samples they do not belong to (clone, one run at the opening
  scale on a 60-sample record; the cause of the offset was not read). Ours limits the placement to the last sample, so
  the line, the band and the status name the same sample. Reason: the original's three readouts of one визир disagree,
  and its line points at no sample it reports; the decision of the 2026-11-27 triage.
- **Keys in a popup menu cut to the work area reach every row (#216).** In the original (CMFCPopupMenu, a work area
  smaller than the menu; the ribbon's «Стиль» menu walked on a clone, a context menu not walked by keys) ↓ keeps the highlighted row in view by
  scrolling, but once the list is scrolled the rows above the window are out of the keys' reach: ↑ from the first
  visible row wraps to the last row, ↓ from the last row wraps to the first row still in view without scrolling, Home
  lights the first row in view (not the first of the menu), End goes to the real last row (↓ from the top reaches it too). The rows above can
  then be reached only with the wheel or the arrow band. Ours walks the whole list: ↑ / ↓ / Home / End go to the
  menu's real rows and scroll them into view. Reason: a key that cannot reach the first rows of a menu, and Home that
  does not go to the first row, is a visible failure; the decision of the 2026-11-27 triage (keep ours if the
  original's keys are worse).
- **«Коэффициенты трансформации»: the cells are selected the spreadsheet way (#214).** The original selects cells
  only by a Shift + double-click (a rectangle from the anchor, `0x47b020`); a single click selects nothing, the edit's
  commit (`0x47b3d0`) writes the typed text into every selected cell and then clears the selection unless Ctrl is
  held, and Enter / Esc in the edit are the dialog's OK / Cancel. Ours works as Excel does: a click selects one cell
  (the anchor), press and drag selects the cells the pointer spans (a drag over the name column takes the whole rows'
  value cells), Shift + click extends the rectangle from the anchor, Ctrl + click toggles a cell and Ctrl + drag adds
  a rectangle; a double-click opens the edit (the first click of it does not reset a selection it lands in), and the
  value typed goes into every selected cell whose own text differs from it when the edit commits — Enter, losing the
  focus or a press on no cell commit and keep the selection, a click on another cell commits and resets the selection
  to that cell; Esc cancels the edit and keeps the selection (neither closes the dialog). Dragging a selection past the
  list's view scrolls it. With the list focused the arrows move the selection (Shift extends it), F2 or Enter edit the anchor cell
  and a typed digit starts an edit replacing the text. The drawing of the selected cells, the `%g` formatting and OK / Cancel
  are the original's; a commit changes only the selected cells whose text differs (the original compares the edited
  cell's text only, so a group whose edited cell already holds the value is left as it is). Reason: the user's request
  (2026-11-27): the familiar spreadsheet way instead of a double-click scheme nobody finds.
- **A sampling rate whose quotient by the network frequency is 0 in «Настройки частотной диаграммы» (#43).** The
  dialog's exchange (`0x46ee70`) bounds the periods by N div (R div trunc f), R the record's sampling rate and f the
  network frequency; for a record whose R div trunc(f) is 0 the original divides by zero. Ours takes that quotient
  as 1, so the periods are bounded by N; the harmonics bound, trunc(R / f · 0.5) · periods, is then 0 and ours refuses
  every harmonics value with MFC's range message, so the dialog can only be cancelled — instead of faulting. Reason:
  a division by zero is no behaviour to keep; the record: L9.133.
- **The window never waits for the loader (#43).** The original opens a record on a worker thread (the per-sample
  recompute `0x428490` and the `.set` apply `0x44f5b0` run there, L9.332) while a modal progress dialog (DIALOG 492,
  timer `0x471870`) is up for a big record; the frame then repaints and the bar moves. The UI thread blocks in the
  worker's infinite wait in two cases: after «Отмена» (until the worker's next cancel poll, or until it ends where
  nothing polls — the `.cff` split, L9.268), and for the whole of a load under the size threshold, which shows no
  dialog (`0x44f1f8`–`0x44f245`, `0x44f311`–`0x44f352`, the worker `0x44f410`; L9.31). Ours never waits: «Отмена»
  drops the load at once and the loader stops at its next poll unseen, and a small record is read on the thread too.
  The counters, the 500 ms bar steps and the cancel rules are the original's. Reason: a window that does not answer
  after a cancel looks like a hang; the difference is small. Whether ours blocks input to the frame during a load as
  the original's modal dialog does was not checked. The record: L9.31, L9.268, L9.332, L9.441.
- **Alt+F4 right after a pane's caption ✕ (#43).** Observed on a clone: right after the caption ✕ hid the vector
  diagram's pane, an Alt+F4 did nothing in the original (the window stayed and nothing was saved); on its own Alt+F4
  closes the window. The cause was not read. Ours closes the window every time. Reason: it looks like a focus slip,
  not a designed behaviour; the record: L9.278.
- **«Участок» in «Параметры ОМП» after it disables itself (#43).** After the toolbar's «Участок» is pressed and the
  command disables itself, the original's button stays in its hot face, not refreshed, under the pointer until the
  pointer moves. Ours repaints on the next frame: a disabled button is never hot. Reason: a repaint lag, not a
  feature; the record: L9.333.
- **The «Каналы» rename with pair rows (#43).** The original's in-place rename stores the list item index the edit
  was opened on (`0x435c31`) and the commit (`0x435c80`) uses it as the index into the record vector
  (`0x435eb2`, `0x435f57`; also the duplicate-name scan's self-skip, `0x435da1`). With a differential or an R/X pair
  row in the list, which has no record of its own, a digital row after them no longer matches its record. Ours acts
  on the record the edited row stands for, whatever pair rows precede it. Reason: renaming the channel that is
  shown is the only sensible result; the record: L9.199.
- **Parallel consecutive lines of a characteristic polygon (#43).** The original takes vertex i of a `<polygon>` as
  the intersection of line i−1 and line i with no check (`0x409860`, fed to GDI by `0x409c00`); for two parallel
  lines the division by zero gives infinity (NaN for coincident ones), which becomes 0x80000000 in device space and
  the GDI call fails (L4; what is painted afterwards was not observed). Ours drops that vertex and fills the rest,
  painting nothing only when fewer than three vertices remain. A scan of the shipped library (62 characteristics,
  97 polygons) finds no such pair at the default parameters; nine characteristics reach it when parameters are set
  to certain values (for example two angle parameters made equal, which makes a slope line and an inverse-slope
  line parallel). Reason: a parameter value or a hand-made `.fvb` that makes two consecutive lines parallel should
  degrade, not fail the figure; the record: L4. Ours also drops a vertex beyond 10^6 units, a guard against a
  runaway near-parallel vertex with no counterpart in the original.
