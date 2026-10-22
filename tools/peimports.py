#!/usr/bin/env python3
"""List a 32-bit PE's imports by IAT slot: `0xADDR DLL name-or-#ordinal`.
Pure stdlib.

Usage: python3 -I tools/peimports.py <file.exe>
"""
import struct
import sys

f = open(sys.argv[1], 'rb').read()
pe = struct.unpack_from('<I', f, 0x3c)[0]
nsec = struct.unpack_from('<H', f, pe + 6)[0]
opt = pe + 24
osz = struct.unpack_from('<H', f, pe + 20)[0]
image_base = struct.unpack_from('<I', f, opt + 28)[0]
secs = []
for i in range(nsec):
    o = opt + osz + 40 * i
    vs, va, rs, ro = struct.unpack_from('<IIII', f, o + 8)
    secs.append((va, ro, max(vs, rs)))


def off(rva):
    for va, ro, s in secs:
        if va <= rva < va + s:
            return ro + rva - va
    raise ValueError(hex(rva))


out = {}
d = off(struct.unpack_from('<I', f, opt + 96 + 8)[0])
while True:
    oft, _ts, _fc, name, ft = struct.unpack_from('<IIIII', f, d)
    if name == 0:
        break
    dll = f[off(name):].split(b'\0')[0].decode()
    t = off(oft or ft)
    i = 0
    while True:
        e = struct.unpack_from('<I', f, t + 4 * i)[0]
        if e == 0:
            break
        nm = '#%d' % (e & 0xffff) if e & 0x80000000 else f[off(e) + 2:].split(b'\0')[0].decode()
        out[image_base + ft + 4 * i] = (dll, nm)
        i += 1
    d += 20
for k, (dll, nm) in sorted(out.items()):
    print(hex(k), dll, nm)
