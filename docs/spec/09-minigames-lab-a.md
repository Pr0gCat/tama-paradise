# 09 Lab 小遊戲 A（00 打地鼠／01 往上跳／02 翻牌＋炸彈／03 三線道收集）

對象：White Glacier dump。範圍：scene 0x5d–0x60（`PSID_LABOGAME_00..03`）。規則背景見 `05-content.md §2.11`；UI 元件格式見 `08-care-ui.md §3`；motion script 見 `07-animation.md`。
工具：`docs/spec/tools/extract_minigames_a.py`（stdlib，`python3 -I extract_minigames_a.py DUMP.bin [OUTDIR] [pram.bin]`）→ `docs/spec/data/minigames/lab00.json … lab03.json`。
座標：128×128 螢幕、y 向下；UI 元件 (x,y) 是 sprite **中心**（加 sprite header 的 offset）。時間單位 frame = 1/20 s。按鍵：`key_pressed(m)` 0x10005ea8（按下邊緣）、`key_held(m)` 0x10006150、`dial_delta()` 0x10006140；位元 1=A、2=B、4=C、8=轉盤按壓、0x40=DIAL DOWN、0x80=DIAL UP（每 3 個原始計數一個單幀脈衝，見 06-system）。音效 `snd_play(id,100)` = `FUN_10023f20`（id ≥ 0x57 忽略）。

---

## 1. 摘要

- 四個遊戲都是「場景 context（task 結構）＋大狀態機」，每 frame：`task_pre_update` → 狀態函式 → `layer_update`（`FUN_1000a268`，跑所有 node callback 與 motion script）**[確認]**。
- **畫面幾乎全由 motion script 與 UI 元件組成**；遊戲碼只負責放置元件、切換 script、改 node 的 x/y/幀/palette。重製時直接重用 07-animation 的 script 直譯器即可。
- **四個遊戲都不畫玩家的寵物**：開場／教學／結果都用 `actor_play`（`FUN_10000854`，清空替換表），script 裡只有固定 sprite 與固定 ChrID（game 00 結果畫面用 ChrID 221 = 小嬰兒角色，sprite 2092）。主角分別是：00 奶瓶游標＋6 隻彩色小嬰兒、01 黃色小怪（sprite 2098）、02 潛水員（sprite 2175）、03 白色小鳥（sprite 2188）**[確認]**。
- 版面全部固定，以 `DAT_18001674` 的不同位元欄位當「遊玩次數」輪替（00：bits0–1、01：bits2–3 mod 3、02：bits4–6 mod 5、03：bit7），**沒有亂數** **[確認]**（01 的 `FUN_10017e24` 有一個 shuffle 但結果沒被使用）。
- 結果 0..3 寫 `DAT_18001c68`，之後切 scene 0x5c（共用結果畫面，給點數）。**門檻不隨難度改變**；難度只影響版面（與 01/02 的時限） **[確認]**。
- 共用開場：READY/GO = script 0x952（元件 0x3c8，(64,64)）：sprite 2065 紫色遮罩 + 子 script 0x953（snd 6，sprite 2073 30 frames → snd 1，sprite 2074 + OBJ 6），60 frames 後 signal **[確認]**。01 額外有 3-2-1 倒數 script 0x95c（sprite 2061 幀 2→1→0 各 20 frames，各 snd 1）。
- 共用 HUD 數字：TEXT 元件 p3 = 字型 3 = **sprite 19**（16×16 數字字型），格式字串 `"%d"`（0x100a4129）；倒數只在剩 < 10 秒時顯示（白色膠囊 sprite 2063 + 數字）**[確認]**。

---

## 2. 詳細規則

### 2.0 共用機制

| # | 規則 | 證據 | 標記 |
|---|---|---|---|
| C1 | Context 開頭為 task 結構：`+0 state`、`+8 substate`、`+0xc done`、`+0x10 wait`、`+0x14 flags`（bit0 = wait 已到 → 才呼叫狀態函式）、`+0x18 UI layer`、`+0x1c 時間`（16.16 秒，每 frame +0xccc = 0.05 s，供 tween 用） | `FUN_100695b8`、`FUN_10009ff4(layer,&t,0xccc)` | [確認] |
| C2 | 離開鍵 C：`key_pressed(4)`（01 用 `key_held(4)`）→ snd 2 → state 0x7fe0（淡出）→ 0x7ff0 done。返回目標由 init 的 `scene_set_next(0x2d/0x2e/0x2f/0x30)` 預設 | 各 state 函式 | [確認]（0x2d–0x30 是哪個畫面 [推測：Lab 遊戲選單]） |
| C3 | 開場 skip：A 或 B（mask 3）→ snd 1、淡出、下一步；actor script 發 signal −1（`actor_is_done` 0x10000584）也前進 | 同上 | [確認] |
| C4 | 結果寫入後切 `scene_set_next(0x5c)`；點數/任務在 0x5c 處理（05-content §2.11） | `FUN_100228c4` | [確認] |
| C5 | 「HOW TO PLAY」標題：元件 0x3e5（sprite 2080，128×24，(64,14)）＋ 0x3e6 文字（字串 813） | `FUN_100500d0` 0x3eb、`FUN_100711d8`、`FUN_1001b760` | [確認] |

### 2.1 Game 00 — 打地鼠（餵奶瓶）`PSID_LABOGAME_00`（0x5d）

Context `DAT_180141ec`（0x8c B）：`+0x20 cursor(s16)`、`+0x24 frame(s16)`、`+0x26 score(s16)`、`+0x2c 教學計數`、`+0x2e` 時間（未使用）、`+0x30 end 旗標`、`+0x34 row 指標`、`+0x38+i*8 {u8 state, s16 timer, node* anim}`（6 洞）、`+0x6c 游標 node`、`+0x74/78/7c HUD`、`+0x88 actor`。

**畫面**

| 物件 | 來源 | 位置 / 說明 |
|---|---|---|
| 背景 | 元件 0x3ea = ANIM 0x96c @(64,64) | 子 script：sprite 2086（牆，128×80）@(64,12)、2087（地板，128×96）@(64,80)、2088（蕾絲條 128×32）@(64,62)/(64,110)、2090（洞底橢圓 128×16）@(60,67)/(60,115) |
| 6 個洞 | 表 0x100a0994 `{u16 hole_elem, cursor_elem, alt}` ×6 | 洞 i = 元件 0x3eb+i：(20,51)(62,51)(104,51)(20,99)(62,99)(104,99)；node palette = i（`+0xc` bits9+ = i）→ sprite **2093**（48×48，6 幀，6 palette）第 i 組顏色 |
| 洞狀態 script | 0x977 睡（幀 0，y 上下 1px，循環 20f）／0x978 醒來（幀 1，x 抖動）／0x979 探頭（幀 2 + 汗滴 sprite 1835）／0x97b 被餵（幀 3 + 奶瓶倒奶 sprite 2094 幀 2 @(+10,−6) + OBJ 47）／0x97d 吵醒（幀 5 + sprite 1821 @(+8,−18)） | 每次換狀態都刪舊 node、在洞位置新建（z = i*2+2） |
| 游標（奶瓶） | script 0x976（sprite **2094** 幀 1）z=100 | 位置 = 元件 0x3f1+cursor：(36,62)(78,62)(120,62)(36,110)(78,110)(120,110)；目前洞 state 3 且 timer > 29 時 alpha 0（倒奶動畫取代） |
| 分數 | 元件 0x403 sprite 2062（32×24 白框）@(15,11) + 0x405 TEXT（字型 3，字距 −6）@(15,11) | `"%d"` score |
| 倒數 | 元件 0x404 sprite 2063 @(120,11)（初始隱藏）+ 0x406 TEXT @(120,11) | s = (500−frame)/20，0 ≤ s < 10 時顯示 |
| 結果 | ANIM 0x980(r3)/0x988(r2)/0x98f(r1)/0x996(r0) @元件 0x407–0x40a (64,64)，`anim_create` 直接播（無 actor） | 6 隻 ChrID 221（sprite 2092，palette 0–5）跳動；snd 59/52/52/48 在 script 內 |

**狀態機**（`FUN_100500d0`；play 每幀 `FUN_1007847c` 再 `FUN_100675ac`）

| state | 內容 |
|---|---|
| 0 | 開音效系統 → 1000 |
| 1000 | actor 播 0x963 @元件 0x3e7(64,64)（開場示範，160f 後 signal）；淡入 |
| 0x3e9 | C3/C2；完成或 A/B → 淡出 wait 10 |
| 0x3ea | 停 actor，wait 10 |
| 0x3eb | HOW TO PLAY + actor 播 0x971 @元件 0x3c8（教學示範）；淡入 |
| 0x3ec | A/B → 下一步；C 離開；否則 `+0x2c`++，與 90 比較成立時前進 **[推測：約 90 frames 自動前進；比較是 double 運算 thunk，未完全確認]** |
| 0x3ed–0x3ee | 淡出、清教學 |
| 0x3ef | layer 播 0x952 READY/GO；wait 50 |
| 0x3f0 → 2000 | 淡出 → `FUN_100709d0` 建場（背景、HUD、6 洞全 state 0 + 0x977、游標 cursor=0）、淡入、wait 10 |
| 0x7d1 | **遊戲**（下表） |
| 0x7d2 | （結束後 60f）淡出 wait 10 → 0x7d3 清場 → 0x7d4 → 3000 |
| 3000 | 依分數播結果 script、`DAT_18001c68`；wait 50 → 0xbb9 淡出 → 0xbba → scene 0x5c |

**遊戲規則 [確認]**（`FUN_1007847c`、`FUN_100675ac`）

1. `frame++`（從 1 起）。`frame == 500`（25 s）→ snd 0x4a、`end=1`、cursor 隱藏、wait 60 → 0x7d2。**總長固定 500，與難度無關**（`+0x2e = {31,26,21}[level]*20` 有算但沒人讀）。
2. 否則若 `frame % 20 == 0`：讀一列 6 個 s16，`row[i] != 0` 的洞設 `state=0, timer=1`（**不管目前狀態**，會打斷正在探頭或被餵的洞），row 指標 +12。⇒ 列 0..23 在 frame 20,40,…,480 讀出。
3. 輸入（優先序 A > B > C）：A（`key_pressed(1)`）→ snd 0、cursor = (cursor+1) % 6（只能往前循環）；B（`key_pressed(2)`，cursor ≥ 0）→ 判定目前洞：
   - state 1 或 2 → **命中**：snd 0x34、state 3、timer 50、播 0x97b、score+1、HUD 更新；score ≥ 10 → snd 0x4a、end、wait 60 → 0x7d2。
   - state 0（睡著）→ **失誤**：snd 0x30、state 4、timer 50、播 0x97d、end、wait 60 → 0x7d2（遊戲立即結束，分數保留）。
   - state 3/4 → 無效果。沒有轉盤操作。
4. 每洞計時（`FUN_100675ac`，0x7d1–0x7d3 都跑）：`timer > 0` 時 −1，歸零時：若 `end` 則先當作 state 3；`0 → 1`（timer 20，0x978）、`1 → 2`（timer 60，0x979）、`2/3/4 → 0`（timer 0 = 停住，0x977）。⇒ 一隻嬰兒可被餵的窗口 = 出現後 80 frames（醒來 20 + 探頭 60）。
5. 結果：門檻表 `0x100a13c8 = {10, 8, 1}`（三難度共用）：score ≥ 10 → 3、≥ 8 → 2、≥ 1 → 1、否則 0。

**版面資料**：RW 資料 `0x18000a28`（由 PRAM 0xe6a0 LZ 解出）= u32[12] 指標，`index = level*3 + (DAT_18001674 & 3)`，**init 時就 +1**（`&3` 循環 0..3）。每版面 = N 列 × 6 × s16（N = 31/26/21 依難度，表 `0x1009de92`）。已知 bug [確認]：(a) 第 4 次（index level*3+3）讀到下一難度第 0 張；難度 2 讀到 NULL（從位址 0 = PRAM 向量表讀，大多非 0 → 洞亂冒）；(b) 難度 2 只有 21 列但讀 24 列：board 6/7 讀到下一張的前 3 列，board 8 讀到 0x100a098a 的字串＋洞表（全非 0 → frame 440/460/480 六洞全開）。`lab00.json` 的 `rows_read` 已照韌體實際讀到的值輸出。

**音效**：0 游標、1 確定、2 離開、0x34(52) 命中、0x30(48) 失誤、0x4a(74) 時間到/滿分；script 內：6、1（READY/GO）、59/52/52/48（結果）。

### 2.2 Game 01 — 往上跳 `PSID_LABOGAME_01`（0x5e）

Context `DAT_180141f0`（0x1784 B）：`+0x20 計時`、`+0x2c 主角 node`、`+0x34 倒數 HUD`、`+0x48 地圖指標`、`+0x4c 段數(10)`、`+0x50 剩餘 frames`、`+0x54 scroll`、`+0x58 分數`、`+0x60 最高落腳高度`、`+0x6c/0x70 復活點 (x, y)`、`+0x74 平台池 32×0xb8`、`+0x177c` 旗標（bit0 = 掉落處理中、bit5 = 自動操作／教學）。主角資料（`+0x11c`，0x24 B）：`[0] x`、`[1] y`、`[6] 前一 y`、`[7] 加速度`（皆 16.16，y = 距「畫面底線」的高度，往上為正）、`[8]` 旗標 u16（bit0 空中、bit1 彈簧、bit2 下落中、bits3–6 落地倒數、bits7–11 降落傘計數）。

**畫面**

| 物件 | 來源 | 位置 / 說明 |
|---|---|---|
| 背景 | 元件 0x40d sprite **2095**（128×128 紅色布幕）兩份 | 視差 1/4：`v = (scroll>>2) mod 256`；第 1 份 y = v + (v<128 ? 64 : −192)、第 2 份 y = v − 64（`FUN_10009c34`）|
| 主角 | 元件 0x414 sprite **2098**（64×64，5 幀） | node x = 主角 x(+抖動)、node y = 96 − y（腳底 ≈ 128 − y）；幀：地面 0、空中 1、降落傘 2（`FUN_100051dc` msg 0xa） |
| 平台 | 型別 t：sprite 元件 `0x1009b272[t]`、碰撞元件 `0x1009b268[t]`（T3，p5 = 寬） | 0 sprite 2100（白毛球，寬 28）、1 sprite 2101（橘，寬 20）、2 sprite 2102（紅，**彈簧**，寬 30）、3 sprite 2103（黃果凍，**碎裂**，寬 24）、4 sprite 2104（泡泡，**移動**，寬 34）；畫在 (x+移動偏移, 128 − y)，被踩後 3 frames 用幀 1；alpha = 淡出值>>10（`FUN_1002bd90`） |
| 高度計 | 元件 0x40f sprite 2096（8×128）@(8,64)；標記 sprite 2097（8×8）| 標記 y = 120 − 112 × 最高高度/6400（`FUN_100245ec`；元件 0x410 y=120、0x411 y=8）[標記元件 id 推測 0x410] |
| 倒數 | TEXT 0x412（字型 3）+ 子 sprite 0x40e（2063）@(120,11) | `(frames+19)/20 < 10` 才顯示（`FUN_10066568`） |
| 3-2-1 | 元件 0x413 = ANIM 0x95c @(64,64) | 開局 60 frames |
| 結果 | actor 播 `0x10095740[r]` = r0 0x9be、r1 0x9b8、r2 0x9b6、r3 0x9ab @元件 0x422 | sprite 2095/2098/2100–2104 拼的場景，snd 48/52/52/59 |

**狀態機**（大狀態 `FUN_100205dc`：0 → 1000 開場 `FUN_100208cc` → 4000 教學 `FUN_100206ec` → 2000 遊戲 `FUN_10020a30` → 3000 結果 `FUN_10021178`；0x7fe0 時「地圖計數 = (計數+1) % 3」）

| 大/子 | 內容 |
|---|---|
| 1000/0 | actor 播 0x9a8 @元件 0x40b（主角站在背景前，120f signal）；淡入 |
| 1000/1 | `key_held(3)` → snd 1 → 2；`key_held(4)` → 離開；actor 完成 → 2 |
| 4000/0 | 教學 widget `FUN_1001b760`：HOW TO PLAY + 示範地圖 0x10092d6c（段 30、style 20 交替型別 0/1）自動跳，左右提示箭頭 0x420/0x421（script 0x95d，(24,108)/(64,108)）；示範時限 100 frames |
| 4000/1 | A/B → snd 1 直接到 READY；C → 離開；示範結束 → 淡出 → 2 |
| 4000/2 | 播 0x3c8（READY/GO），wait 40 → 9000 → 2000 |
| 2000/0 | 選地圖（下述）、建 scroller/spawner/主角/高度計/HUD，`+0x50 = {100,90,80}[level] × 20` frames（`0x1009572c`，int 秒） |
| 2000/1000 | 3-2-1（0x413），wait 60 → 2000 |
| 2000/2000 | **遊戲**（下述）|
| 2000/3000 | 掉落：snd 0x16 播完 → 計時 10 → snd 0x1a 播完 → 淡出 → `scroll = 最高高度 − 12`、清平台池、spawner 重來、主角放到復活點（x, y − scroll，速度歸零）→ 淡入 → 等 20 → 2000 |
| 2000/4000、5000 | 時間到／到頂：wait 60 → 9000 淡出 → 大 3000 |
| 3000 | 結果：`r = 分數≥3000→3、≥1800→2、≥300→1、否則 0`；actor 播結果 script；完成 → scene 0x5c |

**物理與規則 [確認]**（16.16 定點；每幀順序：狀態函式內 `FUN_10004efc`（輸入/起跳）→ layer update：scroller `FUN_10028498`（捲動＋`FUN_1002c148` 平台）→ spawner `FUN_10009d98` → 主角 `FUN_100051dc`）

1. 起跳（`FUN_10004efc`）：在地面（bit0=0）時落地倒數 ≠ 0 則 −1；= 0 則起跳：`[7] = 14.0`（snd 10）或彈簧 `[7] = 25.0`（snd 9，清 bit1），設 bit0。落地倒數 > 2 且非彈簧時按 A/B（`key_pressed(3)`）→ 立即起跳（倒數清 0）。
2. 左右（只在空中）：`key_held(1)` 且 x > 16 → −5.6；`key_held(2)` 且 x < 112 → +5.6；`dial_delta() > 0` → +5.6、`< 0` → −5.6（可疊加）；結果夾在 x ∈ [16, 112]。
3. 垂直（Verlet）：`v = (y − prev) + [7]`、`prev = y`、`y += v`；`bit2 = (y ≤ prev)`；之後 `[7] = −1.4`（重力，`FUN_1007aa98` = 0x16666）。⇒ 一般跳最高 77 px、彈簧約 236 px。
4. 降落傘：時間到（`+0x50 < 1`）且在空中且下落中 → `prev = y`、`[7] = −0.7`（等速 0.7 px/f 下降），x 每 8 幀在 ±2 抖動；幀 2。
5. 碰撞（只在下落中）：主角框 `x−4, top = 124 − (y + h), w = 8, h = 8`（`v < −15` 時 `h = |v| − 7` 防穿透）；平台框 `left = x_center − w/2, top = 124 − y_plat, w, h = 8`；AABB 重疊即落地（`FUN_1002be10`，取池中第一個）。落地：`y = prev = 平台 y`；被踩平台 bits4–5 = 3；型別 2 → 設彈簧；若原本在空中：落地倒數 = 4（**碎裂平台 = 0 → 下一幀直接再跳**）、清 bit0；記錄 rider（移動平台帶著主角走）。若時間 > 0 且 `平台 y + scroll > 最高高度` → 更新最高高度、`分數 = max(分數, min(h,6400)*3000/6400)`、復活點 = (平台 x, 平台 y + scroll)。
6. 捲動（`FUN_10028498`）：若 `最高高度 − scroll > 12` → 本幀捲 min(差, 4)；若主角 y > 80 → 捲 min(y − 80, 24)（後者優先）；scroll 上限 6336（0x18c0）。捲動量從主角 y/prev 與所有平台 y 扣掉。
7. 平台池 32 格（`FUN_1002c148`）：y < −16 → 回收；被踩計時 bits4–5 每幀 −1，**碎裂平台**歸 0 時開始淡出（0.2 s，值 ≤ 0 → 回收）；**移動平台**：x 偏移 = ±32 × sin(2π·(t − t0)/3 s)（週期 60 frames，最大約 3.35 px/f），方向依 spawn 序交替（bit6），夾在中心 ∈ [16 − w/2, 112 + w/2]；掉落處理中（0x177c bit0）且被踩過的移動平台暫停。
8. 產生（`FUN_10009d98`）：地圖 = 10 段 `{u16 seg, u16 style}`；段 i 的平台 j：`x = seg[j].x`、`世界 y = seg[j].y + 640 i`、`型別 = style[j % len]`。依序產生直到世界 y ≥ scroll + 384；只在 `世界 y − scroll > −16` 時真的建立。10 段用完且 scroll + 384 > 6400 → 在 (64, 6400) 放型別 0 終點平台。
9. 結束：`最高高度 ≥ 6400` → snd 0x35 → 5000（優先於時間判定）；時間 `+0x50 < 1` → snd 0x35 → 4000；主角 y < −24 → 3000 掉落（只損失時間；計時在 2000–3999 子狀態都在跑）。
10. 結果門檻 3000/1800/300（= 高度 6400/3840/640）**三難度共用**；分數 3000 只能靠到頂（`FUN_1006aa58`）。

**版面**：`index = level*3 + ((DAT_18001674 >> 2) & 3)`（9 張，0x10091fec 起每 0x174 一張；只讀前 10 筆 4 B）。段表 `0x1009b168`：32 × `{u32 count, u32 ptr → {s16 x, s16 y}[]}`；style 表 `0x10093574`：22 × `{u32 count, u32 ptr → u16 型別[]}`。`lab01.json` 已展開每張地圖的全部平台 `[x, 世界 y, 型別]`（含終點），起點 = 第一段第一個平台。

**音效**：10 跳、9 彈簧、0x16(22) 掉落、0x1a(26) 復活、0x35(53) 時間到/到頂、1、2；script 內 6/1（READY）、1×3（3-2-1）、59/52/52/48（結果）。

### 2.3 Game 02 — 翻牌配對＋炸彈 `PSID_LABOGAME_02`（0x5f）

Context `DAT_180141f4`（0x19c B）：`+0x68+i*0x10 {s16 state, s32 timer, node* back(+0x70), node* face(+0x74)}`（15 張）、`+0x15c cursor`、`+0x170 已選數`、`+0x174/178 兩張的型別`、`+0x17c/180 兩張的 index`、`+0x184 剩餘 frames`、`+0x188 frame 計數`、`+0x18c 判定計時`、`+0x190 版面 index`、`+0x194 配對數`、`+0x198 炸彈旗標`。

**畫面**

| 物件 | 來源 | 位置 / 說明 |
|---|---|---|
| 背景 | 元件 0x4cb sprite **2165**（海底 128×128）@(64,64) | |
| 卡片 15 張 | 元件 0x4cf+i（5 欄 × 3 列）| x = 11,34,57,80,103；y = 12,36,60。背面 sprite **2166**（貝殼）、正面 sprite `0x877 + 型別` = **2167–2174**（型別 7 = 炸彈牌，sprite 2174）|
| 游標 | 元件 0x4cc sprite **2176**（16×16，offset (4,6)）| 位置 = (11 + 23·(i%5), 12 + 24·(i/5))；開局 200 frames 後才顯示 |
| 潛水員 | actor 播 `0x100a008a[n]`：n0 0xa4b 待機（幀 0）、n1 0xa48 配對成功（幀 1 跳）、n2 0xa49 失敗（幀 3）、n3 0xa4a 炸彈（幀 2），皆 @(100,94) | sprite **2175**（64×64，6 幀）|
| 倒數 | TEXT 0x4cd（字型 3）+ sprite 0x4ce（2063）@(120,11) | 剩 < 10 秒顯示；**無分數 HUD** |
| 結果 | actor 播 r3 0xa4c @0x4e2、r2 0xa53、r1 0xa54、r0 0xa55（@0x4e3–0x4e5） | sprite 2165/2175/2177–2181，snd 59/52/52/48 |

**狀態機**（`FUN_10050954`）

| state | 內容 |
|---|---|
| 2000 | actor 播開場 0xa42 @元件 0x4c9（120f signal），wait 40 |
| 0x7d1 | 完成 → 淡出 → 0x7d2；A/B → snd 1 → 0x7d2；C → 離開 |
| 0x7d2 | 教學版面 `FUN_100711d8`（HOW TO PLAY、卡片型別取自 0x100946f0、游標在 5）wait 40 |
| 0x7d3 | 教學動畫 `FUN_10015a44`；A/B → 0x7d4；C → 離開 |
| 0x7d4 → 3000 | 清教學；actor 播 0x952 READY/GO @0x3c8，wait 40 → 0xbb9 淡出 10 → 0xbba → 4000 |
| 4000 | `FUN_1007073c` 建場、`time = {50,40,30}[level] × 20`（`0x10093fd4`，float 秒）；`FUN_10069ed0` 發牌（每張 state 400）；frame 計數 = 0 |
| 0xfa1 | 淡入，wait 20 |
| 0xfa2 | **遊戲**（下述） |
| 0xfa3 | 14 frames 後潛水員回待機（炸彈則保持 n3）→ 0xfa4 再 14 frames → 0xfa5 |
| 0xfa5 | 炸彈 → 淡出 → 6000；配對成功 → 兩張 state 0（**保持正面**）、配對數 +1；失敗 → 兩張 state 300；wait 5 → 0xfa6 |
| 0xfa6 | 已消除（state 0）< 14 且時間 > 0 → 潛水員待機 → 0xfa2；否則淡出 → 6000 |
| 0xfa7 | 時間到：淡出，wait 20 → 6000 |
| 6000 | 隱藏卡/HUD，`r = FUN_1007ad44()`，播結果；0x1771 完成或任意鍵（mask 0xf）→ 0x1772 → scene 0x5c |

**卡片狀態**（`FUN_1006a668`，0xfa2–0xfa4 每幀）**[確認]**：400 → timer = 4i+5 → 401（背面，倒數）→ timer 120 → 402（正面 120 frames）→ 300（timer 5）→ 301（仍正面，倒數 5）→ 100（背面、可選）。200 = 已翻開（正面）。0 = 已消除（不再更新，畫面保持正面）。（401 的倒數是「先比較再減」= 4i+6 frames、402 是 121 frames、301 是 6 frames）⇒ 開局記憶期：卡 i 約在 frame 4i+6 翻正、4i+133 蓋回；最後一張（i=14）約 frame 189 蓋回，早於第 200 frame 開放操作。

**遊戲規則 [確認]**（0xfa2）

1. `frame++`；frame ≤ 200 時不收輸入、**計時不跑**、游標隱藏。
2. 移動：A（`key_pressed(1)`）或 DIAL UP（`key_held(0x80)`）→ 下一張；DIAL DOWN（0x40）→ 上一張；循環 0..14，**跳過 state 0 與 200 的牌**；snd 0。
3. 翻牌：B 或轉盤按壓（`key_pressed(0xa)`）且已選 < 2，目前牌 state 100 → state 200；型別 7 → 炸彈旗標；否則記錄型別/位置、已選 +1。
   - 炸彈：潛水員 n3、snd 0xc、隱藏游標 → 0xfa3（之後結束，結果 0）。
   - 選滿 2 張：型別相同 → n1 + snd 10；不同 → n2 + snd 0xb；→ 0xfa3。
   - 只選 1 張：snd 1。
4. 計時：frame > 200 後每幀 −1（0xfa3/0xfa4 也扣）；`< 1` 且無炸彈 → snd 0x4a，wait 40 → 0xfa7。
5. 結果（`FUN_1007ad44`）：炸彈 或 配對 0 → 0；配對 7 → 3；6 → 2；1–5 → 1。三難度共用。

**版面**：`0x1009436c` = 15 × 15 × u32 型別（0–6 各兩張 + 一張 7），`index = ((DAT_18001674>>4)&7) % 5 + level*5`，離開場景時計數 = (計數+1) % 5。教學版面 `0x100946f0`。

**音效**：0 游標、1 選一張、10 配對、0xb(11) 不同、0xc(12) 炸彈、0x4a(74) 時間到、2 離開；script：6/1（READY）、59/52/52/48（結果）。

### 2.4 Game 03 — 三線道收集 `PSID_LABOGAME_03`（0x60）

Context `DAT_180141f8`（0x78 B）：`+0x28/0x2c 背景 node`、`+0x30 距離`、`+0x34 速度(s16)`、`+0x38 主角 node`、`+0x3c..0x42 主角碰撞框 {x, y, w, h}`、`+0x44 lane`、`+0x46 磁鐵狀態`、`+0x48 磁鐵計時`、`+0x4a 背景轉場步驟`、`+0x4c 物品池 6 個 node`、`+0x6c 終點 node`、`+0x70 收集數`、`+0x72 版面 index`、`+0x74 actor`。

**畫面**

| 物件 | 來源 | 位置 / 說明 |
|---|---|---|
| 背景 | 元件 0x4e8/0x4e9（sprite 2183）| x = 64 + (距離 mod 128) − 128·k（k=0,1，往右捲），y = 64。sprite 依速度：3 → 2183；5 → 2184（轉場）→ 2185；7 → 2186（轉場）→ 2187（表 `0x10091cc4`；`FUN_1008f854` 在相位 > 123/120 時推進轉場） |
| 主角 | 元件 0x4ee = script 0xa64（sprite **2188** 幀 0/1 每 10f）；磁鐵中 0x4ef = 0xa65（幀 2/3）| x = 108；y 依 lane：元件 0x4f1–0x4f3 的 y（42/76/110）−44 為 node 偏移，換線 tween 0.2 s（mode 6）|
| 物品 | 6 個 pool node（元件 0x4f7 sprite **2191**，32×32）| 幀 = 種類：0 椰子、1 鳳梨、2 香蕉、3 磁鐵（緞帶）；出生 x = −16、y = 42/76/110（元件 0x4f7+lane）；每幀 x += 速度；x ≥ 144（元件 0x4fb）回收 |
| 收集特效 | 元件 0x45e（T4，OBJ 48）@主角碰撞點 | |
| 終點 | 元件 0x4ff sprite **2194**（旗竿 32×128）@(−16,73)，每幀 x += 速度 | 距離 ≥ 4864 時出現 |
| 結果 | actor：r3 0xa66、r2 0xa6b、r1 0xa6d、r0 0xa6f（@0x505–0x508）| sprite 2183/2189/2190/2192/2193（廚師企鵝、果汁杯滿→空），另 `snd_play(0x10095768[r] = 48,52,52,59)` |

**狀態機**（`FUN_100514dc`）

| state | 內容 |
|---|---|
| 1000 | actor 播 `0x10094368` = 0xa57 @元件 0x4e6 |
| 0x3e9 | A/B 或完成 → 淡出；C 離開 |
| 0x4b0 | HOW TO PLAY + actor 播 0xa5c @元件 0x504（教學示範），wait 40 |
| 0x4b1 | A/B 或完成 → 淡出 → 0x4b2 wait 5 → 0x4b3 清教學 → 0x578 |
| 0x578 | actor 播 0x952 READY/GO，wait 40 → 0x579 淡出 → 0x57a wait 5 → 2000 |
| 2000 | 建背景、主角（lane 1）、6 物品；淡入 |
| 0x7d1 | **遊戲**（下述） |
| 0x7d2 | 主角 x tween 到 −96（1 s，mode 2）→ 0x7d3 等 tween 完 → 淡出 → 7000 |
| 7000 | 結果 script + 音效；0x1b59 完成或任意鍵 → 0x1b5a → scene 0x5c |

**遊戲規則 [確認]**（每幀順序：速度 → 距離 += 速度 → 背景 → 輸入 `FUN_1007cee4` → 物品 `FUN_1008f344` → 終點 `FUN_1008f27c` → 產生 → 結束判定）

1. 速度：距離 < 1152 → 3；< 2816 → 5；否則 7。變速時 snd 0x12、背景轉場步驟 +1。距離到 4992 約 1028 frames（≈ 51 s）。
2. 換線：A（`key_pressed(1)` 且非同時 B）或 DIAL UP（`key_held(0x80)`）→ lane −1（往上）；B（且非 A）或 DIAL DOWN（0x40）→ lane +1；範圍 0..2，有變動 snd 1。碰撞框立即換到新 lane（畫面 tween 4 frames）。
3. 產生：距離跨過 64 的倍數時 `row = 距離/64`（0..75），讀 `layout[row] = {s8 lane, s8 kind}`，lane ≥ 0 則從 pool 取空 node（6 個都滿則丟棄）。距離 ≥ 4864（0x1300）建終點。
4. 碰撞：主角框 = T3 元件 0x4f0：x = 96 − 8 = 88、w = 8、h = 32、y = lane_y − 20。物品框（T3 0x4fa，16×16）`left = x − 8, top = y − 8`（被磁鐵吸引中 32×32、`left = x − 16`）。條件 `left − 8 < 88 < left + w` 且 `top − 32 < py < top + h` ⇒ 只與同 lane、物品中心 x ∈ (80,104)（吸引中 (72,112)）相撞。
   - 種類 3：磁鐵 ON（`+0x46 = 1`，snd 0x3b）；下一幀換成 0xa65，**61 frames** 後換回。
   - 其他：收集數 +1、snd 10。物品往右飛走並淡出（0.2 s）。
5. 磁鐵中：所有種類 ≠ 3 且 x ≥ 10 的物品被標記吸引，0.1 s 內朝主角位置偏移（極座標 tween，`FUN_1000371c` 取角度）**[推測：細節是視覺吸附，判定靠放大後的框]**。
6. 結束：距離 ≥ 4992 → 結果、snd 0x3d → 0x7d2。C（距離 < 4992 時）→ 離開。
7. 結果（`FUN_1007adbc`）：收集 ≥ 30 → 3、≥ 16 → 2、≥ 1 → 1、0 → 0（三難度共用）。版面可收集數：layout 0/1 = 39、2–5 = 30 ⇒ 難度 1/2 要 3 分必須全收。

**版面**：`0x10093984` = 6 × 76 × `{s8 lane, s8 kind}`（lane −1 = 空列），`index = ((DAT_18001674>>7)&1) | level<<1`，離開場景時 bit7 翻轉。

**音效**：1 換線、0x12(18) 加速、10 收集、0x3b(59) 磁鐵、0x3d(61) 到終點、2 離開、結果 48/52/52/59。

---

## 3. 資料表

| 表 | 位址 | 格式 | 筆數 | 輸出 |
|---|---|---|---|---|
| 00 版面指標 | RW 0x18000a28（初值在 PRAM 0xe6a0 LZ） | u32[12]：0x100a013e,0x100a02b2,0x100a0426 / 0x100a0c78,0x100a0db0,0x100a0ee8 / 0x100a0696,0x100a0792,0x100a088e / 0,0,0 | 9 | `lab00.json boards` |
| 00 版面 | 上列 | N × 6 × s16（非 0 = 冒出） | N = 31/26/21 | 同上（`rows` 原值、`rows_read` 實際讀的 24 列） |
| 00 列數 | 0x1009de92 | s16[3] | 31,26,21 | 只用來算未使用的 +0x2e |
| 00 洞元件 | 0x100a0994 | {u16 hole, cursor, alt} ×6 | 6 | `holes` |
| 00 門檻 | 0x100a13c8 | s16[3] | 10,8,1 | |
| 01 地圖 | 0x10091fec + k·0x174（k=0..8） | 10 × {u16 seg, u16 style} | 9 | `lab01.json maps` |
| 01 教學地圖 | 0x10092d6c | 1 × {30, 20} | 1 | `tutorial_map` |
| 01 段表 | 0x1009b168 | 32 × {u32 n, u32 ptr → {s16 x, s16 y}[n]} | 32 | `segments` |
| 01 style 表 | 0x10093574 | 22 × {u32 n, u32 ptr → u16[n]} | 22 | `styles` |
| 01 平台元件 | 0x1009b268 / 0x1009b272 | u16[5] 碰撞 / sprite 元件 | 5 | `platform_types` |
| 01 時限 | 0x1009572c | u32[3] 秒 | 100,90,80 | |
| 01 結果 script | 0x10095740 | u16[4] | 0x9be,0x9b8,0x9b6,0x9ab | |
| 02 版面 | 0x1009436c | 15 × 15 × u32 | 15 | `lab02.json boards` |
| 02 教學版面 | 0x100946f0 | 15 × u32 | 1 | |
| 02 時限 | 0x10093fd4 | float[3] 秒 | 50,40,30 | |
| 02 潛水員反應 | 0x100a008a | {u16 script, u16 元件} ×4 | (0xa4b,0x4e1)(0xa48,0x4de)(0xa49,0x4df)(0xa4a,0x4e0) | |
| 03 版面 | 0x10093984 | 6 × 76 × {s8 lane, s8 kind} | 6 | `lab03.json layouts` |
| 03 lane 元件 | 0x100a00b2 | u16[3] | 0x4f1,0x4f2,0x4f3 | |
| 03 背景 sprite | 0x10091cc4 | u16[5] | 2183–2187 | |
| 03 開場 script | 0x10094368 | u16[2] | 0xa57, 0xa5c | |
| 03 結果音效 | 0x10095768 | u16[4] | 48,52,52,59 | |

---

## 4. 新命名

```
100202cc lab00_init f
100203b0 lab00_update f
10020284 lab00_exit f
100500d0 lab00_state f
1007847c lab00_play_tick f
100675ac lab00_holes_tick f
100709d0 lab00_setup f
10082084 lab00_cleanup f
100841a0 lab00_set_cursor f
100204d0 lab01_init f
10020578 lab01_update f
10020480 lab01_exit f
100205dc lab01_state f
100208cc lab01_intro f
100206ec lab01_tutorial f
10020a30 lab01_play f
10021178 lab01_result f
10004e90 lab01_player_new f
10004efc lab01_player_input f
100051a8 lab01_player_reset f
100051dc lab01_player_cb f
10066568 lab01_hud_time f
10009cdc lab01_spawner_new f
10009d98 lab01_spawner_cb f
10009d74 lab01_spawner_reset f
10009c34 lab01_bg_scroll f
1002831c lab01_scroller_new f
10028498 lab01_scroller_cb f
1002bcb0 lab01_plat_new f
1002bd90 lab01_plat_draw f
1002be10 lab01_plat_hit f
1002c148 lab01_plat_tick f
1002bf8c lab01_plat_find f
100828b0 lab01_auto_target f
1006aa58 lab01_height_score f
1007aa98 lab01_gravity f
1001b760 lab01_tut_new f
1001b8bc lab01_tut_running f
1001b940 lab01_tut_ai f
10024558 lab01_gauge_new f
100245ec lab01_gauge_cb f
100213e8 lab02_init f
1002149c lab02_update f
10021324 lab02_exit f
10050954 lab02_state f
1007073c lab02_setup f
10069ed0 lab02_deal f
1006a47c lab02_cursor f
1006a668 lab02_cards_tick f
1006a8fc lab02_timer f
10069d94 lab02_hide_cards f
1007ad44 lab02_result f
10071014 lab02_result_anim f
100711d8 lab02_tut_setup f
10083900 lab02_diver_react f
1002158c lab03_init f
10021624 lab03_update f
10021518 lab03_exit f
100514dc lab03_state f
1007cee4 lab03_input f
1008f344 lab03_items_tick f
1008f854 lab03_bg_tick f
1008f760 lab03_player_lane f
1006a9b8 lab03_item_spawn f
100842b0 lab03_goal_new f
100842dc lab03_items_new f
1008434c lab03_player_new f
1008438c lab03_bg_new f
1008f27c lab03_goal_tick f
1007adbc lab03_result f
10023f20 snd_play_checked f
10005ea8 key_pressed f
10006150 key_held f
10006140 dial_delta f
1006953c task_set_state f
10069558 task_wait f
1006956c task_set_substate f
1007eea0 ui_elem_create f
1007f5c4 ui_sprite_create f
1007f240 ui_text_create f
10066abc tween_set f
10066b34 tween_start f
100669e8 tween_eval f
100a0994 lab00_hole_elems l
100a13c8 lab00_thresholds l
10091fec lab01_maps l
1009b168 lab01_segments l
10093574 lab01_styles l
1009572c lab01_time_limit l
1009436c lab02_boards l
10093fd4 lab02_time_limit l
10093984 lab03_layouts l
18000a28 lab00_board_ptrs l
```

---

## 5. 未解問題

1. Game 00 教學頁 0x3ec 的自動前進條件（double 比較 90.0 的 thunk `0xccfa/0xd5d6`，參數被 Ghidra 吃掉）。
2. Game 01 高度計標記元件 id（`FUN_10024558` 呼叫時 r2 被反編譯省略，推測 0x410）；自動操作旗標 `+0x177c` bit5 由誰設定（教學 widget 內部？）；`+0x177c` bit1（暫停計時）沒有寫入點。
3. Game 03 磁鐵吸附的實際軌跡（`FUN_1000371c` 角度、`FUN_10009b84`/sin 表、tween 0x5c 0→0xa00 over 0.1 s）；收集特效 OBJ 48 的外觀（`FUN_10022f38` 物件表）。
4. 各 sprite 錨點：UI 元件座標視為中心（00 的洞對稱驗證），但 02 卡片 x=11..103 偏左（中心則左緣 −1、右側留 13 px），需實機截圖確認。
5. 結果畫面 scene 0x5c 的版面（屬共用部分，未展開）；`scene_set_next(0x2d–0x30)` 的目標畫面。
6. 難度 2 第 4 次遊玩讀 NULL 版面時的實際冒出序列取決於 PRAM 0x0 內容（向量表），重製時建議改成循環 0..2。

---

## 摘要（≤15 行）

1. 四個 Lab 遊戲（scene 0x5d–0x60）全是固定版面＋固定角色，**不畫玩家寵物**；畫面由 UI 元件與 motion script 組成，可直接重用 script 直譯器。
2. 00 打地鼠：500 frames 固定；每 20f 讀一列 6×s16（24 列），洞狀態 0→1(20f)→2(60f)→0；A 移游標、B 餵；打睡著的洞立即結束；10 分結束；門檻 10/8/1。
3. 00 版面指標在 RW 0x18000a28（從 PRAM LZ 解出），9 張 31/26/21 列；`&3` 輪替與難度 2 只有 21 列兩個讀過頭 bug 已照實輸出。
4. 01 往上跳：Verlet 物理（跳 14、彈簧 25、重力 1.4、左右 5.6，x∈[16,112]），落地 4 幀後自動跳（A/B 可提早），碎裂平台立即再跳；移動平台 ±32 px 正弦、週期 60f。
5. 01 地圖 = 10 段×640 px，平台型別 = style[j % len]；時限 100/90/80 s；掉落回最高平台；分數 = 高度×3000/6400，3 分只能到頂；`lab01.json` 已展開全部平台。
6. 02 翻牌：15 張（7 對＋炸彈 7），記憶期 200f（卡 i 在 4i+5 翻正 120f）；B/轉盤按壓翻牌，A/轉盤移動；炸彈 → 0 分；時限 50/40/30 s（記憶期不計）；7/6/1–5 對 → 3/2/1。
7. 03 三線道：速度 3/5/7（1152/2816），終點 4992（≈1028f）；每 64 距離一列 {lane,kind}；kind 3 = 磁鐵 61f；碰撞只看同 lane、x∈(80,104)；門檻 30/16/1，難度 1/2 只有 30 個可收。
8. 結果門檻三難度共用；難度只換版面（與 01/02 時限）。
9. 資料：`docs/spec/data/minigames/lab00–03.json`；腳本 `docs/spec/tools/extract_minigames_a.py`（可選 pram.bin 交叉驗證 RW 指標表，已通過）。
10. 未解：00 教學自動前進條件、01 高度計標記 id、03 磁鐵吸附軌跡、卡片錨點需實機確認。
