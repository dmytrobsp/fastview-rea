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
  original); `area:*` (ui, diagrams, formats, formulas, omp, print, scaling, windows, tools, docs, ci); `size:s` / `size:m` /
  `size:l` (`l` needs a design first); `needs-vm` (needs a run of the original in a Windows clone); `needs-user` (a
  decision only the maintainer can make); `process` (how the work gets done); `triage` (a report nobody has looked at
  yet — the issue forms set it); `accepted` (closed: a difference the maintainer accepted, see below).
  `-label:triage` is the curated queue.
- **Commits that fix an issue** say `Fixes #N` (the pull request too).
- **New items** — a residual difference from the original, a bug found on the way, a review finding outside the
  unit's scope, a wish the maintainer adds — become issues with public wording. Only the controller files them; for
  a batch, show the maintainer the drafts first. A residual is never left as a line in the record: the entry links
  the issue (`Residual: #N`).
- **Decisions live in docs, issues are their entry points.** A deliberate improvement is recorded in
  [IMPROVEMENTS.md](../IMPROVEMENTS.md) by the pull request that makes it (the original's behaviour, ours, the
  reason, the issue). A difference the maintainer accepts — hard to fix for little benefit, almost invisible — is
  recorded in [ACCEPTED-DIFFERENCES.md](../ACCEPTED-DIFFERENCES.md) (what differs, where it shows, why it isn't fixed, the decision with its
  date), and its issue is closed as not planned with the `accepted` label and a link to the entry. Only the
  maintainer accepts a difference; agents propose.
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
   the original, ACCEPTED-DIFFERENCES.md or the milestone ledger** (the report carries the drafts), never run
   `milestone.py`, **never dispatch subagents** (no reviewers of its own: the review is the controller's). A
   deliberate improvement adds its own IMPROVEMENTS.md entry in the unit. The controller records the base commit.
2. **Implementation.** Reproduce first (headless or in a clone); read the original in its binary where behaviour must
   be matched; the test that fails without the fix (shown failing, by reverting the fix); `tools/gates.sh` →
   GATES-OK; one commit per logical step. The implementer reviews its own diff before reporting, but that never
   replaces the independent review.
3. **The report** — written to a report file in the scratch folder (fix rounds append to it); the unit is not done
   while anything below is open:
   - **Status**: DONE, DONE_WITH_CONCERNS (with the concerns), or BLOCKED (with the question and its recommended
     answer). The controller answers a question itself (a ruling in the ledger, with what it costs if wrong) or asks
     the maintainer, then resumes the same implementer, which goes on.
   - **Fixed**: what changed, commits, tests (which failed before the fix, and the command and output), gates.
   - **Follow-ups**: out-of-scope items found, each as a draft issue (title, a few lines, public wording).
   - **Dropped**: true nits only, one line each with why.
   - **Verified how**: UNIT (tests), VM (a clone run, what was seen), NOT (not exercised, and why).
   - **Drafts**: the pull request body (the record entry in prose — what the original does with addresses as
     references, what ours did, what it does now, how it was checked — and "Improvements over the original" where
     any: the original's behaviour, ours, the reason).
4. **Controller's check.** The controller runs the publish guard and the user-data check over every new commit and
   the body (`git log -p` of the range and the body in a scratch folder, compared field by field and line by line
   against the users' records — any value taken from one is a finding, however generic it looks), then pushes
   (`git push -u origin <unit branch>`) and opens or updates the pull request (`--base milestone/NAME`, `Fixes #N`,
   milestone set). CI runs on it. The controller doesn't judge the code here: that is the review's job.
5. **Task review** by an independent reviewer — never the implementer, never a reviewer the implementer dispatched,
   never the controller ([review.md](review.md) has the prompts):
   - The controller writes a **review package** (the commit list, `git diff --stat` and `git diff -U10` for
     BASE..HEAD, BASE being the commit recorded before dispatch — never `HEAD~1`) to a file and gives the reviewer
     its path, the brief and the report file. The package never enters the controller's context.
   - Two verdicts are required: **fidelity and spec** (does it do what the issue and the original require — nothing
     missing, extra or misunderstood) and **quality** (correct, tested — each test failing without its fix —, no
     publishing breach), with findings graded Critical / Important / Minor, each with file:line.
   - The controller never pre-judges: no "don't flag X" in a reviewer's prompt.
6. **The fix loop** runs while the review has a spec ❌ or a Critical / Important finding. Minors don't enter it: they
   go to the ledger as deferred minors for the final review to triage. A round is one fix dispatch plus one **scoped
   re-review** (the findings list and the fix diff only, FIX_BASE..HEAD; each finding ADDRESSED or NOT ADDRESSED;
   new breakage in the fix diff joins the list; observations outside it go to the ledger). Rounds 1–3 resume the
   same implementer with the findings verbatim; rounds 4–5 go to a fresh implementer on a more capable model ("a
   prior implementer tried N times; read the report file"). After round 5 the controller adjudicates each open
   finding — parked with a ruling, or ruled on when later work builds on it — and every adjudication is a ledger
   line. The controller never fixes findings itself, and never ends the loop early by ruling. Each round goes to the
   ledger: `<unit>: fix round R/5 (X addressed, Y open — …; commits a..b)`.
7. **Merge gate**, one unit at a time, after a clean review (or the breaker), never rewriting a pushed branch:
   1. the pull request's head equals the local unit branch; CI green on it;
   2. `milestone.py gate NAME UNIT`; `tools/gates.sh` in the gate worktree on the merge result;
   3. `git push origin milestone/NAME` from the main checkout (GitHub marks the pull request merged);
   4. the record entry (`docs/original/`) and the ledger line (`<unit>: complete (a..b, review clean)` or with the
      parked count), committed on `main`; the follow-up issues filed; `milestone.py rm NAME UNIT`.
   A red gate: `ungate`, the unit goes back to its implementer (it merges the milestone tip into its branch and
   fixes it); a conflict the same way.

### The end of a milestone

1. All units merged. A **final whole-milestone review** (the milestone branch against `main`, as a review package)
   by a fresh reviewer on the most capable model, pointed at the ledger's deferred minors and parked findings to
   triage. Its findings go to **one** fix implementer as a single list, then **one** scoped re-review; residual
   findings are adjudicated as in the breaker. No second fix wave: what is left goes to the maintainer.
2. The checks: `tools/gates.sh`, the Windows test executable run in a clone, ours against the original in a clone
   for each fixed issue's scenario.
3. **The milestone pull request** (`milestone/NAME` into `main`) with the report: per unit what changed and how it
   was verified, the improvements, the follow-ups filed, the health line, and **every ruling the controller made**
   (from the ledger, in order, each with what it costs if wrong). The maintainer says "merge"; the controller merges
   with a merge commit (not squashed, so `Fixes #N` closes the issues), runs the gates on `main`, pushes.
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
- A deliberate improvement goes into [IMPROVEMENTS.md](../IMPROVEMENTS.md) too, with the reason; a difference the
  maintainer accepted into [ACCEPTED-DIFFERENCES.md](../ACCEPTED-DIFFERENCES.md). Neither list is a task list.

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
