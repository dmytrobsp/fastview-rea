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
  - *Record read in KOI8-R* (no і ї є ґ): the original brings such letters
    back as «?» after a switch away and back; ours keeps the name as typed.
    A name KOI8-R holds stays a plain stored name, re-decoded like the
    record's own.
  - *Record read in 1251 / 866, shown in the other one at the paste*: the
    original keeps the name as the shown code page's bytes, so back in the
    first it reads as those bytes (every pasted name is garbled, «(вст.)»
    becomes «(ўбв.)», L9.404, clone). Ours does the same for a name the
    shown code page can hold; a name it cannot hold (і under 866) is kept as
    typed instead of turning into «?».
  - *Record read in UTF-8*: the bytes hold every letter, so a name is never
    an alias; it follows «Кодировка» as the record's own names do (1251 its
    ANSI narrowing, 866 its OEM view).
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
  Reason: about 2 s less on the UI thread for a big record.

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
