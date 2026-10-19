# 09 — Tama Space Arcade（CONNECTION → MINI GAMES，scene 0x83–0x86）

範圍：星球選單 CONNECTION 第 5 項「MINI GAMES」（字串 0x4e）→ PSID_TAMASPACE_ARCADE（0x83）選單 → 兩個連線遊戲 0x85 HOCKEY（`arcade_pong_play` 0x100860f8）、0x86 SHOOTING（`arcade_shoot_play` 0x1008764c）→ PSID_TAMASPACE_ARCADE_RESULT（0x84，`FUN_100883e0`）。
動畫 = motion script（`decode_motion.py <id>`）；UI 元件 = data pack 表 4/5（`ui_ui.py <id>`）；「anim 0xNNN」= motion script，「elem 0xNNN」= UI 元件，「sprite 0xNNN」= sprite pack index。RW 0x18xxxxxx 的初始值 = PRAM 0xe6a0 解壓（`extract_minigames_b.py` 的 `rwdata()`）。
重製：`crates/tama-core/src/mini/arcade01.rs`、`arcade02.rs`、`ui/connect.rs`。沒有連線線材，所以朋友那一側由電腦（CPU）扮演，見 §4。

---

## 1. 摘要

- 兩個遊戲都是**兩台機器各顯示自己那一半**的同一個場地：host（`*thunk_FUN_00001dd0() & 1`）跑物理／產生目標，guest 只送輸入、收結果。座標一律用 host 座標；guest 畫面 = (128 − x, 鏡像 y) **[確認]**。
- 每 frame（20 fps）跟對方交換一個封包（`FUN_10060be8` 送、`FUN_10060a88` 收，最多等 100 ms）。**時間單位是 frame，不是 100 ms**：剩餘秒數顯示 = ceil(ticks / 20)（`FUN_1008fb7c`、`FUN_100664f8`），所以 2400 ticks = 120 秒、1200 ticks = 60 秒 **[確認]**（05-content 的「100 ms」推測要更正）。剩 9 秒以下才顯示倒數（sprite 0x80f (120,11) + 數字）。
- 開場流程（兩個遊戲相同）：scene 1000 連線對話框（`FUN_100088b4`）→ 2000 開場動畫（HOCKEY host anim 0xb4b／guest 0xb4c；SHOOTING host 0xb66／guest 0xb67），role 1 = 對方的 tama → 1000 操作說明 demo（「HOW TO PLAY」字串 0x32d，elem 0x5b1–0x5b5，至少 40 frames，按鍵或 5 秒）→ 3000 遊戲：setup → 3-2-1 倒數（`FUN_10002a24`：60 ticks，sprite 0x8c8 frame = 秒−1，每秒音效 1）→ 遊戲 → 結果面板 → 4000 最後一次交換（失敗 → 字串 110「CONNECTION FAILED」、**沒有點數**）→ scene 0x84 **[確認]**。
- 入口條件（`FUN_1008cffc` case 4）：成長階段 < 3 → 字串 0x6b（107「STILL CAN'T…」）；`tama_stat_test(0x10)`（髒）→ 先處理 EM_REQ_OTEIRE（state 2000）。免費 **[確認]**。
- 結果（scene 0x84，`FUN_100883e0`）：播 ARCADE_RESULT 動畫（表 0x10095750，見 §2.6／§3.6）→ mission 11 計數器 `0x18000cbc` +1（上限 100，輸贏都算）→ 更新朋友紀錄（§1.1，`FUN_10017588`）→ `points_add(DAT_18001c90)` 並顯示點數 **[確認]**。結果碼另存一份到 `DAT_18001ca8`，韌體內沒有任何讀取 **[確認：只有寫入]**。

| 遊戲 | 選單字串 | 時間 | 結果碼 → 點數（`FUN_1008709c`／`FUN_1008805c`） |
|---|---|---|---|
| 01 HOCKEY（0x335 = 821） | 2400 frames | 0 贏 / 1 輸 / 2 平 → 500 / 100 / 250（`FUN_1007afb8` 比較兩邊進球數） |
| 02 SHOOTING（0x336 = 822） | 1200 frames | 分數 ≤10 → 0、11–50 → 1、≥51 → 2 → 100 / 250 / 500（`FUN_1007b00c`） |

### 1.1 朋友紀錄（friendship）[確認]

- 連線對話框（`FUN_100088b4` → callback `FUN_1006e2d8` → `FUN_1006e36c`）交換後，雙方用 `FUN_100173c4` 查自己存的 friendship：任一方是 0（沒見過）→ 本局 friendship = 0；否則雙方都取較大值。存在 link 結構 +0x2c（對方 planet/uid 在 +0x24/+0x28）。
- scene 0x84 state 1000（`FUN_100883e0`）：friendship = clamp(本局 + `FUN_1006cb9c`, 1, 4)，last result = 5，用 `FUN_10017588` 新增／更新朋友（滿了就不記）。log：「FRIENDSHIP: %d RESULT: %d」。
- `FUN_1006cb9c`（arcade 專用，**不是** playdate 的 `FUN_1006cb04`）：本局 friendship 為 0 → +0（第一次見面記成 1）；HOCKEY → +1（輸贏都一樣）；SHOOTING → 結果碼 ≠ 0（分數 ≥ 11）+2，否則 +1。

---

## 2. Arcade 01「HOCKEY」（空氣曲棍球）

### 2.1 場地與物件 [確認]

| 物件 | 內容 | 證據 |
|---|---|---|
| 背景 | elem 0x5d7 = sprite 0x8d3 (64,64)（星空） | `FUN_10083338` |
| 自己的板子 | elem 0x5d9 = sprite 0x8d4（32×32 圓形 mallet），host 在 (64,100)，guest 板子在 host 座標 (64,−116)；palette = 「是不是對方的」 | `FUN_100240ec` |
| 球 | elem 0x5d8 = sprite 0x8d6（16×16），半徑 8 | `FUN_10003c74` |
| 角落擋塊 | 4 個 32×6 矩形，左上角 (−8,122)、(104,122)、(−8,−144)、(104,−144)，畫 sprite 0x8d7 於 (x+16, y+3) | `FUN_100683cc`、`FUN_10068490` |
| HUD | 左上 sprite 0x80e (15,11) + 自己的分數（elem 0x5e5）；計時 sprite 0x80f (120,11) + elem 0x5e7 | `FUN_10083338` |
| 球門線 | 球心 y ≥ 144 → host 失分；y < −159 → host 得分（中線 y = −8，guest 畫面 = −8 − y） | `FUN_10003cb8` |

### 2.2 操作 [確認]

- 鍵 1（A）左、鍵 4（C）右：每 frame ±6 px；dial 每格 ±6 px；x 夾在 13..115。C 是遊戲鍵，遊戲中不能離開。
- 鍵 2|8（B 或 PUSH）按住 = 蓄力：每 frame +0.15，上限 1.0；滿了顯示 elem 0x5db（anim 0xb57）並開始留殘影。
- 放開（有蓄力時）= 往前衝 32 px，時間 0.35 s − 0.2 s × 蓄力（3–7 frames），衝刺中旗標 `smashing`；衝完立刻用 0.15 s 收回。衝刺期間 x 照常移動。（`FUN_100241bc`）

### 2.3 球的物理 [確認]

- `FUN_1000418c`：速度每 frame ×0.99，但最低 4.0 px/frame；≥ 12 時留殘影；左右牆 x < 8 / x > 120 → 夾回並反轉 vx（特效種類 2）。
- 板子碰撞 `FUN_10003cf8`：球和板子的本 frame 位移切成 (|球位移| + |板位移|) / 8 步（至少 1）同步掃過；中心距離 ≤ 21 時，法線 n = 球 − 板；若相對位移在 n 上的分量 d < 0：v += n × (−1.98 d)，球放到板心 + 21 n。另外還有一個依穿透深度的很小的推出量（重製省略）。特效種類 = 板子是否在衝刺（0 普通、1 衝刺）。
- 角落擋塊 `FUN_100684e4`：10 個子步驟，圓（半徑 8）對矩形最近點；v 對法線鏡射，推出到距離 8（特效種類 2）。
- 擊球特效／音效（RW 0x180006da / 0x180006e0）：種類 0 → elem 0x5dd（anim 0xb60）音效 10；種類 1 → elem 0x5dd 音效 61；種類 2 → elem 0x5de（anim 0xb61）音效 0。

### 2.4 流程（host，`ctx+8` 子 state）[確認]

| state | 內容 |
|---|---|
| 0 | setup（`FUN_10083338`）：計時 0x15c = 2400，倒數 0x160 = 60；球放 (6,36)、角度 0x5000、速度 4.0（launch = (cos, sin)(a − 0x4000) × 速度） |
| 1000/0x3e9 | 3-2-1 倒數 60 frames |
| 2000/0x7d1 | 發球：發球方畫面上的球從 (x+48, y−16) 飛到發球點（0.5 s，0.25 s 淡入，`FUN_1000f040`），之後球開始動 |
| 3000 | 遊戲：計時 ≤ 0 → 9000。否則移動球、碰撞、判斷球門。進球 → 4000 |
| 4000/0xfa1 | 得分方：anim 0xb62（「GOAL」，sprite 0x8dd）+ 音效 19；失分方：anim 0xb64 + elem 0x5e1（anim 0xb5d 於 (64,96)）。動畫結束 → 2000。**失分方發球**：host 失分 → (6,36) 角度 0x5000；host 得分 → (122,−52) 角度 0xd000 |
| 9000 | 時間到：球在 1 秒內減速並淡出（`FUN_100043e4`），停止後 → 0x2329 |
| 0x2329–0x232d | 結果面板（§2.5），等雙方都看完 → scene state 4000 |

- 計時只在 state 3000..8999（遊戲、進球動畫）遞減；發球、倒數不算。板子輸入也只在這段期間處理。

### 2.5 結果面板與結果動畫 [確認]

- 面板 `FUN_10002cec`（RW 0x18000000 表 game 0）：elem 0x5b6（sprite 0x8cc 半透明遮罩），依結果 0/1/2：sprite 0x8d0 / 0x8d2 / 0x8d1、寵物 anim 0xb2f（開心）/ 0xb31（難過）/ 0xb30（普通）於 (64,94)，兩邊分數於 (42,48)（自己）、(86,48)（對方）。滑入與關閉見 §2.5.1。
#### 2.5.1 面板的動作（兩個遊戲共用）[確認]

`FUN_10002cec` 建立面板物件（elem 0x5b6），內容放在子群組（`FUN_1006f290`，座標相對於面板中心，群組 x = −64 時內容在原位），群組 x 先設成 −192 = **往左一整個畫面（−128 px）**；面板不透明度（物件 +0x9e，會乘到子物件）先設 0。每 frame 的 callback `FUN_1006b5d0`（事件 8）：

| 步驟 | 內容 |
|---|---|
| 0 | 整個面板不透明度 0 → 1，0.15 s，線性（tween 曲線 1） |
| 1 | 淡入結束 → 內容 x 由 −128 滑到 0，0.25 s，曲線 2（1 − cos(t·90°)，先慢後快）；音效 0x35（53） |
| 2 | 設 40 frames 倒數（`FUN_1006b6fc`：倒數未到 0 或畫面淡出中就不往下走），約 2 s 不接受按鍵 |
| 3 | `idle_reset`（`FUN_10025c6c`／`FUN_10025cb4(1)`）：閒置計時歸零 |
| 4 | 按下任何鍵（`g_key_down & 0xf` = A/B/C/PUSH）或閒置超過 5 s（`FUN_1000278c(5)`）→ 設「看完了」旗標（+8 bit0），遊戲等雙方都看完 |

- 其他 callback：`FUN_1006b598`（事件 9）讓寵物動畫跟著面板時間走；`FUN_1006b534`（事件 5 = 動畫結束）把寵物換成 anim 0xbd 並持續播放 **[推測：0xbd 的內容未看]**。

- guest 的結果碼會用 `FUN_10002f68` 把 0↔1 對調後才選面板。
- scene 0x84 動畫（表 0x10095750，`[game][host?0:1][result]`）：HOCKEY host 0xb27（贏）/ 0xb28（輸）/ 0xb29（平），guest 0xb2a/0xb2b/0xb2c；自己的 tama（chr 佔位 0）和朋友（role 1）並排做反應。

---

## 3. Arcade 02「SHOOTING」（合作打病菌）

### 3.1 場地與物件 [確認]

| 物件 | 內容 | 證據 |
|---|---|---|
| 背景 | elem 0x5eb/0x606 = sprite 0x8df（顯微鏡畫面），下方桌面 elem 0x5ec/0x607 = sprite 0x8e0 於 (64,128)（guest 左右翻轉） | `FUN_10028348` |
| 洞 | 18 個：0–8 = host 畫面 elem 0x5f1–0x5f9（x 24/64/104 × y 16/48/80），9–17 = guest 畫面（同位置，host 座標 y 取負） | `FUN_10063abc` |
| 準心 | elem 0x5ee = sprite 0x8e1（alpha 57）初始 (64,48)；x 夾 16..112，y 夾 −80..80（跨兩個畫面） | `FUN_100319e4`、`FUN_10031a2c` |
| 病菌 | sprite 0x8e2，elem 0x5f1 複製到每個洞；**palette = 種類 0/1/2**（sprite +10，`FUN_10063abc`）；frame（sprite +8）= 0 冒出中、**1 被打中（抖動 0.6 s）**、2 抖完開始淡出（同時在原地播 elem 0x5f0） | `FUN_10063abc`、`FUN_10063ce4`、`FUN_10063eb0` |
| 操縱桿 | 底座 elem 0x600 = sprite 0x8e5 (91,116)；桿頭 sprite 0x8e4 依輸入方向在 x 86/91/96（RW 0x18000a08），被鎖住時 frame 1；寵物 anim 0xb68 坐在桿上（elem 0x5fa），結束時 anim 0xb69（elem 0x5fb） | `FUN_1001eb90`、`FUN_1001ee44` |
| 分數 | 框 sprite 0x8e3 (31,113) + 數字 elem 0x602 (40,114) | `FUN_1002c38c` |

### 3.2 操作：兩人共用一個準心 [確認]

- `FUN_1001ee44`：每台機器的輸入 = 鍵 1（A）−1、鍵 4（C）+1、dial ±1，× 4 px/frame；鍵 2|8（B/PUSH）按下 = 開火。**host 的輸入移動 X，guest 的輸入移動 Y**（`FUN_10031a2c` 用 0xba0 = host、0xbc0 = guest）。
- 開火後自己的桿子卡住 4 frames（不能移動、不能再開火）；**對方開火**時自己被鎖 8 frames（已經被鎖時改成 4）（`FUN_1001ee14`）。任何一方開火都會打：elem 0x5ef（anim 0xb6b）於準心、音效 20。
- 命中 `FUN_10063ce4`：第一個「已經冒出、還沒開始縮回、沒被打過」且中心距離 ≤ 16 的病菌。分數 += 該種類點數，下一次出現的等待 −6 frames，音效 52；病菌抖動，0.6 s 後換 frame 2 並在 0.6 s 內消失（elem 0x5f0，anim 0xb6c）。

### 3.3 出現表 [確認]

- setup 時 host 用輪流計數器 `FUN_10002fe4(3)`（`DAT_1800169c` 低 6 bits，每次呼叫 +1）從 3 張表選一張（XIP 0x1009c4ec：3 × `{u32 ptr, u32 count}`，每張 170 列 `{u16 side<<14 | kind, u16 delay}`，delay 合計 3600 frames）。
- `FUN_10031f10` 每 frame：累加器 ≥ 1 時 −1；當累加器 < 1 時一直讀下一列：side 1 = host 的洞 0–8、2 = guest 的洞 9–17、3 = 全部；在該側**沒被佔用的洞中用 `rng_range` 隨機選一個**（全滿就丟掉這一列），然後累加器 += delay。打中會讓累加器 −6，所以打得越快，出現得越多。
- 種類表 XIP 0x1009df32 `{u16 stay, u16 points}`：kind 0 = 60 frames / 1 分、kind 1 = 50 / 2、kind 2 = 40 / 3。
- 病菌生命週期（`FUN_10063abc`／`FUN_10063eb0`）：0.25 s 放大冒出 → 停留 `stay` frames → 0.15 s 縮回消失。

### 3.4 流程 [確認]

0 setup（計時 0xb8c = 1200、倒數 60）→ 1000/0x3e9 倒數（兩邊都到 0）→ 2000 遊戲（準心、出現、病菌更新、開火、計時 −1）→ 計時 0：所有冒出中的病菌縮回（`FUN_10064068`），寵物下桿（anim 0xb69）→ 9000 等病菌全部消失、動畫結束 → 0x2329 結果面板 → scene state 4000。

### 3.5 結果面板 [確認]

RW 0x18000000 表 game 1：結果 0 → sprite 0x8cf + anim 0xb31；1 → 0x8ce + 0xb30；2 → 0x8cd + 0xb2f；分數框 sprite 0x8cb + 數字於 (64,44)。

### 3.6 結果動畫 [確認]

表 0x10095750 game 1：host 0xb2e（結果 0，兩隻都難過）/ 0xb2d（結果 1、2，開心），guest 0xb2b/0xb2a/0xb2a。

---

## 4. 重製：單機版（offline）

| 項目 | 忠於原作 | 為單機而加的（invented） |
|---|---|---|
| 入口 | CONNECTION 第 4 項 MINI GAMES（原作是第 5 項，DOWNLOAD 未實作）→ HOCKEY / SHOOTING；階段 < 3 → 0x6b | 不檢查「髒」的 EM_REQ_OTEIRE 分支 |
| 開場 | anim 0xb4b / 0xb66、3-2-1 倒數 | 省略連線對話框和 HOW TO PLAY demo；開場不能跳過 |
| 朋友 | role 1 = `pet.friends` 最後一位的 tama；沒有朋友時用 Mametchi（4022）。結束時照 §1.1 更新這位朋友：HOCKEY +1、SHOOTING 結果 ≥ 1 +2（否則 +1），上限 4，last result 5 | 假設對方也記得我們（離線看不到對方的紀錄），所以本局 friendship = 我們的紀錄（1..4，不會是 0）；沒有朋友時不新增紀錄（Mametchi 沒有 uid）；朋友的顏色不套用 |
| HOCKEY | 板子、球、角落、球門、計時、進球動畫、結果碼與點數都照 §2 | 對方板子 = CPU：球朝它來且在 y < 60 時，每 4 frames 重新瞄準球的 x（±8 px 誤差），最快 4 px/frame（玩家 6）；球在前方 40 px、左右 20 px 內時蓄力 1 frame 後衝刺。畫面只顯示自己那一半（跟原作一樣，對方那一半在另一台機器上）。減速以線性 tween 表示 |
| SHOOTING | 出現表、病菌種類、命中、鎖定時間、計時、結果碼與點數都照 §3 | **玩家控制 X（A/C/dial）、CPU 夥伴控制 Y**，雙方都能開火：CPU 選離準心最近的可打病菌，把 Y 移過去，對準後 5 frames 開火。準心在朋友那一半（y < 0）時畫面切到朋友的畫面（host 座標 y + 128，好像兩台機器頭對頭擺）。出現表用亂數選（原作是輪流計數器）；病菌的縮放用淡入淡出代替 |
| 結果 | 面板照 §2.5.1（淡入 3 frames、滑入 5 frames + 音效 53、44 frames 後任何鍵或再 100 frames 自動關閉）→ scene 0x84 動畫（role 1 = 朋友）→ 點數 + mission 11（`m::LINKS`）+ 朋友紀錄 → 回到 arcade 選單 | 只有自己一台，所以不等對方看完；分數數字（HUD）跟著滑但不跟著淡入（淡入時內容還在畫面外，看不出差別） |

選 SHOOTING 的分工的理由：原作的趣味在於「一個人管 X、一個人管 Y」的合作；CPU 管 Y 而且會自己開火時，玩家只要對準 X 就能配合，也看得到 CPU 在朋友畫面上找的目標（畫面跟著準心切換）。玩家一人操作兩軸雖然也可行（dial 管 Y），但會讓「朋友」在遊戲中完全沒有作用。

### 待確認

- guest 在 HOCKEY 的點數：`FUN_1008709c` 只有 host 計算 `DAT_18001c8c`，guest 的值應該來自封包，對調規則未確認。
- 面板結束時寵物換成的 anim 0xbd（`FUN_1006b534`）沒有實作，重製的寵物動畫照原 anim 繼續。
- 步驟 1 的「淡入結束」用 frame 時鐘比較 16.16 秒；0.15 s 是 3 或 4 frames 取決於時鐘的捨入，重製用 3。
