#!/usr/bin/env python3
"""Decode motion scripts (data pack tables 13/14) from the Glacier flash dump.

Usage (from repo root):
  python3 -I docs/spec/tools/decode_motion.py 0x563 0x564   # pretty-print scripts
  python3 -I docs/spec/tools/decode_motion.py --csv docs/spec/data/motions.csv

Format (see docs/spec/07-animation.md):
  table13[i] u16 = offset in u32 units into table14; len = (t13[i+1]-t13[i])*2 u16 words
  word0 = (ncmds << 1) | loop
  cmd   = u16 op (low byte = opcode, bits 15,14,13,11,10,9 = args A,B,C,D,E,W present), then args
  0xFCBD = END marker. Missing args read as 0. W = frames to wait after the cmd (20 fps).
"""
import struct, sys

DUMP = 'dumps/paradise-glacier-20261019-113049.bin'
PACK = 0x111040
ARGS = ((15, 'A'), (14, 'B'), (13, 'C'), (11, 'D'), (10, 'E'), (9, 'W'))
OPS = {0: 'nop', 1: 'sprite', 2: 'frame', 3: 'palette', 4: 'tw_x', 5: 'tw_y', 6: 'tw_xy',
       7: 'tw_scale', 8: 'tw_rot', 9: 'tw_scale_rot', 10: 'tw_alpha', 11: 'tw_fade', 12: 'tw_alpha_fade',
       13: 'flip', 14: 'sound', 15: 'spawn_obj', 16: 'spawn_script', 17: 'xform_mode', 18: 'scene_fx',
       19: 'shake', 20: 'visible', 22: 'layer', 23: 'blend', 25: 'actor_obj', 26: 'signal',
       27: 'loop_here', 28: 'chr', 29: 'anim', 30: 'eyes', 32: 'kill_children', 36: 'mouth'}

f = open(DUMP, 'rb').read()
offs = struct.unpack_from('<23I', f, PACK)
T13, T14 = PACK + offs[13] * 2, PACK + offs[14] * 2
N = offs[14] - offs[13] - 1          # number of scripts (2979)
u16 = lambda a: struct.unpack_from('<H', f, a)[0]
s16 = lambda v: v - 0x10000 if v & 0x8000 else v


def script(i):
    a, b = u16(T13 + 2 * i), u16(T13 + 2 * i + 2)
    w = struct.unpack_from('<%dH' % ((b - a) * 2), f, T14 + a * 4)
    cmds, p = [], 1
    while p < len(w):
        op = w[p]; p += 1
        if op == 0xFCBD:
            cmds.append(('END', {})); continue
        args = {}
        for bit, k in ARGS:
            if op >> bit & 1:
                args[k] = s16(w[p]); p += 1
        cmds.append((OPS.get(op & 0xFF, 'op%d' % (op & 0xFF)), args))
    return w[0] & 1, cmds


def poses(cmds):
    """(anim, frames) steps: anim set by chr/anim, held for the W of following cmds."""
    out, cur = [], None
    for name, a in cmds:
        if name in ('chr', 'anim'):
            cur = a.get('C', 0)
            out.append([cur, 0])
        if out and 'W' in a:
            out[-1][1] += a['W']
    return out


def summary(i):
    loop, cmds = script(i)
    chrs = sorted({a.get('A', 0) for n, a in cmds if n == 'chr'})
    steps = ' '.join('%d:%s' % (an, 'inf' if fr >= 0x7fff else fr) for an, fr in poses(cmds))
    total = sum(a.get('W', 0) for n, a in cmds)
    return [i, hex(i), loop, len(cmds), 'inf' if total >= 0x7fff else total,
            ' '.join(map(str, chrs)), steps]


if __name__ == '__main__':
    if sys.argv[1:2] == ['--csv']:
        import csv
        with open(sys.argv[2], 'w', newline='') as fh:
            w = csv.writer(fh)
            w.writerow(['id', 'hex', 'loop', 'ncmds', 'total_frames', 'chr_ids', 'anim:frames'])
            for i in range(N):
                w.writerow(summary(i))
        sys.exit()
    for i in (int(x, 0) for x in sys.argv[1:]):
        loop, cmds = script(i)
        print('script %d (%#x) loop=%d' % (i, i, loop))
        for n, a in cmds:
            print('   ', n, ' '.join('%s=%d' % kv for kv in a.items()))
    # self-check: script 2 = anims 1,2,3 x 20 frames, looping
    assert script(2)[0] == 1 and poses(script(2)[1]) == [[1, 20], [2, 20], [3, 20]]
