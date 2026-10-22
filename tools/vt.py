#!/usr/bin/env python3
"""List primary RTTI vtables (address, demangled class, slot count) of a
32-bit MSVC PE, via tools/msgmaps.py.

Usage: python3 -I tools/vt.py <FastView.exe>
"""
import os
import runpy
import sys

here = os.path.dirname(os.path.abspath(__file__))
sys.argv = [os.path.join(here, 'msgmaps.py'), sys.argv[1], os.devnull]
g = runpy.run_path(sys.argv[0])
for vt, (c, o) in sorted(g['vtables'].items(), key=lambda x: x[1][0]):
    if o:
        continue
    n = 0
    while True:
        f = g['u32'](vt + n * 4)
        if f is None or not g['in_text'](f):
            break
        n += 1
    print(f"{vt:#x} {g['demangle'](c)} slots={n}")
