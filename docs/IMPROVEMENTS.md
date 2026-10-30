# Deliberate differences from the original

FastView-REA reproduces Fast View 5's behaviour; where it behaves
differently on purpose, the difference is listed here. Each entry says what
the original does (addresses in `FastView.exe` / `mfc120u.dll` as
references), what ours does, and why. A difference that is not listed here
is a bug.

Ours departs from the original only where the original visibly fails or
misbehaves and the better behaviour is obvious, or where reproducing it
is out of reach (those are listed in `notes/COMPLETION_PLAN.md`, W6).

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
- **A `.hdr` beside a UTF-8 `.cfg`.** Ours reads it as UTF-8 too, whatever
  the program encoding. The original reads the `.hdr` in the program
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
  the record's own names, which come back as read. A name pasted or merged
  in comes back only as far as the shown encoding could hold it (KOI8-R has
  no і ї є ґ; a lane pasted under 1251 into a UTF-8 record keeps only what
  1251 holds).
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
