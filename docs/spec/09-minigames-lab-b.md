# 09b — Lab 小遊戲 04 / 05 / 05_EVENT / 06、GAME 選單、共用結果畫面

範圍：Lab 小遊戲 04（CURLING 打磚塊找獎品）、05（PUNCHING METEORITE! 三線道擋隕石）、05_EVENT（流星事件，scene 99）、06（CLOUDY COIN COLLECTION 橫向捲軸收金幣），以及 Lab GAME 選單（scene 0x2c）與共用結果畫面（scene 0x5c）。目標：在 128×128、20 fps 上用**原始圖像**重製。
共通背景見 `05-content.md` §2.11；UI 元件 / 動畫腳本格式見 `08-care-ui.md` §3、`07-animation.md`；sprite 格式見 `04-assets.md` §2.3。
時間單位：**frame（20 fps）**。座標：螢幕像素，原點左上。定點數：16.16（`0x10000` = 1.0）。輸入：`key_pressed(m)` = `FUN_10005ea8`（本 frame 按下）、`key_held(m)` = `FUN_10006150`（按住）、`dial_delta()` = `FUN_10006140`；位元 1 = A、2 = B、4 = C、8 = 轉盤按壓、0x40/0x80 = 轉盤下/上一格（held 遮罩）。
資料：`docs/spec/data/minigames/lab04.json`、`lab05.json`、`lab06.json`、`labmenu.json`、`lab_math.json`，由 `docs/spec/tools/extract_minigames_b.py` 產生（`python3 -I extract_minigames_b.py DUMP.bin [pram.bin] [OUTDIR]`；game 05 的隕石表在 RW data，需解密後的 `pram.bin`）。

---

## 1. 摘要

- 四個遊戲都是「scene ctx + state 機」：`task_set_state`（`FUN_1006953c`）切 state、`task_wait`（`FUN_10069558`）延遲 n frames 再跑 state、`+8` 是子 state（`FUN_1006956c`）。物件（sprite / 群組 / 動畫）由 UI 元件 id 生成，物理寫在**物件 callback**（msg 8 = 每 frame 更新、msg 11 = 繪製前），由 `layer_update`（`FUN_1000a268`）每 frame 呼叫。
- **時間軸**：每個 layer 有自己的時鐘，每 frame +`0xccc`（= 0.05 s，`FUN_10009ff4`）；tween 長度以**秒（16.16）**給（`FUN_10066b34`/`FUN_10066968`：起點 = 目前時鐘、終點 = 起點 + 長度）。例：0x8000 = 0.5 s = 10 frames。
- **沒有亂數影響玩法**：版面、發射角、獎品磚、隕石列、金幣路線都由遊玩計數器 `DAT_18001674`（及 `DAT_18001678`）的位元欄位輪流選；唯一亂數（`FUN_1002a5e4`）只用在 game 06 被撞飛物件的視覺彈跳。
- **Game 04 CURLING**：紅色冰壺（球，3.0 px/f）在 [12,116]×[12,…] 牆內反彈，玩家控制黃色冰壺（板子）。15 塊冰磚中一塊藏獎品角色；擊中獎品磚 → 球減速停止 → 下一回合（共 5 回合，磚重置）。漏球或每回合限時（1200/1000/800 frames）用完 → 結束。結果 = 過關回合數（5→3、3–4→2、1–2→1、0→0）。
- **Game 05 METEORITE**：40 列 × 3 線道固定隕石表，每 20 frames 讀一列；隕石沿直線在 3.0/2.5/2.0 秒落到線道終點；按 A/B/C 在 0/1/2 線出拳（拳套 19 frames 有效、擋一顆就收）。擋下 +1 分；被砸 −1 命（3/2/1 命）。frame 880 或沒命結束；分數 ≥25→3、≥20→2、≥6→1。
- **Game 05_EVENT**：同一份 code，1 命、固定表、不能離開、沒有教學；結束後不進結果畫面，直接 ±10 心情（失敗另加受傷、擺設 −1），回 HOME。
- **Game 06 CLOUDY COIN**：寵物（固定角色「仙人」坐雲）固定在 x=104，世界往右捲動（4.0 px/f + 俯衝加成）。按住 A 上升（−0.24 px/f²），放開下落（+0.24）；下落中每 frame 捲動速度 +0.1。撞障礙物 → 擊退約 2 秒（失去時間、捲動 2.5 px/f）＋ 20 frames 無敵。終點距離 6500，限時 2000 frames；金幣數 ≥40→3、≥25→2、≥1→1。
- **GAME 選單**：7 個遊戲橫向輪播（A/轉盤上 = 下一個、轉盤下 = 上一個、B/按壓 = 確定、C = 返回），解鎖需 planet rank ≥ {1,4,4,1,4,1,1}；難度 EASY/NORMAL/HARD 需 rank ≥ {1,6,9}；不足 → 「AVAILABLE FROM LEVELn」畫面。
- **結果畫面**：顯示星星 0–3 顆（星星從上方縮放落下）、獲得點數 `[level][result]` = 10/100/150/250、10/200/300/500、10/300/450/800，並 `points_add`；rank 3 → mission 13 計數 +1。100 frames 或任意鍵離開，回選單並停在剛玩的遊戲上。

---

## 2. 詳細規則

### 2.0 共用

| # | 規則 | 證據 | 標記 |
|---|---|---|---|
| C1 | 場景結構：init 建 ctx（`FUN_10018f28` 配置）、`layer_create`（`FUN_1000a158`）、`FUN_10069590`（task 初始化）、`scene_set_next(返回場景)`；update = `if (!task_waiting) { if (ctx+0x14 & 1) tick(); layer_update(layer, ctx+0x1c); }` | `100216e8`、`1002180c` 等 | [確認] |
| C2 | 返回場景（init 時設）：game 04 → 0x31、game 05 → 0x32、game 06 → 0x33（= `PSID_LBMENU_GAME04/05/06`，GAME 選單的別名場景，游標停在該遊戲）。05_EVENT 結束時改設 0x1c HOME | `100216e8`、`100218d0`、`10021f6c`、`10058b98` state 0xdae | [確認] |
| C3 | 離開流程 state 0x7fe0/0x7fe1：`idle_reset`、`FUN_10025cb4()`、`fade_set(0xff, …, 1, 4)`（以目前色淡出）→ 0x7ff0：`ctx+0xc = 1`（場景結束） | 各 tick | [確認] |
| C4 | 進入時 `FUN_10025cb4(1)`（關閉閒置/自動返回計時 [推測]），離開時 `FUN_10025cb4(0)` | 各 tick state 0 | [推測] |
| C5 | 淡入 `fade_set(0,0x20,0,4,0)`、淡出 `fade_set(0,0,1,4,0)`；`fade_start(0, cur, dir, 4, 0)`（`FUN_1007b3b0`）從目前亮度開始。速度 4（約 8–16 frames，見 08-care-ui U2） | `FUN_1007b40c`、`FUN_1007b3b0` | [確認]／長度 [推測] |
| C6 | READY 動畫：anim 0x952 放在元件 0x3c8（64,64）：bg sprite 0x811 → 60 frames 後子動畫 0x953：SND 6、sprite 0x819（"READY"，64×32）30 frames → SND 1、sprite 0x81a（128×96，"GO" [推測]）、上下抖動。game 04/05/06 共用；各遊戲在播放後 `task_wait(50)` | anim 0x952/0x953 解碼 | [確認] |
| C7 | 教學標題列：元件 0x3e5（sprite 0x820，128×24，(64,14)）＋ 0x3e6 文字 string 0x32d = "HOW TO PLAY"；手指提示動畫 0x95d（sprite 0x81b） | `1006fa6c`、`100311f8`、`FUN_10052024` state 2000 | [確認] |
| C8 | Sprite 繪製錨點：左上 = `(x + ox − w/2, y + oy − h/2)`，`ox/oy` = sprite header 的 s8 偏移（**y 向下為正**；04-assets 表寫「上正」與 blitter 不符）。下文 sprite 尺寸附 `origin=(ox,oy)` | PRAM `FUN_00008e3c`：`DAT_18014370 = y + (oy + …) − h/2` | [確認] |
| C9 | 數字 HUD：TEXT 元件，格式字串 `"%d"`（0x100a4129），空字串 0x100a3e9b | `FUN_10064f1c` 呼叫點 | [確認] |
| C10 | 角度：16-bit（0x10000 = 一圈），`cos16(a) = sin_tbl[((a>>6)+0x100)&0x3ff]<<4`、`sin16(a) = sin_tbl[(a&0xffff)>>6]<<4`（`sin_tbl` = 0x1009a968，1024 × s16 Q12）；`atan2_16(y,x)` = 12-bit 象限二分查 tan 表 0x1009cbd4（1025 × s32 Q12，取最接近）再 `<<4`。兩表已輸出到 `lab_math.json` | `FUN_10009b84`、`FUN_10031b74`、`FUN_1000371c`/`10003734`/`1000382c` | [確認] |

### 2.1 Game 04 — CURLING（scene 0x61 `PSID_LABOGAME_04`）

名稱 string 818「CURLING／クラッシュ！ひんやりカーリング」。ctx = `DAT_180141fc`（0x6c B）。

#### 2.1.1 畫面

| 物件 | 元件 | sprite（尺寸/幀/origin） | 位置 | 備註 |
|---|---|---|---|---|
| 背景 | 0x50b | 0x893 冰面星空（128×128） | (64,64) | init 建立 |
| 板子（黃冰壺） | 0x512 | 0x895（24×24，1 幀，origin (0,−8)） | x 可動，y=124 | callback `0x10080d20`；朝向 = 水平翻轉（速度 >0 → flip） |
| 板子 x 範圍 | 0x510 / 0x512 | — | x ∈ [16, 112] | 讀元件定義的靜態 x |
| 球（紅冰壺） | 0x50e | 0x894（24×24，1 幀，origin (0,−16)） | 每回合從 (88,124) 出發 | callback `0x1006bd24`；cos>0 → flip |
| 冰磚 ×15 | 0x515–0x523 | 0x896（32×32，origin (0,−16)） | x ∈ {12,38,64,90,116} × y ∈ {32,56,80}（編號 1–15 由左上起橫向） | callback `0x1006c4d4`；出現 tween（`+0x9c` 由 v−0x100 → v，0.1 s） |
| 破磚特效 | anim 0xa8a | 0x898（64×32） | 磚位置，z=15 | |
| 獎品角色 | 0x513（T7 角色元件） | chr = `0x10093320[(cnt>>14)&0x7f]`（49 選 1，見 §3） | 獎品磚位置 | 初始縮放 0（隱藏）；callback `0x10007b84` |
| 獎品（揭曉後） | 0x514（T7） | 同 chr | 同上 | callback `0x10066440`；元件 p3 0x1e/0x21 推測為角色 motion 30/33 [推測] |
| 倒數圖示 | 0x50c | 0x80f（16×24，時鐘） | (120,11) | 剩餘 <10 秒才顯示，之後不再隱藏 |
| 倒數數字 | 0x406 | TEXT 樣式 0x1081 | (120,11) | 顯示 `(limit − t) / 20`（只在 0..9 時寫入） |
| 開場 | anim 0xa73 | 背景 0x893 + 固定角色 chr 0x99（motion 42/43/38 揮桿）＋ 冰壺 0x895 ＋ 15 塊磚 | (64,64) | 約 90 frames，`EVT −1` 結束 |
| 結果動畫 | anim `{0xa87,0xa85,0xa7f,0xa78}[result]` | 背景 + 固定角色（0x99/0x98/0x9e–0xa1）＋ 星星 0x897 | (64,64) | 腳本內播 SND 0x30/0x34/0x34/0x3b |

- 遊玩中**沒有寵物**；開場/結果動畫的角色是腳本內固定 chr（0x99 等，用 `actor_play` = `FUN_10000854` 播放，**沒有**套用「chr 0 → 自己寵物」替換）**[確認]**。chr 0x99/0x98/0x9e–0xa1 依 ChrID 推測為冰河角色（irukatchi/azaratchi/lemmingtchi/raichotchi/yukiusatchi/nerinetchi）**[推測]**。
- 視覺注意：球的 logic 座標 y 比畫面中心低 16（origin −16），板子低 8，磚低 16。碰撞全部用 logic 座標（下文），重製時照 logic 座標算、照 C8 畫即可還原原版的視覺偏差。

#### 2.1.2 State 機（`labo04_tick` 0x10052024）

| state | 動作 | 下一步 |
|---|---|---|
| 0 | `FUN_10025cb4(1)`；actor 播 anim 0xa73 @ (64,64)；淡入 | 100 |
| 100 | A 或 B：SND 1、淡出、wait 10 → 101；C：SND 2 → 0x7fe0；開場動畫結束：淡出、wait 10 → 101 | |
| 101 | 停 actor | 2000（教學） |
| 2000 | 教學場景：標題列 0x3e5/0x3e6、球於 (110,114) 角度 0x6000、板子 x=88；wait 10 | 0x7d1 |
| 0x7d1 | wait 30、淡入 | 0x7d2 |
| 0x7d2 | `labo04_tutorial` 0x1005fd94（見下）；任何時候 A/B：SND 1、淡出、wait 10 → 500；C：SND 2 → 0x7fe0 | |
| 500 | 刪教學物件 | 501 |
| 501 | READY 動畫 0x952（C6）、淡入、wait 50 | 502 |
| 502 | 淡出、wait 5 | 503 |
| 503 | 建倒數圖示 0x50c（隱藏）、倒數文字 0x406（空字串） | 1000 |
| 1000 | 清輸入（`FUN_10006110`）；建球：位置 (88,124)、速度 0、角度 = `angle16[launch_index[level][b][round]]`；板子 x=88、顯示 | 1001 |
| 1001 | 建 15 塊磚與獎品物件（`labo04_bricks_create` 0x1006fc18）；wait 10 | 1002 |
| 1002 | wait 10、淡入 | 1003 |
| 1003 | 球速 = 3.0；SND 9 | 1004 |
| 1004 | `labo04_play` 0x100788d0 每 frame | → 3000（失敗）/ 0x44c（獎品揭曉 30 frames 後，由 0x514 callback 設） |
| 0x44c | 等球速 == 0 | 0x44d |
| 0x44d | round++、計時 t=0、計時旗標 `+0x35`=1、倒數文字清空；wait 10；round < 5 → 淡出、1000；否則 3000 | |
| 3000 | 淡出、wait 20、`result = labo04_result()` 寫入 `DAT_18001c68` | 3001 |
| 3001 | 刪除所有物件；actor 播結果動畫 `0x10095748[result]` @ (64,64)；SND `0x10095770[result]` = {48,52,52,59}；淡入 | 3002 |
| 3002 | 動畫結束或任意鍵（0xf） | 3003 |
| 3003 | `scene_set_next(0x5c)`（結果畫面） | 0x7fe0 |
| 0x7ff0 | 更新計數器（§2.1.6），場景結束 | |

教學 `labo04_tutorial`（子 state）**[確認]**：0：顯示手指 0x525（(24,108)）；1：板子以 A 相同加速度往左 11 frames；2：等 11 frames、顯示手指 0x526（(64,108)）；0x5dc：往右 11 frames；0x5dd：等 7 frames；2000：球速 3.0，角度 = `atan2(−sin, cos)`（往下）；0x7d1：等球停 → 淡出 → state 500。教學中球不會掉出底部；球碰磚即開始減速；碰板子時用固定角度反彈 **[推測：參數被反編譯省略]**。

#### 2.1.3 輸入與板子（`labo04_paddle_input` 0x10027c14，只在計時旗標 `+0x35` ≠ 0 時呼叫）

速度 v（16.16，ctx+0x28）**[確認]**：
1. 按住 A：若 v > 0 → v = 0；v −= 1.0；`v = (v < −4) ? v : −5`（**比較的是原始整數 −4/−5，不是 −4.0**，所以永遠取 v；而 v = −5（原始值）只在 v 為 0 附近時才會發生，實際不會）→ **每 frame 加速 1 px/f，沒有上限**（已知 bug，§5）。
2. 按住 B：若 v < 0 → v = 0；v += 1.0；`v = (v < 5) ? 5 : v`（同理，實際就是 v）。
3. A、B 同時按住 → v = 0；A、B 都沒按 → v = 0。
4. 本 frame 位移 = v；若 `dial_delta() ≠ 0` → 位移 = `dial_delta() × 5.0`（不改 v）。
5. x = clamp(x + 位移, 16, 112)；位移 ≠ 0 時 flip = (位移 > 0)。
6. C 按下：`+0x34`=1（離開旗標）、SND 2 → 0x7fe0（不給結果、不給點數）。

#### 2.1.4 球物理（`labo04_ball_cb` 0x1006bd24，每 frame）

球資料（0x18 B）：`[0]` 速度、`[1]` 角度、`[3],[4]` 上一 frame 的 x,y、`[5]` byte「已擊中獎品」**[確認]**：
1. 若「已擊中獎品」或計時旗標 = 0：速度 −= 0x2666（0.15 px/f²），下限 0 → 從 3.0 約 20 frames 停下。
2. `prev = (x,y)`；`nx = x + cos·spd`、`ny = y − sin·spd`（角度 0x4000 = 正上方）。
3. nx 夾在 [12, 116]；ny 下限 12（沒有上限）。
4. 若 nx ≤ 12 或 nx ≥ 116 → cos = −cos；若 ny ≤ 12 → sin = −sin。每次反彈播 SND `0x1009390e[0]` = 0（獎品揭曉物件 0x514 顯示後不播）。
5. 若 state ≠ 0x7d2（非教學）且 ny ≥ 158 且未按離開 → **漏球**：計時旗標 = 0、速度 = 0。
6. 角度 = `atan2_16(sin, cos)`；寫回位置。

板子碰撞（`labo04_paddle_cb` 0x10080d20，板子物件每 frame）**[確認]**：
- 只在球往下（sin < 0）時檢查。條件：`prev_y ≤ py + 12` 且 `py + 2 ≤ ball_y`（球的 logic y 越過 py+2 這條線）。
- `t = (py + 2 − prev_y) / (ball_y − prev_y)`；`hx = prev_x + t·cos`（**用單位向量 cos 而不是 Δx**，所以 hx ≈ prev_x；原版行為）。
- `c = clamp(hx, px − 14, px + 14)`；若 `(hx − c)² ≤ 36`（板子半寬 14 + 6 px 容差）→ 反彈：
  - 球 x = hx（y 不變）；
  - c 在左端 → 新 cos = −|cos|；在右端 → +|cos|；否則新 cos = cos + (hx − px)/28；
  - 新 sin = |sin|（往上）；角度 = atan2_16(新 sin, 新 cos)（自動正規化）；SND 0。
- 否則（打偏）不處理，球繼續往下 → 漏球。

磚碰撞（`labo04_brick_cb` 0x1006c4d4，每塊磚每 frame；只在磚已完全出現 `byte +0x9f == 1` 時）**[確認]**：
- `bits = circle_rect_hit(bx−10, by−10, 20, 20, ball_x, ball_y+12, r=12)`（`FUN_1007c100`：把圓心夾到矩形取最近點，距離² < r² 時回傳 1=左、2=右、4=上、8=下 邊；圓心完全在矩形內回傳 0 = 不算）。
- bits ≠ 0：SND `0x1009390e[1]` = 10；若此磚 = 獎品磚 → 顯示獎品角色 0x513（縮放 1.0）、球「已擊中獎品」=1；磚移除 callback、縮放 tween → 1.2、alpha → 0（0.3 s）、播破磚特效 0xa8a。
- 反射：bit1 → cos = −|cos|；bit2 → +|cos|；bit4 → sin = +|sin|；bit8 → sin = −|sin|。多塊磚同一 frame 依序累積。

#### 2.1.5 計時、回合、結束（`labo04_play` 0x100788d0）

- 計時 t（ctx+0x20 s16）：旗標 `+0x35` ≠ 0 時每 frame +1；t ≥ `limit[level]` = {1200, 1000, 800}（60/50/40 秒）→ 旗標 = 0（板子凍結、球開始減速）**[確認]**。
- 倒數顯示：`s = (limit − t)/20`，0 ≤ s ≤ 9 時文字寫 s、圖示 0x50c 顯示。
- 失敗判定：球速 < 1（raw ≤ 0）且「未擊中獎品」→ SND 0xd、wait 20 → state 3000。所以漏球、或時間到後球停下而沒打到獎品 → 結束。時間到後球減速的 20 frames 內仍可撞到獎品磚而過關 **[確認：碰撞仍有效]**。
- 獎品流程：擊中 → 獎品 0x513 立即出現；第 11 frame（`DAT_18001d14` > 10）SND 0x34、顯示 0x514、計時旗標 = 0、刪 0x513；0x514 顯示滿 30 frames → state 0x44c（若未按離開）→ 等球停（擊中後約 20 frames）→ 下一回合。
- 每回合都重建 15 塊磚、重新選發射角與獎品磚；獎品角色整場相同。
- **結果** `labo04_result` 0x1007ae24：回合數 r（ctx+0x24）：r ≥ 5 → 3；3–4 → 2；1–2 → 1；0 → 0 **[確認]**。

#### 2.1.6 固定資料與選擇（全部 `lab04.json`）

- 遊玩計數器 `cnt = DAT_18001674`：`b = (cnt>>9)&3`（0..2）、`q = (cnt>>11)&7`（0..4）、`p = (cnt>>14)&0x7f`（0..48）。離開場景時（不論完成或按 C）`b = (b+1)%3`、`q = (q+1)%5`、`p = (p+1)%49` **[確認]**（state 0x7ff0）。
- 發射角：`angle16[ u16 @ 0x1009bb5c + level·30 + (5b + round)·2 ]`，`angle16 = float @ 0x1009bb50 × 65536` = {0x6000（135°，左上）, 0x4000（正上）, 0x2000（45°，右上）}。
- 獎品磚：`u16 @ 0x10091d30 + level·100 + (5q + round)·2` = 磚編號 1–15（實際只出現 1–10，即上兩排）。
- 獎品角色：`u16 @ 0x10093320[p]`，49 個 chr（63, 115–164 跳過 131、148；ChrID 解讀下 131/148 是 SpeciesRank 5 的特殊角色 **[推測]**）。

#### 2.1.7 音效

| id | 時機 |
|---|---|
| 1 / 2 | 確定（跳過開場/教學）/ 離開 |
| 9 | 每回合發球 |
| 0 | 牆/板子反彈（獎品揭曉後靜音） |
| 10 | 撞磚 |
| 0x34 (52) | 獎品揭曉 |
| 0xd (13) | 失敗（漏球/時間到） |
| 48/52/52/59 | 結果 0/1/2/3（state 3001，結果動畫內也各自再播一次 0x30/0x34/0x34/0x3b） |
| 6, 1 | READY 動畫 |

### 2.2 Game 05 — PUNCHING METEORITE!（scene 0x62 `PSID_LABOGAME_05`）

名稱 string 808。ctx = `DAT_18014248`（0x17c B）。`event = (前一場景 == 99)`（`FUN_1000274c`），scene 0x62 與 0x63 共用 init/update/tick。

#### 2.2.1 畫面

| 物件 | 元件 | sprite | 位置 | 備註 |
|---|---|---|---|---|
| 背景 ×2 | 0x48b / 0x48c | anim 0x95f = sprite 0x815（星空 128×128，4 pal）、anim 0x960 = 0x816（128×128） | (64,64) | |
| 主角（一般） | 0x48d 座標 | sprite **230**（64×64，5 幀，origin (4,−1)），`obj_sprite_create(…, 0xe6, frame 0)` | (62,122) | 固定角色，非寵物；被砸 → 幀 2（驚嚇），20 frames 後回幀 0 |
| 主角（事件） | 0x48d（T7 角色元件，chr 1） | 角色模型 | (62,122) | 被砸時角色 `inst+6` = 4；推測為自己的寵物 **[推測]** |
| 拳套（線 0/1/2） | 0x492 / 0x493 / 0x494 | 0x864（拳套，32×32） | (32,94) / (64,88) / (96,94) | 按鍵時建立/顯示 |
| 拳套判定框 | 0x490（T3） | — | 相對拳套 (0,−12)，w 32、h 8 | 只用 y：上緣 = 拳套 y − 12 |
| 隕石 | 起點 0x497 / 0x49b / 0x49f | 0x866（32×32，2 幀：褐色/燒紅） | (−15,−6) / (64,−16) / (143,−6) | callback `0x1005f750` |
| 隕石終點 | 0x498 / 0x49c / 0x4a0 | — | (40,104) / (64,96) / (88,104) | 直線 tween |
| 隕石判定框 | 0x496（T3） | — | w 26、h 24 | 只用 h/2 = 12 |
| 擋下特效 | 0x491 | anim 0xa24（sprite 0x863） | 拳套位置 + (0,−16) | |
| 擊飛目標 | 0x49a / 0x49e / 0x4a2 | — | (−40,64) / (108,−36) / (162,64) | 1.5 s 飛出畫面 |
| 爆炸 | 0x499 / 0x49d / 0x4a1 | anim 0xa25（sprite 0x863） | (46,112) / (64,108) / (80,112) | 被砸時 |
| 生命愛心 | 0x4a3 / 0x4a4 / 0x4a5 | 0x865（16×16，2 pal：pal 1 = 空心） | (95,121) / (107,121) / (119,121) | 只建立 lives 個；失命時 `heart[剩餘命數]` 改 pal 1（由右往左） |
| 教學手指 | 0x4a6 / 0x4a7 / 0x4a8 | anim 0x95d | (24,108) / (64,108) / (104,108) | |
| 開場 | anim 0xa1d | 背景 + 隕石 0x866/0x867/0x868 + chr 0xa motion 1 | (64,64) | |
| 結果動畫 | `{0xa2c, 0xa2a, 0xa28, 0xa26}[result]` 於元件 0x4a9 (64,64) | 背景 + chr 0xa（motion 19/3/2/2）＋ SND 0x30/0x34/0x34/0x3b | | |
| 事件開場/結局 | anim 0x6a9 / 成功 0x6aa / 失敗 0x6ab | 用 `FUN_10000028`（chr 0 → 自己寵物）＋ `actor_map_chr(1/2 → FUN_1001892c(DAT_18000dd7, 0/1))` 夥伴 | (64,64) | |

- 遊玩中沒有分數顯示，只有愛心 **[確認]**。

#### 2.2.2 State 機（`labo05_tick` 0x10058b98）

| state | 動作 |
|---|---|
| 0 | 一般：actor 播 0xa1d；事件：播 0x6a9（寵物 + 夥伴替換）。淡入 → 100 |
| 100 | 一般：C → SND 2、離開；任意鍵（0xf）→ SND 1、淡出、wait 10 → 101；動畫結束 → 同上。事件：只等動畫結束 |
| 101 | 停 actor；一般 → 500（教學）；事件 → 600 |
| 500 | 建教學場景（`1006fa6c`：背景、主角、標題列 "HOW TO PLAY"）、淡入、子 state 9000、wait 40 → 501 |
| 501 | `labo05_tutorial` 0x10021d40 跑教學腳本；C → 離開；任意鍵 → SND 1 → 600 |
| 600 | 清 layer、停 actor → 601 |
| 601 | READY 動畫（C6）、淡入、wait 50 → 602 |
| 602 | 清拳套/特效欄位、淡出、wait 10 → 1000 |
| 1000 | `labo05_build` 0x1006ef80（背景、主角、愛心）；frame 計數 c=0、分數=0；淡入、wait 10 → 1001 |
| 1001 | **遊玩**（每 frame，見下）；結束 → wait 10 → 一般 3000／事件 0xdac |
| 3000 | SND 0x4a → 3001 |
| 3001 | 等音效播完（`FUN_10024c4c` [推測]）→ 淡出、wait 10 → 3002 |
| 3002 | `labo05_result` 0x1006f3f8：算結果、播結果動畫；淡入 → 3003 |
| 3003 | 動畫結束 → 淡出 → 3004 |
| 3004 | 更新計數器（§2.2.5）、`scene_set_next(0x5c)` → 離開 |
| 0xdac/0xdad/0xdae | 事件結局，見 §2.3 |

遊玩 1001 每 frame 順序 **[確認]**：
1. 按鍵（pressed）：A → 線 0、否則 B → 線 1、否則 C → 線 2（一 frame 最多一拳；**C 在遊玩中是第 3 線，不能離開**）。`labo05_shield_raise(lane)`：若該線拳套不存在或未顯示 → 建立/顯示、`hit=0`；**已顯示時再按無效**（不延長）。
2. `labo05_shield_tick`：c += 1；每條顯示中的拳套：計數 +1，計數 ≥ 20 或 `hit` → 隱藏、計數歸 0、hit=0。→ 拳套從按下那 frame 起**有效 19 個 frame 的碰撞檢查**。
3. `labo05_spawn_row`：若 c % 20 == 0 且 `row = c/20 − 1` ∈ [0,39]：對每線，`row[lane].present ≠ 0` → `labo05_meteor_spawn(lane, row[lane].speed)`。
4. `labo05_hurt_tick`：受傷計時 > 0 則 −1，到 0 → 主角回幀 0。
5. 結束檢查 `labo05_is_over`：c ≥ 880（0x370）或命數 == 0。
6. （layer_update 時）各隕石 callback 判定。

#### 2.2.3 隕石（`labo05_meteor_spawn` 0x10032098、`labo05_meteor_cb` 0x1005f750）

- 生成：在起點元件建立 sprite 0x866、x/y 以**線性** tween（mode 1）從起點到終點，長度 `{3.0, 2.5, 2.0}[speed]` 秒 = 60/50/40 frames；SND 0x32（教學中不播）**[確認]**。
- 每 frame（`meteor_cb`）：`my = 隕石 y`；
  - **該線拳套未顯示**：若 `my ≥ 終點 y`（104/96/104）→ 砸中：在爆炸元件播 0xa25、刪隕石、主角受傷姿勢（一般幀 2／事件 motion 4）、若命 > 0 → 命 −1、受傷計時 20、`heart[命]` 改 pal 1、SND 0xe（教學中不播）。
  - **拳套顯示中**：若 `拳套 y − 12 ≤ my + 12`（線 0/2：my ≥ 70；線 1：my ≥ 64）→ 擋下：在拳套位置播 0xa24、隕石標記為無線道（之後不再判定）、拳套 hit=1（下一 frame 收起）、隕石以 1.5 s tween 飛向擊飛目標、**分數 +1**、SND 9（教學中不播）。
- 所以有效視窗：拳套必須在隕石 y 進入 [拳套y−24, 終點y) 期間的某一 frame 是顯示狀態；一拳只擋一顆（同 frame 同線的多顆都會被擋，因為 hit 在下一 frame 才收拳）。
- 最高分 = 該表隕石數：EASY 28、NORMAL 30、HARD 34、事件 30 **[確認：extract 計數]**。最後一列在 c=800 生成、最晚 c=860 落地，早於 880。

#### 2.2.4 命數、結果

- 命數：level 0/1/2 → 3/2/1；事件 → 1 **[確認]** `100218d0`。
- 結果 `labo05_result`：分數 s ≥ 25 → 3；20–24 → 2；6–19 → 1；≤5 → 0 **[確認]**。

#### 2.2.5 版面選擇

- 表在 SRAM（RW 初始資料，開機由 PRAM 0xe6a0 解壓），每張 0xf0 B = 40 列 × 3 線 × `{u8 present, u8 speed}`；init 時 memcpy 到 ctx+0x8c。
- EASY：`0x18000046/0x18000136/0x18000226`，選 `(cnt>>21)&3`；NORMAL：`0x18000720/0x18000810/0x18000900`，`(cnt>>23)&3`；HARD：`0x1800040a/0x180004fa/0x180005ea`，`(cnt>>25)&3`；事件：`0x18000316`。
- 完成一局（state 3004）才 `+1 mod 3` 該難度的欄位；中途離開不更新 **[確認]**。

#### 2.2.6 教學腳本（`0x1009dec4`，8 B：`{s32 delay, u8 cmd, u8 arg, u16 pad}`，`lab05.json`）

每步等 `delay` frames（計數在每步後歸 0）：`(2,spawn 線0) (10,手指 線0) (26,出拳 線0) (30,spawn 線1) (10,手指 線1) (26,出拳 線1) (30,spawn 線2) (10,手指 線2) (26,出拳 線2) (60,結束)`。cmd 0 = 生成速度 0 的隕石；1 = 在 0x4a6+arg 播手指 0x95d；2 = 該線出拳並停手指；3 = 淡出 → state 600 **[確認]**。

#### 2.2.7 音效

0x32（50）隕石出現、9 擋下、0xe（14）被砸、0x4a（74）結束、結果動畫內 0x30/0x34/0x34/0x3b、1/2 確定/離開、READY 6/1。

### 2.3 Game 05_EVENT（scene 0x63 `PSID_LABOGAME_05_EVENT`）

與 §2.2 同 code，差異 **[確認]**（`100218d0`、`10058b98`）：
- 由 HOME 隨機事件進入（METEOR pending、10–15 點、30%；見 05-content §2.9）。
- 命 1、固定表 `0x18000316`（30 顆）、開場 0x6a9 不可跳過、沒有教學（直接 600）、遊玩中無法離開。
- 結束 → 0xdac：淡出 → 0xdad：淡入；清 layer；
  - **命 == 0（失敗）**：播 0x6ab（寵物 + 夥伴替換）、`care_add_gokigen(−10)`、`FUN_10010de4()`（受傷，05-content：`d08` < 4 時）、`FUN_10016900` ×6（3 個 field 各 2 種擺設 −1）。
  - **存活**：播 0x6aa、`care_add_gokigen(+10)`。
- 0xdae：動畫結束 → 淡出、`DAT_18001c6a = (DAT_18001c6a & 0xfc) + 1`（HOME 回到 FIELD 層）、`scene_set_next(0x1c)`、`req_done(0x1e, 0)`（EVENT_METEOR 完成）。
- 不進結果畫面、不給點數、不更新版面計數器、分數不使用。

### 2.4 Game 06 — CLOUDY COIN COLLECTION（scene 0x64 `PSID_LABOGAME_06`）

名稱 string 814「ざくざく！ごっちせんにん」。ctx = `DAT_1801424c`（0xec8 B）：`+0x40` 剩餘時間、`+0x44` 金幣數、`+0x4c` 捲動距離（16.16）、`+0x42` 距離（夾 6500）、`+0x54 + i·0x70` 物件槽 ×32、`+0xe54` 終點專用槽。

#### 2.4.1 畫面

| 物件 | 元件 | sprite | 位置 | 備註 |
|---|---|---|---|---|
| 背景 | 0x4b0–0x4b3（群組 callback `0x1006eb24`） | 0x86b（128×128，6 幀）：0x4b0 幀 4、0x4b1 幀 0、0x4b2 幀 5、0x4b3 幀 0 | 見下 | 視差：`s = (scroll/2) mod 256`；下排（0x4b0/0x4b2 = 山與雲）x = `s+64`（s<128）或 `s−192`、及 `s−64`；上排天空（幀 0）在 y−128；y = 鏡頭 y |
| 主角群組 | 0x4b9 位置 | 雲 0x86d（64×64，3 幀：0 平、1 上升、2 俯衝）＋ 仙人 0x4bb = 0x86c（64×64，5 幀，origin (0,−15)） | x 固定 104（`x(0x4bb)`），y 初始 102 | 固定角色（非寵物）；仙人在 pose 0–2 時上下浮動（tween mode 10，1 s）、pose 3–5 停止並換幀 1 |
| 金幣 | type ≥ 9 → 元件 0x4c2 | 0x872（32×32） | 物件 x/y 為中心 | 判定框 24×24 |
| 障礙 | type 0/1 → 元件 0x4c1 | 0x873（32×32，橘色海星） | | 判定框 type0 12×24、type1 12×12（表中只有 type 0） |
| 終點 | type 2 → 元件 0x4c4 | 0x86e（64×64，旗桿） | 距離 6500，y=104 | 專用槽，type 2 也套用 bob tween |
| 物件繪製層 | 0x4bf（z 0x14）/ 0x4c0（z 0x3c） | — | | 物件 y ≤ 主角 y → 畫在後層 0x4bf，否則前層 0x4c0 |
| 金幣圖示 / 數字 | 0x4b4 / 0x4b5 | 0x80e（32×24）/ TEXT 0x1085 | (15,11) / (14,11) | 金幣數 |
| 時間圖示 / 數字 | 0x4b6 / 0x4b7 | 0x80f（16×24）/ TEXT 0x1086 | (120,11) | `secs = (time+19)/20`；只在 secs < 10 時顯示 |
| GO 提示 | 0x4b8 | anim 0x95c（sprite 0x80d 32×48 幀 2，SND 1 ×3） | (64,64) | 主角滑入後 |
| 開場 | 0x4ad | anim 0xa2e（背景幀 1–3、雲、仙人） | (64,64) | |
| 結果 | 0x4c8/0x4c7/0x4c6/0x4c5（皆 (64,64)） | anim `{0xa3f,0xa3c,0xa3a,0xa36}[result]`，SND 0x30/0x34/0x34/0x3b | | |

鏡頭 y（`ctx+0x28` 群組 y 偏移，套用到背景與物件）：主角 y < 48 → 偏移 = 48 − y（最大 16）；否則 0。教學中門檻 96。

#### 2.4.2 State 機（`labo06_tick` 0x10022220 → 三個子 state 函式）

| state.sub | 動作 |
|---|---|
| 0 | → 1000 |
| 1000.0 | 播開場 0xa2e、淡入 → .1 |
| 1000.1 | 動畫結束 → 淡出 → .1000 |
| 1000.1000 | 啟用 C 離開；建教學（`labo06_tutorial_create` 0x100311f8：主角 y=120、上限 y=96、標題列、手指 0x95d @ (22,100)，示範 callback `0x10083e30`）；淡入；教學計時 80；wait 40 → .1001 |
| 1000.1001 | A 或 B 按下，或 80 frames 到 → 淡出 → .2000 |
| 1000.2000 | 清物件槽、READY 動畫（C6）、淡入、wait 40 → .2001 |
| 1000.2001 | 淡出、wait 10 → 2000 |
| 2000.0 | 啟用 C 離開；`labo06_create` 0x10017828（背景、主角於 x=168、HUD；時間 = 2000、金幣 = 0、距離 = 0）；淡入 → .1000 |
| 2000.1000 | 每 frame 等主物件完成旗標（下表）→ .9000 |
| 2000.9000 | 停用 C、淡出 → 3000 |
| 3000.0 | `result = labo06_result()` |
| 3000.1000 | 播結果動畫、淡入 → .1001；動畫結束 → .9000：`scene_set_next(0x5c)` → 離開 |

全域：update 時若啟用旗標且無淡入淡出且 C 按下 → SND 2、`scene_set_next(0x33)`、離開（不給點數）**[確認]** `10022134`。

主物件 callback `labo06_main_cb` 0x10079f0c（自帶 wait 計數 `[4]`，且淡入淡出中暫停）的內部 state `[2]` **[確認]**：

| 內部 state | 動作 |
|---|---|
| 0 → 1000 | x tween：168 → 104，0.5 s（mode 3） |
| 1001 | tween 完成 → 播 GO 0x4b8、wait 60 |
| 1002 | pose 1 → **2000（遊玩）** |
| 2000 | 見 §2.4.3；撞障礙 → 3000；距離 > 6499 或時間 < 1（且不在擊退中）→ 4000 |
| 3000 | 擊退（`labo06_knockback` 0x100194a0）→ 3001 |
| 3001 | 等擊退結束 → 淡出 → 3002 |
| 3002 | pose 0、y = 102、wait 20 → 3003 |
| 3003 | 淡入 → 3004 |
| 3004 | x tween：從目前 x（擊退後 168）回到 104，0.5 s → 3005 |
| 3005 | tween 完成 → pose 1、**無敵 20 frames** → 2000 |
| 4000 | SND 0x35（終點）、pose 0、wait 50 → 4001：完成旗標 |

state 2000–3999 期間每 frame：時間 −1（下限 0）、更新時間 HUD。state ≥ 4000 捲動 +0.5 px/f（終點後慢慢漂）。

#### 2.4.3 物理（每 frame，state 2000）

捲動 **[確認]** `labo06_scroll_speed` 0x10019530：
- 正常：`Δ = 4.0 + dive`；擊退中（狀態位元 5–6 ≠ 0）：`Δ = 2.5`。`scroll += Δ`；距離 = min(scroll 整數, 6500)。

主角垂直（Verlet，`labo06_input` 0x100195bc → `labo06_set_accel` 0x100193bc、`labo06_pet_physics` 0x100196f0）**[確認]**：
- 加速度 a：按住 A → −0x3d70（−0.24 px/f²）；否則 +0x3d70（+0.24）。
- `y' = clamp(y + (y − y_prev) + a, 32, 120)`（教學中下限 96）。
- 若 a > 0 且 y' > y_prev（往下掉）→ 雲幀 2（俯衝）、`dive += 0.1`（每 frame，無上限；從頂掉到底約 27 frames → 最多約 +2.7 px/f）。
- 否則：雲幀 = A 按住 ? 1 : 0，`dive = 0`。
- 無敵計時（狀態低 5 位元）每 frame −1。

擊退 `labo06_knockback` **[確認]**：狀態 = 1、`y_prev = y`、加速度 = −5.0（第一 frame 往上彈 5 px）、記錄原 y、x tween 104 → 168（1.0 s）、設「擊退中」旗標。擊退中 Verlet 加速度 +0.4（0x6666）；當往下且 y ≥ 原 y → 落回原 y、pose 1、清狀態。x tween 結束時清所有擊退狀態。

物件生成與移動 **[確認]** `10079f0c` 尾段、`labo06_obj_spawn` 0x10025e08、`labo06_obj_scroll` 0x10026224：
- 課程表依 x 排序；每 frame 當 `entry.x < scroll + 256` 生成：物件 x = `scroll + 104 − entry.x`（即 **scroll == entry.x 時物件到達主角 x=104**；生成時約在 x = −152），y = entry.y。物件每 frame `x += Δ`（世界往右移，主角面向左飛）。
- 全部生成後若 `scroll + 256 > 6500`：生成終點（type 2）於 `x = 104 + scroll − 6500`。
- 物件 x > 191 或 y > 191 或 y < −127 → 回收。

碰撞 `labo06_collide` 0x100179d0（state 2000 且不在擊退中）**[確認]**：
- 主角框：x ∈ [px − 20, px + 4)、y ∈ [py − 16, py − 8)（24×8）；物件框：中心 (ox, oy)，w×h（金幣 24×24、障礙 12×24）。AABB 重疊（嚴格不等式）。
- 金幣（type 9）：SND 0x3b、閃光粒子、標記已收集（縮小 8 px/f、x 額外 −1.5 px/f 飄走）、**金幣 +1**、更新 HUD。
- 障礙（type < 9）：先被撞飛（`vy = −rand(4.0..6.0)`、重力 +0.5、x −4 px/f；**只影響視覺**）；若主角**無敵中** → 無懲罰；否則 SND 0xe、pose 4、→ 3000（擊退）。
- 撞到時每槽只處理一次（已收集/撞飛的不再判定）。

#### 2.4.4 結果、版面

- 結果 `labo06_result` 0x1007ae84：金幣 ≥ 40 → 3；25–39 → 2；1–24 → 1；0 → 0 **[確認]**。
- 課程（`lab06.json`）：指標表 0x10092d70，9 × `{u32 ptr, u16 count, u16 pad}`；物件 6 B `{s16 x, s16 y, s16 type}`。EASY 3 張（各 50 金幣 + 5 障礙，55 筆）、NORMAL 3 張（45 + 10）、HARD 3 張（41 + 20，61 筆）。x 範圍 200/328 → 6296–6328；y ∈ {40,56,72,88,104,120}。
- 選擇：EASY `(cnt>>27)&3`、NORMAL `(cnt>>29)&3`、HARD `DAT_18001678 & 3`；**在 scene init 時**就 `+1 mod 3`（就算中途離開也前進）**[確認]** `10021f6c`。
- 最快完成時間：6500 / 4.0 = 1625 frames（無俯衝、無撞擊），限時 2000 frames。

#### 2.4.5 音效

0x3b（59）金幣、0xe（14）撞障礙、0x35（53）終點、1（GO ×3）、2 離開、READY 6/1、結果 0x30/0x34/0x34/0x3b（動畫內）。

### 2.5 Lab GAME 選單（scene 0x2c `PSID_LABOMENU_GAME`，別名 0x2d–0x33）

init `1001fe44`：ctx（0x8c B）`+0x7e` 遊戲 index、`+0x80` 難度 index **都歸 0**，`+0x88` 模式（0 = 選遊戲、1 = 選難度），返回場景 0x2a（LABOMENU）。tick = `labomenu_game_tick` 0x10047508 **[確認]**。

- 進入：前一場景 0x2c（從 LABOMENU）→ state 1500：標題畫面（0x3cd 背景 sprite 0x815、0x3ce sprite 0x81e（128×64）@ (64,54)、0x3d0 sprite 0x3b 名條 @ (64,104)、0x3cf 文字 string 2 "MINI GAMES" @ (64,102)），21 frames 後淡出 → 1750。前一場景 0x2d–0x33（從遊戲/結果回來）→ 游標 = {0,1,2,3,6,5,4}[場景 − 0x2d] → 直接 1750。
- 1750 建清單畫面：背景 0x3d2（sprite 0x815）；遊戲圖示 0x3d1（`0x10093544[i]` sprite，128×72）@ (64,44)；遊戲名 0x3d3 @ (64,44)（string `0x10093560[i]`）；難度框 0x3d5（sprite 0x81f，128×24，pal 1 = 灰＝非作用中）@ (64,100) + 文字 0x3d4（string 191/192/193 EASY/NORMAL/HARD）；箭頭 anim 0xe/0xf：選遊戲時 0x3d6/0x3d7 @ (8,44)/(120,44)，選難度時 0x3d8/0x3d9 @ (8,100)/(120,100) 且難度框 pal 0。→ 模式 0：2001；模式 1：4001。
- 選遊戲（`labomenu_game_select` 0x1002ff88）：
  - A 按下或轉盤上（held 0x80）→ i = (i+1) % 7，SND 0；轉盤下（0x40）→ i−1（−1 → 6），SND 0。
  - 換項動畫：新項目建在 x±128、舊項目 tween 到 x∓128、新項目 tween 到 x（0.3 s，mode 7）；**3 frames 後**整組重建在定位（動畫只播前一半）**[確認]**。
  - B 或轉盤按壓 → SND 1；`rank < unlock[i]` → 淡出 → 鎖定畫面 6000；否則 → 4000（進入選難度：箭頭移到難度列、難度框 pal 0、模式 = 1）。
  - C → SND 2 → 離開到 LABOMENU。
- 選難度（`labomenu_level_select` 0x1002f874）：同樣導覽（0..2 循環，改 0x3d5/0x3d4）；B/按壓 → SND 1，`rank < level_rank[d]` → 鎖定畫面；否則 5000：`scene_set_next({0x5d,0x5e,0x5f,0x60,0x64,0x62,0x61}[i])`、`DAT_18001c69 = d`、離開。C → SND 2、模式 = 0、state 2000（重建清單，回到選遊戲）。
- 鎖定畫面 6000：0xf7（sprite 0x33 藍斜紋 pal 2）、0xf9（sprite 0x4e）@ (64,76)、0xfa（sprite 0x66）@ (64,13)、文字 0xf8 @ (64,76) = string `0x10093fe6[需要的 rank]`（"AVAILABLE FROM LEVELn"，242–250）。A/B/C 任一或 200 frames → 淡出 → 1750（回到原模式）。
- rank = `DAT_18000c6c & 0xf`（planet rank）。

| index | 遊戲 / 場景 | 名稱 string | 圖示 sprite | 解鎖 rank |
|---|---|---|---|---|
| 0 | 00 / 0x5d | 803 FEEDING BABY MILK! | 0x806 | 1 |
| 1 | 01 / 0x5e | 804 DELIVERING VITAMIN! | 0x807 | 4 |
| 2 | 02 / 0x5f | 816 FISH MATCHING | 0x823 | 4 |
| 3 | 03 / 0x60 | 817 FRUIT GATHERING | 0x824 | 1 |
| 4 | 06 / 0x64 | 814 CLOUDY COIN COLLECTION | 0x822 | 4 |
| 5 | 05 / 0x62 | 808 PUNCHING METEORITE! | 0x80b | 1 |
| 6 | 04 / 0x61 | 818 CURLING | 0x825 | 1 |

難度需求 rank：EASY 1、NORMAL 6、HARD 9（`0x10093968`）。

### 2.6 共用結果畫面（scene 0x5c `PSID_LABOGAME_RESULT`，`scene_labogame_result` 0x100228c4）

init **[確認]**：
- 讀 `level = DAT_18001c69`（之後歸 0）、`result = DAT_18001c68`（之後歸 0）。
- result == 3 → `DAT_18000cc0 = min(DAT_18000cc0 + 1, 100)`（mission 13 計數）。
- `points_add(points[level][result])`，表 `0x10091bec` u16[3][4]：EASY 10/100/150/250、NORMAL 10/200/300/500、HARD 10/300/450/800。
- 版面：背景 0x3da（sprite 0x812）；左上點數列 0x3dd 圖示（sprite 0x62）@ (12,16)、0x3db 底條（sprite 0x48，72×16）@ (36,16)、0x3dc 文字 = **加點後的總點數** `DAT_18000ca4` @ (62,16)；中央 3 個空星 0x3df/0x3e0/0x3e1（sprite 0x81d，32×32，3 幀）@ (32,56)/(64,56)/(96,56)；下方面板 0x3e4（sprite 0x45）@ (64,96)、文字 0x3e2 = **本次獲得點數** @ (78,96)、0x3e3 數字字型 sprite 19 幀 16（"G" 點數記號）@ (90,96)。
- 返回場景 = 依前一場景：0x5d→0x2d … 0x61→0x31、0x62→0x32、0x64→0x33。

tick `labogame_result_tick` 0x1003298c：
| state | 動作 |
|---|---|
| 0 | 淡入 → 1000 |
| 1000 | 11 frames 後：對 i < result 生成星星動畫 0x95e 於 x = 星槽 x − 32 [依原碼]；星星動畫：sprite 0x81d 幀 2、縮放 6.0→1.0 且 y −96→0（20 frames）、換幀 1、縮放彈 1.2→1.0（20 frames）、`EVT 15` → callback `0x10032464`：所有 i < result 的星槽改幀 1、播 SND `0x100a13b8[result]` = {36, 41, 41, 24}。result == 0 → 直接播 SND 36 |
| 1001 | 11 frames → 1002 |
| 1002 | 100 frames 內任意鍵（0xf）→ SND 1 → 離開；100 frames 到自動離開 |

- 已知 bug **[確認]**：result == 0 時仍對 `ctx+0x4c`（未建立 = 0）的 `+0x120` 註冊 callback（讀位址 0x120），實機無明顯症狀 **[推測]**；重製直接略過。
- 結果畫面不顯示寵物、不改心情/飢餓、不給物品 **[確認]**。
- 重製：`Scene::LabResult`。星星動畫 0x95e 放在星槽座標（不減 32）；落地時間取 40 frames（20 縮放 + 20 彈）而非等 `EVT 15` callback；結束回 MINI GAMES 選單。

---

## 3. 資料表

| 表 | 位址 | 格式 | 筆數 | 內容 / 檔案 |
|---|---|---|---|---|
| G04 發射角 | 0x1009bb50 | f32 | 3 | 0.375/0.25/0.125 圈 → 0x6000/0x4000/0x2000 |
| G04 發射角索引 | 0x1009bb5c | u16[3 level][3 b][5 round] | 45 | `lab04.json launch_index` |
| G04 獎品磚 | 0x10091d30 | u16[3 level][10][5]（只用前 5 列） | 75 有效 | `lab04.json prize_brick`（值 1–10） |
| G04 每回合時限 | 0x1009c5a4 | u32[3] | 3 | 1200/1000/800 |
| G04 獎品角色 | 0x10093320 | u16[49] | 49 | chr 63,115–130,132–147,149–164 |
| G04 結果動畫 / 音效 | 0x10095748 / 0x10095770 | u16[4] | 4 | 0xa87,0xa85,0xa7f,0xa78 / 48,52,52,59 |
| G04 反彈音效 | 0x1009390e | u16[2] | 2 | 0（牆/板子）、10（磚） |
| G05 隕石表 | SRAM 0x18000046 …（RW 初始資料，PRAM 0xe6a0 解壓） | 40 × 3 × {u8 present, u8 speed} = 0xf0 B | 10 張 | `lab05.json layouts / event_layout` |
| G05 教學腳本 | 0x1009dec4 | {s32 delay, u8 cmd, u8 arg, u16} | 10 | `lab05.json tutorial_script` |
| G06 課程指標 | 0x10092d70 | {u32 ptr, u16 n, u16} | 9 | 0x10092014 … 0x10092bfc |
| G06 課程物件 | 各 ptr | {s16 x, s16 y, s16 type} | 55/55/55/55/55/55/61/61/61 | `lab06.json courses` |
| G06 物件元件 / 判定框 | 0x10094762 / 0x10094768 | u16[3] / {s16 w, s16 h}[3] | type 0–2（≥9 寫死 0x4c2、24×24） | 0x4c1,0x4c1,0x4c4 / 12×24,12×12,12×12 |
| G06 pose → 雲幀 | 0x100938ee | 6 B/筆（只用第一個 u16 &0x1ff） | 6 | 0,0,1,1,0,0 |
| 選單圖示 | 0x10093544 | u32[7] | 7 | 0x806,0x807,0x823,0x824,0x822,0x80b,0x825 |
| 選單名稱 | 0x10093560 | u16[7] | 7 | 803,804,816,817,814,808,818 |
| 解鎖 rank | 0x10093976 | u16[7] | 7 | 1,4,4,1,4,1,1 |
| 難度名稱 / rank | 0x1009de8c / 0x10093968 | u16[3] | 3 | 191–193 / 1,6,9 |
| 鎖定訊息 | 0x10093fe6 | u16[12]（以 rank 索引） | 12 | 244,242,…,250 |
| 結果點數 | 0x10091bec | u16[3][4] | 12 | 見 §2.6 |
| 結果星星音效 | 0x100a13b8 | u16[4] | 4 | 36,41,41,24 |
| sin / tan | 0x1009a968 / 0x1009cbd4 | s16[1024] Q12 / s32[1025] Q12 | | `lab_math.json` |

計數器位元配置（`DAT_18001674`，與 Lab 00–03 共用同一 u32）：bits 9–10 G04 發射角組、11–13 G04 獎品磚組、14–20 G04 獎品角色、21–22/23–24/25–26 G05 EASY/NORMAL/HARD、27–28/29–30 G06 EASY/NORMAL；`DAT_18001678` bits 0–1 G06 HARD **[確認]**。

---

## 4. 新命名

```
100216e8 labo04_init f
1002180c labo04_update f
10052024 labo04_tick f
100788d0 labo04_play f
1007ae24 labo04_result f
1005fd94 labo04_tutorial f
10027c14 labo04_paddle_input f
1006fb74 labo04_ball_create f
1006fc18 labo04_bricks_create f
10080d20 labo04_paddle_cb f
1006bd24 labo04_ball_cb f
1006c4d4 labo04_brick_cb f
10007b84 labo04_prize_cb f
10066440 labo04_prize_shown_cb f
10019940 labo04_bounce_snd f
1007c100 circle_rect_hit f
10009b84 cos16 f
10031b74 sin16 f
1000371c atan2_16 f
10003734 atan2_12 f
1000382c atan_q12 f
100218d0 labo05_init f
10021cdc labo05_update f
1006ef80 labo05_build f
1002860c labo05_shield_raise f
10017b7c labo05_shield_tick f
1005fbb0 labo05_spawn_row f
10032098 labo05_meteor_spawn f
1005f750 labo05_meteor_cb f
10007550 labo05_hurt_tick f
1001de20 labo05_is_over f
1006f3f8 labo05_result f
1006fa6c labo05_tutorial_build f
10021d40 labo05_tutorial f
10011acc labo05_tutorial_end f
10021f6c labo06_init f
10022134 labo06_update f
10022220 labo06_tick f
100222e4 labo06_state_intro f
10022580 labo06_state_play f
10022678 labo06_state_result f
10017828 labo06_create f
10017b64 labo06_is_done f
10079f0c labo06_main_cb f
100179d0 labo06_collide f
1007ae84 labo06_result f
10019530 labo06_scroll_speed f
100193bc labo06_set_accel f
100195bc labo06_input f
100196f0 labo06_pet_physics f
100194a0 labo06_knockback f
10019648 labo06_set_pose f
10019620 labo06_set_invuln f
1001957c labo06_is_invuln f
1001959c labo06_in_knockback f
10019564 labo06_knockback_busy f
10019404 labo06_pet_create f
10009ba8 labo06_bg_create f
1006eb24 labo06_bg_cb f
10025e08 labo06_obj_spawn f
10026224 labo06_obj_scroll f
100260f4 labo06_obj_hit f
100261f8 labo06_obj_free f
10025f98 labo06_obj_draw f
1007a5c8 labo06_hud_coins f
1007a5f4 labo06_hud_timer f
1007a580 labo06_wait_tick f
10009c98 labo06_min_y f
100845ac labo06_vdir f
100311f8 labo06_tutorial_create f
1001fe44 scene_labomenu_game_init f
1001fedc scene_labomenu_game_update f
1003298c labogame_result_tick f
10032464 labogame_result_star_cb f
10066b34 tween32_set f
10066968 tween16_set f
10066abc tween32_snap f
100668e4 tween16_snap f
10009ff4 layer_clock_advance f
1000a268 layer_update f
1006f290 obj_group_create f
1006f7ac obj_sprite_create f
1000996c obj_set_sprite f
10009958 obj_set_callback f
1002a5e4 rng_range_fx f
10005ea8 key_pressed f
10006150 key_held f
10006140 dial_delta f
18001674 g_minigame_counter v
18001678 g_minigame_counter2 v
18001c68 g_labogame_result v
18001c69 g_labogame_level v
18000cc0 g_mission13_count v
```

---

## 5. 未解問題

1. **Game 04 板子加速 bug**：`v = (v < −4) ? v : −5` 用原始整數比較 16.16 值，按住 A/B 時速度每 frame +1 px/f 無上限（20 frames 後 20 px/f，實際被 x ∈ [16,112] 夾住）。設計意圖可能是上限 4–5 px/f。重製要決定照原版還是改成 ±4/±5 px/f。另：板子交點用單位向量 cos 而非 Δx（影響很小）。
2. Sprite 視覺錨點：依 blitter 推得 center = (x+ox, y+oy)，game 04 球的 logic 座標與磚判定有約 12 px 的不對稱（球圓心 = y+12、球畫面中心 = y−16），頂牆 y=12 時球大半在畫面外。需要實機或錄影比對。Game 05 主角 sprite 230 (origin (4,−1)) 放在 (62,122) 也會大半出界，群組物件（`obj_group_create`）的子元件是否用相對座標尚未確認。
3. 角色身分：game 04/05 腳本中的 chr 0x99/0x98/0x9e–0xa1/0xa 是 ChrID 還是角色表 index 未直接驗證（ChrID 解讀下為冰河角色）；game 06 的「仙人」是純 sprite（0x86c）。05_EVENT 的 0x48d T7 元件（chr 1 + `FUN_10007c68`）是否就是自己的寵物待確認。
4. `FUN_10024c4c`（game 05 結束等音效）、`FUN_10025cb4`（進場參數 1）語意為推測。
5. Game 04 教學中板子碰撞的固定反彈角、game 06 教學示範 callback `0x10083e30` 的內容未展開（純展示，不影響遊戲）。
6. 結果畫面 result == 0 時對 NULL+0x120 註冊 callback 的實際效果。
7. Tween mode（1 = 線性、3/7 = 緩動、10 = 循環浮動）的曲線公式在 `FUN_100098d0`/`FUN_100669e8`，未展開；重製可用一般 ease-in-out 近似。

---

## 摘要（≤15 行）

1. Game 04 CURLING：球 3.0 px/f、牆 x∈[12,116]/頂 12、y≥158 漏球；板子 y=124、x∈[16,112]、判定半寬 14+6，偏移/28 改 cos；磚 20×20 框 vs 圓(球y+12, r12)。
2. 5 回合 × 15 磚（重置），獎品磚擊中 → 減速 0.15/f² 停 → 下回合；每回合限時 1200/1000/800 f；結果 = 過關回合 5→3、3–4→2、1–2→1。
3. 板子 bug：速度用 raw −4/5 比較 16.16，按住每 frame +1 px/f 無上限；轉盤 = delta×5 px。
4. 發射角 {135°,90°,45°} 由 `0x1009bb5c`、獎品磚 `0x10091d30`、獎品角色 49 選 1，全由 `DAT_18001674` 位元欄位輪替（離場時前進）。
5. Game 05：40 列×3 線隕石表在 RW data（需 pram.bin 解壓），每 20 f 一列，線性落下 3.0/2.5/2.0 s；A/B/C 出拳 19 f 有效、擋一顆即收、再按無效；C 不能離開。
6. 擋下條件 my ≥ 拳套y−24（70/64/70），未擋則到終點 y（104/96/104）砸中；命 3/2/1；frame 880 結束；≥25/20/6 → 3/2/1；最高 28/30/34。
7. 05_EVENT：1 命、表 0x18000316（30 顆）、不可跳過/離開、無教學；存活 +10 心情、失敗 −10＋受傷＋擺設 −1；回 HOME FIELD，req_done(0x1e)。
8. Game 06：主角 x=104，世界往右捲 4.0+dive（擊退中 2.5）；A 按住 −0.24、放開 +0.24 px/f² Verlet，y∈[32,120]；下落時 dive += 0.1/f。
9. 撞障礙：上彈 −5、x 退到 168 再回、淡出淡入、20 f 無敵；無敵中撞障礙無懲罰。終點 6500、限時 2000 f；金幣 ≥40/25/1 → 3/2/1。
10. 課程 9 張（50+5 / 45+10 / 41+20），物件 {x,y,type}，scroll==x 時到達主角；HARD 用 `DAT_18001678`，選擇在 init 就前進。
11. GAME 選單順序 00,01,02,03,06,05,04；解鎖 rank {1,4,4,1,4,1,1}、難度 rank {1,6,9}；A/轉盤上下輪播、B/按壓確定、C 返回；鎖定畫面 200 f。
12. 結果畫面：points[level][result] = 10/100/150/250、10/200/300/500、10/300/450/800；result 3 → mission13 +1（上限 100）；星星動畫 + SND {36,41,41,24}；100 f 或任意鍵離開。
13. 主要音效：G04 9 發球/0 反彈/10 磚/52 獎品/13 失敗；G05 50 出現/9 擋/14 砸/74 結束；G06 59 金幣/14 撞/53 終點；結果動畫 48/52/52/59。
14. 資料：`data/minigames/lab04/05/06/labmenu/lab_math.json`，由 `tools/extract_minigames_b.py`（自帶 assert 自檢）產生。
