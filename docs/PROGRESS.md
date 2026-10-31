# FastView-REA — where things stand

The current state, in short. This page is rewritten when things change, never appended to; keep it near 100 lines.
What happened before is in the milestones' ledgers ([milestones/](milestones/README.md)), the record of the original
([original/](original/README.md)), [history/](history/) and `git log`.

Last rewritten: 2026-10-31 (v0.5.0 released; milestone 0.6.0 started: the follow-ups of 0.5.0 and 0.4.0).

## Latest release

- **v0.5.0**, 2026-10-31: [release page](https://github.com/dmytrobsp/fastview-rea/releases/tag/v0.5.0). Linux x86-64,
  Windows x86-64 and macOS (universal), each one flat archive (now with `LICENSE-intel-libm.txt`), built by CI from
  the tag.
- What it brought ([milestone 0.5.0](https://github.com/dmytrobsp/fastview-rea/milestone/3), merged by #122):
  calculations to the original's last digit — its sin / cos bit for bit through a port of Intel's BSD SSE2 code (#72),
  the display factor per sample (#60), own rates after «Объединить» and message 239 (#57, #71), the ОМП flags after
  «Кодировка» (#66) — and panes — auto-hide strips, tab buttons, divider and toggle as the original's (#61–#64),
  floating panes as tool windows (#70), the focus back after any close (#77), a short drag to float with a move
  cursor (#90).
- Checked: the whole suite on Linux, Windows and macOS (CI, gates, 633 tests) and the Windows test executables in a
  throwaway clone (630 passed); smoke runs of each fixed issue in a clone. Not run: #66 end to end on a clone (#114).
- Before it: v0.4.0 (2026-10-30), v0.3.0, v0.2.0, v0.1.0.

## Where the reconstruction stands

- Every engine and the UI layer read and reconciled: the record runs L1–L9.413 ([original/](original/README.md)); 91.8 %
  of the program's own code is in cited functions (`tools/coverage.py`).
- Deliberate improvements: [IMPROVEMENTS.md](IMPROVEMENTS.md); differences the maintainer accepted:
  [ACCEPTED-DIFFERENCES.md](ACCEPTED-DIFFERENCES.md).

## In flight

- **Milestone 0.6.0 merged into main** (2026-10-31, PR #151; [ledger](milestones/0.6.0.md)): the follow-ups of 0.5.0
  and 0.4.0 — 27 issues, record L9.414–L9.420. The release is on hold (the maintainer).
- **The forward-only scrub is done** (PR #152, 2026-10-31): no value of the clone test records or of a user's record is
  left in the current tree (code comments / tests, docs) or in the current bodies of #7, #9, #13, #27 — the maintainer's
  choice; history, tags and edit histories stay as they are.

## Next topics

- **After 0.6.0**, candidates: the drag-and-drop gaps #73–#76 (#74 / #75 need the maintainer), #85 (the pre-1607
  DPI fallback).
- **The maintainer's new direction (filed 2026-10-30, all `improvement`):** translations (#88), anti-aliased
  waveforms keeping every sample exact (#89), and — designed
  together, `size:l` — light / dark themes replacing the nine «Стиль» skins (#91) with a simpler modern look (#92);
  #19 / #20 (per-skin fidelity) are on hold for them.
- **Waiting for the maintainer:** #74 (a drop on another record's view: match the original or do nothing), #75
  (accept the cross-instance drag difference?), #93 (flat channels' harmonics as «0»? — asked of the user).
- **Waiting for samples:** #59 (an OSC2 record, for the `.osc` Save As comparison). `.sto` is not supported — an
  accepted difference (#49); #72 needs no more samples (the users' records, at their own rates, were enough).
- The rest of the queue: the `later` milestone (`gh issue list -m later -S '-label:triage'`).

## Where things live

| What | Where |
|---|---|
| Agent instructions, hard rules | [AGENTS.md](../AGENTS.md) (start here); `CLAUDE.local.md` (main checkout) for this machine |
| The workflow | [agents/workflow.md](agents/workflow.md), reviews [agents/review.md](agents/review.md), Windows runs [agents/windows-testing.md](agents/windows-testing.md) |
| The queue | [GitHub Issues](https://github.com/dmytrobsp/fastview-rea/issues), milestones and labels |
| Milestones, designs | [milestones/](milestones/README.md): plan and ledger per milestone; [design/](design/README.md) |
| What the original does | [original/](original/README.md): L1–L9.x and the specs |
| Deliberate differences | [IMPROVEMENTS.md](IMPROVEMENTS.md) (improvements), [ACCEPTED-DIFFERENCES.md](ACCEPTED-DIFFERENCES.md) (accepted by the maintainer) |
| History | [history/](history/): the completion plan (to 2026-10-30), the ledger's session notes |
| Tools | [tools/README.md](../tools/README.md); `tools/milestone/milestone.py`, `tools/review-package.sh` |

## Keeping this page

- Rewrite it when a release ships, a milestone starts or ends, or the next topics change.
- Short plain lines; links to issues, milestones and the record instead of detail.
- Nothing private: no user data, no machine or account names.
