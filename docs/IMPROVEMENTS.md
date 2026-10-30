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
