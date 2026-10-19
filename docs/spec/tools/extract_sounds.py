#!/usr/bin/env python3
"""Dump the 87 buzzer melodies from xip.bin to CSV (+ optional square-wave WAVs). stdlib only.

usage: python3 -I extract_sounds.py xip.bin OUT.csv [WAVDIR]

Format (FUN_10090a80 / FUN_10024c78):
  table  0x100a0a28: u32 pointer[0x57] (count from FUN_10018180)
  melody: {u16 steps, u16 period_us, u16 duty} ... terminated by steps == 0
          period_us == 0 -> rest; duty = PWM duty % at volume 100 (always 10 in this dump)
  one step = (divider + 1) ms of SysTick; divider from table 0x10091b00 {id,div} x5, default 8
"""
import struct, sys, csv, wave, os

BASE, TABLE, COUNT, TEMPO = 0x10000000, 0x100a0a28, 0x57, 0x10091b00


def melodies(x):
    rd = lambda a, f: struct.unpack_from(f, x, a - BASE)
    tempo = dict(zip(*[iter(rd(TEMPO, '<10h'))] * 2))
    for i, p in enumerate(rd(TABLE, f'<{COUNT}I')):
        notes = []
        while True:
            n = rd(p + 6 * len(notes), '<3H')
            if n[0] == 0: break
            notes.append(n)
        yield i, p, tempo.get(i, 8) + 1, notes


def main():
    x = open(sys.argv[1], 'rb').read()
    wavdir = sys.argv[3] if len(sys.argv) > 3 else None
    if wavdir: os.makedirs(wavdir, exist_ok=True)
    with open(sys.argv[2], 'w', newline='') as f:
        w = csv.writer(f)
        w.writerow(['id', 'addr', 'ms_per_step', 'notes', 'total_ms', 'seq(steps:Hz)'])
        for i, p, ms, notes in melodies(x):
            w.writerow([i, hex(p), ms, len(notes), sum(n[0] for n in notes) * ms,
                        ' '.join(f'{s}:{round(1e6 / per) if per else 0}' for s, per, _ in notes)])
            if wavdir:  # ponytail: ideal square wave, ignores the piezo's frequency response
                sr, out = 22050, bytearray()
                for s, per, duty in notes:
                    for k in range(s * ms * sr // 1000):
                        hi = per and (k * 1e6 / sr) % per < per * duty / 100
                        out.append(200 if hi else 128)
                with wave.open(f'{wavdir}/sound_{i:02d}.wav', 'wb') as wv:
                    wv.setnchannels(1); wv.setsampwidth(1); wv.setframerate(sr); wv.writeframes(bytes(out))
    print('ok')


if __name__ == '__main__':
    main()
