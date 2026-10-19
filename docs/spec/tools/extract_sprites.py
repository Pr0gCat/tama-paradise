#!/usr/bin/env python3
"""Decode Tamagotchi Paradise sprites straight from a raw 16 MiB flash dump (stdlib only).

usage: python3 -I extract_sprites.py DUMP.bin OUTDIR [first [last]]
       python3 -I extract_sprites.py DUMP.bin --check SPRITES.zip   # compare with Paramix export

Layout (verified on the White Glacier dump):
  flash 0x111000  ARC2 root  -> [0] data pack, [1] sprite pack, [2] strings ARC2 (9 tables)
  sprite pack = u32 offset table (count = first_offset/4) + sprite records.
One PNG per sprite record: all subimages (frames) left-to-right, wrapped at 1280 px, palette set 0.
"""
import struct, sys, zlib, zipfile, io

ROOT = 0x111000


def arc2(buf, base):
    magic, cksum, length, n = struct.unpack_from('<4I', buf, base)
    assert magic == 0x32435241, hex(magic)
    assert sum(buf[base + 8:base + 16 + length]) & 0xffffffff == cksum, 'ARC2 checksum'
    return [struct.unpack_from('<4I', buf, base + 16 + 16 * i) for i in range(n)]  # flags, off, clen, ulen


def sprite_pack(buf):
    _, off, _, ln = arc2(buf, ROOT)[1]
    return buf[ROOT + off:ROOT + off + ln]


def sprite_offsets(pack):
    n = struct.unpack_from('<I', pack, 0)[0] // 4
    return list(struct.unpack_from(f'<{n}I', pack, 0))


def rle(src, word):
    """Bytewise (word=False) or wordwise (word=True) RLE per GMMan sprites.md."""
    out, i, step = bytearray(), 0, 4 if word else 1
    while i + step <= len(src):
        c = int.from_bytes(src[i:i + step], 'little'); i += step
        if c == 0:
            break
        if c >> (31 if word else 7):
            n = (c & 0x0fffffff if word else c & 0x7f) * step
            out += src[i:i + n]; i += n
        else:
            out += src[i:i + step] * c; i += step
    return bytes(out)


def decode(rec):
    """-> (width, height_total, rgba bytes, header dict)."""
    (dlen, flags, bpp, nspr, sw, sh, ox, oy, iw, ih, pbpp, npal, tidx, poff, doff,
     _) = struct.unpack_from('<IBBHBBbbBBBBHHHH', rec, 0)
    hdr = dict(len=dlen, flags=flags, bpp=bpp, nspr=nspr, sw=sw, sh=sh, ox=ox, oy=oy, iw=iw, ih=ih,
               npal=npal, tidx=tidx, poff=poff, doff=doff)
    bits = {0: 1, 1: 2, 2: 4, 3: 8, 0x10: 16, 0x11: 16, 0x12: 24}[bpp]
    per = sw * sh * bits // 8 if bits < 24 else sw * sh * 2  # ponytail: 0x12 (RGB888) never seen in this dump
    # per-sprite raw pixel bytes
    sprites = []
    if flags & 0x60:
        for k in range(nspr):
            o, ln = struct.unpack_from('<II', rec, doff + 8 * k)
            d = rec[doff + (o & 0x7fffffff):doff + (o & 0x7fffffff) + ln]
            if flags & 0x80:
                d = bytes(b ^ 0x53 for b in d)
            sprites.append(d if o & 0x80000000 else rle(d, bool(flags & 0x40)))
    else:
        for k in range(nspr):
            d = rec[doff + k * per:doff + (k + 1) * per]
            sprites.append(bytes(b ^ 0x53 for b in d) if flags & 0x80 else d)
    if bits <= 8:
        pal = struct.unpack_from(f'<{1 << bits}H', rec, poff)  # palette set 0
    trans = flags & 4

    def px(d, i):
        if bits == 16:
            c = struct.unpack_from('<H', d, 2 * i)[0]
            a = 0 if trans and c == tidx else 255
        else:
            bi = i * bits
            idx = (d[bi >> 3] >> (bi & 7)) & ((1 << bits) - 1) if (bi >> 3) < len(d) else 0
            c = pal[idx]; a = 0 if trans and idx == tidx else 255
        r, g, b = c >> 11, (c >> 5) & 63, c & 31
        return bytes((r * 255 // 31, g * 255 // 63, b * 255 // 31, a))  # same rounding as Paramix

    W, H = sw * iw, sh * ih
    nsub = nspr // (iw * ih)
    cols = min(nsub, max(1, 1280 // W))  # frames left-to-right, wrapped at 1280 px (as Paramix does)
    rows = -(-nsub // cols)
    img = bytearray(W * cols * H * rows * 4)
    for k, d in enumerate(sprites):
        sub, cell = divmod(k, iw * ih)
        cy, cx = divmod(cell, iw)
        for y in range(sh):
            row = b''.join(px(d, y * sw + x) for x in range(sw))
            fr, fc = divmod(sub, cols)
            p = ((fr * H + cy * sh + y) * W * cols + fc * W + cx * sw) * 4
            img[p:p + len(row)] = row
    return W * cols, H * rows, bytes(img), hdr


def png(w, h, rgba):
    raw = b''.join(b'\0' + rgba[y * w * 4:(y + 1) * w * 4] for y in range(h))
    ch = lambda t, d: struct.pack('>I', len(d)) + t + d + struct.pack('>I', zlib.crc32(t + d))
    return (b'\x89PNG\r\n\x1a\n' + ch(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 6, 0, 0, 0))
            + ch(b'IDAT', zlib.compress(raw)) + ch(b'IEND', b''))


def unpng(data):
    """Minimal PNG reader for Paramix exports -> (w, h, rgba). Handles 8-bit RGB/RGBA/palette, no interlace."""
    pos, idat, plte, trns = 8, b'', None, b''
    while pos < len(data):
        ln, = struct.unpack_from('>I', data, pos); t = data[pos + 4:pos + 8]; d = data[pos + 8:pos + 8 + ln]
        if t == b'IHDR': w, h, depth, ct = struct.unpack('>IIBB', d[:10])
        elif t == b'IDAT': idat += d
        elif t == b'PLTE': plte = d
        elif t == b'tRNS': trns = d
        pos += 12 + ln
    bpp = {2: 3, 6: 4, 3: 1, 0: 1, 4: 2}[ct]
    assert depth == 8, depth
    raw, prev, out, i = zlib.decompress(idat), bytearray(w * bpp), bytearray(), 0
    for _ in range(h):
        f = raw[i]; line = bytearray(raw[i + 1:i + 1 + w * bpp]); i += 1 + w * bpp
        for x in range(len(line)):
            a = line[x - bpp] if x >= bpp else 0; b = prev[x]; c = prev[x - bpp] if x >= bpp else 0
            if f == 1: line[x] = (line[x] + a) & 255
            elif f == 2: line[x] = (line[x] + b) & 255
            elif f == 3: line[x] = (line[x] + (a + b) // 2) & 255
            elif f == 4:
                p = a + b - c; pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[x] = (line[x] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        prev = line
        for x in range(w):
            if ct == 6: out += line[x * 4:x * 4 + 4]
            elif ct == 2: out += line[x * 3:x * 3 + 3] + b'\xff'
            elif ct == 3:
                k = line[x]; out += plte[k * 3:k * 3 + 3] + bytes([trns[k] if k < len(trns) else 255])
            elif ct == 0: out += bytes([line[x]] * 3) + b'\xff'
            else: out += bytes([line[x * 2]] * 3) + bytes([line[x * 2 + 1]])
    return w, h, bytes(out)


def same(a, b):
    """Pixel-equal, treating all fully transparent pixels as equal."""
    return len(a) == len(b) and all(
        a[i + 3] == b[i + 3] == 0 or a[i:i + 4] == b[i:i + 4] for i in range(0, len(a), 4))


def main():
    buf = open(sys.argv[1], 'rb').read()
    pack = sprite_pack(buf)
    offs = sprite_offsets(pack) + [len(pack)]
    if sys.argv[2] == '--check':
        z = zipfile.ZipFile(sys.argv[3])
        ids = [int(a) for a in sys.argv[4:]] or range(0, len(offs) - 1, 37)
        ok = 0
        for i in ids:
            w, h, rgba, hd = decode(pack[offs[i]:offs[i + 1]])
            ew, eh, erg = unpng(z.read(f'sprite_{i:04d}.png'))
            good = (w, h) == (ew, eh) and same(rgba, erg)
            ok += good
            print(f'{i:4d} {w}x{h} vs {ew}x{eh} {"MATCH" if good else "DIFF"} {hd}')
        print(f'{ok}/{len(ids)} match')
        sys.exit(ok != len(ids))
    out = sys.argv[2]
    lo = int(sys.argv[3]) if len(sys.argv) > 3 else 0
    hi = int(sys.argv[4]) if len(sys.argv) > 4 else len(offs) - 2
    import os; os.makedirs(out, exist_ok=True)
    for i in range(lo, hi + 1):
        w, h, rgba, _ = decode(pack[offs[i]:offs[i + 1]])
        open(f'{out}/sprite_{i:04d}.png', 'wb').write(png(w, h, rgba))


if __name__ == '__main__':
    main()
