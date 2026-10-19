import struct, sys
F = open('/Users/seith/Desktop/tama-paradise-web/dumps/paradise-glacier-20261019-113049.bin','rb').read()
DP = 0x111040
offs = struct.unpack_from('<23I', F, DP)
def table(i): return DP + offs[i]*2
T13, T14 = table(13), table(14)
N13 = (offs[14]-offs[13])  # halfwords
OPN = {1:'SPR',2:'FRM',3:'PAL',4:'TX',5:'TY',6:'TXY',7:'T7',8:'T8',9:'T9',10:'T10',11:'T11',12:'T12',13:'D',14:'SND',15:'OBJ',16:'CHILD',17:'FLIP',18:'L12',19:'L13',20:'O14',22:'O16',23:'O17',25:'BG',26:'EVT',27:'LOOP',28:'CHARA',29:'MOTION',30:'O1E',32:'KILL',36:'O24'}
def s16(v): return v-0x10000 if v>=0x8000 else v
def anim(i):
    a = struct.unpack_from('<H', F, T13+2*i)[0]; b = struct.unpack_from('<H', F, T13+2*i+2)[0]
    base = T14 + a*4; cnt = (b-a)*2
    w = struct.unpack_from(f'<{cnt}H', F, base)
    out = [f'hdr={w[0]:#x}']
    j = 1
    while j < cnt:
        op = w[j]; j += 1
        if op == 0xfcbd: out.append('END'); break
        args = {}
        for bit, nm in ((15,'a'),(14,'z'),(13,'p'),(11,'q'),(10,'r'),(9,'wait')):
            if op >> bit & 1: args[nm] = s16(w[j]); j += 1
        if op & 0x1000: args['?b12'] = 1
        out.append(f"{OPN.get(op&0xff, hex(op&0xff))}" + ''.join(f' {k}={v:#x}' if k=='a' else f' {k}={v}' for k,v in args.items()))
    return out
if __name__ == '__main__':
    for x in sys.argv[1:]:
        i = int(x, 0); print(f'== anim {i:#x}'); print('  ' + '\n  '.join(anim(i)))
