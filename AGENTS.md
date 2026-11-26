# FastView-REA — instructions for coding agents

FastView-REA is a clean-room Rust / egui reconstruction of НТЦ «Механотроника» Fast View 5, a Windows viewer for
fault records (COMTRADE and others) with analysis views, ОМП and protection characteristics. The goal is behaviour
identical to the original, proven by reading its binary and by side-by-side runs, with deliberate improvements where
the original visibly fails. One maintainer owns the project and the GitHub repository `dmytrobsp/fastview-rea`; agents
do most of the building. This page is the front door: the rules that always apply and the workflow in short.

**Read `CLAUDE.local.md` in the main checkout** (git-ignored; a worktree has none, so the brief gives its path): it
holds this machine's specifics — the shared test boxes and the claim tool, the Windows clones.

## Where things are

| Path | What |
|---|---|
| `crates/fvb-core` | the engine: readers / writers (COMTRADE, `.cff`, `.osc`), formulas, signal maths, ОМП, `.fvb` characteristics |
| `crates/fastview-gui` | the egui application (`fastview`): views, ribbon, panes, dialogs, printing, the software runner |
| [docs/PROGRESS.md](docs/PROGRESS.md) | where the project stands now: latest release, the milestone in flight, next topics |
| [docs/original/](docs/original/README.md) | the record of what the original does, item by item (`L1`–`L9.x`), and the distilled specs |
| [docs/IMPROVEMENTS.md](docs/IMPROVEMENTS.md) | every deliberate improvement over the original, with its reason |
| [docs/ACCEPTED-DIFFERENCES.md](docs/ACCEPTED-DIFFERENCES.md) | differences the maintainer accepted (hard to fix, little benefit, almost invisible) |
| [docs/milestones/](docs/milestones/README.md) | each milestone's plan and committed ledger |
| [docs/design/](docs/design/README.md) | designs of large issues, approved before they are built |
| `docs/agents/` | for agents: [workflow.md](docs/agents/workflow.md), [review.md](docs/agents/review.md), [windows-testing.md](docs/agents/windows-testing.md) |
| `docs/history/` | the old completion plan and the ledger's session notes |
| `index/` | factual indexes of the original (addresses, ids, class / import names, our own symbol names) |
| `tools/` | analysis tools (regenerate the dumps from your own copy), the publish guard, the VM / clone harness, the milestone helper ([tools/README.md](tools/README.md)) |

The work queue is [GitHub Issues](https://github.com/dmytrobsp/fastview-rea/issues) (milestones, labels), not a file.

## Build and test

```sh
cargo build                                             # target/debug/fastview
cargo test --workspace                                  # the whole suite, before every Rust commit
tools/gates.sh                                          # clippy (Linux + Windows target), tests, publish guard: GATES-OK
cargo build -p fastview-gui --release --target x86_64-pc-windows-gnu   # ours for a Windows clone
python3 -I -m unittest discover -s tools/milestone -p 'test_*.py'      # the milestone helper's tests
```

Run `tools/gates.sh` as its own command, never chained with a commit.

The Rust toolchain is pinned in `rust-toolchain.toml` (local builds and CI use it; rustup installs it on first use);
bump it on purpose: change `channel` in that one file, run `tools/gates.sh`, and let CI confirm on all three platforms.

## Hard rules

- **Never publish the original or raw analysis output.** Never commit or push the original program or any part of
  it (`extracted/` — installer, `FastView.exe`, DLLs, help, `vcredist` —, carved payloads, any
  `.exe`/`.dll`/`.chm`/`.cab`/`.msi`), nor raw analysis output: disassembly (objdump listings), decompiler pseudocode
  (Ghidra / REA output, in any form or renaming), resource / string-table / dialog dumps (`dumps/`, `notes/ui/`,
  `notes/*.txt|json|log`). They stay local and git-ignored. What is published is our own work: the Rust code, the
  record and specs (behaviour in our own words, addresses as references), the tools that regenerate the dumps from a
  copy the reader supplies (`tools/regen-dumps.sh`), and the factual indexes in `index/` (approved 2026-10-23).
  Commits, pull requests, issues and comments carry behaviour in prose, never code listings.
- **Never publish user data.** Records users send (`extracted/samples/`) stay local: never commit them, never copy
  their names, values, file names or numbers into tests, commits, issues or pull requests. Tests use synthetic
  records with the same features.
- **The publish guard.** `git config core.hooksPath tools/git-hooks` enables the pre-commit and pre-push hooks; they
  run `tools/publish_guard.py` (forbidden paths, oversized files, pseudocode / objdump-looking lines, non-approved
  emails). CI runs it over the whole history first. Never bypass it (`--no-verify`) and never weaken its rules to make
  a commit pass — fix the commit instead.
- **Commit identity.** Commits use the repository's configured identity (`git config --local user.email`); never
  change it, never put another email into an author, committer or message. `tools/publish_guard.py` holds the
  allowlist. The trailer: `Co-Authored-By: Claude <model> <noreply@anthropic.com>`, naming the model that did the work.
- **Shell.** Agent shells are bash even though the maintainer's login shell is fish: `NAME=value`, never
  `set NAME value`; start every multi-command script with `set -euo pipefail`. Never run a recursive `rm`, `chmod`,
  `chown`, `mv`, `cp` (or `find -delete`) on a path built from a variable: the literal path, or `${VAR:?}` after
  checking it is set and lies where you expect. (An empty variable once turned `chmod -R 700 $S/home` into
  `chmod -R 700 /home`.) No redirects to `/dev/stdin`; close stdin (`</dev/null`) for claim-tool commands in loops.
- **Shared test boxes** (the Windows VM, the Mac) are used only under a claim, through the claim tool named in
  `CLAUDE.local.md`. Runs of the original and of ours go to **throwaway Windows clones**
  ([windows-testing.md](docs/agents/windows-testing.md)), never the shared Windows VM. Never shut down, restart or
  revert a shared box.
- **Nothing on the maintainer's screen**: no screenshots of the real desktop, no windows popping up on it. Look at
  ours through the headless window dump, at the original through a clone.
- **Behaviour first, then looks.** Behaviour identical to the original; looks generally the same — small visual
  differences (a pixel, a font width) are acceptable and noted, not chased. Deliberate improvements over the original
  are allowed where it visibly fails and the better behaviour is obvious; each one is documented in
  [IMPROVEMENTS.md](docs/IMPROVEMENTS.md) with the original's behaviour and the reason. A difference left unfixed is
  recorded in [ACCEPTED-DIFFERENCES.md](docs/ACCEPTED-DIFFERENCES.md) only when the maintainer accepts it; until then it is an open issue.
- **A refused action** (a permission prompt, a safety check): stop, check `git status` for partial writes, explain,
  and give the maintainer the exact command. Never reword or retry it to get around the refusal.
- **The maintainer's word** is needed for: tags and releases, force-pushes and history rewrites, deleting anything on
  GitHub (other than a merged milestone's own branches), repository settings, merging a milestone into `main`.
  Routine pushes of `main` (docs, the milestone ledger, tooling) and of milestone / unit branches need no asking.

## The workflow in short

Full text: [docs/agents/workflow.md](docs/agents/workflow.md).

1. **Where we are**: [docs/PROGRESS.md](docs/PROGRESS.md), the open milestone's issues
   (`gh issue list -m <milestone>`), its ledger in [docs/milestones/](docs/milestones/README.md), `git log`, the
   open pull requests.
2. **Pick work** from the open milestone. With none open, propose one (a few related issues, and why); the
   maintainer decides. `needs-user` issues wait for the maintainer; `triage` ones are reports nobody has looked at.
3. **A milestone** runs with one controller and several implementers, one unit (an issue or a few) per branch and
   pull request into `milestone/<name>`. Only the controller talks to the maintainer, pushes, opens pull requests and
   files issues.
4. **A unit is done only when it is done**: every in-scope problem fixed (review findings too), questions asked and
   answered, follow-ups listed for the controller to file as issues. Never report done with something known left.
   Every unit is reviewed by an independent reviewer the controller dispatches (never the implementer's own), in a
   fix loop of at most five rounds, each with a scoped re-review ([review.md](docs/agents/review.md)).
5. **Large issues are designed first** (`size:l`): brainstorm with the maintainer, a design in `docs/design/`, a fresh
   builder's read, the maintainer's approval, then a plan with Global Constraints. Each dispatch names its model
   (cheap for mechanical work, standard for judgment, the most capable for reviews and design).
6. **Build in small verified steps**: the test that fails without the fix first, `tools/gates.sh` green, one commit
   each. Check the original in its binary and in a clone; say what was checked how (UNIT, VM, NOT).
7. **Record**: what the original does goes into [docs/original/](docs/original/README.md); deliberate differences
   into [IMPROVEMENTS.md](docs/IMPROVEMENTS.md), accepted ones into [ACCEPTED-DIFFERENCES.md](docs/ACCEPTED-DIFFERENCES.md) (decisions live
   in docs; issues are their entry points); the milestone's events into its ledger. Anything left open is an issue,
   never a line in a file.
