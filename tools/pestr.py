#!/usr/bin/env python3
"""Dump the RT_STRING table of a PE file as `id<TAB>text` lines (a string's
own newlines become continuation lines). Pure stdlib.

Usage: python3 -I tools/pestr.py <file.exe|file.dll>
"""
import struct
import sys

data = open(sys.argv[1], 'rb').read()
pe = struct.unpack_from('<I', data, 0x3c)[0]
nsec = struct.unpack_from('<H', data, pe + 6)[0]
optsz = struct.unpack_from('<H', data, pe + 20)[0]
magic = struct.unpack_from('<H', data, pe + 24)[0]
dirs = pe + 24 + (96 if magic == 0x10b else 112)
secs = []
for i in range(nsec):
    o = pe + 24 + optsz + 40 * i
    vs, va, rs, ra = struct.unpack_from('<IIII', data, o + 8)
    secs.append((va, max(vs, rs), ra))


def off(rva):
    for va, size, ra in secs:
        if va <= rva < va + size:
            return ra + rva - va
    raise ValueError(hex(rva))


base = off(struct.unpack_from('<I', data, dirs + 2 * 8)[0])


def entries(d):
    named, ids = struct.unpack_from('<HH', data, d + 12)
    for k in range(named + ids):
        yield struct.unpack_from('<II', data, d + 16 + 8 * k)


for tid, p in entries(base):
    if tid != 6:
        continue
    for bid, p2 in entries(base + (p & 0x7fffffff)):
        for _lang, p3 in entries(base + (p2 & 0x7fffffff)):
            rva, _size = struct.unpack_from('<II', data, base + p3)
            o = off(rva)
            for idx in range(16):
                n = struct.unpack_from('<H', data, o)[0]
                o += 2
                if n:
                    print(f"{(bid - 1) * 16 + idx}\t{data[o:o + 2 * n].decode('utf-16le')}")
                o += 2 * n
