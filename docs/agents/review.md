# Reviews

How a unit is reviewed: by an independent reviewer the controller dispatches — never the implementer, never a
reviewer the implementer dispatched, never the controller itself. The discipline follows the
[superpowers](https://github.com/obra/superpowers) subagent-driven development skill, adapted; the loop around it is
in [workflow.md](workflow.md#a-unit-from-brief-to-merge).

Why: on milestone 0.4.0 every unit's own review loop (two to seven rounds by a reviewer the implementer briefed)
passed work in which a fresh reviewer then found real bugs in one pass — wrong data merged, focus models that
disagree, tests that pass without their fix, values copied from a user's record.

## The review package

`tools/review-package.sh BASE HEAD OUT` writes the commit list, `git diff --stat` and `git diff -U10` for BASE..HEAD
into OUT (a file in the controller's scratch folder). BASE is the commit recorded before the implementer was
dispatched (or, for a re-review, the head the previous review saw) — never `HEAD~1`. A branch that merged another
unit's branch is reviewed against that branch's head instead of BASE, so the other unit's commits stay out. The
package never enters the controller's context: the reviewer reads it.

## Who reviews, on which model

Every review and re-review is dispatched with the model named: the most capable one for the task review, the final
review and any re-review of an Important finding; the standard model is enough for a scoped re-review of a small,
mechanical fix. The reviewer gets three paths — the brief, the report file, the review package — and the Global
Constraints that bind the unit (the hard rules, the maintainer's decisions, a design's binding values), copied
verbatim: they are its attention lens.

## Task review prompt

Fill the brackets; give the reviewer nothing else from the controller's own view (no hypotheses, no "don't flag").

```
You are reviewing one unit of FastView-REA (a clean-room Rust / egui reconstruction of Fast View 5; behaviour
identical to the original, deliberate improvements documented). This is a gate before the unit merges into its
milestone branch; a whole-milestone review happens separately.

Rules that bind you: AGENTS.md in the main checkout ([MAIN]) — read its hard rules. You are read-only: don't edit,
commit, push or write to GitHub, and don't dispatch subagents (do the review yourself, in passes if it is large).
Bash runs bash (NAME=value, `set -euo pipefail`, no recursive rm / chmod on variable paths). Report the original's
behaviour in prose; never paste disassembly or decompiler output.

What was requested: the issue(s) [ISSUES] (`gh issue view N`), the brief [BRIEF_FILE], and the Global Constraints
that bind it, verbatim: [CONSTRAINTS].
What the implementer claims: [REPORT_FILE] — unverified claims. Rationales in it ("kept simple", "as the original")
are the implementer grading its own work; judge the code.
The change: [PACKAGE_FILE] (BASE [BASE], HEAD [HEAD]); the code at HEAD is in [WORKTREE]. Read the package once;
look outside it only for a risk you can name, and name it.

Tests: the implementer ran them and its report carries the evidence (commands and output) — a claim of passing
tests without that evidence is a finding. Don't re-run the suite; run a focused test (or a mutation: remove a fix, see its
test fail) only when the code raises a specific doubt — tests that pass without their fix are Important findings.
The original: check claims about its behaviour against [MAIN]/extracted/stage1/FastView.exe (MFC:
extracted/mfc/mfc120u.dll) with objdump where a finding depends on them.
User data: compare the change's test data and texts against the users' records in [MAIN]/extracted/samples/ — any
value, name, label or line taken from one is a Critical finding (report the match, never quote it).

Report, starting directly with the verdicts:
### Fidelity and spec — ✅ | ❌ missing / extra / misunderstood (file:line); ⚠️ cannot verify from the change
### Strengths
### Issues — Critical / Important / Minor, each: file:line, what is wrong, the concrete failure scenario, the fix
### Assessment — Approved | Needs fixes, and one or two sentences why
```

Severity: **Critical** — wrong data, a crash, a publishing or user-data breach. **Important** — behaviour that
differs from the original (or from the stated improvement), a missed requirement, fragile state, a test that
asserts nothing or passes without its fix. **Minor** — polish, coverage that could be broader.

## Scoped re-review prompt

```
You are re-reviewing one fix round of a FastView-REA unit. Same rules as the review (AGENTS.md in [MAIN];
read-only; no subagents; prose only).
The findings under verification: [FINDINGS, verbatim]
The fix: [REPORT_FILE] (fix reports appended at the end — where the implementer disputes a finding, its evidence is
there: judge it, and say whether you are convinced); [PACKAGE_FILE] (FIX_BASE [FIX_BASE] — the head the previous
review saw — HEAD [HEAD]); the code in [WORKTREE].
Verdict every finding ADDRESSED, NOT ADDRESSED or DISPUTED (the implementer's case, and yours) with file:line, check that each new test fails without its fix,
and inspect the fix diff for new breakage. Don't re-review code the fix didn't touch: anything outside it goes under
"Out of scope", which never extends the loop.
Report: ### Findings (each ADDRESSED / NOT ADDRESSED) ### New issues in the fix (Critical / Important / Minor)
### Out of scope ### Assessment — Approved | Needs fixes
```

## The final review

The whole milestone (the milestone branch against `main`) as one package, a fresh reviewer on the most capable
model, the task-review prompt with the milestone's issues as "what was requested" and the ledger's deferred minors
and parked findings to triage (each: fix before merge, or file). Its findings go to **one** fix implementer as a
single list, then one scoped re-review; what is left is adjudicated with rulings and reaches the maintainer in the
milestone pull request.
