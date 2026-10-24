#!/usr/bin/env python3
"""Print the geometry of a PE file's dialog templates (RT_DIALOG).

    tools/dlg_templates.py <path/to/FastView.exe> [dialog ids...]

For each template: its style, extended style, position and size in dialog
units, font and caption, then every control's class, id, rectangle (DLU) and
style. Our dialogs are laid out from these numbers: with the templates'
MS Shell Dlg 8 the base units are 6 x 13 px, so x px = MulDiv(x, 6, 4) and
y px = MulDiv(y, 13, 8). The output quotes the original's resource text:
keep it local (never commit it), like the other dumps.
"""
import struct, sys
data = open(sys.argv[1], 'rb').read()
pe = struct.unpack_from('<I', data, 0x3c)[0]
nsec = struct.unpack_from('<H', data, pe + 6)[0]
opt = pe + 24
magic = struct.unpack_from('<H', data, opt)[0]
dd = opt + (96 if magic == 0x10b else 112)
rsrc_rva = struct.unpack_from('<I', data, dd + 2 * 8)[0]
secs = []
so = opt + struct.unpack_from('<H', data, pe + 20)[0]
for i in range(nsec):
    va, vs, raw = struct.unpack_from('<I', data, so + i * 40 + 12)[0], struct.unpack_from('<I', data, so + i * 40 + 8)[0], struct.unpack_from('<I', data, so + i * 40 + 20)[0]
    secs.append((va, vs, raw))
def off(rva):
    for va, vs, raw in secs:
        if va <= rva < va + max(vs, 1):
            return rva - va + raw
base = off(rsrc_rva)
def entries(o):
    n1, n2 = struct.unpack_from('<HH', data, base + o + 12)
    for i in range(n1 + n2):
        name, tgt = struct.unpack_from('<II', data, base + o + 16 + i * 8)
        yield name, tgt
def leaf(t):
    while t & 0x80000000:
        t = next(entries(t & 0x7fffffff))[1]
    rva, size = struct.unpack_from('<II', data, base + t)
    return off(rva), size
want = {int(a) for a in sys.argv[2:]}
for typ, t in entries(0):
    if typ != 5: continue
    for name, t2 in entries(t & 0x7fffffff):
        if want and name not in want: continue
        o, size = leaf(t2)
        d = data[o:o + size]
        def sz(p):
            if struct.unpack_from('<H', d, p)[0] == 0xffff: return p + 4, '#%d' % struct.unpack_from('<H', d, p + 2)[0]
            e = p
            while struct.unpack_from('<H', d, e)[0]: e += 2
            return e + 2, d[p:e].decode('utf-16le')
        ex = struct.unpack_from('<HH', d, 0) == (1, 0xffff)
        if ex:
            _, _, hid, exst, style, n, x, y, cx, cy = struct.unpack_from('<HHIIIHhhhh', d, 0); p = 26
        else:
            style, exst, n, x, y, cx, cy = struct.unpack_from('<IIHhhhh', d, 0); p = 18
        p, menu = sz(p); p, cls = sz(p); p, title = sz(p)
        font = ''
        if style & 0x40:
            pt = struct.unpack_from('<H', d, p)[0]; p += 2
            if ex: p += 4
            p, face = sz(p); font = '%s %d' % (face, pt)
        print('DIALOG %d style=%08x ex=%08x at %d,%d size %dx%d font %s title %r' % (name, style, exst, x, y, cx, cy, font, title))
        for i in range(n):
            p = (p + 3) & ~3
            if ex:
                _, cexst, cstyle, cx_, cy_, cw, ch, cid = struct.unpack_from('<IIIhhhhI', d, p); p += 24
            else:
                cstyle, cexst, cx_, cy_, cw, ch, cid = struct.unpack_from('<IIhhhhH', d, p); p += 18
            p, ccls = sz(p); p, ctext = sz(p)
            extra = struct.unpack_from('<H', d, p)[0]; p += 2 + extra
            print('  %-16s id=%-6d %d,%d %dx%d style=%08x %r' % (ccls, cid, cx_, cy_, cw, ch, cstyle, ctext))
