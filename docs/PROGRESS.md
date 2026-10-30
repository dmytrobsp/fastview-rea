# FastView-REA — where things stand

The current state, in short. This page is rewritten when things change, never appended to; keep it near 100 lines.
What happened before is in the milestones' ledgers ([milestones/](milestones/README.md)), the record of the original
([original/](original/README.md)), [history/](history/) and `git log`.

Last rewritten: 2026-10-30 (v0.4.0 released; milestone 0.4.0 closed; the maintainer's new issues #88–#93 filed).

## Latest release

- **v0.4.0**, 2026-10-30: [release page](https://github.com/dmytrobsp/fastview-rea/releases/tag/v0.4.0). Linux x86-64,
  Windows x86-64 and macOS (universal), each one flat archive, built by CI from the tag.
- What it brought ([milestone 0.4.0](https://github.com/dmytrobsp/fastview-rea/milestone/1), merged by #87): the first
  user feedback on v0.3.0 — auto-hidden panes that stay out (#4), crisp display scaling with MFC's chrome growing as
  the original's (#5, #6, #12), quick analysis and every channel reference after «Кодировка» (#7) —, COMTRADE 2013
  UTF-8 exports such as Siemens SIPROTEC 7KE85 (#8).
- Checked: the whole suite on Linux (CI, gates) and on Windows in a throwaway clone (590 passed); the release exe
  starts on Windows 10 before 1607 again (no statically imported DPI functions). Not run: the eframe GPU path on
  Windows, two monitors at different scales on hardware.
- Before it: v0.3.0 (2026-10-25), v0.2.0, v0.1.0.

## Where the reconstruction stands

- Every engine and the UI layer read and reconciled: the record runs L1–L9.408 ([original/](original/README.md)); 91.8 %
  of the program's own code is in cited functions (`tools/coverage.py`).
- Deliberate improvements: [IMPROVEMENTS.md](IMPROVEMENTS.md); differences the maintainer accepted:
  [ACCEPTED-DIFFERENCES.md](ACCEPTED-DIFFERENCES.md).

## In flight

- Nothing: no milestone is open. The workflow ([AGENTS.md](../AGENTS.md), [agents/workflow.md](agents/workflow.md),
  [agents/review.md](agents/review.md)) was run end to end for the first time in 0.4.0; its ledger has the retro.

## Next topics

- **The next milestone**, proposed to the maintainer: candidates are #60 + #72 (the display factor per sample and
  the original's sin / cos bit for bit — a spike already matched them), #57 / #71 (own rates after a merge), #85 (the
  pre-1607 DPI fallback), the drag-and-drop gaps #73–#76, and the 0.4.0 follow-ups (#61–#70, #77–#83, #86).
- **The maintainer's new direction (filed 2026-10-30, all `improvement`):** translations (#88), anti-aliased
  waveforms keeping every sample exact (#89), a short drag to float a pane with a grab cursor (#90), and — designed
  together, `size:l` — light / dark themes replacing the nine «Стиль» skins (#91) with a simpler modern look (#92);
  #19 / #20 (per-skin fidelity) are on hold for them.
- **Waiting for the maintainer:** #74 (a drop on another record's view: match the original or do nothing), #75
  (accept the cross-instance drag difference?), #93 (flat channels' harmonics as «0»? — asked of the user).
- **Waiting for samples:** #59 (an OSC2 record, for the `.osc` Save As comparison). `.sto` is not supported — an
  accepted difference (#49); #72 needs no more samples (the user's 1 kHz and 16 kHz records were enough).
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
