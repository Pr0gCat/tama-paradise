#!/usr/bin/env python3
"""Extract fixed data of Lab mini-games 04/05/06 (+ meteor event, GAME menu, result screen) to JSON.
stdlib only. See docs/spec/09-minigames-lab-b.md.

usage: python3 -I extract_minigames_b.py DUMP.bin [PRAM.bin] [OUTDIR]
  DUMP.bin  raw 16 MiB flash dump (XIP code+rodata at flash 0x11000 = vaddr 0x10000000; data pack at 0x111040)
  PRAM.bin  decrypted PRAM (default: pram.bin next to DUMP). Needed for the game-05 meteor tables, which live in
            the RW data section (0x18000000) that boot decompresses from PRAM 0xe6a0 (__decompress1).
  OUTDIR    default docs/spec/data/minigames
"""
import json, os, struct, sys

XIP, DP = 0x11000, 0x111040
f = open(sys.argv[1], 'rb').read()
pram = open(sys.argv[2] if len(sys.argv) > 2 else os.path.join(os.path.dirname(sys.argv[1]), 'pram.bin'), 'rb').read()
out = sys.argv[3] if len(sys.argv) > 3 else 'docs/spec/data/minigames'


def x(fmt, va, n=1):
    """n values of struct fmt at XIP vaddr va."""
    v = struct.unpack_from('<' + fmt * n, f, XIP + va - 0x10000000)
    return list(v) if n > 1 else v[0]


def rwdata(src=0xe6a0, n=0xb30):
    """ARM __decompress1 of the RW init image (PRAM 0xe6a0 -> SRAM 0x18000000), see 06-system §3.5."""
    o, i = bytearray(), src
    while len(o) < n:
        t = pram[i]; i += 1
        lit = t & 7 or pram[i]; i += (t & 7) == 0
        m = t >> 4 or pram[i]; i += (t >> 4) == 0
        o += pram[i:i + lit - 1]; i += lit - 1
        if t & 8:
            off = pram[i]; i += 1
            for _ in range(m + 2): o.append(o[-off])
        else:
            o += bytes(m)
    return bytes(o[:n])


offs = struct.unpack_from('<23I', f, DP)
T4, T5 = DP + offs[4] * 2, DP + offs[5] * 2


def elem_xy(i):
    """(x, y) of UI element i (data pack tables 4/5, 08-care-ui §3.1)."""
    a = T5 + struct.unpack_from('<H', f, T4 + 2 * i)[0] * 2
    return list(struct.unpack_from('<hh', f, a + 2))


def elems(ids):
    return {hex(i): elem_xy(i) for i in ids}


rw = rwdata()
assert rw[0x406:0x40a] == bytes([9, 11, 10, 8]), 'RW data check (key pin table) failed'

# ---------------- game 04 (curling breakout), scene 0x61 ----------------
ang_f = x('f', 0x1009bb50, 3)
lab04 = {
    'scene': 0x61, 'name_str': 818,
    'launch_angle_turns': ang_f,                                  # float, x65536 -> 16-bit angle
    'launch_angle16': [int(a * 65536) for a in ang_f],
    'launch_index': [[x('H', 0x1009bb5c + lv * 30 + (b * 5 + r) * 2) for r in range(5)] for lv in range(3) for b in range(3)],
    'launch_index_layout': 'row = level*3 + ((counter>>9)&3); column = round 0..4; value indexes launch_angle16',
    'prize_brick': [[x('H', 0x10091d30 + lv * 100 + (q * 5 + r) * 2) for r in range(5)] for lv in range(3) for q in range(5)],
    'prize_brick_layout': 'row = level*5 + ((counter>>11)&7); column = round; value = brick number 1..15 (element 0x514+n)',
    'round_time_frames': x('I', 0x1009c5a4, 3),
    'prize_chr': x('H', 0x10093320, 49),                          # index (counter>>14)&0x7f
    'result_anim': x('H', 0x10095748, 4), 'result_sound': x('H', 0x10095770, 4),
    'bounce_sound': x('H', 0x1009390e, 2),                        # [0] wall/paddle, [1] brick
    'bricks': [elem_xy(0x515 + i) for i in range(15)],
    'elements': elems([0x50b, 0x50c, 0x50e, 0x510, 0x512, 0x513, 0x514, 0x525, 0x526, 0x3c8, 0x3e5, 0x3e6, 0x406]),
}

# ---------------- game 05 (meteor punch), scene 0x62 / event 0x63 ----------------
def meteor(off):
    t = rw[off:off + 0xf0]
    return [[[t[r * 6 + l * 2], t[r * 6 + l * 2 + 1]] for l in range(3)] for r in range(40)]


tables = {'L0': [0x46, 0x136, 0x226], 'L1': [0x720, 0x810, 0x900], 'L2': [0x40a, 0x4fa, 0x5ea]}
lab05 = {
    'scene': 0x62, 'event_scene': 0x63, 'name_str': 808,
    'row_format': '40 rows (one every 20 frames) x 3 lanes x [present, speed_index]; speed_index -> fall_time_s',
    'fall_time_s': [3.0, 2.5, 2.0], 'lives': [3, 2, 1], 'event_lives': 1,
    'layouts': {k: [{'sram': hex(0x18000000 + o), 'rows': meteor(o)} for o in v] for k, v in tables.items()},
    'layout_select': 'L0: (counter>>21)&3, L1: (counter>>23)&3, L2: (counter>>25)&3 (each cycles 0..2 after a finished game)',
    'event_layout': {'sram': hex(0x18000316), 'rows': meteor(0x316)},
    'tutorial_script': [{'delay': x('i', 0x1009dec4 + 8 * i), 'cmd': x('B', 0x1009dec8 + 8 * i),
                         'arg': x('B', 0x1009deca + 8 * i)} for i in range(10)],
    'tutorial_cmds': {0: 'spawn meteor in lane arg (speed 0)', 1: 'hand hint 0x95d at element 0x4a6+arg', 2: 'raise shield lane arg, stop hint', 3: 'end tutorial'},
    'elements': elems(list(range(0x48b, 0x4ab))),
}

# ---------------- game 06 (cloudy coin collection), scene 0x64 ----------------
courses = []
for i in range(9):
    p, n = x('I', 0x10092d70 + 8 * i), x('H', 0x10092d74 + 8 * i)
    courses.append({'level': i // 3, 'variant': i % 3, 'addr': hex(p),
                    'objects': [x('h', p + 6 * k, 3) for k in range(n)]})
lab06 = {
    'scene': 0x64, 'name_str': 814,
    'object_format': '[x_distance, y, type]; type 0 = obstacle (sprite 0x873, box 12x24), 9 = coin (sprite 0x872, box 24x24)',
    'course_select': 'L0: (counter>>27)&3, L1: (counter>>29)&3, L2: counter2&3 (DAT_18001678); +1 mod 3 at scene init',
    'goal_x': 6500, 'goal_object': [6500, 104, 2], 'time_frames': 2000,
    'courses': courses,
    'cloud_frame_by_pose': [x('H', 0x100938ee + 6 * i) & 0x1ff for i in range(6)],
    'elements': elems(list(range(0x4ad, 0x4c9))),
}

# ---------------- GAME menu (scene 0x2c) + result (scene 0x5c) ----------------
labmenu = {
    'menu_order_scene': [0x5d, 0x5e, 0x5f, 0x60, 0x64, 0x62, 0x61],
    'icon_sprite': x('I', 0x10093544, 7), 'name_str': x('H', 0x10093560, 7),
    'unlock_rank': x('H', 0x10093976, 7), 'level_str': x('H', 0x1009de8c, 3), 'level_rank': x('H', 0x10093968, 3),
    'locked_str_by_rank': x('H', 0x10093fe6, 12),
    'points': [x('H', 0x10091bec + 8 * lv, 4) for lv in range(3)],   # [level][result]
    'result_star_sound': x('H', 0x100a13b8, 4),
    'elements': elems(list(range(0x3cd, 0x3e5)) + [0xf7, 0xf8, 0xf9, 0xfa]),
}

# fixed-point math tables used by game 04 physics
math_tbl = {'sin_q12_1024': x('h', 0x1009a968, 1024), 'tan_q12_1025': x('i', 0x1009cbd4, 1025)}

os.makedirs(out, exist_ok=True)
for name, obj in (('lab04', lab04), ('lab05', lab05), ('lab06', lab06), ('labmenu', labmenu), ('lab_math', math_tbl)):
    with open(os.path.join(out, name + '.json'), 'w') as fp:
        json.dump(obj, fp, indent=1)

if __name__ == '__main__':
    # self-check against values confirmed in the decompilation
    assert lab04['launch_angle16'] == [0x6000, 0x4000, 0x2000] and lab04['round_time_frames'] == [1200, 1000, 800]
    assert [sum(c[0] for r in t['rows'] for c in r) for t in lab05['layouts']['L2']] == [34, 34, 34]
    assert [len(c['objects']) for c in courses] == [55] * 6 + [61] * 3
    assert labmenu['unlock_rank'] == [1, 4, 4, 1, 4, 1, 1] and labmenu['points'][2] == [10, 300, 450, 800]
    print('ok ->', out)
