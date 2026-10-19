#!/usr/bin/env python3
"""Decode Tamagotchi Paradise save data (stdlib only). See docs/spec/03-save.md.

usage: python3 -I decode_save.py DUMP.bin|BACKUP.tamasave

- 16 MiB flash dump: read as-is.
- .tamasave (Paramix Studio, v3): 0xc0-byte header, then chunks {u32 page, u32 len=0x1000, data},
  then trailer 'PMXE' u32 field_type, u32 start, u32 end. Chunks are overlaid on a 0xff image.
Save slot (flash 0xefe000 main / 0xeff000 backup, 0x1000 each):
  u16 crc, u16 ~crc, u8 image[0xffc]  -> image is copied to SRAM 0x18000be8.
  crc = CRC-16/ARC (poly 0xA001 reflected, init 0, xorout 0) over image[0xffc]
  (hardware CRC engine 0x40038000 mode 0, table 0x100a00bc; PRAM FUN_000088c0).
"""
import struct, sys

RAM = 0x18000be8
SLOTS = (0xefe000, 0xeff000)


def crc16_arc(data):
    c = 0
    for b in data:
        c ^= b
        for _ in range(8):
            c = (c >> 1) ^ 0xA001 if c & 1 else c >> 1
    return c


def load(path):
    raw = open(path, 'rb').read()
    if raw[:8] != b'TAMASAVE':
        return raw
    ver, ts = struct.unpack_from('<II', raw, 8)
    img = bytearray(b'\xff' * 0x1000000)
    o = 0xc0
    while o + 8 <= len(raw):
        page, ln = struct.unpack_from('<II', raw, o)
        if ln != 0x1000:
            break
        img[page << 12:(page << 12) + ln] = raw[o + 8:o + 8 + ln]
        o += 8 + ln
    magic, field, start, end = struct.unpack_from('<4sIII', raw, o)
    assert magic == b'PMXE', magic
    print(f'tamasave v{ver} unix_time={ts} field_type={field} flash {start:#x}-{end:#x} '
          f'({(o - 0xc0) // 0x1008} chunks)')
    return bytes(img)


def slot(flash, i):
    blk = flash[SLOTS[i]:SLOTS[i] + 0x1000]
    crc, ncrc = struct.unpack_from('<HH', blk)
    calc = crc16_arc(blk[4:])
    return blk[4:], crc == (~ncrc & 0xffff) and crc == calc, crc, calc


def dt(img, a):
    y, mo, d, h, mi, s = struct.unpack_from('<6H', img, a - RAM)
    return f'{y:04d}-{mo:02d}-{d:02d} {h:02d}:{mi:02d}:{s:02d}'


def days_to_date(n):  # day 0 = 2025-01-01 (FUN_10006808 / FUN_1000ada0)
    y = 2025
    while True:
        yl = 366 if (y % 4 == 0 and y % 100) or y % 400 == 0 else 365
        if n < yl:
            break
        n -= yl; y += 1
    ml = [31, 29 if yl == 366 else 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    m = 0
    while n >= ml[m]:
        n -= ml[m]; m += 1
    return f'{y:04d}-{m + 1:02d}-{n + 1:02d}'


def main():
    flash = load(sys.argv[1])
    s0, s1 = slot(flash, 0), slot(flash, 1)
    for i, s in enumerate((s0, s1)):
        print(f'slot{i} @{SLOTS[i]:#x}: stored={s[2]:#06x} calc={s[3]:#06x} valid={s[1]}')
    # boot order (PRAM FUN_00008b5c): load slot0, then slot1 overwrites RAM if valid
    img = s1[0] if s1[1] else s0[0]
    assert s0[1] or s1[1], 'no valid slot -> firmware would reinit defaults (FUN_000089a8)'
    u8 = lambda a: img[a - RAM]
    u16 = lambda a: struct.unpack_from('<H', img, a - RAM)[0]
    i16 = lambda a: struct.unpack_from('<h', img, a - RAM)[0]
    u32 = lambda a: struct.unpack_from('<I', img, a - RAM)[0]
    i32 = lambda a: struct.unpack_from('<i', img, a - RAM)[0]
    P = lambda a, name, v: print(f'  save+{a - RAM:#05x} RAM {a:#010x} {name:<28} {v}')

    print('-- system')
    P(0x18000be8, 'sys_flags', f'{u8(0x18000be8):#04x}')
    P(0x18000be9, 'sys_flags2 (u16)', f'{u16(0x18000be9):#06x}')
    P(0x18000bec, 'settings (vol=bits3-4)', f'{u32(0x18000bec):#x} vol={(u32(0x18000bec) & 0x1f) >> 3}')
    bd, bs = i32(0x18000bf0), u32(0x18000bf4)
    P(0x18000bf0, 'rtc_base_day', f'{bd} = {days_to_date(bd)}')
    P(0x18000bf4, 'rtc_base_sec', f'{bs} = {bs // 3600:02d}:{bs // 60 % 60:02d}:{bs % 60:02d} at RTC count 0')
    P(0x18000bf8, 'dial_turns_plus', i32(0x18000bf8))
    P(0x18000bfc, 'dial_turns_minus', i32(0x18000bfc))
    P(0x18000c00, 'last_tick_time', dt(img, 0x18000c00))
    P(0x18000c0c, 'setup_date', dt(img, 0x18000c0c))
    P(0x18000c18, 'next_alarm (RTC count)', u32(0x18000c18))
    P(0x18000c1c, 'rtcint_mode', u32(0x18000c1c))
    P(0x18000c54, 'device_random (start)', f'{u32(0x18000c54):#010x}')
    P(0x18000c58, 'planet_name u16[9]', ' '.join(f'{u16(0x18000c58 + 2 * i):#x}' for i in range(9)))
    c6a = u16(0x18000c6a)
    P(0x18000c6a, 'profile/lang (u16)', f'{c6a:#06x} lang={((c6a >> 9) & 0xf) - 1} lo4={c6a & 0xf} b4_8={(c6a & 0x1ff) >> 4}')
    P(0x18000c6c, 'planet_rank', f'{u8(0x18000c6c):#04x} rank={u8(0x18000c6c) & 0xf}')
    P(0x18000c74, 'adults_raised', u8(0x18000c74))
    P(0x18000c75, 'grazing_count[3]', [u8(0x18000c75 + i) for i in range(3)])
    P(0x18000c78, 'chara_book bitset[0x20]', img[0x18000c78 - RAM:0x18000c98 - RAM].hex())
    P(0x18000c98, 'play_days', u16(0x18000c98))
    P(0x18000c9a, 'boot_count', u16(0x18000c9a))
    P(0x18000ca4, 'gotchi_points', u32(0x18000ca4))
    P(0x18000ca8, 'mission ctr ca8..cca', img[0x18000ca8 - RAM:0x18000cd9 - RAM].hex())
    P(0x18000cc4, 'mission14 points_total', u32(0x18000cc4))
    P(0x18000cdc, 'mission bits cdc/ce0/ce4', f'{u32(0x18000cdc):#x} {u32(0x18000ce0):#x} {u32(0x18000ce4):#x}')
    print('-- pet (g_tama 0x18000cf8, g_care 0x18000d14)')
    P(0x18000cfc, 'chara_id', f'{u16(0x18000cfc)} ({u16(0x18000cfc):#x})')
    P(0x18000cfe, 'chara_id2', u16(0x18000cfe))
    P(0x18000d00, 'day_secs', u32(0x18000d00))
    P(0x18000d04, 'tama_days', u16(0x18000d04))
    P(0x18000d06, 'generation', u16(0x18000d06))
    P(0x18000d08, 'cond', u8(0x18000d08))
    P(0x18000d0c, 'status_flags', f'{u32(0x18000d0c):#x}')
    P(0x18000d10, 'stage', (u8(0x18000d10) & 0x1f) >> 2)
    P(0x18000d11, 'color', u8(0x18000d11) & 0x1f)
    for a, n in [(0x18000d14, 'onaka'), (0x18000d15, 'gokigen'), (0x18000d16, 'osewamiss'),
                 (0x18000d18, 'cold'), (0x18000d32, 'onaka_max'), (0x18000d33, 'gokigen_max'),
                 (0x18000d34, 'unchi'), (0x18000d36, 'yogore_count')]:
        P(a, n, u8(a))
    P(0x18000d38, 'onaka_timer', u16(0x18000d38))
    P(0x18000d3a, 'gokigen_timer', u16(0x18000d3a))
    P(0x18000d40, 'age_timer', u32(0x18000d40))
    P(0x18000d84, 'alarm_reason', u8(0x18000d84))
    P(0x18000d88, 'equipped item u16[3]', [u16(0x18000d88 + 2 * i) for i in range(3)])
    P(0x18000dd7, 'field_index', u8(0x18000dd7))
    P(0x18000dd8, 'weather', u8(0x18000dd8))
    P(0x18000ddc, 'grazing[3] (u32)', [f'{u32(0x18000ddc + 4 * i):#x}' for i in range(3)])
    print('-- item storage (640 slots)')
    for i in range(640):
        n = (u32(0x18001354 + (i >> 3) * 4) >> ((i & 7) * 4)) & 0xf
        if n:
            print(f'    slot {i}: item {u16(0x18000e54 + 2 * i)} x{n}')
    print('-- friends / dlc')
    P(0x180015d4, 'friend_mask (12)', f'{u16(0x180015d4):#x}')
    P(0x18001694, 'dlc_mask u32[2] (40)', f'{u32(0x18001694):#x} {u32(0x18001698):#x}')
    # version block
    ver = flash[0xd29000:0xd2a000]
    print(f'-- version block 0xd29000: field_type={struct.unpack_from("<I", ver)[0]} '
          f'rom {struct.unpack_from("<H", ver, 0xff4)[0]}.{struct.unpack_from("<H", ver, 0xff6)[0]:02d}')


if __name__ == '__main__':
    main()
