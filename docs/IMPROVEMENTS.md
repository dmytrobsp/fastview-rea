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
