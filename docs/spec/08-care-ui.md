# 08 — 照顧畫面 UI（HOME / 圈選單 / 餵食 / 打理 / 清大便 / 藥 / 受傷 / 起床睡覺 / 狀態畫面）

範圍：重製照顧相關畫面需要的輸入對應、版面（sprite / UI 元件 / 動畫腳本）、狀態機與時長、呼叫哪個照顧函式。照顧規則本身見 `01-care.md`，場景表與輸入底層見 `06-system.md`，sprite 格式見 `04-assets.md`。
時間單位：**frame（20 fps）**，除非另註。位址皆為 XIP（0x10xxxxxx）或 SRAM（0x18xxxxxx）。

輸入記號：A=bit0、B=bit1、C=bit2、P=轉盤按壓 bit3、DN=轉盤下 bit6（負）、UP=轉盤上 bit7（正）；「任意鍵」= `key_pressed(0xf)`（A/B/C/P，不含轉盤）；「確定」= `key_pressed(0xa)`（B 或 P）。`dial_delta()`（0x10006140）回傳**本 frame 的編碼器原始計數**（有號），不是 bit6/7 脈衝；縮放與圈選單直接用原始計數。

---

## 1. 摘要

- 畫面幾乎全是**資料驅動**：data pack 表 4/5 = **UI 元件定義**（座標 + sprite/文字/動畫），表 13/14 = **動畫腳本**（opcode 位元組碼，可放 sprite、移動、播音效、生子動畫、叫出寵物並指定 motion 編號、在某點暫停等按鍵）。照顧場景的 C 程式只負責「播哪支動畫、等它結束/暫停、按鍵後繼續、呼叫 `req_done`」。格式見 §3.1/§3.2，兩者皆已寫出解碼器並驗證（解出的 sprite 都與畫面語意相符）。
- **HOME 的 4 個 zoom 是同一個連續縮放**：轉盤原始計數直接加減縮放值 z（每計數 1 單位），倍率 = 4^(z/16)（z=0 → 1.0 = 場地、z=8 → 2.0 = 寵物、z≥16 → 4.0 = 細胞、倍率 ≤0.5（z≤−8）→ 星球）。停手 6 frames 後吸附到最近的停駐點 {0, 8}。HOME_FIELD(0x1e) 與 HOME_TAMA(0x1f) 共用同一組 init/update，只差起始 z。
- A = 開該層的**圈選單**（垂直轉輪，3–5 項）；長按 P（>13 frames）= LABOMENU；C（場地層）= 切換鏡頭焦點物件；寵物有特殊狀態時（想大便、受傷、瀕死、要叫醒/哄睡、感冒（細胞層））**任意鍵**直接進對應事件場景。
- 餵食：分類清單（FOOD/SNACKS）→ 橫向食物轉盤（永遠附送免費 PELLETS(5002)/JELLY STICK(6001)，嬰兒期只有 MILK(5001)/JELLY STICK）→ 「EAT THIS? YES/NO」→ 拒絕檢查 → `req_done(4,item)` + 背包 −1 → 2–3 段動畫 → 回食物清單（要按 C 兩次才回 HOME）。
- 打理（OTEIRE）/ 吃藥 / 受傷治療都是「動畫在暫停點等任意鍵」型：OTEIRE 共 5 次按鍵，第 4 次時 `req_done(5)`；藥 5 次按鍵（每次打掉一隻病菌）後再播 120 frames 才 `req_done(10)`；受傷 4 次按鍵後 `req_done(0xb)`。
- 清大便：每個已開放的 field 播一次吸塵器動畫（60 frames），之後 `care_clean_unchi()`；有清到 → 心情 +1、「UNCHI METER」加分動畫、寵物開心動畫。
- 起床/睡覺：寵物層任意鍵 → 「WAKE UP YOUR TAMA? / TIME TO SLEEP?」YES → `req_done(2)` / `req_done(3)`（**這就是玩家主動哄睡的路徑**，補完 01-care 的未解問題）；NO → 當天不再詢問。
- 狀態畫面（CELL_STATUS，只能從細胞層 A 進入）：2 頁（飢餓 6 格 + 心情 21 段 + 年齡 / 名字），A 或 UP 翻頁，B 顯示 QR，C 或 200 frames 無操作離開。
- 新遊戲背包：TROPICAL FRUITS(5068)、FROZEN MEAT(5076)、BAMBOO GRASS(5058) 各 3 個 + NICE BOX(7998)；商店食物分頁要 rank ≥1，所以新玩家主要靠免費 PELLETS/JELLY STICK。

---

## 2. 詳細規則

### 2.1 共用機制

| # | 規則 | 證據 | 標記 |
|---|---|---|---|
| U1 | 場景 context 共用 task 結構（`+0 state`、`+8 substep`、`+0xc done`、`+0x18 layer`）。照顧場景慣例：state 0 → 1000 → …；`0x7fe0/0x7fe1` = 淡出、`0x7ff0` = done。init 都 `scene_set_next(0x1c HOME)` 當返回目標 | 0x1006953c/0x1006956c/0x100695b8 | [確認] |
| U2 | 淡入淡出 `fade_set(color, level, dir, speed, …)` 0x1007b40c：`(0,0x20,0,4,0)` = 從黑淡入、`(0,0,1,4,0)` = 淡出到黑，speed 4；`(7,…)` = 白色；`(0xff,…)` = 沿用目前色 | 0x1007b40c | [確認]（速度單位 [推測] 每 frame 4/0x40 → 約 16 frames） |
| U3 | **寵物動畫物件**（pet-anim）：`petanim_play(obj, anim_id, x, y, z)` 0x10000028 建立動畫（原點固定 (64,64) = 畫面中心）並把 chara 佔位 0 換成目前寵物；`petanim_done(obj)` 0x10000584 = 腳本跑到 `EVT -1`；`petanim_paused(obj)` 0x10000594 = 跑到 `EVT 4`（等待）；`petanim_resume(obj)` 0x10000568 清等待；`petanim_subst(obj,0,0x682,spr)` 0x100006a8 把腳本內 sprite 0x682 換成指定 sprite | 0x10000028、0x10000164 case 2/3/5 | [確認] |
| U4 | 動畫腳本的 `wait` 單位 = frame（每 frame 遞減一次）；tween 長度 `r` 以 `(r<<16)/20` 轉成內部速度 | 0x100016d0、0x1008ed84 | [確認]（r 也是 frame [推測]） |
| U5 | 子動畫（CHILD）座標相對父動畫原點；寵物通常放在 (0,+32) → 畫面 (64,96) | 0x100016d0 case 0x10、0x1001f708 | [推測，強] |
| U6 | 音效 id：0 = 游標移動、1 = 確定、2 = 取消、0xb = 不可（錯誤嗶）、0x28 = 縮放/轉盤、0x2b = 拉遠到星球、9 = zoom 層切換 | 各 `snd_play` 呼叫點 | [確認] |
| U7 | `idle_reset()` 0x10025c6c 在動畫播放中的 state 每 frame 呼叫，避免 200 frames 閒置回 HOME | 0x1003ff80 等 | [確認] |

### 2.2 HOME（0x1c 路由 → 0x1d SPACE / 0x1e FIELD / 0x1f TAMA / 0x20 CELL）

**2.2.1 縮放（zoom）[確認]**（`zoom_update` 0x1000ed48，物件在 HOME ctx +0xc4）

- 狀態：`+0xc` 目前 z（16.16）、`+0x10` 目標 z、`+0x14/+0x18` 下/上限、`+0x20..` 停駐點、`+0x30` 停駐點數、`+0x34` 吸附倒數。
- 每 frame：`d = dial_delta()`；d≠0 → 目標 += d·1.0、倒數 = 6、若目前沒在播 0x28/0x2b 就播音效 0x28（vol 40）。d=0 → 倒數到 0 時 `zoom_snap`（0x1000ec3c）：目標 = 最近的停駐點。目前值每 frame 追 1/4 差距（差 ≤0x2000 直接到位）；超出上/下限也立即吸附。
- 輸出倍率 `scale = 65536 · 4^(z/16)`（0x1000ed48 尾端 double 運算：×0.0625 後 pow(4,·)×65536）。
- HOME_FIELD/TAMA 設定（`home_scene_build` 0x10084df8）：下限 z=−16、上限 z=24（寵物有 `+0x17c bit0`（=沒有可放大的細胞動畫，`FUN_1008461c(2)==0` 即蛋）時上限 10）、停駐點 {0, 8}。進場：field 層 z=0、tama 層 z=8（`FUN_100859f8`）。
- 依倍率切層（`home_zoom_dispatch` 0x100687dc）：
  - scale == 1.0 → state 1000（場地層）；scale == 2.0 → state 2000（寵物層）。中間值只做鏡頭插值、不接受按鍵。
  - scale ≤ 0.5（z ≤ −8）→ state 4000：`FUN_10085c14`（動畫 0x2c6、白色淡出、音效 0x2b）→ `PSID_HOME_SPACE`。
  - scale ≥ 4.0（z ≥ 16）且非蛋 → state 0x1004：`FUN_10085bb4`（音效 0x28）→ `PSID_HOME_CELL`。
- 重製簡化：「轉盤上（正）= 拉近、下 = 拉遠」；從場地到寵物需累積 +8 計數、到細胞再 +8、到星球 −8（連續轉、中間停 >6 frames 會被吸回 0 或 8）。

**2.2.2 鏡頭與畫面 [確認/部分推測]**（`home_camera_update` 0x100793fc）

- 兩個 layer：`+0x1c` 世界層（背景、寵物、玩具、大便）縮放 = scale；`+0x20` 寵物特寫層縮放 = scale/2，座標 ×2−64。
- 鏡頭中心：場地層 = 目前焦點物件位置（焦點是寵物時用寵物座標 `+0x9c/+0xa0`）；寵物層 = 寵物座標；1.0↔2.0 之間線性插值；細胞層 = 寵物座標 + 偏移 `+0xac/+0xb0`。鏡頭追蹤每 frame 移動距離的 1/2（上限 16 px/frame）。
- 背景：`field_bg_create` 0x10015d90 從表 `0x10092e06 + field*0x40 + variant*0x20`（16×u16）取 `[1 + period*3 + weather]` 的 UI 元件，位置用 `[0]` 元件座標 (64,64)。field 0 的背景 = sprite 0x46c–0x471（**512×512、8bpp**），依時段 period 0–4 × 天氣 0–2 選圖。場地層倍率 1.0 時只看得到 128×128 的一塊（鏡頭跟著焦點），拉遠到 0.25 才看到全圖 [推測：依 layer 縮放推得]。
- 大便（背景元件 callback `FUN_10015ef0`）：`unchi` 坨，位置 = UI 元件 0x2e8–0x2ed（(112,56)、(24,48)、(96,100)、(28,112)、(88,72)、(44,84)），sprite 0x458（16×16），frame 每 15 frames 在 0/1 間切換，第 i 坨 `i < old_unchi` → palette 1（變色的舊大便）。透明度隨縮放：scale=1.0 全顯示，往 0.5 或 2.0 線性淡到 0 → **只在場地層看得到大便**。天氣 2（降水）另畫粒子元件 0x2d5（sprite 0x457）。
- 寵物動作（`home_pick_behavior` 0x10078c94 決定行為碼 `+0xbc`；`home_behavior_anim` 0x1008461c 查表 `0x1009dbd8[行為*3 + 層]`，層 0=場地、1=寵物、2=細胞）：

| 行為 | 條件 | 場地/寵物/細胞 動畫 | 寵物層按鍵 |
|---|---|---|---|
| 0 | 蛋（stage 1） | 0x538 / 0x539 / – | 無 |
| 2 | 睡覺中 | 0x581 / 0x583 / 0x583（motion 50 / 15 + zzz sprite 0x72d） | WAKE 待處理時任意鍵 → EVENT_TO_WAKE(0x55) |
| 3 / 4 | 睡覺中且感冒 / 受傷 | 0x585 / 0x587 … | –（+0x17c bit3：A 不能開選單） |
| 0x0e | 一般待機（onaka ≤2） | 0x56d / 0x56e（motion 45 / 3） | – |
| 0x11 | SOWASOWA 待處理 | 0x571 / 0x572（motion 3 + 臭味線 0x728） | **任意鍵** → `req_done(0xe)` → EVENT_UNCHI_KAISHU(0x53)（場地層也可） |
| 0x12/0x13/0x14 | LOVE / IMMUNITY / HAPPY 狀態 | 0x589 / 0x58d / 0x591 … | – |
| 0x15/0x16 | DANCE / KIRAKIRA（且對應 c6b 旗標） | 0x5a1 / 0x5a5 … | – |
| 0x17 | 感冒 cond 2 | 0x5aa / 0x5ac / 0x5ac（骷髏 0x733） | 無；**細胞層**任意鍵 → EVENT_CURE_COLD(0x50) |
| 0x18 | 受傷 cond 3 | 0x5bc / 0x5be（motion 19 + 傷痕 0x724） | **任意鍵** → EVENT_CURE_INJURY(0x51) |
| 0x19 | 瀕死 cond 4 | 0x5ca / 0x5cc（0x74d） | 任意鍵 → EVENT_RESCUE(0x54) |
| 0x1a | EVOL_SUPPORT 待處理 | 0x545 / 0x546 | 細胞層任意鍵 → EVENT_EVOL_SUPPORT(0x4b) |
| 0x1b / 0x1c | ONAKA / GOKIGEN 呼叫 | 0x646 / 0x647（motion 3/4）、0x648 / 0x649（motion 14/10/11） | – |
| 0x1d / 0x1e | DODODO / DODODO_IGNORE | 0x64a / 0x64b … | 任意鍵 → 0x5a / 0x5b（場地層） |
| 1, 5–0xd, 0xf, 0x10 | 一般隨機待機：依 onaka/gokigen 等級與 27 筆循環表 `0x100a05b4` 挑 | 0x560–0x570 | – |

  `onaka_level`(0x10010b0c)：onaka 0–2→0、3–4→1、5–6→2；`gokigen_level`(0x10010ac4)：0–5→0、6–15→1、16–20→2。行為 0xe 用在 onaka_level==0；gokigen_level==0 → 行為 0xb/0xc（隨機）。
  行為在**進入 HOME 時決定一次**（init 呼叫），不會在畫面中途變。

**2.2.3 場地層 state 1000 輸入 [確認]**（`home_field_tama_state` 0x10039eb8；蛋期整段不接受）

| 輸入 | 動作 |
|---|---|
| 任意鍵 | 行為 0x11 → SOWASOWA 處理（見上）；0x1d/0x1e → DODODO 事件 |
| C | 物件 ≥2 個且鏡頭不在移動 → 焦點 `+0xb6 = (+0xb6+1) % 物件數`、音效 0；繞回起點且該 field 擴充區已解鎖 → `PSID_HOME_FIELD2`(0x21)。物件 <2 且已解鎖 → 直接 FIELD2。+0x17c bit3 時音效 0xb |
| A | 開圈選單（state 5000，表 1 = 場地選單）；寵物生病/受傷/瀕死（+0x17c bit3）或 DODODO（bit7）→ 音效 0xb |
| 長按 P（`key_long_held(8)`，>13 frames） | 音效 1 → `PSID_LABOMENU`(0x2a)。bit3 狀態下只有 `0x18001c6b bit3` 設時可 |
| 轉盤 | 縮放（§2.2.1） |

**2.2.4 寵物層 state 2000 輸入 [確認]**

| 輸入 | 條件 | 動作 |
|---|---|---|
| 任意鍵 | +0x17c bit5（WAKE 待處理且在睡）且 `c6a bit7`=0 | 音效 1 → EVENT_TO_WAKE(0x55) |
| 任意鍵 | 行為 0x11 / 0x18 / 0x19 | → 0x53 / 0x51 / 0x54 |
| 任意鍵 | +0x17c bit6（SLEEP 為最高優先請求）且 `c6b bit0`=0 | 音效 1 → EVENT_TO_SLEEP(0x56) |
| A | 非 bit3/bit7 | 開圈選單（表 2 = 寵物選單） |
| 長按 P | | LABOMENU |
| 轉盤 | | 縮放 |

**2.2.5 細胞層 HOME_CELL（0x20，`home_cell_state` 0x100398c8）[確認]**：同一套縮放（scale ≤0x8000 → 回 `PSID_HOME_FIELD` 0x1e）。行為 0x17（感冒）任意鍵 → 0x50；0x1a → 0x4b；**A → `PSID_CELL_STATUS`(0x29)**（細胞選單表 3 只有一項）；長按 P → LABOMENU。
**2.2.6 星球層 HOME_SPACE（0x1d，0x1003aff0）[確認]**：A → 圈選單（表 0）；長按 P → LABOMENU；zoom 回到 ≥2.0（其自身尺度）→ `PSID_HOME_FIELD`。

### 2.3 圈選單（ring menu，`ringmenu_create` 0x1002af28）

- 建立：`ringmenu_create(layer, style=1, count, items, init_idx, 0)`；style 表 `0x10094774`（3 × {u16 背景元件, 項目元件, 文字元件}）：style 1 = {0x2ad, 0xa4, 0x9e}。項目 12 B = `{u32 text_id, u32 scene, u32 flags(bit0 灰)}`（**第一欄是字串 id，不是 icon**；06-system §3.3 的「icon」欄應更正）。[確認]
- 畫面 [確認]：背景元件 0x2ad = sprite 204（128×128 粉紅斜紋）於 (64,64)；每項 = 元件 0xa4（sprite 64，128×48 標籤框，palette 1 = 灰）+ 文字元件 0x9e（置中；灰項 palette 4）。
- 版面 = **垂直轉輪** [確認：0x1002ad90]：第 k 項（相對目前選中，k 取 −n/2..n/2）`y = 64 − 88·sin(25°·k)`、`x = 64 + 10·(1 − cos(45°·k))`、縮放 `0.6 + 0.4·cos(45°·k)`、z 依 cos 排序（中間在最上層）。中心點取自元件 0x57 (64,64)。sin 表 `0x1009a968`（1024 項 ×16）。
- 捲動（`listscroll_update` 0x10005f30，每項 4 單位）：轉盤原始計數每 1 → 位置 ±1/4 項，停止時往移動方向吸附到整數項；**A（含連發）= 下一項**；循環。索引改變播音效 0。[確認]
- 確定（B/P）：灰項 → 音效 0xb；否則音效 1、結果 = 索引。C → 音效 2、取消。[確認 0x1002b220]
- 開關動畫：開啟時 `FUN_1001af50(8)`；關閉 `FUN_1001af78(8)`、`ringmenu_destroy`。閒置 200 frames（`flags2 bit4`）自動關閉。[確認]
- 選中後檢查（0x10039eb8 state 5000）：不可用 → `0x18001c70` = 訊息 id、`0x18001c72` = 返回場景（0x1e/0x1f）、`c6a = (c6a & 0xC7) | 4 | idx<<3` → `PSID_NOTICE_MSG`(0x1b)；可用 → `scene_set_next(item.scene)`。回 HOME 時若 `c6a bit2` 設，直接重開選單並把游標放在 `c6a bits3–5`。[確認]

| 選單 | 順序（text → scene） | 不可用條件 → 訊息 |
|---|---|---|
| 寵物（表 2，0x10094290） | FEED 0x44 → GOHAN 0x22、PLAY 0x47 → TOGETHER 0x24、CLEAN TAMA 0x48 → OTEIRE 0x23 | 睡覺中（`tama_is_sleeping_now`）→ 0x69「WAIT UNTIL YOUR TAMA WAKES UP!」 |
| 場地（表 1，0x10094224） | OUTSIDE TOY 0x37 → SETSUBI、EGG HUNT 0x38 → HUNT、CLEAN POOP 0x39 → POOPCLEANING、FIELDS 0x3a → FIELDSWITCH | 睡覺中 → 0x69；SETSUBI rank<2 → 0xf3；HUNT stage<3 → 0x6b、rank 0 → 0xf2；FIELDSWITCH rank<4 → 0xf5 |

### 2.4 PSID_OSEWA_GOHAN（0x22，`gohan_state` 0x1003ff80）

**狀態機 [確認]**

| state | 內容 | 轉移 |
|---|---|---|
| 0 | → 1000 | |
| 1000 分類清單 | `listmenu_create(layer,0,0,0,2,0x10094038,bg=4)`：FOOD(0x33)→值 1、SNACKS(0x34)→值 2；游標記在 ctx+0x24 | 確定 → 記分類 → 0x44c；C → `c6a |= 4`（回 HOME 並重開圈選單）、淡出 |
| 0x44c 食物轉盤 | `inv_list_for_eat(2, cat)`(0x1001e250) 建清單 → `itemlist_create(layer,0,mode 2,…,init,bg 1)` | 確定 → 記 item → 0x4b0；C → 1000 |
| 0x4b0 確認 | `yesno_create(layer,0,0,0x9d "EAT THIS?",1)`，選項 YES(0xff)/NO(0x100) | YES → 拒絕檢查；NO/C → 0x44c |
| 2000 拒絕 | 播 `0x100933a2[sick]`：0x615（吃飽，60 f）或 0x617（生病，50 f），食物 sprite 換成該 item | 動畫完 → 0x44c；任意鍵 → 0xc1c |
| 3000 吃 | substep 0：`req_done(4,item)`、`item_give(item,−1)`、算旗標、播「端上」動畫；1：播「吃」動畫；2：吃飽才播 0x613；3：結束 | 每段任意鍵 → 0xc1c（跳過） |
| 0xc1c | 淡出；有狀態請求 → 0xc80，否則 → 0x44c | |
| 0xc80 狀態特效 | 依待處理請求順序 COLOR(0x15) / LOVE / IMMUNITY / HAPPY / REDU_CM：`req_done(id)` 並播 0x622 / 0x623 / 0x624 / 0x625 / 0x626（COLOR 另播音效 0x1e、把寵物 palette 換新色）；期間暫時清 DIRTY 旗標、結束時還原 | 動畫完或任意鍵 → 0x44c |

**拒絕條件 [確認]**（state 0x4b0）：`onaka == 6` 且分類 ≠ SNACK → 吃飽拒絕；`cond ∈ {2..6}`（感冒/受傷/瀕死/dying/dead）→ 生病拒絕。注意 HOME 本身在感冒/受傷/瀕死時 A 已無法開選單（§2.2.3）。

**動畫選擇 [確認]**：`pref = food_pref_bonus()`（−1→2 討厭、+1→1 喜歡、0→0）；`thin = (cat==SNACK) || onaka_level≥1`（state 0x4b0 時算）；`full = (吃後 onaka ≥6) && cat≠SNACK`；`drink = item[+0x14] bit0`。

| 段 | 動畫 | 長度（f） | 寵物 motion（chara 0 = 目前寵物） |
|---|---|---|---|
| 端上 0x93388[pref] | 0x60c / 0x60f / 0x611 | 75 / 70 / 70 | motion 3→1，喜歡時額外飛出音符 0x716（anim 0x4d8）、討厭 0x4e7 |
| 吃（非飲料）0x93394[pref*2+thin] | 普通 0x5fa/0x600、喜歡 0x5fd/0x603、討厭 0x606/0x609 | 103/155、95/168、160/100 | 0x5fa：motion 1→5→6→7 反覆咀嚼，食物 sprite 0x682 的 frame 0→1→2（每 26 f）表示吃掉 |
| 吃（飲料）0x9338e[pref] | 0x619 / 0x61c / 0x61f | 92 / 105 / 97 | |
| 吃飽 0x933a0 | 0x613 | 80 | motion 17 + 音效 0x11 |

畫面（動畫原點 (64,64)）：背景 `BG -1`（目前 field 背景）；寵物在 (64,96)；食物（sprite 0x682 → 換成 `item[+4]`）在 (64,99)；桌子 sprite 0x786（64×32）在 (64,106)。

**食物轉盤 UI [確認]**（`itemlist_create` 0x1002cc90 + callback 0x1002d2c8；mode 2 版面表 `0x10093f70`）
- 背景元件 0x2af = sprite 204。一次畫 3 張卡（前/中/後，x 偏移 −128/0/+128，隨捲動滑動）。每張卡：白面板元件 0xdd（sprite 67，128×72）於 (64,40)、物品圖元件 0xdc 於 (64,40)（sprite = `item[+6]`）、名字條元件 0xdf（sprite 59 深藍，α 0x33）於 (64,94)、名字文字元件 0xde 於 (64,92)（字串 = `item[+2]`）、數量文字元件 0xd9 於 (8,7)（數量；`buy==0xffff` 的免費品改畫 sprite 0x67 = 無限符號）、元件 0xe6（sprite 71 白色膠囊）於 (64,115)。左右箭頭動畫 0xe/0xf（sprite 89 frame 2/3）於 (10,116)/(120,116)，只在項目 ≥2 時顯示。
- 捲動：每項 6 單位；轉盤原始計數 ±1 單位、停手吸附；**A = 下一項**；確定 = B/P（灰項 0xb）；C = 取消。
- 清單內容（`inv_list_for_eat` 0x1001e250，mode 2）：FOOD 分類**第一項固定**為 PELLETS(0x138a=5002)，嬰兒期（stage 2）**只有** MILK(0x1389=5001)；SNACK 分類第一項固定 JELLY STICK(0x1771=6001)，嬰兒期只有它。之後依背包順序加入符合分類的物品（`FUN_1007d968` 過濾），以 `0x1007dd84` 比較函式排序。免費品吃了也呼叫 `item_give(−1)`（數量本來就 0，被夾住）。

**分類清單 / YES-NO UI [確認]**（`listmenu_create` 0x1006968c、`choice_create` 0x1002efe0，共用 callback 邏輯 0x1002d8ac / 0x1002f524）：A 或 UP → 下一項、DN → 上一項（循環，音效 0）；確定 B/P（音效 1）；C 取消（音效 2）。背景元件 0x2af（sprite 204）；樣式表 `0x10093e5a`（每樣式 14 B：上/下箭頭元件 0xa7/0xa8 = anim 0xc/0xd 於 (96,122)/(116,120)、標題文字 0xa9、列底 0xad（sprite 0x41）、游標 0xb1（sprite 0x5a）、…，每頁列數在 +0xc）。

### 2.5 PSID_OSEWA_OTEIRE（0x23，`scene_oteire` 0x10040df4）[確認]

| state | 內容 |
|---|---|
| 0 | 播 0x64c（背景 + 寵物 motion 1 上下彈，60 f），並在 `elem(5,0x10)` 位置 +(64,86) 疊 0x64d（毛巾 0x787 + 泡泡 0x737 的開場） |
| 1000 | 0x64c 結束 → 播 0x64e（約 320 f，含 5 個 `EVT 4` 暫停點） |
| 1001 | 每到暫停點等**任意鍵**：計數+1，**第 4 次按鍵時 `req_done(5,0)`**（清 DIRTY、yogore=0、gokigen+1），然後繼續；動畫結束 → 淡出回 HOME |

0x64e 內容：海綿 sprite 0x788（32×32）從 (+32,−48) 左右擦（每段 20 f，音效 0x14），泡泡 sprite 0x737（3 frames 漸大）累積在寵物身上，寵物 motion 1→（q=6）。
說明：腳本有 5 個暫停點，所以玩家實際要按 **5 次**；照顧完成在第 4 次。

### 2.6 PSID_OSEWA_POOPCLEANING（0x28，`poopclean_state` 0x10044f78）[確認]

- init：`field_list_visible`(0x100167d8) 列出可見 field（`field_visible` 0x1006258c：rank<4 只有家鄉 field（`0x18001cb0` 5/6/4 → field 0/1/2）；rank 4–5 → field 0、1（cb0==4 時無）；rank ≥6 → 全部），從目前 field 開始輪。
- state 1000，每個 field：畫該 field 背景；**第一個 field** 播 UI 元件 0x2da（anim 0xe8：吸塵機器人 sprite 0x459（48×96，2 frames 交替 10 f）從 x=+88 掃到 −88，寵物 motion 30→41 跟著跑，音效 0x3d，60 f），並把每坨大便（元件 0x2dc+i，舊大便 0x2e2+i，位置同 §2.2.2；anim 0xe6/0xe7 = sprite 0x458 frame 0/1 每 15 f）往上飛 128 px 吸走；其他 field 播 0x2db（anim 0xe9，只有機器人，60 f）。
- 全部 field 播完 → `r = care_clean_unchi()`（0x100108d4）。r == 0 → 直接淡出回 HOME。
- r > 0 → `care_add_gokigen(1)`（反組譯：BL 前 `movs r0,#1`）→ state 2000：UNCHI METER 視窗（`unchimeter_create` 0x10067204），`unchimeter_add(w, r)` 0x1006706c：`0x18000d13 += r`（上限 60，`FUN_10062748`），播 anim `0x114 + min(r−1,6)`（代幣 sprite 0x4ad 飛入），log `UNCHI METER: %d ADD: %d` → 等它結束 → state 3000：播 0x576（寵物 motion 30→31 開心跳 + 星星 0x719，84 f）→ 淡出。

### 2.7 吃藥（PSID_EVENT_CURE_COLD 0x50，`scene_medicine` 0x1003e4a4）[確認]

- 進入：**HOME_CELL** 行為 0x17（cond 2 感冒）時任意鍵（不是選單項目）。
- state 0：`0x18001be8 |= 0x40`（暫停 care 時間推進）；播 0x5ae。
- state 1000：每遇 `EVT 4` 暫停 → 任意鍵繼續。0x5ae = 細胞內部背景 sprite 0x488（256×256）+ 胞器 0x489 漂浮 + 5–6 隻病菌 sprite 0x48c（32×32；frame 1 = 被打中）；每次按鍵一隻病菌抖動（x ±3、每 2 f）後飛出畫面（音效 0xe）；寵物在中央（chara 1 = 細胞內寵物，motion 3 → 2 → 1 逐步好轉）。共 **5 個暫停點 = 5 次按鍵**，總長約 160 f。
- 0x5ae 結束 → 播 0x5bb（寵物 motion 1→2 開心，音效 0xf，120 f）。
- state 1001：0x5bb 結束 → `c6a` zoom 設為寵物層、`req_done(10,0)`（gokigen+1、cond 0 或 3、DeathByCold=0）→ 淡出回 HOME。

### 2.8 受傷治療（PSID_EVENT_CURE_INJURY 0x51，`scene_injury_treat` 0x1003e664）[確認]

與 §2.7 同結構：寵物層行為 0x18 任意鍵進入 → 播 0x5c0（4 個暫停點 = **4 次按鍵**，約 180 f）→ 播 0x5c8（120 f）→ zoom 設為場地層、`req_done(0xb,0)` → 回 HOME。

### 2.9 起床 / 睡覺互動（PSID_EVENT_TO_WAKE 0x55 / TO_SLEEP 0x56，`wakesleep_state` 0x1003ecd4）[確認]

- 進入：寵物層 §2.2.4。
- state 0：WAKE 待處理 → `yesno(0x66 "WAKE UP YOUR TAMA?")`；否則 SLEEP 待處理 → `yesno(0x67 "TIME TO SLEEP?")`；都沒有 → 直接離開。
- YES：WAKE → `0x18000cba++`（≤50）、`req_done(2)`、白色淡出；SLEEP → `0x18000cb8++`（≤50）、`req_done(3)`（→ 睡覺 done 的副作用見 01-care §2.6/2.8）。
- NO / C：WAKE → `c6a |= 0x80`；SLEEP → `c6b |= 1`。兩者在 `care_day_change`（0x10013234）清除 → **當天不再提示**（之後仍會在 10:00 自動起床、22:00 自動睡）。
- 動作外觀：睡覺中行為 2（motion 50/15 + zzz）。

### 2.10 想大便 → 廁所（PSID_EVENT_UNCHI_KAISHU 0x53，`unchikaishu_state` 0x1003e90c）[確認]

HOME 行為 0x11 任意鍵 → 先 `req_done(0xe,0)`（不產生大便、gokigen+1，見 01-care §2.4）→ 本場景：播 0x575（180 f）→ UNCHI METER 視窗 `+2` → `req_done(0x19 DOSSARI,0)` → 回 HOME。

### 2.11 PSID_CELL_STATUS（0x29，`cellstatus_state` 0x100452f0）[確認]

| 頁 | 元件（座標） |
|---|---|
| 0（state 2000） | 背景 0x296 = sprite 200（紫斜紋）；框 0x297 = sprite 201（含 6 個空飢餓格 + 20 段空心情條 + 年齡框）；「HUNGER」(0x9e) 文字 0x298 於 (10,11)；飢餓格 0x299–0x29e = sprite 202 於 x=16,35,54,73,92,111、y=34，第 i 格在 `i < onaka` 時顯示；「HAPPY」(0x9f) 0x29f 於 (10,58)；心情條 0x2a0 = sprite 203（128×24，21 frames）於 (64,81)，**frame = gokigen（0–20）**；年齡數字 0x2a1 於 (56,110) = `g_tama_days`；「YEARS」(0xa0) 0x2a2 於 (72,110)；頁面箭頭 0x98 = anim 0xd 於 (116,120) |
| 1（state 3000） | 背景 0x2a5（sprite 200）；名字條 0x2a7（sprite 61）於 (64,20) + 名字文字 0x2a6 於 (64,18)（`FUN_10018394(chara)`）；說明條 0x2a9（sprite 65）於 (64,70) + 文字 0x2a8 = `chara_def[+4]` 字串 |
| QR（state 4000） | `HTTPS://TMGCP.TYB.JP/` + 加密的 QRUID/寵物資料 → QR 碼（`FUN_100290a8`，64×64 於中央）；標題 0x2ba「SECRET CODE」(0xa4) 於 (64,16) |

輸入（state 5000）：A 或 UP → 下一頁（0↔1，循環）；DN → 上一頁；B → QR 頁；C → 離開；**200 frames 無操作 → 離開**。QR 頁（0xfa1）：任意鍵 → 回原頁；600 frames → 離開。翻頁有淡出/淡入，音效 1（離開 2）。首次進入時產生 QRUID（`0x1800168c/90`）。

### 2.12 新遊戲背包與食物來源

| # | 規則 | 證據 | 標記 |
|---|---|---|---|
| I1 | 遊戲開始（PRAM `game_start` 0x8ab0：設 `be8 bit0`）呼叫 `inv_give_starter` 0x1001e558：表 `0x10093d20` 3 × {id, n} = **5068 TROPICAL FRUITS ×3、5076 FROZEN MEAT ×3、5058 BAMBOO GRASS ×3** | ram.c 0x8ab0、XIP 0x1001e558 | [確認] |
| I2 | 存檔重設（PRAM 0x89a8）放入 7998 NICE BOX（小型戶外玩具），並由出廠資訊 `0x60d29000`（`0x18001cb0`，缺值 → 1）決定起始 field（`FUN_1002b314`：1/5→0、2/6→1、3/4→2） | ram.c 0x89a8 | [確認]（NICE BOX 數量參數 [推測] 1） |
| I3 | 免費無限食物：PELLETS 5002（嬰兒期換 MILK 5001）、JELLY STICK 6001（買價 0xffff、不在背包、永遠在清單第一項） | 0x1001e250 | [確認] |
| I4 | 取得食物：商店（LABOMENU→SHOP，Food/Snack 分頁需 rank ≥1，見 05-content §2.4）；EGG HUNT 每顆蛋 `item_give(food,1)`，表 `0x100a0600`（6 B `{u16 sprite, u16 item, u16 idx}`，索引 `field*8 + 變體*4 + rand(0..3)`，field 0 → 5066–5071、1 → 5076–5081、2 → 5056–5061）；料理（消耗 3 個材料）| 0x10041df8、0x1004b624 | [確認] |
| I5 | 新玩家 rank 0 → 商店食物分頁不能用 → 開局可吃的就是 I1 + I3 | 同上 | [確認] |

### 2.13 HOME 路由到事件場景（`FUN_1001abd0`）[確認]

每次進 HOME（0x1c）：先 done HOMETRG/UNCHI，然後依序：cond 6 → **EVENT_DEAD 0x4a**；cond 5 → **EVENT_DYING 0x49**；rank 上升 → 0x7d；mission → 0x39；**cond 1 → HELP_SITTER 0x41**；狀態 request → 0x4e；隨機事件。所以死亡、臨終、保母期間玩家根本看不到 HOME。cond 4（DEATH 急救窗口）不路由，HOME 顯示狀態 0x19。

### 2.14 急救（PSID_EVENT_RESCUE 0x54，`FUN_1003eae4`）[確認]

- 進入：寵物層（zoom 0x20000）HOME 狀態 0x19 時任意鍵（0xf）→ 音效 1 → 0x54（`home_field_tama_state` 0x10039eb8 state 0x7d0）。場地層、細胞層不行。
- 若 DEATH 不在待處理 → 直接離開。否則播 0x5ce；腳本暫停（signal 4）時任意鍵或逾時（場景時鐘差 > 0xa0000）繼續；0x5ce 結束 → 播 0x5d4 → 結束時 `req_done(DEATH,0)`（cond=0、DeathRemain=DeathByCold=cold=0、DeathByHungry = onaka==0 ? 129600 : 0）→ HOME。
- 重製：`ui/event.rs::rescue`（Play 0x5ce→0x5d4，`OnPress::Rescue`）。逾時自動繼續沒做（ponytail）。

### 2.15 臨終（PSID_EVENT_DYING 0x49，`FUN_1003d474`）[確認]

| state | 內容 |
|---|---|
| 0 | `g_tama_days` < 30 → 播 0x5db，否則 0x5ed（大往生） |
| 1000 | 等 DeathRemain（TamaCare+0x64，DYING 時設 60 s）到 0。然後 `FUN_1000af5c` 找死亡紀錄中第一筆 **bit12（大往生）仍設著**的格子：有 → 2000；沒有 → 天數 < 30 → 3000，否則 5000 |
| 2000 | **祖先來救**：播 0x5e0，角色 0 = 寵物（0xbd）、角色 1 = 0xd6+格子（ghost slot 0x19+格子，也就是那位大往生的前代）。結束 → `FUN_1000b134` 清掉該筆 bit12（每位祖先只救一次）→ `req_done(DYING,1)`：cond=0、DeathRemain=0 → HOME |
| 3000 | 年輕死亡：播 0x5e7 → 結束 `req_done(DYING,0)`：清 DIRTY、發 DEAD（寫死亡紀錄，cond 6）→ 4000 |
| 5000 | 大往生：播 0x5ef → 結束 → 清 DIRTY、播 0x5f4 → 結束 `req_done(DYING,0)` → 4000 |
| 4000 | 墓地：UI 0x2c8（anim 191）、0x2c9（anim 192）；5 個墓位 0x2d0+i（sprite 1107，座標 (30,28)(58,56)(92,26)(22,68)(44,104)），新墓播 anim `200 + bit12`，其他已用的墓 frame = bit12；UI 0x2ca（anim 194，(90,92)）播完 → HOME → 路由到 EVENT_DEAD |

- Rust 核心原本在 DYING 的 60 s 到時自己發 DEAD；改成等臨終場景（`Pet::end_dying`），和原版一樣。離線期間 DYING 停在 0，下次開 HOME 時場景立刻接續。
- 重製：`Grave::spirit`（bit12）、`Pet::dying_ancestor`、`Pet::end_dying(Some(i)|None)`、`Scene::Dying`、`Scene::Graveyard`。大往生的清 DIRTY 提前到 0x5ef 之前（ponytail）。

### 2.16 墓碑（PSID_EVENT_DEAD 0x4a，`FUN_1003ccb0`）[確認]

- 頁 0（墓碑）：layout 0x13f（sprite 109）；花 0x142/0x143（anim 21，(24,110)/(104,110)，進場有 y tween 未做）；墓碑 0x140（anim 169）或大往生 0x141（anim 170），(64,96)。
- 輸入：**B** → 音效 1 → 紀錄頁；**按住 C 再按 A** → 音效 1 → 單項清單「NEW EGG」(0x18)。
- 紀錄頁（state 2000）：背景 0x194（sprite 141），條 0x147/0x14a/0x14d；名字 0x146 (64,18)、天數 0x148 (60,47)（最後一筆死亡紀錄的 days）、「YEARS」0x149、「CAUSE」(0x79) 0x14b、死因 0x14c：cause 1 → 「TAMA SICK」0x7a、cause 3 → 「HUNGER」0x7c。C（或閒置逾時）→ 音效 2 → 回墓碑。
- NEW EGG 清單：選定 → 場地背景、播 0x672、HOME 層 = 場地（`0x18001c6a` = 1）→ 結束 `tama_birth_reset(0)` 0x1000f708 → HOME。取消 → 回墓碑。
- 重製：`Scene::Dead`、`Action::Tomb/NewEgg`、`OnPress::NewEgg`。

### 2.17 嬰兒進化支援（PSID_EVENT_EVOL_SUPPORT 0x4b，`FUN_1003bd20`）[確認]

- 進入：細胞層（zoom 0x10000）HOME 狀態 0x1a（EVOL_SUPPORT 呼叫）時任意鍵 → 音效 1 → 0x4b（`FUN_100398c8` state 1000）。15 分鐘內沒按，`care_tick` 逾時：刪掉 dominant 區域的紀錄、support miss +1（02-evolution §2.4）。
- state 0：stage 2、age < 14400 且 EVOL_SUPPORT 待處理 → 1000；只有 EVOLUTION → 直接 7000；其他（非嬰兒或 age ≥ 14400）→ 3000（把整個區域／食物紀錄做成 icon 一個個吸進去，然後 7000）。
- 1000：寵物播 0x54a（等待）；4 個 icon 放在 UI 0x35a/0x377/0x374/0x364 的座標（(38,42)(98,28)(18,114)(104,98)），sprite = `FUN_1000ae44(dominant)`：0x4a0/0x4a1/0x49c（dominant 3 → 0x49c）；目標點 (64,54)。
- 2000…0x7d7：四輪。每輪任意鍵（0xf）或 200 frames → 寵物播 0x547、一個 icon tween 到目標並淡出、音效 1；40 frames 後寵物回 0x54a。順序是 icon 0、3、2、1。
- 0x7d8：20 frames → 7000：support 計時器清零、進 EVENT_EVOLUTION 0x4c，那裡 `req_done(EVOL_SUPPORT)` → `tama_evolve`（用 dominant 區域進化，+300）。
- 重製：`Scene::Support`、`Pet::support`。icon 飛行用線性 20 frames（ponytail：tween 曲線 5 未解）。

### 2.18 保母（PSID_HELP_SITTER 0x41，`FUN_1004d304`）[確認]

- 叫保母（LABOMENU→HELP→TAMA SITTER）：state 0 → 檢查 stage ≥3（0x6b）、06:00–18:58（0xd6）→ 說明 0xd5「到 PM7:00 為止照顧，一次 300」→ YES/NO 0xd3（預設 YES）→ 再檢查時間、onaka/gokigen > 0（0xd7）、未睡（0xd9）→ **cond = 1**、`be8 |= 0x80`、扣 300 → 播 0x831（保母來）→ 2000。
- 2000：0x832 / 0x833（80% / 20%，每次播完重抽）循環。cond 不再是 1（19:00 由 `care_time_update` 結束並 gokigen −5）→ 3000：播 0x834 → HOME，層 = 場地。任意鍵 → 0xd4「END TAMA SITTING?」（**預設 NO**）：YES → 播 0x834 → cond=0（不扣心情、不退點數）→ HOME；NO → 回 0x832。
- 保母期間 HOME 路由永遠進 0x41，玩家看不到 HOME。`care_tick`：cond 1 只跑 onaka 計時器、累加 UNCHI METER，不掉飢餓、心情（01-care §2）。
- 重製：`Scene::Sitter`、`Action::SitterView/EndSitter`、`Pet::end_sitter`。

---

## 3. 資料表與格式

### 3.1 UI 元件定義（data pack 表 4 索引 + 表 5 資料，`ui_element_def_get` 0x10023124）[確認]

`rec = T5 + T4[id]*2`（T4 為 u16 半字偏移）。`u16 hdr`：高 4 bit = 型別；接 `s16 x, s16 y`（固定）；其後依 hdr 位元依序出現選填 u16。輸出陣列 p[0..10]：

| 型別 | 名稱 | 選填欄（bit → p[slot]，預設） |
|---|---|---|
| 0 | 群組 | 0x800→p0 |
| 1 | SPRITE（bit15 → 型別 9） | 0x1→p3 **sprite id**、0x8→p4 旋轉、0x10→p5 縮放(0x1000)、0x20→p6 alpha(0x40)、0x40→p7 frame、0x80→p8 palette、0x100→p9 旗標、0x400→p10 z、0x800→p0 |
| 2 | TEXT | 0x1→p3、0x8→p4、0x10→p5、0x40→p6 alpha、0x80→p7、0x100→p8、0x400→p9、0x800→p0（p0 為 0x10xx 文字格式 id） |
| 3 | ? | 0x8→p3(1)、0x10→p4(1)、0x20→p5、0x40→p6、0x100→p7、0x400→p8、0x800→p0 |
| 4 | ? | 0x1→p3、0x8→p4、0x10→p5、0x20→p6、0x100→p7、0x200→p8、0x800→p0 |
| 5 | ANIM | 0x1→p3 **動畫 id**、0x100→p4、0x200→p5 z、0x800→p0 |
| 6 | ? | 0x1→p3 |
| 7 | ? | 0x1→p8、0x8→p3、0x10→p4、0x20→p5、0x40→p7、0x80→p6、0x800→p0 |
| 8 | ? | 0x1→p0、0x8→p4、0x10→p5、0x40→p6(0x40)、0x80→p7、0x100→p8、0x400→p9、0x800→p3 |

`FUN_10023a74(id)` / `FUN_10023ab4(id)` = 取元件 x / y（常被當作版面錨點）。

### 3.2 動畫腳本（data pack 表 13 索引 + 表 14 資料，`anim_create` 0x100024c0、直譯器 `anim_step` 0x100016d0）[確認]

- `data = T14 + T13[id]*4`，長度（u16 個數）= `(T13[id+1]−T13[id])*2`。`data[0]` = 旗標（bit0 = 結尾循環到 LOOP 點，否則結束時自刪）。之後為指令串，`0xfcbd` = 結束。
- 指令 u16：低 8 bit = opcode；高位元決定參數：bit15 → a（id）、bit14 → z（ease 0–6 或 z 偏移）、bit13 → p、bit11 → q、bit10 → r（tween 長度）、bit9 → wait（執行後等待 frames，0x7fff = 永遠）。tween 的 p=0x7fff 表示「從目前值」。
- opcode：1 SPR(a=sprite, p=frame)、2 FRM(p)、3 PAL(p)、4 TX、5 TY、6 TXY（x/y tween：from p → to q，長 r，ease z）、7–0xc 縮放/旋轉 tween（7 為縮放，256=1.0）、0xd 顯示層/alpha、0xe SND(a=音效 id, p=音量%；p=0 → 停止)、0xf OBJ（生成道具物件）、0x10 CHILD(a=動畫, p=x, q=y, z)、0x11 翻轉旗標、0x12/0x13 layer 特效、0x16/0x17 旗標、0x19 BG（callback 4：畫 field 背景，p=−1 目前 field，−2 無）、0x1a EVT（callback 2：p=−1 → 動畫完成旗標；p=4 → 暫停等外部 resume）、0x1b LOOP（記錄循環點）、**0x1c CHARA**（a=角色佔位：0 = 目前寵物、1 = 細胞內寵物、0x1f = 蛋；p=motion 編號 = 角色模型動畫 index（04-assets C7）、q=表情/變體）、**0x1d MOTION**(p, q)、0x1e、0x20 KILL（刪除所有子動畫）、0x24（寵物子屬性，p）。
- 寵物動畫物件替換表（`petanim_subst` 0x100006dc，槽表 `0x1009cbac`）：槽 0 = sprite 替換（例：0x682 → 食物 sprite），槽 1 = chara 替換，槽 4 = 另一角色欄位。

### 3.3 其他表

| 位址 | 格式 | 內容 |
|---|---|---|
| 0x10094774 | 3 × {u16 bg, item, label} | 圈選單樣式：0={0x192,0xa4,0x9e}、1={0x2ad,0xa4,0x9e}、2={0x9f,0xa4,0x9e} |
| 0x10094120 | 4 × {ptr, count} | 圈選單項目表（星球 5、場地 4、寵物 3、細胞 1） |
| 0x10094038 | 12 B × 2 | 餵食分類：{0x33 FOOD, 0, 1}、{0x34 SNACKS, 0, 2} |
| 0x10093388 / 0x1009338e / 0x10093394 / 0x100933a0 / 0x100933a2 | u16 | 餵食動畫（§2.4） |
| 0x10093f70 | 15 × u16 | 食物轉盤（mode 2）版面元件：0xd7,0xd8,0xdc,0xdd,0xd9,0xda,0xde,0xdf,0xdb,0xe2,0xe3,0xe4,0xe5,0xe6,0 |
| 0x10093eb6 / 0x10093e9a | {u16 elem, u8 pal} | 清單背景：0x2af = sprite 204、0xa5 = sprite 51、0x19a |
| 0x10093e5a | 14 B / 樣式 | 清單樣式（箭頭、列底、游標元件、每頁列數） |
| 0x1009dbd8 | 32 × 3 × u16 | HOME 寵物行為 → 動畫（場地/寵物/細胞），§2.2.2 |
| 0x100a05b4 | u8[27] | 隨機待機行為循環表：5,1,2,6,4,9,7,3,8,9,2,7,1,8,5,6,3,4,2,5,3,4,1,8,7,9,6 |
| 0x10092e06 | 3 field × 2 × 16 × u16 | field 背景元件（[0] 位置，[1+period*3+weather]） |
| 0x100a1062 | 6 × u16 | 場地大便位置元件 0x2e8–0x2ed |
| 0x100a106e | 12 × u16 | 清大便動畫元件 0x2dc–0x2e1（新）、0x2e2–0x2e7（舊） |
| 0x10093d20 | 3 × {u16 id, s16 n} | 開局背包 5068×3、5076×3、5058×3 |
| 0x1009a968 | s16[1024] | sin 表（×16 → 16.16） |
| 0x100914a4 | 2 × 12 B | YES(0xff)/NO(0x100) 選項 |

**物品定義欄位更正**：`item[+2]` = **名稱字串 id**（items.csv 的 `f1_idx` 欄，例：5001→297 "MILK"），`item[+4]` = 吃東西時的 sprite、`item[+6]` = 清單圖示 sprite（items.csv 誤標為 `name_txt/name_txt2`；5001→1666 奶瓶、5002→1667、6001→1751）。`item[+0x14]` bit0 = 飲料（用 0x619 系列動畫）。

### 3.4 Sprite 對照（已用 `extract_sprites.py` 輸出 PNG 目視確認）

| sprite（dec/hex） | 尺寸 | frames | 內容 / 用途 |
|---|---|---|---|
| 204 / 0xcc | 128×128 | 1 | 粉紅斜紋背景（圈選單、食物轉盤、分類清單） |
| 64 / 0x40 | 128×48 | 1（2 palette） | 圈選單標籤框（palette 1 = 灰） |
| 38 / 0x26 | 128×32 | 2 | 白/灰橫條（元件 0x57 = 轉輪中心參考） |
| 51 / 0x33 | 128×128 | 1（4 pal） | 藍斜紋背景（元件 0xa5） |
| 46 / 0x2e | 96×64 | 1 | 物品圖佔位（木馬，被 item sprite 取代） |
| 67 / 0x43 | 128×72 | 1 | 白色物品面板 |
| 59 / 0x3b | 128×24 | 1 | 深藍名字條 |
| 71 / 0x47 | 96×24 | 1 | 白色膠囊（價格/說明欄） |
| 89 / 0x59 | 16×16 | 4 | 箭頭：上、下、左、右 |
| 97 / 0x61 | 40×16 | 1 | 「50% OFF!」標籤 |
| 19 / 0x13 | 16×16 | 17 | 數字字型 0–9 + − ♪ / : G |
| 47 / 0x2f | 16×16 | 1 | 藍色小圓點（新品記號） |
| 1666 / 0x682 | 32×32 | 3 | MILK 奶瓶（滿/半/空）；也是腳本內食物佔位 |
| 1667 / 0x683 | 32×32 | 3 | PELLETS（滿/半/空） |
| 1751 / 0x6d7 | 32×32 | 3 | JELLY STICK |
| 1926 / 0x786 | 64×32 | 1 | 餵食用玻璃桌 |
| 1814 / 0x716 | 32×32 | 2 | 音符（喜歡/開心反應） |
| 1817 / 0x719 | 16×16 | 2 | 橘色星星閃光（清大便後開心） |
| 1112 / 0x458 | 16×16 | 4（2 pal） | 大便（palette 1 = 舊大便） |
| 1113 / 0x459 | 48×96 | 2 | 吸塵機器人 |
| 1197 / 0x4ad | 16×16 | 2 | UNCHI METER 代幣 |
| 1928 / 0x788 | 32×32 | 1 | 海綿（OTEIRE） |
| 1847 / 0x737 | 32×32 | 3（5 pal） | 泡泡（小→大） |
| 1927 / 0x787 | 32×16 | 2 | OTEIRE 開場道具（黃色條狀，推測毛巾） |
| 1160 / 0x488 | 256×256 | 2 | 細胞內部背景（吃藥） |
| 1161 / 0x489 | 32×32 | 7 | 胞器（粒線體、核糖體…） |
| 1162 / 0x48a | 32×32 | 1 | 陰沉線條（生病寵物頭上） |
| 1164 / 0x48c | 32×32 | 2 | 病菌（一般 / 被打中） |
| 200 / 0xc8 | 128×128 | 1 | 狀態畫面紫斜紋背景 |
| 201 / 0xc9 | 128×128 | 1 | 狀態頁框：6 個灰色飢餓格、20 段空心情條、年齡框 |
| 202 / 0xca | 16×16 | 1 | 填滿的飢餓格（解碼後為小橘色塊；形狀待與實機比對） |
| 203 / 0xcb | 128×24 | 21 | 心情條 0–20 段（青→黃→粉漸層） |
| 1132–1137 / 0x46c–0x471 | 512×512 | 1 | field 0（熱帶）背景，依時段/天氣（1134 = 白天晴） |
| 1837 / 0x72d | 16×16 | 4 | zzz（睡覺） |
| 1832 / 0x728 | 32×32 | 1 | 波浪線（想大便） |
| 1843 / 0x733 | 16×16 | 2 | 骷髏（感冒） |
| 1828 / 0x724 | 32×32 | 2 | 傷痕 +/×（受傷） |
| 1869 / 0x74d | 32×32 | 1 | 黑色天使（瀕死） |
| 141 / 0x8d | 128×128 | 1 | 粉紅方格背景（圈選單樣式 0） |

---

## 4. 新命名

```
10006140 dial_delta f
1000ed48 zoom_update f
1000ec3c zoom_snap f
1000ed14 zoom_init f
1000ef2c zoom_anim_to f
1000ef84 zoom_anim_from_current f
1000eff4 zoom_set_limits f
1000f028 zoom_set f
100687dc home_zoom_dispatch f
100793fc home_camera_update f
10078c94 home_pick_behavior f
1008461c home_behavior_anim f
1008467c home_pet_anim_set f
10084df8 home_scene_build f
100859f8 home_zoom_enter f
10085c14 home_to_space f
10085bb4 home_to_cell f
10039eb8 home_field_tama_state f
100398c8 home_cell_state f
1003aff0 home_space_state f
1001abb4 ringmenu_table_get f
1002b220 ringmenu_input f
1002ad90 ringmenu_item_layout f
1002b188 ringmenu_result f
1002b174 ringmenu_selected f
1002b160 ringmenu_cancelled f
1002b124 ringmenu_item f
10005ee8 listscroll_init f
10005f30 listscroll_update f
1006968c listmenu_create f
1002d8ac listmenu_input f
1002cc90 itemlist_create f
1002d2c8 itemlist_cb f
1002d038 itemlist_card_info f
1002d224 itemlist_card_draw f
1002f1f8 yesno_create f
1002efe0 choice_create f
1002f524 choice_input f
1002f270 choice_result f
1003ff80 gohan_state f
10044f78 poopclean_state f
10070194 poopclean_show_field f
100167d8 field_list_visible f
1006258c field_visible f
10062748 unchi_meter_add f
10067204 unchimeter_create f
1006706c unchimeter_add f
100452f0 cellstatus_state f
1003ecd4 wakesleep_state f
1003e90c unchikaishu_state f
1001e250 inv_list_for_eat f
1001e558 inv_give_starter f
1001e594 inv_set_count f
10010b0c onaka_level f
10010ac4 gokigen_level f
10000028 petanim_play f
10000154 petanim_stop f
10000568 petanim_resume f
10000584 petanim_done f
10000594 petanim_paused f
100006a8 petanim_subst f
10000674 petanim_subst_chara f
100006dc petanim_subst_set f
10000164 petanim_event_cb f
100024c0 anim_create f
100016d0 anim_step f
10022d98 anim_data_get f
10022dd8 anim_data_len f
1007eea0 ui_element_create f
1007f5c4 ui_sprite_create f
1007f34c ui_text_create f
1007ecdc ui_anim_create f
10023a74 ui_element_x f
10023ab4 ui_element_y f
10015d90 field_bg_create f
10015ef0 field_bg_cb f
1000aeac field_bg_table f
1007b40c fade_set f
10031b74 sin_q16 f
10009b84 cos_q16 f
1000a158 layer_create f
00008ab0 game_start f
000089a8 save_reset_defaults f
10094774 ringmenu_styles l
10094038 gohan_categories l
10093388 gohan_anim_present l
10093394 gohan_anim_eat l
1009338e gohan_anim_drink l
100933a2 gohan_anim_refuse l
10093f70 itemlist_layout_eat l
1009dbd8 home_behavior_anims l
100a05b4 home_idle_cycle l
10092e06 field_bg_layouts l
100a1062 field_poop_elems l
100a106e poopclean_elems l
10093d20 starter_items l
1009a968 sin_table l
```

---

## 5. 未解問題

1. **轉盤每格（detent）對應幾個原始計數**：縮放/轉輪都用原始計數，手感取決於硬體（06-system：3 計數 = 1 個 bit6/7 脈衝，推測 1 detent ≈ 3 計數？）。需實機量測。
2. **世界座標與縮放**：512×512 背景在倍率 1.0 時是否真的只顯示 128×128（鏡頭跟焦點），還是 layer 另有 1/4 基礎縮放；需在模擬或實機截圖比對（`home_camera_update` 寫 layer +0xc..+0x20）。
3. 動畫 opcode 7–0xc、0x12/0x13/0x16/0x17/0x24、`OBJ`(0xf) 生成的物件類型（`FUN_10022f38` 表）、CHARA 的 q 參數（表情？）語意未完全確認；motion 編號對應的動作需配合 04-assets C7 角色模型逐一渲染命名。
4. HOME 場地層物件清單（`FUN_10016c64`：玩具、寵物，最多 5 個）與 C 鍵焦點順序的細節；FIELD2 畫面未展開。
5. 文字元件 p[0]（0x10xx）是字型/樣式 id，其版面（字型、對齊、顏色）細節在 `textbox_init` 0x10064d90，未展開。
6. 分類清單樣式表 0x10093e5a 各欄的完整語意；圈選單開/關的過場（`FUN_1001af50/af78(8)`）。
7. 狀態第 2 頁說明文字 `chara_def[+4]` 與名字 `FUN_10018394` 的確切來源；QR 資料加密（`FUN_10069de0`）屬另一子系統。
8. ~~細胞層的 EVOL_SUPPORT（0x4b）~~ 已解（§2.17）；星球層選單各項目未在本文件展開（見 05-content）。
9. 急救腳本 0x5ce 暫停點的逾時（場景時鐘差 > 0xa0000）換算成幾秒；墓碑頁花的進場 tween（`FUN_10066968` +0xb0，從 y−256）；支援場景 icon 的 tween 曲線 5。
