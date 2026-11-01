# FastView-REA — where things stand

The current state, in short. This page is rewritten when things change, never appended to; keep it near 100 lines.
What happened before is in the milestones' ledgers ([milestones/](milestones/README.md)), the record of the original
([original/](original/README.md)), [history/](history/) and `git log`.

Last rewritten: 2026-11-02 (milestone 0.7.1 paused at a checkpoint for a usage-limit reset; v0.7.0 is the latest release).

## Latest release

- **v0.7.0**, 2026-11-01: [release page](https://github.com/dmytrobsp/fastview-rea/releases/tag/v0.7.0). Linux x86-64,
  Windows x86-64 and macOS (universal) archives. Milestone 0.7.0: channel drags and pasted lanes, panes, the formula
  editor's definitions, files byte for byte with the original's, display scaling above 100 %, Windows 11's frame, the
  keyboard in the frame's controls and dialogs, the print path; the record's stale verdicts corrected.

## Where the reconstruction stands

- Every engine and the UI layer read and reconciled: the record runs L1–L9.430 ([original/](original/README.md)); 91.8 %
  of the program's own code is in cited functions (`tools/coverage.py`).
- Deliberate improvements: [IMPROVEMENTS.md](IMPROVEMENTS.md); differences the maintainer accepted:
  [ACCEPTED-DIFFERENCES.md](ACCEPTED-DIFFERENCES.md).

## In flight

- **Milestone 0.7.1** (plan and ledger: [milestones/0.7.1.md](milestones/0.7.1.md)) — **paused 2026-11-02 for a usage-limit
  reset** at a clean checkpoint: six of ten units merged into `milestone/0.7.1` (tooling, analysis-2, formats-2, panes-2,
  keys-2, platform-2), record-2 applied on main; print-2 (PR #213, review asks for small fixes), scaling-2 (PR #218)
  and verify-ui-2 (PR #217) pushed and waiting for review. The ledger's checkpoint lists the exact next steps.

## Next topics

- **Waiting for the maintainer:** #198 / #199 (accept MSVCR120's atof rounding and towlower differences?), #180 (the
  original's black print margins — decode first), #155 (scale the graph — design) and #156 (several diagrams, pop-out —
  design); a lean rebuild of the testbox Windows base (clones die at the overlay cap; the cap is 4 GB meanwhile).
- **The maintainer's new direction (filed 2026-10-30, all `improvement`):** translations (#88), anti-aliased
  waveforms keeping every sample exact (#89), and — designed
  together, `size:l` — light / dark themes replacing the nine «Стиль» skins (#91) with a simpler modern look (#92);
  #19 / #20 (per-skin fidelity) are on hold for them.
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
