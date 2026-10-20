# Ghidra headless postScript (Jython 2.7 / Python2 syntax).
# Finds functions that reference the FastView expression-engine tokens and the
# .fvb XML geometry tag names, decompiles them, and writes them to OUT_FILE.
import os
from ghidra.app.decompiler import DecompInterface
from ghidra.util.task import ConsoleTaskMonitor

OUT_FILE = os.environ.get("FV_DUMP_OUT", "/tmp/fv_dump.txt")

# Literals that mark the code we care about. Matched case-sensitively as a
# substring of each defined string's value (covers ascii + unicode strings).
TOKENS = [
    "tg:", "ctg:", "sin:", "cos:", "mod:", "arcsin:", "arccos:", "arctg:",
    "sqrt:", "abs:", "exp:", "ln:", "lg:",
]
TAGS = [
    "polygon", "polypoint", "ellipse", "circle", "characteristic", "parameter",
    "expression", "dimension", "xcenter", "ycenter", "radius", "closed",
    "blocks", "block",
]
WANT = TOKENS + TAGS

prog = currentProgram
listing = prog.getListing()
fm = prog.getFunctionManager()
refmgr = prog.getReferenceManager()

# 1) collect (literal -> set of addresses where that string lives)
hits = {}  # addr(str) -> (value, set(matched literals))
data_iter = listing.getDefinedData(True)
cnt = 0
for d in data_iter:
    cnt += 1
    try:
        if not d.hasStringValue():
            continue
        v = d.getValue()
        if v is None:
            continue
        s = str(v)
    except:
        continue
    matched = [w for w in WANT if w in s]
    if matched:
        hits[d.getAddress()] = (s, matched)

# 2) map each string addr -> functions referencing it
func_reason = {}  # func entry addr -> (func, set(reasons))
for saddr, (sval, matched) in hits.items():
    refs = refmgr.getReferencesTo(saddr)
    for r in refs:
        fa = r.getFromAddress()
        fn = fm.getFunctionContaining(fa)
        if fn is None:
            continue
        key = fn.getEntryPoint()
        if key not in func_reason:
            func_reason[key] = (fn, set())
        for m in matched:
            func_reason[key][1].add((m, sval[:60]))

# 3) decompile each such function
ifc = DecompInterface()
ifc.openProgram(prog)
mon = ConsoleTaskMonitor()

out = open(OUT_FILE, "w")
out.write("# FastView.exe targeted decompilation\n")
out.write("# defined strings scanned: %d ; matched string locations: %d ; functions: %d\n\n"
          % (cnt, len(hits), len(func_reason)))

# also list the matched strings themselves for reference
out.write("## Matched string literals (addr : literal-tags : value)\n")
for saddr, (sval, matched) in sorted(hits.items(), key=lambda kv: str(kv[0])):
    out.write("%s : %s : %r\n" % (saddr, ",".join(sorted(set(matched))), sval[:120]))
out.write("\n")

def decompile(fn):
    try:
        res = ifc.decompileFunction(fn, 120, mon)
        if res and res.decompileCompleted():
            return res.getDecompiledFunction().getC()
    except Exception as e:
        return "/* decompile error: %s */" % e
    return "/* decompile failed */"

for key, (fn, reasons) in sorted(func_reason.items(), key=lambda kv: str(kv[0])):
    out.write("\n" + "=" * 80 + "\n")
    out.write("FUNC %s @ %s\n" % (fn.getName(), fn.getEntryPoint()))
    out.write("REASONS: %s\n" % ("; ".join(sorted("%s=>%r" % (m, v) for (m, v) in reasons))))
    out.write("=" * 80 + "\n")
    out.write(decompile(fn))
    out.write("\n")

out.close()
print("WROTE %s (%d functions)" % (OUT_FILE, len(func_reason)))
