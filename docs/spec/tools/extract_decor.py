#!/usr/bin/env python3
"""Extract outside-toy / planet-decoration / travel tables (docs/spec/10-decor-planet.md) to JSON.

usage: python3 -I extract_decor.py DUMP.bin [PRAM.bin] > docs/spec/data/decor.json
  DUMP.bin = full 16 MiB flash (XIP = flash 0x11000, data pack = flash 0x111040).
  PRAM.bin = decrypted PRAM (optional; only needed for the travel souvenir table, which lives
             in the RW-data image at PRAM 0xe6a0 -> SRAM 0x18000000; without it the souvenirs
             are taken from the equip-set table, which lists the same ids in the same order).
"""
import json, struct, sys

F = open(sys.argv[1], 'rb').read()
XIP = 0x11000                       # vaddr 0x10000000 -> flash offset
DP = 0x111040                       # data pack
OFFS = struct.unpack_from('<23I', F, DP)
T = lambda i: DP + OFFS[i] * 2
u16 = lambda a: struct.unpack_from('<H', F, a)[0]
s16 = lambda a: struct.unpack_from('<h', F, a)[0]
x16 = lambda va, n: list(struct.unpack_from(f'<{n}H', F, va - 0x10000000 + XIP))
xs16 = lambda va, n: list(struct.unpack_from(f'<{n}h', F, va - 0x10000000 + XIP))
xb = lambda va, n: list(F[va - 0x10000000 + XIP:va - 0x10000000 + XIP + n])
FIELDS = ['TROPICAL', 'FROZEN', 'MORI']


def elem(i):
    """UI element (data tables 4/5): type, x, y, and the sprite/anim id when present (p3)."""
    a = T(5) + u16(T(4) + 2 * i) * 2
    h = u16(a)
    t = h >> 12
    e = {'elem': i, 'type': {1: 'sprite', 2: 'text', 5: 'anim'}.get(t, t), 'x': s16(a + 2), 'y': s16(a + 4)}
    if t in (1, 5) and h & 1:
        e['sprite' if t == 1 else 'anim'] = u16(a + 6)
    return e


def items():
    """Item table rows: (id, name_txt, spr4, spr6, anim8, animA, animC, animE, flags14, type)."""
    a, out = 0x125972, []           # flash offset of bank-0 item table, 343 x 46 B
    while a < 0x129714:
        w = struct.unpack_from('<23H', F, a)
        out.append(w); a += 46
    return out


def kf(idx):
    """Character-model keyframe (tables 15/16) -> (kind, sprite, x, y, frame)."""
    lo, hi = struct.unpack_from('<HH', F, T(16) + idx * 4)
    p = T(15) + ((hi << 16) | lo) * 2
    fl = u16(p); o = [0] * 11; o[1] = s16(p + 2); o[2] = s16(p + 4); q = p + 6
    for bit, slot in ((2, 6), (3, 7), (4, 4), (5, 3), (6, 5), (7, 8), (8, 9), (9, 10), (10, 0)):
        if fl >> bit & 1:
            o[slot] = s16(q); q += 2
    return o


def model_parts(chr_id):
    r = T(18) + chr_id * 0xd8
    res = {}
    for anim in range(54):
        first, n = struct.unpack_from('<hh', F, r + anim * 4)
        if n > 0:
            res[anim] = [dict(kind=p[10], sprite=p[0], x=p[1], y=p[2], frame=p[6]) for p in (kf(first + k) for k in range(n))]
    return res


def rwdata(pram, src=0xe6a0, n=0xb30):
    """ARM __decompress1 of the SRAM RW image (see 06-system 3.5)."""
    out, i = bytearray(), src
    while len(out) < n:
        t = pram[i]; i += 1
        lit = t & 7 or pram[i]; i += (t & 7) == 0
        m = t >> 4 or pram[i]; i += (t >> 4) == 0
        out += pram[i:i + lit - 1]; i += lit - 1
        if t & 8:
            off = pram[i]; i += 1
            for _ in range(m + 2):
                out.append(out[-off])
        else:
            out += bytes(m)
    return bytes(out[:n])


def main():
    d = {}
    # ---- outside toys -------------------------------------------------------------------
    small, large = xb(0x1009bb22, 4), xb(0x1009ba90, 2)
    d['toy_slots'] = {
        'note': 'slot 0-3 small (type 3), 4-5 large (type 4); large 4 covers small 0,1; large 5 covers small 2,3',
        'small_cursor_order': small, 'large_cursor_order': large, 'remove_cursor_order': xb(0x1009bb26, 6),
        'setsubi_pos': {}, 'home_pos': {}}
    for f in range(3):
        for area in range(2):
            t = xs16(0x1009ba92 + f * 0x30 + area * 0x18, 12)
            d['toy_slots']['setsubi_pos'][f'{FIELDS[f]}/{area}'] = [dict(slot=s, z=t[2 * s + 1], **elem(t[2 * s] & 0xffff)) for s in range(6)]
            h = xs16(0x100933ae + f * 0x54 + 0xc + area * 0x24, 18)
            names = ['slot0', 'slot1', 'slot2', 'slot3', 'slot4', 'slot5', 'pet', 'pet_egg', 'guest']
            d['toy_slots']['home_pos'][f'{FIELDS[f]}/{area}'] = [dict(role=names[s], z=h[2 * s + 1], **elem(h[2 * s] & 0xffff)) for s in range(9)]
    d['toy_slots']['empty_slot_idle_anims'] = {'slot0': 0x11e, 'slot1': 0x11f, 'slot2': 0x120, 'slot3': 0x11e}
    it = items()
    d['toys'] = [dict(id=w[0], name_txt=w[1], type=w[11], size='small' if w[11] == 3 else 'large',
                      icon_sprite=w[2], list_sprite=w[3], place_anim=w[4],
                      play_anim=[w[5], w[6], w[7]] if w[10] & 1 else w[5], per_field_play=bool(w[10] & 1),
                      buy=w[12], sell=w[13]) for w in it if w[11] in (3, 4)]
    # ---- worn items (planet decoration) -------------------------------------------------
    cat = xb(0x100a09b8, 3)
    d['deco'] = {'category_types': cat, 'category_icon_elems': x16(0x100a0084, 3),
                 'equip_ram': ['0x18000d88 hat (part kind 4)', '0x18000d8a glasses (kind 6)', '0x18000d8c accessory (kind 8)']}
    d['worn_items'] = [dict(id=w[0], name_txt=w[1], type=w[11], slot=w[11] - 5, sprite=w[2], icon=w[3], buy=w[12], sell=w[13])
                       for w in it if w[11] in (5, 6, 7)]
    sets = [x16(0x100947c8 + i * 10, 5) for i in range(7)]
    d['equip_sets'] = [dict(hat=s[0], glasses=s[1], accessory=s[2], guests=[s[3], s[4]]) for s in sets]
    # ---- planet ---------------------------------------------------------------------------
    rows = {'romtype_class0(romtype1/5)': 0x10094786, 'romtype_class1(romtype2/6)': 0x100947b2, 'romtype_class2(romtype3/4)': 0x1009479c}
    d['planet_chr_by_rank'] = {k: x16(v, 11) for k, v in rows.items()}
    chrs = sorted({c for v in rows.values() for c in x16(v, 11)})
    d['planet_models'] = {c: model_parts(c) for c in chrs}
    d['space_view'] = {'bg_anim_elem': elem(0x2c8), 'stars_overlay_elem': elem(0x2c9),
                       'planet_awake_elem': elem(0x2ca), 'planet_asleep_elem': elem(0x2cb),
                       'dead_tama_star_elems': [elem(0x2d0 + i) for i in range(5)], 'star_cursor_elem': elem(0x2cc)}
    d['rank'] = {'goal_txt_by_rank': x16(0x100a138e, 10), 'rankup_txt_by_rank': x16(0x100a13a2, 11)}
    # ---- travel ---------------------------------------------------------------------------
    dest_items = [0x26ad, 0x26ae, 0x26af, 0x26b6, 0x26b4, 0x26b5, 0x26b3]
    min_rank = [3, 3, 4, 5, 6, 7, 7]
    anims = [0x866, 0x867, 0x868, 0x86f, 0x86d, 0x86e, 0x86c]
    npc = x16(0x10091634, 14)
    if len(sys.argv) > 2:
        rw = rwdata(open(sys.argv[2], 'rb').read())
        souv = list(struct.unpack_from('<14H', rw, 0x6e6)); src = 'rwdata 0x180006e6'
    else:
        souv = [v for s in sets for v in (s[0], s[2])]; src = 'equip_sets (same order)'
    d['travel'] = {'souvenir_source': src, 'meter_ram': '0x18000d13 (TamaData+0x1b), max 60, cost 60',
                   'destinations': [dict(index=i, item=dest_items[i], min_rank=min_rank[i], anim=anims[i],
                                         npc_chara=[npc[2 * i], npc[2 * i + 1]], souvenirs=[souv[2 * i], souv[2 * i + 1]])
                                    for i in range(7)]}
    json.dump(d, sys.stdout, indent=1)


if __name__ == '__main__':
    main()
