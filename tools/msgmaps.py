#!/usr/bin/env python3
"""Inventory MFC message maps of a 32-bit PE: RTTI classes -> vtables ->
GetMessageMap -> AFX_MSGMAP_ENTRY tables. Pure stdlib."""
import struct, sys, json, re

path = sys.argv[1]
data = open(path, 'rb').read()
pe = struct.unpack_from('<I', data, 0x3c)[0]
nsec = struct.unpack_from('<H', data, pe + 6)[0]
optsz = struct.unpack_from('<H', data, pe + 20)[0]
opt = pe + 24
base = struct.unpack_from('<I', data, opt + 28)[0]
secs = []
for i in range(nsec):
    o = opt + optsz + i * 40
    name = data[o:o + 8].rstrip(b'\0').decode()
    vsz, va, rsz, rptr = struct.unpack_from('<IIII', data, o + 8)
    secs.append((name, base + va, vsz, rptr, rsz))

def off(a):
    for n, va, vsz, rp, rs in secs:
        if va <= a < va + max(vsz, rs):
            r = a - va
            return rp + r if r < rs else None
    return None

def sec_of(a):
    for n, va, vsz, rp, rs in secs:
        if va <= a < va + max(vsz, rs):
            return n
    return None

def u32(a):
    o = off(a)
    return None if o is None else struct.unpack_from('<I', data, o)[0]

text = [s for s in secs if s[0] == '.text'][0]
def in_text(a):
    return text[1] <= a < text[1] + text[2]

# --- RTTI: type descriptors ".?AV...@@"
tds = {}
for m in re.finditer(rb'\.\?A[VU][^\0]{1,200}?@@\0', data):
    fo = m.start() - 8  # TypeDescriptor: pVFTable, spare, name
    for n, va, vsz, rp, rs in secs:
        if rp <= fo < rp + rs:
            tds[va + fo - rp] = m.group()[:-1].decode('latin1')
# --- COLs: {sig=0, offset, cdOffset, pTD, pCHD}
cols = {}
for n, va, vsz, rp, rs in secs:
    if n not in ('.rdata', '.data'):
        continue
    for fo in range(rp, rp + rs - 20, 4):
        sig, offs, cd, ptd, pchd = struct.unpack_from('<IIIII', data, fo)
        if sig == 0 and ptd in tds and offs < 0x1000:
            cols[va + fo - rp] = (tds[ptd], offs)
# --- vtables: dword == COL address, followed by text pointers
vtables = {}
for n, va, vsz, rp, rs in secs:
    if n != '.rdata':
        continue
    for fo in range(rp, rp + rs - 8, 4):
        v = struct.unpack_from('<I', data, fo)[0]
        if v in cols:
            vt = va + fo - rp + 4
            if in_text(u32(vt) or 0):
                vtables[vt] = cols[v]

# --- message-map getters: "mov eax, imm32; ret" where imm32 -> {fn, entries}
WM = {0x1: 'WM_CREATE', 0x2: 'WM_DESTROY', 0x3: 'WM_MOVE', 0x5: 'WM_SIZE', 0x6: 'WM_ACTIVATE',
      0x7: 'WM_SETFOCUS', 0x8: 'WM_KILLFOCUS', 0xf: 'WM_PAINT', 0x10: 'WM_CLOSE', 0x14: 'WM_ERASEBKGND',
      0x18: 'WM_SHOWWINDOW', 0x20: 'WM_SETCURSOR', 0x21: 'WM_MOUSEACTIVATE', 0x24: 'WM_GETMINMAXINFO',
      0x2b: 'WM_DRAWITEM', 0x2c: 'WM_MEASUREITEM', 0x4e: 'WM_NOTIFY', 0x7b: 'WM_CONTEXTMENU',
      0x83: 'WM_NCCALCSIZE', 0x84: 'WM_NCHITTEST', 0x87: 'WM_GETDLGCODE', 0x100: 'WM_KEYDOWN',
      0x101: 'WM_KEYUP', 0x102: 'WM_CHAR', 0x110: 'WM_INITDIALOG', 0x111: 'WM_COMMAND',
      0x113: 'WM_TIMER', 0x114: 'WM_HSCROLL', 0x115: 'WM_VSCROLL', 0x116: 'WM_INITMENU',
      0x117: 'WM_INITMENUPOPUP', 0x133: 'WM_CTLCOLOREDIT', 0x134: 'WM_CTLCOLORLISTBOX',
      0x135: 'WM_CTLCOLORBTN', 0x136: 'WM_CTLCOLORDLG', 0x138: 'WM_CTLCOLORSTATIC',
      0x200: 'WM_MOUSEMOVE', 0x201: 'WM_LBUTTONDOWN', 0x202: 'WM_LBUTTONUP', 0x203: 'WM_LBUTTONDBLCLK',
      0x204: 'WM_RBUTTONDOWN', 0x205: 'WM_RBUTTONUP', 0x206: 'WM_RBUTTONDBLCLK', 0x207: 'WM_MBUTTONDOWN',
      0x208: 'WM_MBUTTONUP', 0x20a: 'WM_MOUSEWHEEL', 0x215: 'WM_CAPTURECHANGED', 0x2a3: 'WM_MOUSELEAVE',
      0x233: 'WM_DROPFILES', 0x231: 'WM_ENTERSIZEMOVE', 0x232: 'WM_EXITSIZEMOVE', 0x47: 'WM_WINDOWPOSCHANGED',
      0x46: 'WM_WINDOWPOSCHANGING', 0x1c: 'WM_ACTIVATEAPP', 0x86: 'WM_NCACTIVATE', 0x11f: 'WM_MENUSELECT',
      0x2b1: 'WM_MOUSEHWHEEL', 0x363: 'WM_IDLEUPDATECMDUI', 0x36a: 'WM_KICKIDLE', 0x210: 'WM_PARENTNOTIFY',
      0x7e: 'WM_DISPLAYCHANGE', 0x1a: 'WM_SETTINGCHANGE', 0x15: 'WM_SYSCOLORCHANGE', 0x112: 'WM_SYSCOMMAND',
      0x104: 'WM_SYSKEYDOWN', 0x105: 'WM_SYSKEYUP', 0x2a1: 'WM_MOUSEHOVER', 0x0c: 'WM_SETTEXT',
      0x30: 'WM_SETFONT', 0x31: 'WM_GETFONT', 0x4a: 'WM_COPYDATA', 0x53: 'WM_HELP', 0x2e: 'WM_VKEYTOITEM',
      0x11: 'WM_QUERYENDSESSION', 0x16: 'WM_ENDSESSION'}
NOTIFY = {}

def read_entries(ea):
    out = []
    for i in range(400):
        e = [u32(ea + i * 24 + k * 4) for k in range(6)]
        if None in e:
            return None
        msg, code, idv, last, sig, pfn = e
        if msg == 0 and code == 0 and idv == 0 and last == 0 and pfn == 0:
            return out
        if pfn and not in_text(pfn):
            return None
        out.append(dict(msg=msg, code=code, id=idv, last=last, sig=sig, pfn=pfn))
    return None

getters = {}
to = text[3]
for fo in range(to, to + text[4] - 6):
    if data[fo] == 0xB8 and data[fo + 5] == 0xC3:
        imm = struct.unpack_from('<I', data, fo + 1)[0]
        if sec_of(imm) not in ('.rdata', '.data'):
            continue
        gb, ents = u32(imm), u32(imm + 4)
        if gb is None or ents is None or not in_text(gb) or sec_of(ents) not in ('.rdata', '.data'):
            continue
        es = read_entries(ents)
        if es is None:
            continue
        getters[text[1] + fo - to] = dict(map=imm, base_getter=gb, entries=es)

# class for each getter via vtables containing it
cls_of = {}
for vt, (cname, offs) in vtables.items():
    for slot in range(0, 120):
        f = u32(vt + slot * 4)
        if f is None or not in_text(f):
            break
        if f in getters and offs == 0:
            cls_of.setdefault(f, set()).add(cname)

def demangle(n):
    m = re.match(r'\.\?A[VU](.*)@@', n)
    return m.group(1) if m else n

result = []
for g, info in sorted(getters.items()):
    names = sorted(demangle(c) for c in cls_of.get(g, []))
    ents = []
    for e in info['entries']:
        m = e['msg']
        if m == 0x111:
            kind = 'COMMAND' if e['code'] == 0 else ('CMDUI' if e['code'] == 0xFFFFFFFF else f"CMD code={e['code']:#x}")
        elif m == 0x4e:
            kind = f"NOTIFY code={e['code'] & 0xffffffff:#x}"
        elif m == 0xC000 or m >= 0xC000:
            kind = f"REGMSG@{m:#x}"
        else:
            kind = WM.get(m, f"msg={m:#x}")
        ents.append(dict(kind=kind, id=e['id'], last=e['last'], sig=e['sig'], pfn=f"{e['pfn']:#x}"))
    result.append(dict(getter=f"{g:#x}", map=f"{info['map']:#x}", base_getter=f"{info['base_getter']:#x}",
                       classes=names, n=len(ents), entries=ents))
json.dump(result, open(sys.argv[2], 'w'), indent=1, ensure_ascii=False)
print('classes(TD):', len(tds), 'COLs:', len(cols), 'vtables:', len(vtables), 'msgmaps:', len(result))
for r in result:
    if r['n']:
        print(r['getter'], r['map'], ','.join(r['classes']) or '?', r['n'])
