import struct, sys
sys.path.insert(0,'/Users/seith/Desktop/tama-paradise-web/docs/spec/tools')
import extract_sprites as E
F=open('/Users/seith/Desktop/tama-paradise-web/dumps/paradise-glacier-20261019-113049.bin','rb').read()
P=E.sprite_pack(F); O=E.sprite_offsets(P)
def hdr(i):
    (dlen, flags, bpp, nspr, sw, sh, ox, oy, iw, ih, pbpp, npal, tidx, poff, doff,_) = struct.unpack_from('<IBBHBBbbBBBBHHHH', P, O[i])
    return dict(w=sw*iw,h=sh*ih,frames=nspr//(iw*ih),ox=ox,oy=oy,npal=npal,bpp=bpp)
for x in sys.argv[1:]:
    i=int(x,0); print(i, hex(i), hdr(i))
