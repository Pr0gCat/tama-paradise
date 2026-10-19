import sys, re
sys.path.insert(0, __import__("os").path.dirname(__file__))
from anim import anim
def tree(i, d=0, seen=None):
    seen = seen if seen is not None else set()
    print('  '*d + f'[anim {i:#x}]')
    if i in seen: return
    seen.add(i)
    for l in anim(i):
        print('  '*d + '  ' + l)
        m = re.match(r'CHILD a=(0x[0-9a-f]+)', l)
        if m: tree(int(m.group(1),16), d+2, seen)
for x in sys.argv[1:]: tree(int(x,0))
