#!/usr/bin/env python3
"""How much of FastView.exe our notes cover: the program's own functions
(call targets and vtable slots below the library code), which of them the
notes cite (any address inside a function counts), and the uncited ones —
largest first, each labelled with its class (from `index/symbols.tsv`) or,
failing that, the nearest labelled caller.

Prints counts and addresses only (Markdown), no code. Needs the Intel
listing `tools/regen-dumps.sh` makes from your copy.

Usage: python3 -I tools/coverage.py [dumps/fv.asm] [--min N] [--all]
(an objdump listing with or without the raw bytes)
"""
import bisect
import collections
import glob
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
args = [a for i, a in enumerate(sys.argv[1:], 1) if not a.startswith('--') and sys.argv[i - 1] != '--min']
listing = args[0] if args else os.path.join(ROOT, 'dumps', 'fv.asm')
min_size = int(sys.argv[sys.argv.index('--min') + 1]) if '--min' in sys.argv else 150
show_all = '--all' in sys.argv

# The program's own code; past it the statically linked library code.
APP_LO, APP_HI = 0x401000, 0x4a8000

asm = open(listing, encoding='utf-8', errors='replace').read()
ins = [int(x, 16) for x in re.findall(r'^ +([0-9a-f]+):\t(?:[0-9a-f]{2} )*\s*\t?[a-z]', asm, re.M)]
calls = [(int(a, 16), int(t, 16)) for a, t in re.findall(r'^ +([0-9a-f]+):\t(?:[0-9a-f]{2} )*\s*\t?call +0x([0-9a-f]+)\s*$', asm, re.M)]

starts = {t for _, t in calls}
labels = {}
for line in open(os.path.join(ROOT, 'index', 'vtables.tsv'), encoding='utf-8'):
    for x in re.findall(r'0x([0-9a-f]{6})', line.split('\t')[2] if line.count('\t') >= 2 else ''):
        starts.add(int(x, 16))
for line in open(os.path.join(ROOT, 'index', 'symbols.tsv'), encoding='utf-8'):
    f = line.rstrip('\n').split('\t')
    if len(f) >= 3 and f[0].startswith('0x') and f[1] in ('func', 'code'):
        a = int(f[0], 16)
        if '__' in f[2] and f[2].split('__')[0]:
            labels.setdefault(a, f[2].split('__')[0])
        if f[1] == 'func':
            starts.add(a)
S = sorted(a for a in starts if APP_LO <= a < 0x4c0000)


def owner(a):
    k = bisect.bisect_right(S, a) - 1
    return S[k] if k >= 0 else None


def size(i):
    hi = S[i + 1] if i + 1 < len(S) else S[i] + 1
    return bisect.bisect_left(ins, hi) - bisect.bisect_left(ins, S[i])


# The plan lists what is still uncited: it is not a note.
notes = ''.join(open(f, encoding='utf-8').read() for f in glob.glob(os.path.join(ROOT, 'docs', 'original', '**', '*.md'), recursive=True))
refs = re.findall(r'0x(4[0-9a-f]{5})\b', notes) + re.findall(r'FUN_00(4[0-9a-f]{5})\b', notes)
cited = {owner(int(x, 16)) for x in refs if APP_LO <= int(x, 16) < 0x4c0000}
cited.discard(None)

callers = collections.defaultdict(set)
for site, target in calls:
    o = owner(site)
    if o is not None:
        callers[target].add(o)


def label(a):
    seen, frontier = {a}, [a]
    for _ in range(4):
        for f in frontier:
            if f in labels:
                return labels[f]
        nxt = []
        for f in frontier:
            for c in callers.get(f, ()):
                if c not in seen:
                    seen.add(c)
                    nxt.append(c)
        frontier = nxt
    return '?'


app = [(S[i], size(i)) for i in range(len(S)) if S[i] < APP_HI]
tot_ins = sum(n for _, n in app)
cov_ins = sum(n for a, n in app if a in cited)
big = [(a, n) for a, n in app if n >= min_size]
mid = [(a, n) for a, n in app if 50 <= n < min_size]
print(f'functions: {len(app)}, cited {sum(1 for a, _ in app if a in cited)}')
print(f'instructions in cited functions: {100 * cov_ins / tot_ins:.1f}%')
print(f'large (>= {min_size} instructions): {len(big)}, uncited {sum(1 for a, _ in big if a not in cited)}')
print(f'medium (50..{min_size - 1}): {len(mid)}, uncited {sum(1 for a, _ in mid if a not in cited)}')
unc = sorted(((n, a) for a, n in (app if show_all else big) if a not in cited), reverse=True)
by = collections.defaultdict(list)
for n, a in unc:
    by[label(a)].append((a, n))
print()
print('| area (class or nearest labelled caller) | uncited functions (instructions) |')
print('|---|---|')
for k in sorted(by, key=lambda k: -sum(n for _, n in by[k])):
    print(f"| {k} | {', '.join(f'`0x{a:x}` ({n})' for a, n in by[k])} |")
