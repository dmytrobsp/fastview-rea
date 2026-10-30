# How work is done on FastView-REA

The full workflow behind [AGENTS.md](../../AGENTS.md): where things stand, the queue, running a milestone, a unit
from brief to merge, recording what the original does, verifying, committing. Runs of the original and of ours on
Windows have their own page: [windows-testing.md](windows-testing.md). The approach follows the maintainer's other
project, adapted.

## Where things stand

Read, in this order:

1. [docs/PROGRESS.md](../PROGRESS.md) — the latest release, the milestone in flight, the next topics. A short
   current-state page: rewritten when things change, never appended to.
2. The open milestone's issues: `gh issue list -m <milestone>`, and its pull requests: `gh pr list`.
3. The milestone's ledger in [docs/milestones/](../milestones/README.md): the start sitting's answers, the units, what
   happened to each.
4. `git log --oneline -20`, `git status`, `python3 tools/milestone/milestone.py ls`, the latest CI runs
   (`gh run list -L 5`).

Never re-run a unit the ledger or `git log` shows done. What the original does is in
[docs/original/](../original/README.md); older history in `docs/history/`.

## The queue: GitHub Issues

Everything to do is an issue; nothing is queued in a file — not in the record of the original, not in a plan.

- **Milestones**: one per coming release (`0.4.0`, …) plus `later`. When a release milestone is open, take its
  issues. When none is open, don't pick from `later` on your own: propose a milestone (a few related issues, and why
  they go together); the maintainer decides.
- **Labels**: the kind (`bug`, `minor`, `feature`, `known-issue`, `improvement` — a deliberate difference from the
  original); `area:*` (ui, diagrams, formats, omp, print, scaling, windows, tools, docs, ci); `size:s` / `size:m` /
  `size:l` (`l` needs a design first); `needs-vm` (needs a run of the original in a Windows clone); `needs-user` (a
  decision only the maintainer can make); `process` (how the work gets done); `triage` (a report nobody has looked at
  yet — the issue forms set it). `-label:triage` is the curated queue.
- **Commits that fix an issue** say `Fixes #N` (the pull request too).
- **New items** — a residual difference from the original, a bug found on the way, a review finding outside the
  unit's scope, a wish the maintainer adds — become issues with public wording. Only the controller files them; for
  a batch, show the maintainer the drafts first. A residual is never left as a line in the record: the entry links
  the issue (`Residual: #N`).
- **User reports** (often in Ukrainian or Russian) stay open as filed; the controller splits them into issues it can
  work, links them, and doesn't close or comment on the reporter's issue without the maintainer.

## Running a milestone

One controller session drives a milestone; implementers run at once in their own worktrees; the maintainer answers
questions at the start and looks again at the end.

- **A unit** is one issue, or a few related ones: its own branch `m/<milestone>/<unit>`, its own worktree with its
  own `target/`, its own implementer, its own pull request into `milestone/<milestone>`. Tiny unrelated issues are
  bundled into one unit.
- **The controller** is the only agent that talks to the maintainer, pushes, opens or edits pull requests, files
  issues, or writes the record of the original, IMPROVEMENTS.md and the ledger. It plans units and their order, writes
  briefs, dispatches implementers and reviewers, answers implementers' questions, runs the merge gate, the final
  review and the report. It keeps its own context lean: short reports, details in scratch files.
- **Agent ids**: the controller keeps every background agent's id in its scratch folder (`agents.md`: unit, role, id),
  so a fix round, a conflict or a lost session can resume the agent with its context.
- **How many at once**: three implementers that build; at most three Windows clones exist at once (each 6.5 GB of
  RAM; `<claim tool> slot status` shows the room), so units needing the original's runs are staggered.

### The start sitting

1. Agree on the milestone's issues: proposed by the controller when none is open, decided by the maintainer.
2. The controller reads every issue, its history and the code it touches, then brings, in one message, every question
   the milestone needs answered (behaviour, a deliberate improvement or not, `needs-user` items) and its batching:
   which units run at once because they touch separate code, which wait for another, and why.
3. The answers and the batching go into the milestone's file in [docs/milestones/](../milestones/README.md),
   committed on `main` before the first dispatch.

### Branches and worktrees

`tools/milestone/milestone.py` does the git part (exit 0 done, 2 refused, 3 merge conflict):
`start NAME` (branch `milestone/NAME` from `main` and the gate worktree `<root>/NAME/_gate`), `add NAME UNIT` (branch
`m/NAME/UNIT` and its worktree from the milestone tip), `gate NAME UNIT` (merge `--no-ff` into the gate), `ungate
NAME`, `rm NAME UNIT`, `end NAME`, `ls`. The root is `$FV_WORKTREES` or `<repository>-worktrees` beside the
repository. Only the controller runs it. Pushes run from the main checkout, which stays on `main`.

### A unit, from brief to merge

1. **Brief.** Written to the controller's scratch folder with the Write tool, then dispatched in the background. It
   holds: the issue(s) and the maintainer's answers; the worktree, branch and the main checkout's absolute path
   (`extracted/` and `CLAUDE.local.md` are only there); the hard rules of AGENTS.md (publishing, user data, shell,
   identity and trailer, guard); the clone rules of [windows-testing.md](windows-testing.md) with the unit's own
   `FV_CLONE` file; and the limits: **never `git push`, never `gh pr` / `gh issue` writes, never edit the record of
   the original, IMPROVEMENTS.md or the milestone ledger** (the report carries the drafts), never run
   `milestone.py`.
2. **Implementation.** Reproduce first (headless or in a clone); read the original in its binary where behaviour must
   be matched; the test that fails without the fix (shown failing, by reverting the fix); `tools/gates.sh` →
   GATES-OK; one commit per logical step.
3. **Own review loop.** The implementer has a fresh reviewer read its diff (correctness and fidelity to the original,
   not style) and fixes what is real, until the reviewer finds nothing significant.
4. **The report** — the unit is not done while anything below is open:
   - **Fixed**: what changed, commits, tests (which failed before the fix), the gates result.
   - **Questions**: anything needing a decision, each with the recommended answer. An implementer with an open
     question reports **BLOCKED**, not done. The controller answers it (a ruling in the ledger, with what it costs if
     wrong) or asks the maintainer, then resumes the same implementer, which goes on.
   - **Follow-ups**: out-of-scope items found, each as a draft issue (title, a few lines, public wording).
   - **Dropped**: true nits only, one line each with why.
   - **Verified how**: UNIT (tests), VM (a clone run, what was seen), NOT (not exercised, and why).
   - **Drafts**: the pull request body (the record entry in prose — what the original does with addresses as
     references, what ours did, what it does now, how it was checked — and "Improvements over the original" where
     any: the original's behaviour, ours, the reason).
5. **Controller's check.** The controller reads the diff for obvious problems left (sends them back: they are
   fixed, not filed), runs the publish guard and a user-data check over every new commit and the body
   (`git log -p milestone/NAME..m/NAME/UNIT` and the body in a scratch folder: no sample names or values, no binary
   listings), then pushes (`git push -u origin m/NAME/UNIT`) and opens or updates the pull request
   (`--base milestone/NAME`, `Fixes #N`, milestone set). CI runs on it.
6. **Review.** A fresh reviewer reads the pushed diff against the issue and the rules; findings go to the same
   implementer (resumed), then a scoped re-review. Every real finding in scope is fixed; nits dropped with a reason in
   the ledger; out-of-scope items and the maintainer's decisions become issues.
7. **Merge gate**, one unit at a time, never rewriting a pushed branch:
   1. the pull request's head equals the local unit branch; CI green on it;
   2. `milestone.py gate NAME UNIT`; `tools/gates.sh` in the gate worktree on the merge result;
   3. `git push origin milestone/NAME` from the main checkout (GitHub marks the pull request merged);
   4. the record entry (`docs/original/`), IMPROVEMENTS.md entries and the ledger line, committed on `main`; the
      follow-up issues filed; `milestone.py rm NAME UNIT`.
   A red gate: `ungate`, the unit goes back to its implementer (it merges the milestone tip into its branch and
   fixes it); a conflict the same way.

### The end of a milestone

1. All units merged; a final whole-milestone review (the milestone branch against `main`) and one fix wave through
   the same review and gate.
2. The checks: `tools/gates.sh`, the Windows test executable run in a clone, ours against the original in a clone
   for each fixed issue's scenario.
3. **The milestone pull request** (`milestone/NAME` into `main`) with the report: per unit what changed and how it
   was verified, the improvements, the follow-ups filed, the health line. The maintainer says "merge"; the controller
   merges with a merge commit (not squashed, so `Fixes #N` closes the issues), runs the gates on `main`, pushes.
4. **Cleanup**: unit and gate worktrees removed (`rm`, `end`), the milestone's own branches deleted on GitHub and
   locally, the GitHub milestone closed once nothing is open in it, clones removed. A release follows only on the
   maintainer's word.

## Recording what the original does

[docs/original/](../original/README.md) is the record of the original's behaviour — not a task list.

- An entry per behaviour read or settled: `### L9.N — <behaviour as a sentence>  ·  <status>`; what the original
  does (addresses as references), what ours did and does now, the test that pins it, how it was checked.
- Our own words only: no disassembly, no decompiler output, no resource text.
- Entries come from the unit's pull request body and are written on `main` by the controller at the merge, so units
  never conflict over the record.
- A deliberate difference goes into [IMPROVEMENTS.md](../IMPROVEMENTS.md) too, with the reason.

## Making a change

- **Small verified steps**: one focused change, verified, committed; then the next.
- **Test first**: the failing test, shown failing, then the fix. A test that passes without the fix proves nothing.
- **Look at ours headless**: `FV_WINDOW_DUMP=<png> FV_WINDOW_SIZE=1280,800 FV_WINDOW_RECORD=<cfg>
  FV_WINDOW_CLICK=x,y FV_WINDOW_HOVER=x,y cargo test -p fastview-gui --bin fastview window_dump` (the `window_dump`
  test lists every knob); compare pixel columns with `convert x.png -crop 1xN+X+Y txt:-` against a clone's shot.
- **Read the original**: objdump or the REA tools on `extracted/stage1/FastView.exe` (main checkout), MFC in
  `extracted/mfc/mfc120u.dll`; a sub-agent's claim about the binary is spot-checked by the controller.
- **Behaviour first**: identical behaviour; small visual differences acceptable; improvements only where the original
  visibly fails, documented.

## Verification and honesty

- Every unit says what was checked how: **UNIT** (tests), **VM** (in a Windows clone, ours and / or the original,
  with what was seen), **NOT** (written, not exercised — and why). Never round it up.
- Evidence before claims: run the command, read the output, then say it passed.
- A failing test is never put down to the machine: find the cause.

## Commits and pushes

- **One step, one commit**; `git add` / `git commit` with explicit paths; never a scripted edit chained with a commit.
- **The trailer** names the model; the repository's identity is never changed.
- **Pushes**: `main` (docs, ledger, tooling) in batches, never tiny commits alone; unit and milestone branches by the
  controller as above. Tags, releases, force-pushes, deleting on GitHub and merging a milestone into `main` need the
  maintainer's word.
- **CI** runs on pushes to `main` and on every pull request: the publish guard over the whole history, clippy, tests,
  the release builds. Watch it after a push that runs it.
