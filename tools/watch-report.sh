#!/usr/bin/env bash
# watch-report.sh FILE PATTERN MAX_SECONDS [LABEL]: the controller's watchdog for a dispatched agent. Waits until FILE
# exists and contains a line matching PATTERN (grep -E), e.g. '^# Round 3' or '^### Assessment', then prints
# "READY <label>" and exits 0; after MAX_SECONDS without it prints "TIMEOUT <label>" and exits 3. Run it in the
# background right after every dispatch or resume, so the controller is woken by its exit even when the agent's own
# completion notice never arrives (an agent with a live background child sends none; a second hand-back in one run is
# refused).
set -euo pipefail
f=${1:?file} pat=${2:?pattern} max=${3:?max seconds} label=${4:-$1}
end=$(( $(date +%s) + max ))
while [ "$(date +%s)" -lt "$end" ]; do
  if [ -f "$f" ] && grep -Eq -- "$pat" "$f"; then
    sleep 20   # let the agent finish appending its section
    echo "READY $label: $(grep -E -- "$pat" "$f" | tail -1 | cut -c1-120)"
    exit 0
  fi
  sleep 30
done
echo "TIMEOUT $label after ${max}s: no line matching '$pat' in $f"
exit 3
