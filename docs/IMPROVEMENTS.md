# Deliberate differences from the original

FastView-REA reproduces Fast View 5's behaviour; where it behaves
differently on purpose, the difference is listed here. Each entry says what
the original does (addresses in `FastView.exe` / `mfc120u.dll` as
references), what ours does, and why. A difference that is not listed here
is a bug.

Ours departs from the original only where the original visibly fails or
misbehaves and the better behaviour is obvious, or where reproducing it
is out of reach (those are listed in `docs/history/completion-plan.md`, W6).

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
