#!/usr/bin/env python3
"""Build the factual indexes of FastView.exe in index/ from a copy YOU supply.

  python3 -I tools/index.py <path/to/FastView.exe> [index_dir=index]

Writes (facts about the binary — addresses, ids, names; no code, no
resource text):
  msgmaps.json   every MFC message map (tools/msgmaps.py)
  vtables.tsv    RTTI class, primary vtable address, its slot addresses
  imports.tsv    IAT slot, DLL, function name or #ordinal
  symbols.tsv    address, kind, name, source — the curated names of
                 index/names.tsv (our notes' functions, in our words) over
                 names derived from the above (vtables, message-map
                 handlers, import slots)
tools/symbols_export.py turns symbols.tsv into a Ghidra / IDA / rizin script.
"""
import hashlib
import json
import os
import re
import runpy
import subprocess
import sys

SHA256 = "6c503790d13d9d2d0b80711f27b4daba57b209462cfb626cc5a6f719c150d046"

exe = sys.argv[1]
out = sys.argv[2] if len(sys.argv) > 2 else "index"
here = os.path.dirname(os.path.abspath(__file__))
digest = hashlib.sha256(open(exe, "rb").read()).hexdigest()
if digest != SHA256:
    sys.exit(f"{exe}: sha256 {digest} is not the FastView.exe the notes describe ({SHA256})")
os.makedirs(out, exist_ok=True)

# Message maps (and, through msgmaps.py's globals, the RTTI vtables).
sys.argv = [os.path.join(here, "msgmaps.py"), exe, os.path.join(out, "msgmaps.json")]
g = runpy.run_path(sys.argv[0])
maps = json.load(open(os.path.join(out, "msgmaps.json")))

vt_rows = []
for vt, (cls, offset) in sorted(g["vtables"].items(), key=lambda x: x[0]):
    if offset:
        continue
    slots = []
    while True:
        f = g["u32"](vt + len(slots) * 4)
        if f is None or not g["in_text"](f):
            break
        slots.append(f)
    vt_rows.append((vt, g["demangle"](cls), slots))
with open(os.path.join(out, "vtables.tsv"), "w") as f:
    f.write("vtable\tclass\tslots\n")
    for vt, cls, slots in vt_rows:
        f.write(f"{vt:#x}\t{cls}\t{','.join(hex(s) for s in slots)}\n")

imports = subprocess.run([sys.executable, "-I", os.path.join(here, "peimports.py"), exe], capture_output=True, text=True, check=True).stdout
imp_rows = [line.split(" ", 2) for line in imports.splitlines() if line.startswith("0x")]
with open(os.path.join(out, "imports.tsv"), "w") as f:
    f.write("iat\tdll\tname\n")
    for addr, dll, name in imp_rows:
        f.write(f"{addr}\t{dll}\t{name}\n")


def ident(s):
    return re.sub(r"[^0-9A-Za-z_]", "_", s)


symbols = {}  # address -> (kind, name, source)


def add(addr, kind, name, source):
    if addr not in symbols:
        symbols[addr] = (kind, name, source)


# Curated names first, then message-map handlers, vtable slots and import
# slots: the first name given to an address stays.
names_path = os.path.join(out, "names.tsv")
if os.path.exists(names_path):
    for line in open(names_path, encoding="utf-8").read().splitlines()[1:]:
        if line.strip():
            addr, kind, name, ref = line.split("\t")
            add(int(addr, 16), kind, name, ref)

for m in maps:
    c = ident(m["classes"][-1]) if m["classes"] else "map_" + m["map"]
    add(int(m["map"], 16), "data", f"{c}__messageMap", "msgmaps")
    add(int(m["getter"], 16), "func", f"{c}__GetMessageMap", "msgmaps")
    for e in m["entries"]:
        kind = e["kind"]
        rng = f"{e['id']}" if e["id"] == e["last"] else f"{e['id']}_{e['last']}"
        if kind in ("COMMAND", "CMD"):
            what = f"OnCommand_{rng}"
        elif kind == "CMDUI":
            what = f"OnUpdate_{rng}"
        elif kind.startswith("NOTIFY"):
            what = f"OnNotify_{kind.split('=')[-1]}_{rng}"
        elif kind.startswith("WM_"):
            what = f"On{kind}"
        else:
            what = f"OnMessage_{ident(kind)}"
        if int(e["pfn"], 16):
            add(int(e["pfn"], 16), "func", f"{c}__{what}", "msgmaps")

for vt, cls, slots in vt_rows:
    c = ident(cls)
    add(vt, "data", f"{c}__vftable", "vtables")
    for i, s in enumerate(slots):
        owners = sum(s in other for _, _, other in vt_rows)
        if owners == 1:
            add(s, "func", f"{c}__vf{i}", "vtables")

for addr, dll, name in imp_rows:
    add(int(addr, 16), "data", f"__imp_{ident(dll.rsplit('.', 1)[0])}_{ident(name)}", "imports")

# Names must be unique for the import scripts.
seen = {}
with open(os.path.join(out, "symbols.tsv"), "w", encoding="utf-8") as f:
    f.write("address\tkind\tname\tsource\n")
    for addr in sorted(symbols):
        kind, name, src = symbols[addr]
        n = seen.get(name, 0)
        seen[name] = n + 1
        if n:
            name = f"{name}_{n + 1}"
        f.write(f"{addr:#08x}\t{kind}\t{name}\t{src}\n")
print(f"{out}/: {len(maps)} message maps, {len(vt_rows)} vtables, {len(imp_rows)} imports, {len(symbols)} symbols")
