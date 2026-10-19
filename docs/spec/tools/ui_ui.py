import struct, sys
sys.path.insert(0, __import__("os").path.dirname(__file__))
from anim import F, table, offs
T4, T5 = table(4), table(5)
N4 = (offs[5]-offs[4])
def u(a): return struct.unpack_from('<H', F, a)[0]
def s(v): return v-0x10000 if v>=0x8000 else v
# field order per type, from FUN_10023124: list of (bitmask, slot, default)
SPEC = {
 0:[(0x800,0,0)],
 1:[(0x1,3,0),(0x8,4,0),(0x10,5,0x1000),(0x20,6,0x40),(0x40,7,0),(0x80,8,0),(0x100,9,0),(0x400,10,0),(0x800,0,0)],
 2:[(0x1,3,0),(0x8,4,0),(0x10,5,0),(0x40,6,0x40),(0x80,7,0),(0x100,8,0),(0x400,9,0),(0x800,0,0)],
 3:[(0x8,3,1),(0x10,4,1),(0x20,5,0),(0x40,6,0),(0x100,7,0),(0x400,8,0),(0x800,0,0)],
 4:[(0x1,3,0),(0x8,4,0),(0x10,5,0),(0x20,6,0),(0x100,7,0),(0x200,8,0),(0x800,0,0)],
 5:[(0x1,3,0),(0x100,4,0),(0x200,5,0),(0x800,0,0)],
 6:[(0x1,3,0)],
 7:[(0x1,8,0),(0x8,3,0),(0x10,4,0),(0x20,5,0),(0x40,7,0),(0x80,6,0),(0x800,0,0)],
 8:[(0x1,0,0),(0x8,4,0),(0x10,5,0),(0x40,6,0x40),(0x80,7,0),(0x100,8,0),(0x400,9,0),(0x800,3,0)],
}
def elem(i):
    a = T5 + u(T4+2*i)*2
    h = u(a); t = h >> 12
    p = [0]*11; p[1] = s(u(a+2)); p[2] = s(u(a+4)); q = a+6
    if t == 0:
        p[0] = u(a+6) if h & 0x800 else 0
        return t, h, p
    for m, slot, d in SPEC.get(t, []):
        if h & m: p[slot] = s(u(q)); q += 2
        else: p[slot] = d
    if t == 1 and h & 0x8000: t = 9
    return t, h, p
NAMES={0:'GROUP?',1:'SPRITE',2:'TEXT',3:'T3',4:'T4',5:'ANIM',6:'T6',7:'T7',8:'T8',9:'SPRITE9'}
if __name__=='__main__':
    for x in sys.argv[1:]:
        i=int(x,0); t,h,p=elem(i); print(f'elem {i:#x}: {NAMES[t]} hdr={h:#06x} x={p[1]} y={p[2]} p={[hex(v&0xffff) for v in p]}')
