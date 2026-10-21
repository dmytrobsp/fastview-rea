# List primary RTTI vtables (address, slot count) via tools/msgmaps.py.
import runpy, sys, re
sys.argv=['/home/poniraq/projects/fastview-rea/tools/msgmaps.py','/home/poniraq/projects/fastview-rea/extracted/stage1/FastView.exe','/dev/null']
g = runpy.run_path('/home/poniraq/projects/fastview-rea/tools/msgmaps.py')
for vt,(c,o) in sorted(g['vtables'].items(), key=lambda x:x[1][0]):
    if o: continue
    n=0
    while True:
        f=g['u32'](vt+n*4)
        if f is None or not g['in_text'](f): break
        n+=1
    print(f"{vt:#x} {g['demangle'](c)} slots={n}")
