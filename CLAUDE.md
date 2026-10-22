# Rules for agents working in this repository

## Never publish the original or raw analysis output
- Never commit or push the original program or any part of it: `extracted/`
  (installer, `FastView.exe`, DLLs, help, `vcredist`), carved payloads
  (`vcr/`), or any `.exe`/`.dll`/`.chm`/`.cab`/`.msi` file.
- Never commit raw analysis output: disassembly (`fv.asm`, objdump
  listings), decompiler pseudocode (Ghidra/REA output, in any form or
  renaming), resource / string-table / dialog dumps (`dumps/`, `notes/ui/`,
  `notes/*.txt|json|log`). They stay local and git-ignored.
- What is published is our own work: the Rust code, the distilled specs and
  the ledger (behaviour in our own words, addresses as references), and the
  tools that regenerate the dumps from a copy the reader supplies
  (`tools/regen-dumps.sh`).
- When briefing sub-agents, include these rules and ask for behaviour in
  prose, never code listings.

## Commit identity
- Commits are authored with the repository's configured identity
  (`git config --local user.email`); never change it and never put any other
  email address into a commit's author, committer or message.
  `tools/publish_guard.py` holds the allowlist.

## The guard
- `git config core.hooksPath tools/git-hooks` enables the pre-commit and
  pre-push hooks (set in the maintainer's clone); they run
  `tools/publish_guard.py`, which refuses forbidden paths, oversized files,
  pseudocode / objdump-looking lines and non-approved emails. CI runs it on
  the whole history (`Publish guard` job) before anything is built or
  released. Never bypass it (`--no-verify`) and never weaken its rules to
  make a commit pass — fix the commit instead.
