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
