#!/usr/bin/env bash
# gates.sh: clippy (Linux and the Windows cross-target, no warnings), the
# whole test suite, the publish guard over the history. Prints GATES-OK only
# when everything passed; logs in target/gates/. Run it as its own command,
# never chained with a commit.
set -euo pipefail
cd "$(dirname "$0")/.."
L=target/gates
mkdir -p "$L"
cargo clippy --workspace --all-targets 2>"$L/clippy.log" || { tail -30 "$L/clippy.log"; echo GATES-FAIL clippy; exit 1; }
w=$(grep -c '^warning' "$L/clippy.log" || true)
[ "$w" = 0 ] || { grep -A6 '^warning' "$L/clippy.log" | head -60; echo "GATES-FAIL clippy warnings $w"; exit 1; }
cargo clippy -p fastview-gui --all-targets --target x86_64-pc-windows-gnu 2>"$L/clippyw.log" || { tail -30 "$L/clippyw.log"; echo GATES-FAIL clippy-win; exit 1; }
w=$(grep -c '^warning' "$L/clippyw.log" || true)
[ "$w" = 0 ] || { grep -A6 '^warning' "$L/clippyw.log" | head -60; echo "GATES-FAIL win clippy warnings $w"; exit 1; }
cargo test --workspace >"$L/test.log" 2>&1 || { grep -E '^error|FAILED|panicked' "$L/test.log" | head -40; echo GATES-FAIL tests; exit 1; }
if grep -qE '^error|FAILED' "$L/test.log"; then grep -E '^error|FAILED' "$L/test.log" | head; echo GATES-FAIL tests; exit 1; fi
passed=$(grep -o '[0-9]* passed' "$L/test.log" | awk '{s+=$1} END {print s}')
python3 -I tools/publish_guard.py --all >/dev/null || { echo GATES-FAIL guard; exit 1; }
echo "tests passed: $passed"
echo GATES-OK
