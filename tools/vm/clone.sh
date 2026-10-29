#!/usr/bin/env bash
# clone.sh: runs of the original and ours in a throwaway Windows clone of
# testbox (`instance new --desktop windows`), instead of a shared VM.
#
#   clone.sh new                 start a clone and prepare it (below); prints its name
#   clone.sh run o|u "<keys>" [record]
#                                drive.ps1 in the clone: the original (o) or ours (u),
#                                maximised first (not with FV_NOMAX=1), with
#                                records\<record> (WIN_1999.cfg, "-" for none); FV_FRESH=1
#                                first drops its saved state (a first run);
#                                every SHOT named in the keys lands in $FV_SHOTS
#   clone.sh push SRC DST | pull SRC DST | exec CMD... | desk CMD... | shot OUT.png
#   clone.sh ours                push a fresh ours (after a rebuild)
#   clone.sh rm                  remove the clone
#
# Preparing: the original from your own copy (FV_ORIG, default extracted/stage1:
# its files, the English resource DLL moved to eng\ so the UI is Russian), the
# VC++ 2013 x86 runtime from it, ru-RU as system locale and Format (the
# original narrows names through the thread code page) and one restart, the
# records (FV_RECORDS, default ~/Downloads: BINARY, DOC_1999, WIN_1999), this
# folder as harness\, and ours (target/x86_64-pc-windows-gnu/release). Layout
# in the guest: C:\Users\tbuser\fv-test\{FastView5,harness,records,out,ours}.
# The clone's name and id are kept in FV_CLONE (0600); the id is the key to
# the clone, so it is never printed.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
TB=(python3 "$HOME/projects/testbox/tb.py")
state=${FV_CLONE:-$HOME/.cache/fastview-rea/clone}
orig=${FV_ORIG:-$repo/extracted/stage1}
records=${FV_RECORDS:-$HOME/Downloads}
shots=${FV_SHOTS:-$PWD}
ours=$repo/target/x86_64-pc-windows-gnu/release/fastview.exe
G='C:\Users\tbuser\fv-test'

load() {
    [ -f "$state" ] || { echo "no clone (run: clone.sh new)" >&2; exit 2; }
    read -r name id <"$state"
}
tb_exec() { "${TB[@]}" instance exec "$name" "$id" -- "$@"; }
tb_desk() { "${TB[@]}" instance exec --desktop "$name" "$id" -- "$@"; }
tb_push() { "${TB[@]}" instance push "$name" "$id" "$1" "$2"; }

# The guest answers ssh and its desktop (explorer) is up again.
wait_desktop() {
    local t
    for t in $(seq 1 60); do
        if tb_exec 'tasklist /fi "imagename eq explorer.exe" /nh' 2>/dev/null | grep -qi explorer; then return 0; fi
        sleep 5
    done
    echo "clone: no desktop after the restart" >&2; exit 5
}

push_ours() {
    [ -f "$ours" ] || { echo "no $ours (cargo build -p fastview-gui --release --target x86_64-pc-windows-gnu)" >&2; exit 2; }
    tb_push "$ours" "$G\\ours\\fastview.exe"
}

case ${1:-} in
new)
    [ ! -f "$state" ] || { echo "a clone is already recorded in $state (clone.sh rm first)" >&2; exit 2; }
    [ -f "$orig/FastView.exe" ] && [ -f "$orig/vcredist_vs13_x86.exe" ] || { echo "no original in $orig (FV_ORIG)" >&2; exit 2; }
    out=$("${TB[@]}" instance new --desktop windows --purpose "fastview-rea: runs of the original and ours" --wait 15m)
    name=$(printf '%s\n' "$out" | sed -n 's/^instance \([^ ]*\) id=\([^ ]*\) .*/\1/p')
    id=$(printf '%s\n' "$out" | sed -n 's/^instance \([^ ]*\) id=\([^ ]*\) .*/\2/p')
    [ -n "$name" ] && [ -n "$id" ] || { echo "clone: instance new said: $out" >&2; exit 5; }
    mkdir -p "$(dirname "$state")"; (umask 077; printf '%s %s\n' "$name" "$id" >"$state")
    echo "clone $name: preparing"
    tb_exec "mkdir $G\\FastView5\\eng $G\\records $G\\out $G\\ours $G\\harness\\shots"
    for f in "$orig"/*; do
        case $(basename "$f") in
        FastView_ResEng.dll) tb_push "$f" "$G\\FastView5\\eng\\FastView_ResEng.dll" ;;
        *) tb_push "$f" "$G\\FastView5\\$(basename "$f")" ;;
        esac
    done
    tb_exec "$G\\FastView5\\vcredist_vs13_x86.exe /install /quiet /norestart"
    for r in BINARY DOC_1999 WIN_1999; do
        for e in cfg dat; do tb_push "$records/$r.$e" "$G\\records\\$r.$e"; done
    done
    for f in "$here"/*.ps1; do tb_push "$f" "$G\\harness\\$(basename "$f")"; done
    push_ours
    tb_exec 'powershell -NoProfile -Command "Set-WinSystemLocale ru-RU; Set-Culture ru-RU"'
    tb_exec 'shutdown /r /t 0' || true
    sleep 20
    wait_desktop
    tb_exec 'powershell -NoProfile -Command "(Get-WinSystemLocale).Name; (Get-Culture).Name"'
    echo "clone $name: ready"
    ;;
run)
    load
    who=$2; keys=$3; rec=${4:-WIN_1999.cfg}
    H="$G\\harness"
    open="-Open $G\\records\\$rec"; [ "$rec" != - ] || open=""
    [ -n "${FV_NOMAX:-}" ] || keys="% ;x;;$keys"
    tb_exec "powershell -NoProfile -ExecutionPolicy Bypass -File $H\\cleanup.ps1" >/dev/null 2>&1 || true
    if [ "$who" = o ]; then exe="$G\\FastView5\\FastView.exe"; extra=""; else exe="$G\\ours\\fastview.exe"; extra="-Soft"; fi
    if [ -n "${FV_FRESH:-}" ]; then
        if [ "$who" = o ]; then
            tb_exec 'powershell -NoProfile -Command "Get-ChildItem HKCU:\Software | Get-ChildItem | Where-Object PSChildName -like *FastView | Remove-Item -Recurse"'
        else
            tb_exec 'if exist "%APPDATA%\fastview-rea" rmdir /s /q "%APPDATA%\fastview-rea"'
        fi
    fi
    tb_exec "powershell -NoProfile -ExecutionPolicy Bypass -File $H\\drive.ps1 -Exe $exe $open $extra -Keys \"$keys\"" 2>&1 | grep -v '^step' | tail -3
    for s in $(printf '%s' "$keys" | grep -o 'SHOT [^;]*' | cut -d' ' -f2); do
        "${TB[@]}" instance pull "$name" "$id" "$H\\shots\\$s" "$shots/$s"
    done
    ;;
push) load; tb_push "$2" "$3" ;;
pull) load; "${TB[@]}" instance pull "$name" "$id" "$2" "$3" ;;
exec) load; shift; tb_exec "$@" ;;
desk) load; shift; tb_desk "$@" ;;
shot) load; "${TB[@]}" instance shot "$name" "$id" "$2" ;;
ours) load; push_ours ;;
rm)
    load
    "${TB[@]}" instance rm "$name" "$id"
    rm -f "$state"
    ;;
*) sed -n '2,13p' "$0" | sed 's/^# \{0,1\}//'; exit 2 ;;
esac
