#!/usr/bin/env python3
"""Print MFC message maps from msgmaps.json with command ids resolved to the
string table (and menu labels, when a menus.txt dump is present).

Usage: python3 -I tools/show.py [--dumps DIR] [ClassSubstring...]
DIR (default `dumps/`) holds msgmaps.json and strings.txt as written by
tools/regen-dumps.sh, and optionally menus.txt (`'label' id=N` lines).
"""
import json
import os
import re
import sys

args = sys.argv[1:]
dumps = 'dumps'
if args[:1] == ['--dumps']:
    dumps, args = args[1], args[2:]
strs = {}
cur = None
for line in open(os.path.join(dumps, 'strings.txt'), encoding='utf-8'):
    m = re.match(r'^(\d+)\t(.*)', line.rstrip('\n'))
    if m:
        cur = int(m.group(1))
        strs[cur] = m.group(2)
    elif cur is not None and line.strip():
        strs[cur] += ' | ' + line.strip()
menus = {}
menus_path = os.path.join(dumps, 'menus.txt')
if os.path.exists(menus_path):
    for line in open(menus_path, encoding='utf-8'):
        m = re.search(r"'(.*)' id=(\d+)", line)
        if m:
            menus.setdefault(int(m.group(2)), m.group(1))
for mp in json.load(open(os.path.join(dumps, 'msgmaps.json'))):
    if args and not any(w in ','.join(mp['classes']) for w in args):
        continue
    print(f"== {','.join(mp['classes'])} getter={mp['getter']} base={mp['base_getter']}")
    for e in mp['entries']:
        i = e['id']
        lab = strs.get(i) or menus.get(i) or ''
        rng = f"..{e['last']}" if e['last'] and e['last'] != i else ''
        print(f"  {e['kind']:<22} id={i}{rng:<8} sig={e['sig']:<3} {e['pfn']}  {lab[:90]}")
