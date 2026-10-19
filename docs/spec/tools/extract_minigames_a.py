#!/usr/bin/env python3
"""Extract fixed level data of Lab mini-games 00-03 (White Glacier) to JSON.

usage: python3 -I extract_minigames_a.py DUMP.bin [OUTDIR] [PRAM.bin]
  DUMP.bin  raw 16 MiB flash dump (XIP vaddr 0x10000000 = flash 0x11000)
  OUTDIR    default: docs/spec/data/minigames (next to this script's ../data)
  PRAM.bin  optional decrypted PRAM; only used to cross-check the game-00
            board pointer table, which lives in RW data 0x18000a28 (LZ-packed
            in PRAM at 0xe6a0, so it cannot be read from the raw flash).
Writes lab00.json .. lab03.json. See docs/spec/09-minigames-lab-a.md.
"""
import json, os, struct, sys

XIP = 0x11000 - 0x10000000          # XIP vaddr -> flash offset
DP = 0x111040                       # data pack (tables 4/5 = UI elements)

# RW 0x18000a28: u32[12] board pointers, index = level*3 + (count & 3); 9..11 are NULL.
LAB00_TABLE = [0x100a013e, 0x100a02b2, 0x100a0426, 0x100a0c78, 0x100a0db0, 0x100a0ee8,
               0x100a0696, 0x100a0792, 0x100a088e, 0, 0, 0]


def main():
    f = open(sys.argv[1], 'rb').read()
    out = sys.argv[2] if len(sys.argv) > 2 else os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'data', 'minigames')
    os.makedirs(out, exist_ok=True)
    rd = lambda fmt, va: struct.unpack_from('<' + fmt, f, va + XIP)
    assert rd('2H', 0x10094368) == (0xa57, 0xa5c), 'not the White Glacier dump?'
    if len(sys.argv) > 3:
        check_pram(open(sys.argv[3], 'rb').read())
    offs = struct.unpack_from('<23I', f, DP)
    t4, t5 = DP + offs[4] * 2, DP + offs[5] * 2

    def elem_xy(i):  # UI element fixed x/y (data pack tables 4/5)
        a = t5 + struct.unpack_from('<H', f, t4 + 2 * i)[0] * 2
        return list(struct.unpack_from('<hh', f, a + 2))

    def dump(name, obj):
        with open(os.path.join(out, name), 'w') as fh:
            json.dump(obj, fh, indent=1)
            fh.write('\n')

    # ---- game 00: 6 holes, one row of 6 s16 per 20 frames -------------------------------
    rows_decl = rd('3h', 0x1009de92)                      # rows per board by level: 31/26/21
    boards = []
    for idx, ptr in enumerate(LAB00_TABLE):
        b = {'table_index': idx, 'level': idx // 3, 'slot': idx % 3, 'addr': hex(ptr)}
        if ptr:
            lvl = [i for i, p in enumerate(LAB00_TABLE[:9]) if p == ptr][0] // 3
            b['rows_declared'] = rows_decl[lvl]
            b['rows'] = [list(rd('6h', ptr + 12 * r)) for r in range(rows_decl[lvl])]
            # exactly what the firmware reads: rows 0..23 at frames 20..480 (overrun when < 24 rows)
            b['rows_read'] = [[int(v != 0) for v in rd('6h', ptr + 12 * r)] for r in range(24)]
        else:
            b['rows'] = None                              # firmware reads from address 0 (bug)
        boards.append(b)
    holes = [dict(zip(('hole_elem', 'cursor_elem', 'cursor_alt_elem'), rd('3H', 0x100a0994 + 6 * i))) for i in range(6)]
    for h in holes:
        h['hole_xy'], h['cursor_xy'] = elem_xy(h['hole_elem']), elem_xy(h['cursor_elem'])
    dump('lab00.json', {
        'game': 'LABOGAME_00 whack-a-mole', 'frames_total': 500, 'row_period': 20,
        'board_index': 'level*3 + (DAT_18001674 & 3); counter += 1 on scene init',
        'board_table_rw_addr': '0x18000a28', 'boards': boards, 'holes': holes,
        'result_thresholds_3_2_1': list(rd('3h', 0x100a13c8)),
        'unused_time_rows_by_level': list(rows_decl)})

    # ---- game 01: jump --------------------------------------------------------------------
    maps_va = [0x10091fec, 0x10092160, 0x100922d4, 0x10092448, 0x100925bc, 0x10092730,
               0x100928a4, 0x10092a3c, 0x10092bd4]
    segs = []
    for s in range(32):
        n, p = rd('2I', 0x1009b168 + 8 * s)
        segs.append([list(rd('2h', p + 4 * k)) for k in range(n)])
    styles = []
    for s in range(22):
        n, p = rd('2I', 0x10093574 + 8 * s)
        styles.append(list(rd('%dH' % n, p)))
    ptypes = []
    for t in range(5):
        se, he = rd('H', 0x1009b272 + 2 * t)[0], rd('H', 0x1009b268 + 2 * t)[0]
        a = t5 + struct.unpack_from('<H', f, t4 + 2 * he)[0] * 2
        hdr = struct.unpack_from('<H', f, a)[0]
        w = struct.unpack_from('<h', f, a + 6 + 2 * (bin(hdr & 0x18).count('1')))[0]  # p5 after p3/p4
        sa = t5 + struct.unpack_from('<H', f, t4 + 2 * se)[0] * 2
        ptypes.append({'type': t, 'sprite_elem': se, 'hitbox_elem': he,
                       'sprite': struct.unpack_from('<H', f, sa + 6)[0], 'width': w,
                       'kind': ['normal', 'normal', 'spring', 'crumble', 'moving'][t]})

    def expand(entries):
        plats = []
        for i, (seg, sty) in enumerate(entries):
            for j, (x, y) in enumerate(segs[seg]):
                plats.append([x, y + 640 * i, styles[sty][j % len(styles[sty])]])
        return plats

    maps = []
    for idx, va in enumerate(maps_va):
        ent = [list(rd('2H', va + 4 * i)) for i in range(10)]
        maps.append({'index': idx, 'level': idx // 3, 'slot': idx % 3, 'addr': hex(va),
                     'segments': [{'seg': s, 'style': t} for s, t in ent],
                     'start': segs[ent[0][0]][0],
                     'platforms_xy_type': expand(ent) + [[64, 6400, 0]]})
    tut = [list(rd('2H', 0x10092d6c))]
    dump('lab01.json', {
        'game': 'LABOGAME_01 jump', 'segment_height': 640, 'goal_height': 6400,
        'map_index': 'level*3 + ((DAT_18001674 >> 2) & 3); counter = (counter+1) % 3 on exit',
        'time_limit_s': list(rd('3I', 0x1009572c)), 'platform_types': ptypes,
        'maps': maps, 'tutorial_map': {'addr': '0x10092d6c', 'segments': [{'seg': s, 'style': t} for s, t in tut],
                                       'platforms_xy_type': expand(tut)},
        'segments': segs, 'styles': styles,
        'note': 'platform y is world height above the start line (screen_y = 128 - (y - scroll)); type = style[j % len]'})

    # ---- game 02: memory cards ------------------------------------------------------------
    cards = [list(rd('15I', 0x1009436c + 60 * b)) for b in range(15)]
    for c in cards:
        assert sorted(c) == [0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7]
    dump('lab02.json', {
        'game': 'LABOGAME_02 memory cards', 'cols': 5, 'rows': 3, 'bomb_type': 7,
        'board_index': '((DAT_18001674 >> 4) & 7) % 5 + level*5; counter = (counter+1) % 5 on exit',
        'time_limit_s': [struct.unpack('<f', struct.pack('<I', v))[0] for v in rd('3I', 0x10093fd4)],
        'card_xy': [elem_xy(0x4cf + i) for i in range(15)], 'back_sprite': 0x876,
        'face_sprite': '0x877 + type',
        'boards': [{'index': b, 'level': b // 5, 'slot': b % 5, 'cards': c} for b, c in enumerate(cards)],
        'tutorial_cards': list(rd('15I', 0x100946f0))})

    # ---- game 03: three-lane collect ------------------------------------------------------
    lays = []
    for L in range(6):
        rows = [list(rd('2b', 0x10093984 + 0x98 * L + 2 * r)) for r in range(76)]
        lays.append({'index': L, 'level': L >> 1, 'slot': L & 1,
                     'rows': [r if r[0] >= 0 else None for r in rows],
                     'collectibles': sum(1 for a, k in rows if a >= 0 and k != 3),
                     'magnets': sum(1 for a, k in rows if a >= 0 and k == 3)})
    dump('lab03.json', {
        'game': 'LABOGAME_03 three-lane collect', 'row_distance': 64, 'goal_distance': 4992,
        'goal_spawn_distance': 4864, 'speed_steps': [[0, 3], [1152, 5], [2816, 7]],
        'layout_index': '((DAT_18001674 >> 7) & 1) | level<<1; bit toggles on exit',
        'lane_y': [elem_xy(0x4f7 + i)[1] for i in range(3)], 'spawn_x': elem_xy(0x4f7)[0],
        'despawn_x': elem_xy(0x4fb)[0], 'row_format': '[lane 0..2, kind 0..3] (kind 3 = magnet, frame of sprite 0x88f); null = empty',
        'layouts': lays})
    print('wrote lab00..lab03.json to', out)


def check_pram(p):
    """Unpack RW data (ARM __decompress1 at PRAM 0x1a4) and compare the game-00 pointer table."""
    src, out = 0xe6a0, bytearray()
    while len(out) < 0xb30:
        t = p[src]; src += 1
        lit = t & 7
        if not lit: lit = p[src]; src += 1
        m = t >> 4
        if not m: m = p[src]; src += 1
        out += p[src:src + lit - 1]; src += lit - 1
        if t & 8:
            off = p[src]; src += 1
            for _ in range(m + 2): out.append(out[-off])
        else:
            out += bytes(m)
    assert list(struct.unpack_from('<12I', out, 0xa28)) == LAB00_TABLE, 'RW board table mismatch'


if __name__ == '__main__':
    main()
