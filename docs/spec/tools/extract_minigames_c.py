#!/usr/bin/env python3
"""Extract fixed data of the TOGETHER ("PLAY") games 00-07 and EGG HUNT to JSON. stdlib only.
See docs/spec/09-minigames-together.md.

usage: python3 -I extract_minigames_c.py DUMP.bin [OUTDIR]
  DUMP.bin  raw 16 MiB flash dump (XIP code+rodata at flash 0x11000 = vaddr 0x10000000; data pack at 0x111040)
  OUTDIR    default docs/spec/data/minigames
Writes together_menu.json, together00/01/02/03/04/06/07.json, hunt.json.
"""
import json, os, struct, sys

XIP, DP = 0x11000, 0x111040
f = open(sys.argv[1], 'rb').read()
out = sys.argv[2] if len(sys.argv) > 2 else 'docs/spec/data/minigames'


def x(fmt, va, n=1):
    """n values of struct fmt at XIP vaddr va."""
    v = struct.unpack_from('<' + fmt * n, f, XIP + va - 0x10000000)
    return list(v) if n > 1 else v[0]


offs = struct.unpack_from('<23I', f, DP)
T4, T5 = DP + offs[4] * 2, DP + offs[5] * 2


def elem_xy(i):
    """(x, y) of UI element i (data pack tables 4/5, 08-care-ui §3.1)."""
    a = T5 + struct.unpack_from('<H', f, T4 + 2 * i)[0] * 2
    return list(struct.unpack_from('<hh', f, a + 2))


def elems(ids):
    return {hex(i): elem_xy(i) for i in ids}


def grid(va, w, h, n):
    """n char maps of w x h bytes, row strings."""
    raw = f[XIP + va - 0x10000000: XIP + va - 0x10000000 + w * h * n].decode('ascii')
    return [[raw[m * w * h + r * w: m * w * h + (r + 1) * w] for r in range(h)] for m in range(n)]


def h(v):
    return [hex(a) for a in v] if isinstance(v, list) else hex(v)


def save(name, obj):
    os.makedirs(out, exist_ok=True)
    with open(os.path.join(out, name), 'w') as fh:
        json.dump(obj, fh, indent=1, ensure_ascii=False)


# ---------------- TOGETHER menu, scene 0x24 FUN_10040ff4 ----------------
items = [x('I', 0x10094140 + 12 * i) for i in range(4)], [x('H', 0x10094144 + 12 * i) for i in range(4)]
menu = {
    'items_0x10094140': [{'text_id': t, 'scene': hex(s)} for t, s in zip(*items)],
    'rank_req_0x1009396e': x('H', 0x1009396e, 4),
    'locked_msg_by_rank_0x10093fe0': x('H', 0x10093fe0, 4),
    'note': '12 B items {u32 text, u16 scene, pad}; listmenu style 0, bg 4 (elem 0x2af = sprite 204)',
}
assert [s for s in items[1]] == [0x69, 0x66, 0x6a, 0x6b] and menu['rank_req_0x1009396e'] == [0, 0, 0, 0]
save('together_menu.json', menu)

# ---------------- game 00 ARROW (scene 0x65, unreachable) ----------------
g00 = {
    'answers_0x100a113a': x('H', 0x100a113a, 18),  # 0 = left (A), 1 = right (B); index = save bits 2..6 of 0x18001678
    'look_anim_0x100a009a': h(x('H', 0x100a009a, 2)),
    'ok_anim_0x100a009e': h(x('H', 0x100a009e)), 'ng_anim_0x100a00a0': h(x('H', 0x100a00a0)),
    'thresholds_0x100a13d0': x('H', 0x100a13d0, 3),  # score >= [0] perfect(+10), >= [1] +5, >= [2] +1
    'result_anims': h([0xa91, 0xa92, 0xa93, 0xa94]), 'intro_anim': '0xa8b', 'idle_anim': '0xa8c',
    'rounds': 5, 'auto_pick_frames': 200,
    'elems': elems([0x52b, 0x52c, 0x52d, 0x52e, 0x52f, 0x530, 0x531, 0x535, 0x536, 0x537, 0x538, 0x539]),
}
assert g00['thresholds_0x100a13d0'] == [5, 3, 1] and set(g00['answers_0x100a113a']) == {0, 1}
save('together00.json', g00)

# ---------------- game 01 FLAGS (scene 0x66) ----------------
cmd = x('H', 0x100a10c2, 60)
g01 = {
    'commands_0x100a10c2': [[{'feint': cmd[(s * 10 + i) * 2], 'answer': cmd[(s * 10 + i) * 2 + 1]}
                             for i in range(10)] for s in range(3)],
    'answer_meaning': {'1': 'A only (screen-left flag, palette 1, up)', '2': 'B only (screen-right flag, palette 0 red, up)',
                       '3': 'A and B', '4': 'no button'},
    'feint_anim_by_value_0x100a00a0': {str(v): hex(x('H', 0x100a00a0 + 2 * v)) for v in range(1, 5)},
    'answer_anim_by_value_0x100a00a8': {str(v): hex(x('H', 0x100a00a8 + 2 * v)) for v in range(1, 5)},
    'result_anims_0x100a13c0': h(x('H', 0x100a13c0, 4)),  # perfect, +5, +1, 0
    'thresholds_0x100a13d8': x('H', 0x100a13d8, 3),
    'ok_anim': '0xa9e', 'ng_anim': '0xa9f', 'intro_anim': '0xa95', 'input_window_frames': 20,
    'elems': elems([0x53b, 0x53c, 0x53d]),
}
assert g01['thresholds_0x100a13d8'] == [10, 8, 1] and set(cmd) == {1, 2, 3, 4}
save('together01.json', g01)

# ---------------- game 02 3 UFOS (scene 0x67, unreachable) ----------------
pos = x('H', 0x1009a710, 300)
g02 = {
    'layouts_0x1009a710': [[pos[(s * 20 + i) * 3:(s * 20 + i) * 3 + 3] for i in range(20)] for s in range(5)],
    'layout_note': 'entry = UFO id (1..3) standing at positions 0/1/2; the prize is under UFO id 2',
    'swaps_0x1009a6fe': [x('H', 0x1009a6fe + 6 * s, 3) for s in range(3)],  # [set][round]
    'move_seconds_0x1009c598': [round(v, 3) for v in x('f', 0x1009c598, 3)],  # per round
    'mood_0x100a0c60': x('h', 0x100a0c60, 4),  # by score 0..3
    'result_anims_0x10095738': h(x('H', 0x10095738, 4)),  # by score 0..3
    'ufo_elems_0x10093e38': h(x('H', 0x10093e38, 3)), 'cursor_elems_0x10091cb8': h(x('H', 0x10091cb8, 3)),
    'pet_elems_0x10091f9e': h(x('H', 0x10091f9e, 3)),
    'elems': elems(list(range(0x542, 0x54d))),
}
assert g02['swaps_0x1009a6fe'] == [[10, 16, 20], [12, 14, 19], [11, 15, 18]]
assert all(sorted(e) == [1, 2, 3] for s in g02['layouts_0x1009a710'] for e in s)
save('together02.json', g02)

# ---------------- game 03 WALKING (scene 0x68, unreachable) ----------------
ft3 = [x('H', 0x1009472c + 10 * i, 5) for i in range(3)]
g03 = {
    'maps_0x1009c5b0': grid(0x1009c5b0, 12, 14, 3),
    'legend': {'w': 'wall', 'S': 'start', 'G': 'goal', 'H': 'obstacle moving +-48 px horizontally',
               'V': 'obstacle moving +-48 px vertically', '-': 'obstacle track (no logic)', ' ': 'floor'},
    'cell_px': 48,
    'fieldtype_0x1009472c': [dict(zip(['intro_anim', 'bg_sprite', 'obstacle_sprite', 'wall_sprite', 'goal_sprite'],
                                      h(v))) for v in ft3],
    'mood_0x100a0c68': x('h', 0x100a0c68, 4),  # by hit count 0,1,>=2 ; timeout = 0
    'result_anims': {'hits0': '0xadd', 'hits1': '0xade', 'hits2plus': '0xadf', 'timeout': '0xae0'},
    'elems': elems(list(range(0x550, 0x563))),
}
assert all(sum(r.count('S') for r in m) == 1 for m in g03['maps_0x1009c5b0'])
save('together03.json', g03)

# ---------------- game 04 MATCHING (scene 0x69) ----------------
vals = x('H', 0x10091ece, 30)
petpos = x('H', 0x10091f94, 5)
rows = [vals[i * 3:i * 3 + 3] for i in range(10)]
g04 = {
    'card_values_0x10091ece': rows,  # [p][slot], p = save 0x1800167a bits0-3 (mod 10), advanced every round
    'pet_slot_0x10091f94': petpos,  # [q], q = save 0x1800167a bits4-6 (mod 5), advanced every round
    'card_frames_0x10091f10': [x('H', 0x10091f10 + 4 * r, 2) for r in range(3)],  # [round][value] -> sprite 0x8b4 frame
    'slot_elems_0x10091f0a': h(x('H', 0x10091f0a, 3)), 'pet_elems_0x10091f8e': h(x('H', 0x10091f8e, 3)),
    'cursor_elems_0x10091cbe': h(x('H', 0x10091cbe, 3)),
    'winnable_q_p': [[any(rows[p][c] == rows[p][petpos[q]] for c in range(3) if c != petpos[q])
                      for p in range(10)] for q in range(5)],
    'result_anims': {'3': '0xae7', '2': '0xae8', '1': '0xae9', '0': '0xaea'}, 'mood': {'3': 10, '2': 5, '1': 1, '0': 0},
    'elems': elems(list(range(0x563, 0x57c))),
}
assert len(set(petpos)) == 3 and all(set(r) == {0, 1} for r in rows)
save('together04.json', g04)

# ---------------- game 06 CATCH (scene 0x6a) ----------------
dur = x('I', 0x10092dc0, 3)
g06 = {
    'sweep_16_16_seconds_0x10092dc0': dur, 'sweep_frames': [round(d * 20 / 65536, 2) for d in dur],
    'zone_anims_0x10093e92': h(x('H', 0x10093e92, 4)),  # zone 0..3
    'marker_path_x': [elem_xy(0x5a1)[0], elem_xy(0x5a5)[0]],
    'zone_min_x': {'1': elem_xy(0x5a2)[0], '2': elem_xy(0x5a3)[0], '3': elem_xy(0x5a4)[0]},
    'grade_anims': {'3': '0xb21', '2': '0xb22', '1': '0xb23', '0': '0xb24'}, 'mood': {'3': 10, '2': 5, '1': 1, '0': 0},
    'elems': elems(list(range(0x59c, 0x5b0))),
}
assert g06['zone_min_x'] == {'1': 57, '2': 89, '3': 109} and g06['marker_path_x'] == [9, 119]
save('together06.json', g06)

# ---------------- game 07 SHIROTAMAS (scene 0x6b) ----------------
ft7 = [x('H', 0x1009474a + 8 * i, 4) for i in range(3)]
g07 = {
    'maps_0x1009c7a8': grid(0x1009c7a8, 18, 19, 3),
    'legend': {'w': 'wall (tree sprite)', 'S': 'start', 'T': 'shirotama to collect', 'G': 'bump creature (stun)',
               ' ': 'floor'},
    'cell_px': 32,
    'fieldtype_0x1009474a': [dict(zip(['intro_anim', 'bg_sprite', 'bump_elem', 'wall_sprite'], h(v))) for v in ft7],
    'mood_0x100a0c70': x('h', 0x100a0c70, 4),  # result 0..3
    'result_anims': {'0': ['0xb00', '0x59a'], '1': ['0xb01', '0x59b'], '2': ['0xb02', '0x59c'], 'fail': ['0xb03', '0x59d']},
    'elems': elems(list(range(0x57c, 0x59a))),
}
for m in g07['maps_0x1009c7a8']:
    s = ''.join(m)
    assert s.count('T') == 5 and s.count('G') == 2 and s.count('S') == 1
save('together07.json', g07)

# ---------------- EGG HUNT (scene 0x26 FUN_10041df8) ----------------
rew = [x('H', 0x100a0600 + 6 * i, 3) for i in range(25)]
hunt = {
    'rewards_0x100a0600': [[{'sprite': hex(r[0]), 'item': r[1], 'mission17_bit': r[2]}
                            for r in rew[ft * 8 + k * 4: ft * 8 + k * 4 + 4]] for ft in range(3) for k in range(2)],
    'rewards_index': 'list index = field*2 + kind (field 0 TROPICAL,1 FROZEN,2 MORI; kind 0 VOLCANO,1 SPRING); pick rand(0,3)',
    'golden_peach_entry_24': {'sprite': hex(rew[24][0]), 'item': rew[24][1]},
    'kind_menu_0x10094170': [x('I', 0x10094170 + 12 * i) for i in range(2)],
    'msg_text_0x10094364': {'cant_now': x('H', 0x10094364), 'no_more_today': x('H', 0x10094366)},
    'trip_anims_0x10093920': [[h(x('H', 0x10093920 + (k + ft * 2) * 8, 4)) for k in range(2)] for ft in range(3)],
    'return_anims_0x10095778': [h(x('H', 0x10095778 + ft * 8, 4)) for ft in range(3)],  # [field][result]
    'egg_anim_0x10095790': h(x('H', 0x10095790, 3)), 'peach_mound_sprite_0x10093382': h(x('H', 0x10093382, 3)),
    'filler_npc_tamaid': 0x2336, 'self_marker': 60000,
    'elems': elems([0x176, 0x177, 0x179, 0x17b, 0x17c, 0x17d, 0x17e, 0x17f] + list(range(0x180, 0x192))
                   + [0x165, 0x166] + list(range(0x167, 0x176)) + [0x9d, 0x9e]),
}
assert rew[24][1] == 6048 and sorted({r[2] for r in rew[:24]}) == list(range(18))
save('hunt.json', hunt)
print('ok', out)
