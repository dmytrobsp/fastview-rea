#!/usr/bin/env bash
# review-package.sh BASE HEAD OUT: a unit's review package — the commit list, the stat summary and the diff with
# ten lines of context for BASE..HEAD — written to OUT for a reviewer to read once (docs/agents/review.md).
set -euo pipefail
[ $# -eq 3 ] || { echo "usage: review-package.sh BASE HEAD OUT" >&2; exit 2; }
base=$1 head=$2 out=$3
git rev-parse --verify --quiet "$base^{commit}" >/dev/null || { echo "no commit $base" >&2; exit 2; }
git rev-parse --verify --quiet "$head^{commit}" >/dev/null || { echo "no commit $head" >&2; exit 2; }
{
    echo "# Review package: $base..$head"
    echo
    echo "## Commits"
    git log --reverse --format='%h %s' "$base..$head"
    echo
    echo "## Stat"
    git diff --stat "$base" "$head"
    echo
    echo "## Diff"
    git diff -U10 "$base" "$head"
} >"$out"
echo "$out: $(wc -l <"$out") lines, $(git rev-list --count "$base..$head") commits"
