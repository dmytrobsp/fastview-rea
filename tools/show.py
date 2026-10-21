# Print MFC message maps from msgmaps.json (in cwd) with ids resolved via notes/ui dumps.
# Usage: python3 -I tools/show.py [ClassSubstring...]
import json, re, sys
strs = {}
cur = None
for line in open('/home/poniraq/projects/fastview-rea/notes/ui/strings.txt', encoding='utf-8'):
    m = re.match(r'^(\d+)\t(.*)', line.rstrip('\n'))
    if m:
        cur = int(m.group(1)); strs[cur] = m.group(2)
    elif cur is not None and line.strip():
        strs[cur] += ' | ' + line.strip()
menus = {}
for line in open('/home/poniraq/projects/fastview-rea/notes/ui/menus.txt', encoding='utf-8'):
    m = re.search(r"'(.*)' id=(\d+)", line)
    if m: menus.setdefault(int(m.group(2)), m.group(1))
r = json.load(open('msgmaps.json'))
want = sys.argv[1:]
for mp in r:
    if want and not any(w in ','.join(mp['classes']) for w in want): continue
    print(f"== {','.join(mp['classes'])} getter={mp['getter']} base={mp['base_getter']}")
    for e in mp['entries']:
        i = e['id']; lab = strs.get(i) or menus.get(i) or ''
        rng = f"..{e['last']}" if e['last'] and e['last'] != i else ''
        print(f"  {e['kind']:<22} id={i}{rng:<8} sig={e['sig']:<3} {e['pfn']}  {lab[:90]}")
