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
