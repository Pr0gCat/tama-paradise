"""Generate crates/tama-core/src/hant_table.rs and hant.bin: Traditional Chinese (zh-Hant) for the remake.

The firmware's CJK font holds only GB2312 (simplified) Hanzi. Traditional characters it lacks are
drawn from Cubic 11 (https://github.com/ACh-K/Cubic-11, free license in LICENSE-Cubic11.txt) at 12 px with raster.swift.

Also the remake's own menu texts (TOUCH ON, SAVE DATA, …) in all ten languages.

usage: python3 -I re/hant/gen.py data/game.json CJK_CHARSET Cubic_11.ttf RASTER_BIN STRINGS_CSV
  CJK_CHARSET: GMMan's cjk_charset.txt (glyph code = 0x1E1 + index).
  STRINGS_CSV: docs/spec/data/strings.csv (local only), for the kana / Latin glyph codes.
  RASTER_BIN:  `swiftc -O re/hant/raster.swift -o raster`.
Needs macOS (ICU "Hans-Hant" via Foundation, CoreText).
"""
import json, os, subprocess, sys, tempfile

game, charset, font, raster, strings_csv = sys.argv[1:6]
cs = open(charset, encoding='utf-8').read()
FIRST = 0x1E1
d = json.load(open(game))
zh = d['strings'][7]
font2 = d['fonts'][2]
NEW = len(font2['adj'])  # first new glyph code (font 2 frame NEW - 1)

def ch(c): return cs[c - FIRST] if FIRST <= c < FIRST + len(cs) else None

# Simplified Hanzi used by the zh strings, then ICU's character conversion of each.
used = sorted({ch(c) for s in zh for c in s if ch(c) and '一' <= ch(c) <= '鿿'})
swift = 'import Foundation\nfor l in readLine(strippingNewline: true).map({ Array($0) })! { print(String(l).applyingTransform(StringTransform("Hans-Hant"), reverse: false)!) }\n'
with tempfile.TemporaryDirectory() as t:
    p = os.path.join(t, 's2t.swift'); open(p, 'w').write(swift)
    out = subprocess.run(['swift', p], input=''.join(used), capture_output=True, text=True, check=True).stdout.split('\n')
rules = {a: b for a, b in zip(used, out) if a != b}
# Words the character conversion gets wrong (checked against ICU's phrase conversion and Taiwan usage).
WORDS = {'游戏': '遊戲', '准备': '準備', '这里': '這裡', '手里': '手裡', '制作': '製作', '这只': '這隻',
         '整只': '整隻', '伙伴': '夥伴', '萝卜': '蘿蔔', '面包': '麵包', '便面': '便麵', '利面': '利麵',
         '拉面': '拉麵', '冬面': '冬麵', '秋千': '鞦韆', '杂志': '雜誌', '发带': '髮帶', '发卡': '髮卡',
         '发冠': '髮冠', '发箍': '髮箍', '卷轴': '捲軸', '斗笠': '斗笠', '荧光': '螢光', '疗愈': '療癒'}
rules.update(WORDS)
# Taiwan usage over mainland words (applied to the simplified text, longest match first).
TAIWAN = {'拓麻歌子': '塔麻可吉', '拓麻': '塔麻', '信息': '資訊', '用户': '使用者', '设置': '設定',
          '数据': '資料', '界面': '畫面', '设备': '裝置', '搜索': '搜尋', '获取': '取得', '投喂': '餵',
          '出错': '錯誤', '当前': '目前', '起个名字': '取個名字', '文件': '檔案', '网络': '網路',
          '哦': '喔', '酸奶': '優格', '薯片': '洋芋片', '方便面': '泡麵', '意大利面': '義大利麵',
          '比萨饼': '披薩', '炸薯条': '薯條', '乌冬面': '烏龍麵', '咖喱': '咖哩', '盖浇饭': '燴飯',
          '汉堡包': '漢堡', '软冰淇淋': '霜淇淋', '蹦床': '彈跳床', '滑梯': '溜滑梯', '沙池': '沙坑',
          '墨西哥夹饼': '墨西哥捲餅', '投喂食物次数': '餵食次數', '更改表示': '更改顯示', '通过': '透過',
          '太阳镜': '太陽眼鏡', '路锥': '錐', '购物筐': '購物籃', '海洋球池': '球池', '威化': '威化餅',
          '巴士': '公車', '菠萝': '鳳梨', '牛油果': '酪梨', '鳄': '鱷', '歌子': '可吉'}
rules.update(TAIWAN)
NAME = '正體中文'
# The remake's menu texts (ui.rs REMAKE_TEXT ids), per language in data::LANGS order.
REMAKE = [
    ['タッチ ＯＮ', 'タッチ ＯＦＦ', 'セーブデータ', 'かきだし', 'よみこみ', 'はじめから', NAME,
     'かいはつしゃ', 'せいちょう', '+10000', 'ぜんぶおせわ'],
    ['TOUCH ON', 'TOUCH OFF', 'SAVE DATA', 'EXPORT', 'IMPORT', 'NEW GAME', NAME,
     'DEVELOPER', 'GROW UP', '+10000', 'FULL CARE'],
    ['TACTILE ON', 'TACTILE OFF', 'SAUVEGARDE', 'EXPORTER', 'IMPORTER', 'NOUVEAU JEU', NAME,
     'DÉVELOPPEUR', 'GRANDIR', '+10000', 'SOINS'],
    ['TOUCH EIN', 'TOUCH AUS', 'SPIELSTAND', 'EXPORT', 'IMPORT', 'NEUES SPIEL', NAME,
     'ENTWICKLER', 'WACHSEN', '+10000', 'PFLEGE'],
    ['TOQUE ON', 'TOQUE OFF', 'DADOS', 'EXPORTAR', 'IMPORTAR', 'NOVO JOGO', NAME,
     'DESENVOLV.', 'CRESCER', '+10000', 'CUIDAR'],
    ['TÁCTIL ON', 'TÁCTIL OFF', 'DATOS', 'EXPORTAR', 'IMPORTAR', 'NUEVO JUEGO', NAME,
     'DESARROLLO', 'CRECER', '+10000', 'CUIDAR'],
    ['TOCCO ON', 'TOCCO OFF', 'SALVATAGGIO', 'ESPORTA', 'IMPORTA', 'NUOVO GIOCO', NAME,
     'SVILUPPO', 'CRESCERE', '+10000', 'CURA'],
    ['触屏 开', '触屏 关', '存档', '导出', '导入', '新游戏', NAME, '开发者', '成长', '+10000', '全部照顾'],
    ['터치 ON', '터치 OFF', '저장 데이터', '내보내기', '가져오기', '새로 하기', NAME,
     '개발자', '성장', '+10000', '모두 돌보기'],
    ['觸控 開', '觸控 關', '存檔', '匯出', '匯入', '新遊戲', NAME, '開發者', '成長', '+10000', '全部照顧'],
]

trad = sorted({c for v in rules.values() for c in v} | set(NAME) | {c for c in ''.join(REMAKE[9]) if c >= '\u2e80'})
missing = [c for c in trad if c not in cs]
code = {c: FIRST + cs.index(c) for c in trad if c in cs}
code.update({c: NEW + i for i, c in enumerate(missing)})
def enc_s(s): return [FIRST + cs.index(c) for c in s]
def enc_t(s): return [code[c] for c in s]

# Kana / Latin glyph codes, read off the firmware's strings next to their text.
import collections, csv
SPACE = 305
seen = collections.defaultdict(collections.Counter)
for r in list(csv.reader(open(strings_csv, encoding='utf-8')))[1:]:
    for l in range(9):
        t, codes = r[1 + l].replace('\n', ''), [c for c in d['strings'][l][int(r[0])] if c != 0xF000]
        if len(t) == len(codes):
            for a, c in zip(t, codes):
                seen[a][c] += 1
def enc_any(s, hant):
    out = []
    for c in s:
        if c == ' ': out.append(SPACE)
        elif hant and c in code: out.append(code[c])
        elif c in cs: out.append(FIRST + cs.index(c))
        elif seen[c]: out.append(seen[c].most_common(1)[0][0])
        else: sys.exit(f'no glyph for {c!r} in {s!r}')
    return out

# Glyphs: 16×16, 1 bit per pixel (2 bytes a row, MSB left), moved 1 left and 2 down to sit like the
# firmware's glyphs.
with tempfile.TemporaryDirectory() as t:
    cp, bp = os.path.join(t, 'c.txt'), os.path.join(t, 'g.bin')
    open(cp, 'w', encoding='utf-8').write(''.join(missing))
    subprocess.run([raster, font, '12', '16', cp, bp], check=True)
    raw = open(bp, 'rb').read()
bits = bytearray()
for i in range(len(missing)):
    g = raw[i * 256:(i + 1) * 256]
    assert any(g), missing[i]
    for y in range(16):
        row = 0
        for x in range(16):
            sx, sy = x + 1, y - 2
            if 0 <= sx < 16 and 0 <= sy < 16 and g[sy * 16 + sx]: row |= 0x8000 >> x
        bits += row.to_bytes(2, 'big')
here = os.path.join(os.path.dirname(__file__), '../../crates/tama-core/src')
open(os.path.join(here, 'hant.bin'), 'wb').write(bits)

rs = ['// Generated by re/hant/gen.py: simplified → Traditional Chinese glyph codes for the zh-Hant table.',
      '// Codes from `FIRST` are glyphs the remake adds to font 2 (Cubic 11, re/hant/LICENSE-Cubic11.txt; bitmaps in hant.bin).',
      '', f'pub const FIRST: u16 = {NEW};', f'pub const GLYPHS: usize = {len(missing)};',
      f'/// "{NAME}"', f'pub const NAME: &[u16] = &{enc_t(NAME)};',
      '/// (simplified, traditional), longest first.', 'pub const RULES: &[(&[u16], &[u16])] = &[']
for a, b in sorted(rules.items(), key=lambda kv: (-len(kv[0]), kv[0])):
    rs.append(f'    (&{enc_s(a)}, &{enc_t(b)}),')
rs.append('];')
rs.append('/// The remake\'s menu texts, `[language][text]`.')
rs.append(f'pub const REMAKE: [[&[u16]; {len(REMAKE[0])}]; 10] = [')
for l, texts in enumerate(REMAKE):
    rs.append('    [' + ', '.join(f'&{enc_any(t, l == 9 or t == NAME)}' for t in texts) + '],')
rs.append('];')
open(os.path.join(here, 'hant_table.rs'), 'w').write('\n'.join(rs) + '\n')
print(len(rules), 'rules,', len(missing), 'new glyphs', file=sys.stderr)

# HANT_REVIEW=path: the converted strings as text, to read through (local only: ROM text).
if os.environ.get('HANT_REVIEW'):
    back = {v: k for k, v in code.items()}
    srules = sorted(rules.items(), key=lambda kv: -len(kv[0]))
    def conv(text):
        out, i = '', 0
        while i < len(text):
            for a, b in srules:
                if text.startswith(a, i):
                    out += b; i += len(a); break
            else:
                out += text[i]; i += 1
        return out
    with open(os.environ['HANT_REVIEW'], 'w', encoding='utf-8') as f:
        for n, s in enumerate(zh):
            f.write(f'{n} ' + conv(''.join(ch(c) or ('/' if c == 0xF000 else '·') for c in s)) + '\n')
