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
- **Saving a record read as UTF-8.** The original never reads a record as
  UTF-8 by itself. Ours writes such a record — while its names go out in
  UTF-8 — all in UTF-8 (the `.cff` sections and markers too), with a BOM
  when it had one or when it would not read back as UTF-8 without.
  Reason: the saved file reads back as it was, with no 1251 / UTF-8 mix.
- **«Кодировка» on a record read in UTF-8 / KOI8-R.** The original offers
  OEM / ANSI only (`doc+0x6e8`, `0x44a112`); its names exist only as ANSI
  strings. Ours lists the record's own encoding as a third entry, current
  at open (a stored `Codepage` other than 0 selects it), and its 866 /
  1251 entries read the name bytes in those code pages. Reason: the
  combo shows the record's real encoding and can come back to it.
- **ADC codes beyond ±2³¹** (float32-range samples with a tiny `a`). The
  original truncates the code to a 32-bit int (`0x422250` → `0x411bf0`),
  every such code −2147483648; ours keeps it whole. Reason: the true raw
  sample; ours' writer already writes such codes whole, the original's
  saved file would hold −2147483648 throughout.
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
- **The lanes' names after «Кодировка» (#7).** The original's lanes keep
  the old names until their next layout (a missing repaint, ledger
  L9.350); ours shows the new names at once. Reason: a stale repaint.
