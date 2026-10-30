# Milestones

One file per milestone, `<name>.md` (e.g. [0.4.0.md](0.4.0.md)): its plan and its ledger — what was agreed at the
start and what happened to each unit. It is how the work resumes after a crash, a lost session or on another machine.
The workflow around it: [Running a milestone](../agents/workflow.md#running-a-milestone).

## Where

Committed on `main` (not on the milestone branch), by the controller only, in docs-only commits: the plan part
before the first dispatch, then an entry at every event. Bulky scratch — briefs, review packages, reports, agent ids —
stays in the controller's scratch folder, uncommitted.

## What goes in

**The plan** (at the top):

- The milestone's issues and why they go together; the start sitting's answers, with the date and the maintainer's
  words where they matter.
- The units (unit → issues, branch, pull request) and the batching: which run at once, which wait, and why.

**The ledger** (below it), one short line per event, in order:

- **Per unit**: dispatched (base commit), its commits, the controller's check, pushed, review rounds, the gate
  (conflicts, red runs), merged (time).
- **What was checked**: UNIT, VM (a clone run, what was seen), NOT (and why).
- **Rulings**: a decision the controller made without the maintainer, with what it costs if wrong.
- **The maintainer's decisions**, with the date.
- **Not fixed**: a nit dropped (why); an item filed (`#N`) because it is out of scope or the maintainer's call.
- **Outside events**: a red CI run and its fix, a refused action, no clone room.
- **At the end**: the final review's outcome, the checks, the milestone pull request, a short retro.

Nothing private: no user data, no machine or account names, no emails. Public wording, as in commits and issues.
