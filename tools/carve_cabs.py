#!/usr/bin/env python3
"""Carve Microsoft cabinets (`MSCF`) out of a file, e.g. the WiX-attached
payload of `vcredist_vs13_x86.exe` that 7-Zip does not list. Writes
cab0.cab, cab1.cab, … into OUT_DIR; extract them with `7z x`.

Usage: python3 -I tools/carve_cabs.py <file> <out_dir>
"""
import os
import struct
import sys

data = open(sys.argv[1], 'rb').read()
out = sys.argv[2]
os.makedirs(out, exist_ok=True)
i = n = 0
while True:
    i = data.find(b'MSCF\0\0\0\0', i)
    if i < 0:
        break
    size = struct.unpack_from('<I', data, i + 8)[0]
    if 0 < size <= len(data) - i:
        open(os.path.join(out, f'cab{n}.cab'), 'wb').write(data[i:i + size])
        print(n, hex(i), size)
        n += 1
        i += size
    else:
        i += 4
