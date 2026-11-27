# Accepted differences from the original

Differences from Fast View 5 that we know about and don't fix, because fixing them is hard or impossible for little
or no benefit and they are almost invisible to a user. Only the maintainer accepts a difference; each entry records
that decision. Deliberate improvements over the original are a different list: [IMPROVEMENTS.md](IMPROVEMENTS.md).

How an entry gets here: the difference is filed as an issue; the maintainer accepts it; the entry is added here, and
the issue is closed as not planned with the `accepted` label and a link to its entry. Reopening the issue reopens the
decision. A difference that is not here and not in IMPROVEMENTS.md is a bug.

Each entry: **what differs**, **where it shows** (how visible), **why it isn't fixed**, **decided** (date, the
maintainer), **issue**.

## The original's artwork

- **What differs**: the original's own bitmaps — its logo, splash screen, About picture, the ribbon and menu images,
  the comment bubble (L9.6), MFC's docking markers (L9.101) — are replaced by our own drawings of the same size in the
  same places.
- **Where it shows**: the images themselves; layout, sizes and behaviour are the original's.
- **Why it isn't fixed**: they are the vendor's and Microsoft's copyrighted assets, and FastView-REA never ships any
  part of the original program.
- **Decided**: 2026-10-30, the maintainer.
- **Issue**: [#50](https://github.com/dmytrobsp/fastview-rea/issues/50).

## The stats column is 1 px narrower

- **What differs**: the oscillogram's stats column is 1 px narrower than the original's, so the plot starts 1 px
  further left and a given screen pixel falls a few samples later. The drawing and every value agree (L9.288).
- **Where it shows**: only when the two programs are compared pixel by pixel, cursor at the same screen pixel.
- **Why it isn't fixed**: the original's width comes from Tahoma Bold's per-size pixel widths (its `hdmx` table);
  Wine's Tahoma Bold, the font ours ships, has none. Matching it would mean hand-made per-size widths for one font
  for a 1 px gain.
- **Decided**: 2026-10-24, the maintainer: a non-issue, not to be fixed.
- **Issue**: [#51](https://github.com/dmytrobsp/fastview-rea/issues/51).

## Glyph widths and text rendering

- **What differs**: ours draws all text in Wine's Tahoma (L9.270) where the original uses Segoe UI in the ribbon,
  menus and some captions, so those labels are a few per cent narrower (ribbon groups 10–20 px narrower, menus a few
  px: L9.274, L9.334, L9.355); and egui rasterises glyphs, not GDI with ClearType, so text looks a little heavier
  (L9.280).
- **Where it shows**: side by side, in label widths and stroke weight; layout, wording and behaviour are the same.
- **Why it isn't fixed**: Segoe UI can't be shipped, every layout is tuned to Tahoma (the maintainer's choice of font,
  2026-10-24), and GDI's rasterisation isn't available outside Windows.
- **Decided**: 2026-10-30, the maintainer.
- **Issue**: [#52](https://github.com/dmytrobsp/fastview-rea/issues/52).

## Splitter and tab-drag pointers

- **What differs**: over the split bars and while dragging an MDI tab the original shows MFC's own cursors; ours shows
  the system's resize / move cursors in the same places (L9.263, L9.392).
- **Where it shows**: the pointer's picture only; where it changes and what dragging does are the same.
- **Why it isn't fixed**: egui sets system cursors only and resets the cursor every frame; MFC's cursor images are
  Microsoft's.
- **Decided**: 2026-10-30, the maintainer.
- **Issue**: [#53](https://github.com/dmytrobsp/fastview-rea/issues/53).

## Scroll-bar timing and animation, the first window's position

- **What differs**: the original's scroll-bar arrows repeat at the system's settings and its scroll bars animate on
  hover as Windows 11 draws them; ours repeats after 0.4 s every 0.05 s (Windows' defaults) and draws without the
  animation (L9.40, L9.226). A first window's position is Windows' cascade position, which depends on system state
  (L9.349).
- **Where it shows**: only with non-default system settings, in a hover animation, or in a first window's exact
  position.
- **Why it isn't fixed**: system state and Windows' own drawing; the defaults already match.
- **Decided**: 2026-10-30, the maintainer.
- **Issue**: [#55](https://github.com/dmytrobsp/fastview-rea/issues/55).

## `.sto` records can't be opened

- **What differs**: the original opens `.sto` (Configurator-MT) records by running the vendor's native converter on
  them and reading its output (L3, L9.207); ours stops at the converter step, so a `.sto` record doesn't open.
- **Where it shows**: only for users with `.sto` records, a vendor-specific proprietary format.
- **Why it isn't fixed**: the converter is a separate proprietary program of the original's vendor, and no sample
  record exists to reverse or verify the format against; the vendor's own software reads these records.
- **Decided**: 2026-10-30, the maintainer.
- **Issue**: [#49](https://github.com/dmytrobsp/fastview-rea/issues/49).

## sin / cos of very large angles in formulas

- **What differs**: for |x| ≥ 90 112 rad ours computes a formula's `sin:` / `cos:` with the platform's sin / cos
  (non-finite arguments give NaN in both); the original's runtime (msvcr120) uses a large-argument reduction of its own, less accurate
  than either, so the results differ. Below 90 112 ours matches the original bit for bit (L9.412).
- **Where it shows**: only in a formula channel taking `sin:` / `cos:` of an angle beyond about 5.16 million degrees;
  the signal maths (DFT, phasors, ОМП) never reach such angles.
- **Why it isn't fixed**: reproducing msvcr120's large-argument reduction would mean reading Microsoft's runtime,
  whose terms forbid reverse engineering; the port it matches below that bound is Intel's BSD-licensed code.
- **Decided**: 2026-10-30, the maintainer (with #72: "values beyond 90 112 and non-finite ones fall back to std").
- **Issue**: [#116](https://github.com/dmytrobsp/fastview-rea/issues/116).

## tan of very large angles in formulas

- **What differs**: for |x| ≥ 12 800 rad ours computes a formula's `tg:` / `ctg:` with the platform's tan; the original's
  runtime (msvcr120) takes its own large-argument path there, so the results can differ in the last digits. Below
  12 800 rad ours is a port of Intel's SSE2 tan that matches the original bit for bit (about 3.4 million angles checked
  on a clone, L9.416).
- **Where it shows**: only in a formula channel taking `tg:` / `ctg:` of an angle beyond about 733 000 degrees, and only
  around the 16th significant digit — no display, cursor readout or printout shows that many.
- **Why it isn't fixed**: porting the large-argument path as well would be about a day's work and more of Intel's code
  under its licence for a difference nobody can see; the same bound was accepted for sin / cos (#116).
- **Decided**: 2026-10-31, the maintainer ("accept it").
- **Issue**: [#131](https://github.com/dmytrobsp/fastview-rea/issues/131).

## A channel drag out of the window

- **What differs**: the original's channel drag is an OLE drag carrying channel numbers only: dropped on another
  running Fast View, that instance applies *its own* channels with the same numbers (clone: its own channel 0 on its
  vector diagram, its own channels combined on its view); dropped in another program it offers an embedded object whose
  picture can't be drawn (`0x42bd50`, OnDraw `0x45a7c0` returns FALSE). Ours ends a channel drag at the window's edge.
- **Where it shows**: only when a channel is dragged out of the window.
- **Why it isn't fixed**: the original's results there are wrong (another instance's channels, a broken object);
  Copy / Paste moves channels between records in both programs.
- **Decided**: 2026-10-31, the maintainer.
- **Issue**: [#75](https://github.com/dmytrobsp/fastview-rea/issues/75).

## Small pixel offsets in the main window

- **What differs**: a few parts of the main window sit 1–2 px off the original's: the «ОМП» category tab is 2 px
  narrower (Segoe UI label width, L9.397); «Частотная» is 1 px taller; the plane's centre is 2 px lower (L9.236); the
  circle grid starts 1 px lower (L9.280); after the same splitter drag ours ends 1 px lower (MFC moves by the drag
  distance, egui to the pointer; L9.284); the merge bar's controls sit a few px lower (L9.315); the outer frame lines'
  colour (L9.232). The first-run frame, the size grip and the scroll bands were matched in milestone 0.7.1 (L9.437,
  L9.439); the window's own 1–2 px cut at its right and bottom is still open (#241).
- **Where it shows**: only when the two programs are compared pixel by pixel; layout and behaviour are the same.
- **Why it isn't fixed**: each is a pixel with no visible effect, and several come from fonts and egui's pointer model;
  the project rule accepts small visual differences.
- **Decided**: 2026-11-27, the controller (triage of old issues).
- **Issue**: [#18](https://github.com/dmytrobsp/fastview-rea/issues/18).

## An edited text value in «Параметры ОМП» is not bold

- **What differs**: in the ОМП parameters grid the original draws a modified text value bold also inside its edit
  field while it is being edited; ours draws it bold in the cell but in the normal font in the edit field (L9.215). The
  toolbar's height and the bold rule itself were matched (L9.209).
- **Where it shows**: only while a modified text value is being edited.
- **Why it isn't fixed**: egui's text edit takes one font per field; the value's boldness is shown as soon as the edit
  ends.
- **Decided**: 2026-11-27, the controller (triage of old issues).
- **Issue**: [#29](https://github.com/dmytrobsp/fastview-rea/issues/29).

## A printer driver that resets the DC at StartPage

- **What differs**: not known. Ours sets its print DC's attributes once per job; whether the original's output changes
  with a driver that resets DC attributes at StartPage was never observed (L9.121).
- **Where it shows**: at most on paper, with such a driver.
- **Why it isn't fixed**: neither Microsoft Print to PDF nor the clones' drivers reset the DC, so the case can't be
  produced or compared.
- **Decided**: 2026-11-27, the controller (triage of old issues): out of reach.
- **Issue**: [#195](https://github.com/dmytrobsp/fastview-rea/issues/195).

## A `.cfg` line missing a field

- **What differs**: the original reads a `.cfg` through a token stream (`0x477050` / `0x4771f0`), so a field missing
  from a line, or empty after a trailing comma, takes the next line's first token. Ours reads line by line: a missing
  field keeps its default, an empty one reads 0 (atof of an empty token, #170).
- **Where it shows**: only with malformed `.cfg` files; well-formed records read the same.
- **Why it isn't fixed**: the original's result there is a misread record; ours keeps the line structure.
- **Decided**: 2026-11-27, the controller (triage of old issues).
- **Issue**: [#204](https://github.com/dmytrobsp/fastview-rea/issues/204).

## «Объединить» after a key press

- **What differs**: with «Объединить» focused, Space or Enter do nothing at once in the original, but the merge then
  happens when the pointer next moves off the button (clone, milestone 0.7.1); no cause was found in the binary. Ours
  merges on a click only (#182).
- **Where it shows**: only after a key press on the focused button, then a pointer move.
- **Why it isn't fixed**: the cause isn't known, and the delayed merge looks like an accident of the original rather
  than a behaviour a user relies on.
- **Decided**: 2026-11-27, the controller (triage of old issues).
- **Issue**: [#206](https://github.com/dmytrobsp/fastview-rea/issues/206).

## The «Стиль» looks other than the default

- **What differs**: of the original's nine «Стиль» looks, the default (Office 2007 Blue) is matched pixel by pixel.
  The other three Office 2007 looks (Black, Silver, Aqua) use derived or Blue colours in a few parts — pane captions,
  the frame's splitter bands and border, the MDI tab group edge, the document strip's ▾ band, the application menu's
  panel (L9.186, L9.232, L9.234, L9.263, L9.265, L9.341). The five older looks (Windows 2000, XP, Office XP, Office 2003,
  VS 2005) have their base colours matched, but their hot and pressed faces, active pane caption, menus, dialogs and
  panel details are drawn flat or in Office 2007 colours (L9.223).
- **Where it shows**: only after switching «Стиль» away from the default, in those states and parts.
- **Why it isn't fixed**: a large amount of per-look pixel capture for looks that the planned light / dark themes and
  modern look (#91, #92) are meant to replace.
- **Decided**: 2026-11-27, the controller (triage of old issues).
- **Issues**: [#19](https://github.com/dmytrobsp/fastview-rea/issues/19), [#20](https://github.com/dmytrobsp/fastview-rea/issues/20).

## MSVCR120's number parsing in the last bit

- **What differs**: the original reads a COMTRADE ASCII record's analog values with MSVCR120's `atof`, which goes
  through an 80-bit intermediate: a value of 13 or more significant digits with an exponent above about 1e16 can differ
  from ours in the last bit (ours is correctly rounded), and an underflowing denormal such as 3e-324 reads 0 in the
  original, the nearest denormal in ours (clone probe of about 6000 inputs, L9.424).
- **Where it shows**: nowhere visible: shown values have 6–7 digits, and every value up to 9 digits reads the same.
- **Why it isn't fixed**: modelling an x87-style 80-bit parse for a last-bit difference nobody can see.
- **Decided**: 2026-11-27, the controller (triage of old issues).
- **Issue**: [#198](https://github.com/dmytrobsp/fastview-rea/issues/198).

## Lower-casing 199 rare characters

- **What differs**: the original lower-cases wide text with MSVCR120's `towlower`, which leaves 199 BMP characters
  unchanged that ours lower-cases — parts of Latin extended, Greek (U+037F, U+03F4), Cherokee, Georgian, newer
  Cyrillic-supplement, Coptic, Glagolitic, capital sharp s (U+1E9E) and the Ohm, Kelvin and Angstrom signs (U+2126,
  U+212A, U+212B) (clone probe, L9.424). The single-byte 1251 fold is the same for all 256 bytes.
- **Where it shows**: only for a UTF-8 channel or file name containing one of those characters, in a case-insensitive
  comparison (a formula's name lookup, the window list's sort).
- **Why it isn't fixed**: ours follows Unicode; copying the runtime's gaps would buy nothing for such names.
- **Decided**: 2026-11-27, the controller (triage of old issues).
- **Issue**: [#199](https://github.com/dmytrobsp/fastview-rea/issues/199).

## Windows 10 before 1607 above 100 %

- **What differs**: on Windows 10 1507 / 1511 at more than 100 % display scaling ours starts (the DPI functions are
  resolved at run time) but keeps one point per device pixel and the chrome's 96 dpi: content crisp at 1:1, the ribbon,
  captions, menus and dialogs at their 100 % size — not v0.3.0's egui-scaled fallback (from the code, not run).
- **Where it shows**: only on those two Windows 10 releases (2015), out of Microsoft's support since 2017, at more
  than 100 %.
- **Why it isn't fixed**: no such system is available to test a fix on, and the releases are long unsupported.
- **Decided**: 2026-11-27, the controller (triage of old issues).
- **Issue**: [#85](https://github.com/dmytrobsp/fastview-rea/issues/85).

## The driver properties dialog's place at higher scales

- **What differs**: the printer driver's «Свойства...» dialog, opened from «Печать», sits 1–2 px off the original's
  place at 100 % and up to 5 px at 125 % (L9.438; a clone with Microsoft Print to PDF); 150 % and other drivers not
  measured.
- **Where it shows**: only in the position of a driver's dialog, by a few pixels.
- **Why it isn't fixed**: the window belongs to the printer driver, and each driver may place its own sheet; the gain
  is a few pixels.
- **Decided**: 2026-11-27, the controller (triage of open issues).
- **Issue**: [#228](https://github.com/dmytrobsp/fastview-rea/issues/228).

## The printer list while the print dialog is open

- **What differs**: ours reads the printer list when its print dialog opens; a printer added in Windows while the
  dialog is open appears after the dialog is opened again. The original's behaviour in that case was not compared.
- **Where it shows**: only when a printer is installed while the print dialog is open.
- **Why it isn't fixed**: closing and opening the dialog shows the new printer in both programs; the case is rare.
- **Decided**: 2026-11-27, the controller (triage of open issues).
- **Issue**: [#235](https://github.com/dmytrobsp/fastview-rea/issues/235).

## A pressed caption button

- **What differs**: the original's caption buttons are Windows 11's own, which may show a pressed face while the mouse
  button is held on them; ours draws the hover face for a held press too (checked on a real desktop, 2026-11-27).
- **Where it shows**: only while the mouse button is held on a caption button.
- **Why it isn't fixed**: a transient look with no effect on behaviour; not worth a clone measurement.
- **Decided**: 2026-11-27, the controller (triage of open issues).
- **Issue**: [#253](https://github.com/dmytrobsp/fastview-rea/issues/253).
