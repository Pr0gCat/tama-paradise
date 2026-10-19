#!/usr/bin/env python3
"""Dump the 9 built-in string tables of a Tamagotchi Paradise flash dump to CSV (stdlib only).

usage: python3 -I extract_strings.py DUMP.bin cjk_charset.txt OUT.csv
  cjk_charset.txt: https://raw.githubusercontent.com/GMMan/tama-para-research/master/formats/cjk_charset.txt

Glyph code -> text:
  1..0x1e0   : char code stored in font 1 glyph table (data pack table 1, font #1, u16 per glyph;
               Shift-JIS for Japanese/full-width, ASCII for Latin). Accented Latin glyphs only carry
               their base letter there; ACCENTED (read off the font sprite's pixels) gives the letter.
  >= 0x1e1   : cjk_charset[code - 0x1e1] (2350 Hangul, Zhuyin, circled ideographs, ~6800 Hanzi)
  0xf000     : newline, 0xf001: page break, other 0xf0xx: control/placeholder -> {Fxxx}
"""
import struct, sys, csv

ROOT, LANGS = 0x111000, ['ja', 'en', 'fr', 'de', 'pt', 'es', 'it', 'zh', 'ko']


def entries(buf, base):
    n = struct.unpack_from('<I', buf, base + 12)[0]
    return [struct.unpack_from('<4I', buf, base + 16 + 16 * i) for i in range(n)]


def glyph_map(buf):
    dp = ROOT + entries(buf, ROOT)[0][1]                       # data pack
    offs = struct.unpack_from('<23I', buf, dp)                 # 23 tables, halfword offsets
    t0, t1 = dp + offs[0] * 2, dp + offs[1] * 2
    font1 = t1 + struct.unpack_from('<H', buf, t0 + 2)[0] * 2  # font index -> font #1
    count = struct.unpack_from('<h', buf, font1 + 6)[0]        # glyph 0 slot holds count-1
    m = {}
    for c in range(1, count + 1):
        v = struct.unpack_from('<H', buf, font1 + 4 + 4 * c)[0]
        try:
            s = chr(v) if v < 0x80 else bytes([v >> 8, v & 255]).decode('cp932')
        except UnicodeDecodeError:
            s = '{%04X}' % c
        m[c] = s
    return m


def tables(buf):
    sb = ROOT + entries(buf, ROOT)[2][1]                       # strings ARC2
    for _, off, _, ln in entries(buf, sb):
        h = struct.unpack_from(f'<{ln // 2}H', buf, sb + off)
        n = h[0]                                               # string count
        yield [h[h[i + 1]:h[i + 2]] for i in range(n)]         # h[1..n+1] = offsets (+ end)


# Accented capitals and signs of font sprite 17 (glyph code -> character), read off the pixels.
ACCENTED = dict(zip(range(400, 430), 'ÀÁÂÃÄÆÇÉÈÊËÎÏÓÒÔÕÖŒÙÚÛÜŸÍÌÑ¿¡º'))


def decode(s, gm, cjk):
    out = []
    for c in s:
        if c == 0: break
        if c == 0xf000: out.append('\n')
        elif c == 0xf001: out.append('\f')
        elif c >= 0xf000: out.append('{F%03X}' % (c & 0xfff))
        elif c >= 0x1e1: out.append(cjk[c - 0x1e1] if c - 0x1e1 < len(cjk) else '{%04X}' % c)
        else:
            out.append(ACCENTED.get(c) or gm.get(c, '{%04X}' % c))
    return ''.join(out)


def main():
    buf = open(sys.argv[1], 'rb').read()
    cjk = open(sys.argv[2], encoding='utf-8').read()
    gm = glyph_map(buf)
    tbls = list(tables(buf))
    assert len(tbls) == 9 and all(len(t) == len(tbls[0]) for t in tbls)
    # self-check: language names (0x58 FRANÇAIS, 0x5b ESPAÑOL, 0x5a PORTUGUÊS), 정보/信息 in ko/zh
    assert [decode(tbls[1][i], gm, cjk) for i in (0x58, 0x5a, 0x5b)] == ['FRANÇAIS', 'PORTUGUÊS', 'ESPAÑOL']
    assert decode(tbls[8][0], gm, cjk) == '정보' and decode(tbls[7][0], gm, cjk) == '信息'
    with open(sys.argv[3], 'w', newline='', encoding='utf-8') as f:
        w = csv.writer(f)
        w.writerow(['id'] + LANGS)
        for i in range(len(tbls[0])):
            w.writerow([i] + [decode(t[i], gm, cjk) for t in tbls])
    print(f'{len(tbls[0])} strings x 9 languages -> {sys.argv[3]}')


if __name__ == '__main__':
    main()
