#!/usr/bin/env bash
# Regenerate the analysis dumps this project's notes refer to, from a copy of
# the original program that YOU supply (it is not distributed here).
#
#   tools/regen-dumps.sh <path/to/FastView.exe> [out_dir=dumps]
#
# Writes into out_dir: fv.asm (objdump, Intel syntax), imports.txt (IAT slots),
# strings.txt (string table), msgmaps.json (MFC message maps), vtables.txt
# (RTTI vtables). Decompilation (Ghidra via REA) is described in tools/README.md.
set -euo pipefail
exe="${1:?usage: tools/regen-dumps.sh <FastView.exe> [out_dir]}"
out="${2:-dumps}"
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
mkdir -p "$out"
objdump -d -M intel "$exe" > "$out/fv.asm"
# -X utf8: the dumps are UTF-8 everywhere. Without it Python on Windows writes the
# redirected output in the ANSI code page (cp1251), which stops at the first
# character it lacks (an «Ω» in the string table); -I ignores PYTHONUTF8.
py=(python3 -I -X utf8)
"${py[@]}" "$here/peimports.py" "$exe" > "$out/imports.txt"
"${py[@]}" "$here/pestr.py" "$exe" > "$out/strings.txt"
"${py[@]}" "$here/msgmaps.py" "$exe" "$out/msgmaps.json"
"${py[@]}" "$here/vt.py" "$exe" > "$out/vtables.txt"
echo "dumps written to $out/"
