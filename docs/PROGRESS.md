# FastView-REA — where things stand

The current state, in short. This page is rewritten when things change, never appended to; keep it near 100 lines.
What happened before is in the milestones' ledgers ([milestones/](milestones/README.md)), the record of the original
([original/](original/README.md)), [history/](history/) and `git log`.

Last rewritten: 2026-10-30 (milestone 0.4.0 in flight; tracking moved to GitHub Issues).

## Latest release

- **v0.3.0**, 2026-10-25: [release page](https://github.com/dmytrobsp/fastview-rea/releases/tag/v0.3.0). Builds for
  Linux x86-64, Windows x86-64 and macOS (universal), each one flat archive, made by CI.
- Since then on `main`: the runs of the original moved to throwaway Windows clones at 1280 × 800, which surfaced and
  fixed a series of size-dependent differences (L9.392–L9.403).

## Where the reconstruction stands

- Every engine (formulas, readers / writers, signal maths, ОМП, characteristics, analysis views) and the UI layer
  read and reconciled: the record runs L1–L9.403 ([original/](original/README.md)); the program's own code is
  91.8 % covered by cited functions (`tools/coverage.py`: 1118 of 1803 functions, none of the large or medium ones
  uncited).
- Ours runs on Linux, Windows (GPU or its software runner) and macOS. Deliberate differences:
  [IMPROVEMENTS.md](IMPROVEMENTS.md). Out of reach (no sample, proprietary assets, GDI's exact glyphs): the W6 list in
  [history/completion-plan.md](history/completion-plan.md), to be moved into issues.

## In flight

- **Milestone [0.4.0](https://github.com/dmytrobsp/fastview-rea/milestone/1)** — the first user feedback on v0.3.0
  (#1–#3) and a new record format (#8). Plan and ledger: [milestones/0.4.0.md](milestones/0.4.0.md). Pull requests
  into `milestone/0.4.0`: #9 (auto-hide, #4), #10 (COMTRADE 2013 UTF-8 exports, #8), #11 (fractional display
  scaling, #5 #6), #13 (quick analysis after «Кодировка», #7); #12 (the chrome at display scaling) in progress,
  stacked on #11.
- **The workflow** moved to the milestone model on 2026-10-30: [AGENTS.md](../AGENTS.md),
  [agents/workflow.md](agents/workflow.md), [agents/windows-testing.md](agents/windows-testing.md). The completion
  plan's open items and the record's residuals are being filed as issues.

## Next topics

- Finish 0.4.0: the units' follow-ups, the merge gates, the final review, the milestone pull request, then a release
  on the maintainer's word.
- The backlog, once filed: the old plan's open items (the scenario checklist against the original, per-skin looks,
  the remaining residuals). With no milestone open, the controller proposes the next one; the maintainer decides.

List the queue: `gh issue list -S '-label:triage'`.

## Where things live

| What | Where |
|---|---|
| Agent instructions, hard rules | [AGENTS.md](../AGENTS.md) (start here); `CLAUDE.local.md` (main checkout) for this machine |
| The workflow | [agents/workflow.md](agents/workflow.md); Windows runs: [agents/windows-testing.md](agents/windows-testing.md) |
| The queue | [GitHub Issues](https://github.com/dmytrobsp/fastview-rea/issues), milestones and labels |
| Milestones | [milestones/](milestones/README.md): plan and ledger per milestone |
| What the original does | [original/](original/README.md): L1–L9.x and the specs |
| Deliberate differences | [IMPROVEMENTS.md](IMPROVEMENTS.md) |
| History | [history/](history/): the completion plan (to 2026-10-30), the ledger's session notes |
| Tools | [tools/README.md](../tools/README.md); the milestone helper `tools/milestone/milestone.py` |

## Keeping this page

- Rewrite it when a release ships, a milestone starts or ends, or the next topics change.
- Short plain lines; links to issues, milestones and the record instead of detail.
- Nothing private: no user data, no machine or account names.
