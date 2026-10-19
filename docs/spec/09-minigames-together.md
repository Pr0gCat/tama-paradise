# 09 — 「一起玩」小遊戲（TOGETHER / PLAY）與 EGG HUNT（探險）

範圍：寵物圈選單 PLAY → PSID_OSEWA_TOGETHER（0x24，`FUN_10040ff4`）選單、可進入的 4 個遊戲（scene 0x69/0x66/0x6a/0x6b），韌體內另外 3 個無入口遊戲（0x65/0x67/0x68），以及 EGG HUNT（PSID_OSEWA_HUNT 0x26，`FUN_10041df8`）。
資料：`docs/spec/data/minigames/together_menu.json`、`together00/01/02/03/04/06/07.json`、`hunt.json`；抽取工具 `docs/spec/tools/extract_minigames_c.py`（`python3 -I extract_minigames_c.py DUMP.bin [OUTDIR]`，內含 assert 自我檢查）。
動畫 = motion script（07-animation §2.2，用 `decode_motion.py <id>` 看）；UI 元件 = data pack 表 4/5（08-care-ui §3.1，用 `ui_ui.py <id>` 看）。下文「anim 0xNNN」都是 motion script id，「elem 0xNNN」都是 UI 元件 id，「sprite 0xNNN」是 sprite pack index。

---

## 1. 摘要

- **05-content §2.11 的遊戲類型有三個要更正**：選單上的 4 個遊戲是 **0x69 = game 04「MATCHING」翻牌配對**（不是猜箱子）、**0x66 = game 01「FLAGS」舉旗反應**、**0x6a = game 06「CATCH」接球計量條**（時機停止）、**0x6b = game 07「SHIROTAMAS」帶著寵物走、收集 5 隻 Shirotama**（用轉盤控制方向，不是傾斜滾球）。無入口的是 game 00「ARROW」（猜左右）、game 02「3 UFOS」（三杯猜球）、game 03「WALKING」（轉盤轉動整個迷宮）。
- 所有遊戲都是固定 20 fps 的狀態機（`ctx+0` state，`task_wait` 以 frame 計）。tween／場景時間用 16.16「秒」：每 frame +0xccc（1/20 s），0x10000 = 1 秒 = 20 frames。
- 遊戲內容**沒有亂數**：用存檔中的循環計數器（`0x18001678`、`0x1800167a`）挑選固定的版面或序列，每玩一次（MATCHING 是每回合一次）+1。EGG HUNT 是唯一用到亂數（`rng_range`）的部分。
- 獎勵只有心情：`care_add_gokigen` 加 0／+1／+5／+10；拿到完美結果時 mission 4 計數器 `0x18000cae` +1（上限 100）。不給點數、不給物品。玩到一半按 C 離開：音效 2，沒有獎勵，回到 TOGETHER 選單。
- 寵物一律是**玩家自己的 tama**：腳本中的角色佔位 0 會被換成 `chara_def_get(60000)+2`。大圖遊戲（00/01/04/06）用 pose 1–20，小圖遊戲（02/03/07）用 pose 30–53。
- 共用的背景 = 目前 field 的時段／天氣背景元件（`FUN_10018214`）放大 2 倍並置中在 (64,64)。共用的 HUD = 左上角分數框（sprite 0x80e 位於 (15,11)，文字 `"\x9a%d"` 位於 (14,11)）。
- EGG HUNT：選擇 VOLCANO／SPRING → 從名單中挑 3 位隊員（自己、目前 field 的放牧 tama、依 rank 決定的 NPC 9014 ×2–4；每個名單位置每天只能出發一次）→ 確認「BRING THEM?」→ 出發動畫 → 依心情與 rand(1,100) 決定結果 0–3 → 結果 0/1 才會開蛋：9 顆（結果 0）或 rand(7,8) 顆，每顆從 `0x100a0600` 表（field×kind 各 4 種）挑一種食材；結果 0 再加 1 顆 GOLDEN PEACH。

---

## 2. 詳細規則

### 2.0 共用框架 [確認]

| # | 規則 | 證據 |
|---|---|---|
| C1 | 每個遊戲場景的 init：配置 ctx、`layer_create`、1–2 個 actor（`actor_new` 0x1000053c）、ctx 中的背景元件 = `FUN_10018214()`（= `field_bg_table(fieldtype)[period*3+weather+1]`，與 HOME 背景相同）、`scene_set_next(0x24)`（返回目標 = TOGETHER 選單）。update：`task_pre_update` → 若 `ctx+0x14 bit0` 就跑遊戲 tick → `FUN_1000a268(layer, ctx+0x1c)`（node 更新，並把場景時間 `ctx+0x1c` 加 0xccc） | `FUN_10026d70`/`10026e38` 等 7 組、`FUN_10009ff4(...,0xccc)` |
| C2 | 背景：`ui_element_create(layer, grp, bgElem)`，位置 = elem 0x314 (64,64)，`+0x7c` 縮放 = 0x200（2.0）。field 背景 sprite 是 512×512，所以畫面上看到的是中心 64×64 區域放大 2 倍 [推測：sprite 以中心對齊繪製] | 各遊戲 state 100/1000 |
| C3 | 淡入淡出：開始時 `fade_set(0,0x20,0,4,0)`（從黑色淡入）；結束時 0x7fe0 → `fade_set(0xff, 目前值, 1, 4, 0)` → 0x7ff0 → done。game 03/07 換畫面時用白色淡入淡出 `FUN_1007b3b0(7,…)` | 各遊戲 |
| C4 | `FUN_10025cb4(1)`：開始時設 `0x18001cbb bit3`，結束時清除 [推測：暫停閒置自動回 HOME] | |
| C5 | Tween `FUN_10066b34(field, curve, from, to, start, dur)`，16.16 秒；曲線 1 線性、2 `1−cos(t·90°)`（ease-in）、3 `sin(t·90°)`（ease-out）、4 `t − sin(2πt)/2π`、5 `(1−cos(t·90°))²`、6 `1−(1−sin(t·90°))²`、7 先慢後快再慢的 S 形。振盪器 `FUN_10066aec(field, kind, amp, start, period)`：kind 5 = 三角波 ±amp（0→+amp→0→−amp→0），kind 10 = 方波（前半週期 0、後半週期 amp） | `FUN_1008ebec`、`FUN_100621f8`…`10061df0`、`FUN_1008ea08`、`FUN_10061534`、`FUN_10061c34`；sin 用 `FUN_10031b74`（0x10000 = 360°）[曲線公式：sin 函式的定義為推測] |
| C6 | 寵物：actor 播 motion script，`petanim_subst(obj,1,0,chr)` 或 `actor_map_chr(actor,0,0,chr)`，chr = `chara_def_get(60000)+2`（60000 = 「自己」標記） | 各遊戲 |
| C7 | 獎勵：`care_add_gokigen(n)`；完美 → `0x18000cae = min(+1, 100)`（mission 4）。C 鍵離開 → `snd_play(2)`、`scene_set_next(0x24)`、0x7fe0，**沒有獎勵** | 各遊戲 |
| C8 | 結果動畫的格式都一樣（anim 編號見各遊戲）：完美 = 音效 59＋spawn_obj 3、9（閃光）＋pose 4 跳躍；+5 = 音效 52＋spawn_obj 1＋pose 2；+1 = 音效 52＋pose 1；0 = 音效 48＋pose 3（左右晃）。都在第 25–35 frame 送出完成訊號 | `decode_motion` 0xae7–0xaea、0xaa0–0xaa3、0xb21–0xb24、0xb00–0xb03 |
| C9 | 輸入：`key_pressed` 1 = A、2 = B、4 = C、8 = 轉盤按下；`key_held(0x80/0x40)` = 轉盤正轉／反轉一格（每 3 個原始計數產生一次脈衝，06-system §2.5）；`dial_angle()` 0x10006104 = 轉盤絕對位置 ×65536/24；`FUN_10006110` 把轉盤位置歸零 | 06-system |
| C10 | 存檔中的循環計數器（每個都在 init 讀取後 +1）：`0x18001678` bits 2–6 = game 00 答案索引（0..17，**離開時寫回**）；bits 7–8 = game 01 序列**與** game 03 地圖（兩個遊戲**共用同一個**計數器，mod 3）；bits 9–11 = game 02 版面（mod 5）；bits 12–13 = game 02 交換次數組（mod 3）；bits 14–15 = game 07 地圖（mod 3）。`0x1800167a` bits 0–3 = game 04 牌組 p（mod 10），bits 4–6 = game 04 寵物位置 q（mod 5），**每回合**各 +1。game 06 沒有計數器 | 各 init |

**HUD 分數框**（00/01/04/07 共用的版面）：sprite 0x80e（32×24 白色方塊，alpha 0x36）位於 (15,11)，文字元件（格式 0x108x）位於 (14,11)，內容 = `"\x9a%d"`（0x100a4128；0x9a 是字型中的圖示字元 [推測]）。倒數框（03/07）：sprite 0x80f（16×24）位於 (120,11)。

### 2.1 TOGETHER 選單（scene 0x24，`FUN_10040ff4`）[確認]

- 進入：寵物圈選單 PLAY（文字 0x47）；寵物睡著時圈選單會拒絕並顯示 0x69「WAIT UNTIL YOUR TAMA WAKES UP!」（08-care-ui §2.3）。
- state 0：`listmenu_create(layer,0,style 0,flags 0,count 4,items 0x10094140,bg 4)`（bg 4 = elem 0x2af = sprite 204 粉紅斜紋；清單外觀見 08-care-ui §2.4）。起始游標依上一個場景決定：從 0x66 回來 → 1，從 0x67/0x6a → 2，從 0x68/0x6b → 3，從 0x69 → 0。從黑色淡入。
- 項目（12 B `{u32 text, u16 scene, pad}`）：

| idx | 文字 | scene | 遊戲 |
|---|---|---|---|
| 0 | 0x32f MATCHING（カードあわせ） | 0x69 | game 04 |
| 1 | 0x32a FLAGS（はたあげ） | 0x66 | game 01 |
| 2 | 0x333 CATCH（キャッチボール） | 0x6a | game 06 |
| 3 | 0x334 SHIROTAMAS（しろたまフレンズ） | 0x6b | game 07 |

- state 1000：確定 → 若 `planet_rank < 0x1009396e[idx]`（全部是 0，**永遠開放**）→ state 2000 鎖定訊息（elem 0xf7 藍色背景 sprite 0x33、0xf9、0xfa、文字 0xf8 = `0x10093fe0[需求 rank]` = 0xf2.. 「AVAILABLE FROM LEVELn」；A/B/C 或 200 frames → 回 state 0）；否則 `scene_set_next(item.scene)` → 淡出。C → `0x18001c6a |= 4`（回 HOME 並重新打開圈選單）→ `scene_set_next(0x1c)`。
- 其他文字（0x329 ARROW、0x32b 3 UFOS、0x32c WALKING）存在字串表中，但沒有任何選單引用 → 0x65/0x67/0x68 在出貨版韌體沒有入口。

---

### 2.2 game 04「MATCHING」（scene 0x69，init `FUN_10026d70`，tick `FUN_10059600`）

**玩法**：3 回合。每回合 3 張蓋著的牌滑入，寵物走到其中一張牌後面「拿走」它，玩家從**另外兩張**中選一張；全部翻開後，玩家選的牌圖案與寵物那張相同就得 1 分。翻牌前看不到任何牌面 → **純猜**（有些回合根本沒有相同的牌，見資料）。

**畫面** [確認]

| 物件 | 元件 / sprite | 位置 | 說明 |
|---|---|---|---|
| 背景 | field bg ×2 | (64,64) | C2 |
| 開場 | anim 0xae1 於 elem 0x563 (64,64) | | 140 f：子腳本 0xae2 = 寵物從 y+80 往上升到 0（pose 1 → 1 → 4），0xae3 = 桌子 sprite 0x8b3（128×64 紅色圓桌）從 y+118 升到 y+38 |
| 桌子 | elem 0x566 sprite 0x8b3 | (64,102) | state 1000 起常駐 |
| 寵物 | anim 0xae4（pose 1，上下彈 1 px，30 f 循環）掛在 node elem 0x571 | (64,80) | 寵物 actor 綁在一個 node 上（`node+0x120`），移動 node 就會移動寵物 |
| 牌 ×3 | elem 0x567 sprite 0x8b4（32×32，7 frames：0 = 背面藍色，1–6 = 6 種圖案） | 起點 x=144（elem 0x56a）、y=96 → 終點 x = 32/64/96（elem 0x567/0x568/0x569） | |
| 寵物牌框 | elem 0x56f sprite 0x8b5 palette 1，z 100 | 每張牌各一個子物件 | alpha 0 / 0x100 切換 |
| 游標框 | elem 0x56f sprite 0x8b5 palette 0，z 101 | 同上 | 選中的牌 |
| 游標箭頭 | elem 0x56c sprite 0x8b6（24×24 粉紅上箭頭） | x = elem 0x56c/0x56d/0x56e（32/64/96），y 116 | |
| 分數 | elem 0x564 sprite 0x80e (15,11)＋文字 0x565 (14,11) | | 每 frame 更新 `"\x9a%d"` |

**狀態機**（`ctx` = `DAT_18014250`）[確認]

| state | 動作 | 下一步 |
|---|---|---|
| 0 | `idle flag`；actor[9] 播 0xae1 於 (64,64)；建背景；淡入 | 1 |
| 1 | 等 0xae1 完成（140 f） | 1000 |
| 1000 | actor[10] 播 0xae4、建寵物 node（elem 0x571）並綁定；停止 0xae1；建桌子、分數框、分數文字 | 0x3e9 |
| 0x3e9（每回合開始） | 建牌 group：3 張牌放在 x=144、y=96；`q = 0x1800167a bits4–6`、q←(q+1)%5；`p = bits0–3`、p←(p+1)%10；等 20 f | 0x3ea |
| 0x3ea | 3 張牌同時滑到 x=32/64/96（曲線 3，1.0 s = 20 f）；等 20 f | 0x3eb |
| 0x3eb | 等 10 f | 2000 |
| 2000 | 音效 9；`pos = 0x10091f94[q]`。pos==1（中間）：寵物 node y → 72（曲線 3，0.5 s）。否則：寵物 x → elem x（32/96）（曲線 3，1 s）＋ y 曲線 7 從 72→80（1 s，跳躍）。寵物牌框 alpha = 0x100；等 10 f | 0x7d1 |
| 0x7d1 | pos==1：寵物 y → 80（0.5 s）；等 20 f | 0x7d2 |
| 0x7d2 | 寵物那張牌 y → 88（往上 8 px，曲線 1，0.5 s）；音效 1；等 20 f | 0x7d3 |
| 0x7d3 | 音效 9；寵物回中間（pos==1：y→72；否則 x→64 並跳躍，1 s）；等 10 f | 0x7d4 |
| 0x7d4 | pos==1：y→80；等 10 f | 3000 |
| 3000 | 建游標箭頭；游標 c 預設 0，若 c==pos 則 c=1；游標框 alpha = 0x100；計時器 `ctx[8]=200`（scene update 每 frame −1）；等 10 f | 0xbb9 |
| 0xbb9（輸入） | A 或轉盤正轉：音效 0、計時重設 200、c=(c+1)%3，若等於 pos 再跳一格；轉盤反轉：c−1（循環），同樣跳過 pos。B、轉盤按下、或計時歸零 → 確定 | 0xbba |
| 0xbba | 音效 1；選中的牌 y → 104（往下 8 px，0.5 s）；刪除游標；等 20 f | 4000 |
| 4000/0xfa1 | 依序翻開第 i 張牌（i=0,1,2，每 20 f 一張）：frame = `0x10091f10[round*2 + v]`，v = `0x10091ece[p*3+i]`；音效 10 | 0xfa2 |
| 0xfa2 | v[c] == v[pos] → 分數 +1、寵物 anim 0xae5（開心，音效 17，30 f）；否則 0xae6（難過，音效 13，30 f）。播在 elem 0x571 | 0xfa3 |
| 0xfa3 | 等動畫完成 | 0xfa4 |
| 0xfa4 | 寵物回到 0xae4 並重新綁定 node | 0xfa5 |
| 0xfa5/0xfa6 | 第 i 張牌 y → 146（掉出畫面，曲線 3，0.5 s），每 5 f 一張；三張都掉完 → round+1；等 20 f；round<3 → 0xfa7，否則 5000 | |
| 0xfa7 | 刪除牌 group | 0x3e9 |
| 5000 | 停止 actor；依分數：3 → anim 0xae7（elem 0x578）、mission 4 +1、心情 +10；2 → 0xae8（0x579）+5；1 → 0xae9（0x57a）+1；0 → 0xaea（0x57b） | 0x1389 |
| 0x1389 | 等結果動畫完成（30 f） | 0x7fe0（淡出 → 回選單） |

- C 鍵：state 1000..0xfa6 期間有效（C7）。
- **資料**：牌值表 `0x10091ece` u16[10][3]（每列都是兩個同值、一個不同值）、寵物位置 `0x10091f94` u16[5] = 1,2,0,2,1、圖案 frame `0x10091f10` u16[3][2] = (1,2)(3,4)(5,6)（第 r 回合的兩種圖案）。`together04.json` 的 `winnable_q_p[q][p]` 標出另外兩張牌中是否有與寵物相同的牌（false = 寵物拿到「單獨的那個值」，此回合必輸）。50 個組合中有 17 個 false；但 p、q 每回合各 +1，只要起始時 p ≡ q (mod 5)（新存檔兩者應都是 0 [推測]），就只會走到 (n%5, n%10) 這 10 個組合，**而這 10 個全部是 true** → 每回合恰好有一張相同、另一張不同，等於 **50% 猜中**。只有計數器不同步時才會遇到必輸回合 [確認：資料計算；起始值推測]。
- **音效**：9（寵物移動）、1（牌被拿起／確定）、0（游標）、10（翻牌）、17／13（對／錯，腳本內）、59/52/52/48（結果）、2（離開）。

### 2.3 game 01「FLAGS」舉旗（scene 0x66，init `FUN_1002683c`，tick `FUN_100539dc`）

**玩法**：寵物雙手各拿一面旗子，先做一段「假動作」，然後擺出最終姿勢；玩家要在 **20 frames（1 秒）** 內用按鍵照著做：只有左旗舉起 = 按 A、只有右旗舉起 = 按 B、兩旗都舉起 = A＋B、兩旗都放下 = 什麼都不按。假動作期間按任何 A/B = 搶按，直接結束。最多 10 題，錯一次就結束。

**畫面** [確認]
- 背景 C2；寵物 actor（`ctx[0xf]`）播放在 elem 0x53b (64,88)。
- 旗子 = sprite 0x89d（4 frames：0 = 拿在身側、1 = 放下、2 = 舉起、3 = 揮動中）；左旗（畫面左邊）在寵物 (−26,−20) 附近、palette 1、水平翻轉；右旗在 (+26,−20)、palette 0（紅色）。旗子的子腳本 0xab0–0xab7。
- 分數框 elem 0x53c (15,11)＋文字 0x53d (14,11)，文字 0.3 s 內淡入。

**狀態機**（`ctx` = `DAT_18014204`）[確認]

| state | 動作 | 下一步 |
|---|---|---|
| 0 | idle flag | 100 |
| 100 | 開場 anim 0xa95 於 (64,88)（寵物＋兩面旗子上下彈，75 f）；背景；淡入 | 0x65 |
| 0x65 | 等開場結束 | 1000 |
| 1000 | 建分數 HUD；播第 0 題的假動作 `A[cmd.feint]`（`FUN_10067960(0,0)`） | 0x3ea |
| 0x3e9 | 清除輸入（`input=4`、A/B 旗標清 0）；播下一題的假動作 | 0x3ea |
| 0x3ea | 等假動作播完 → 0x3eb；**期間按 A 或 B → 0x3f3（搶按）** | |
| 0x3eb | 音效 10；播答案姿勢 `B[cmd.answer]`（靜止姿勢，持續） | 0x3ec |
| 0x3ec | 輸入窗口：計數 +1，第 20 frame → 0x3ed。A 第一次按下：input = (B 已按 ? 3 : 1)、音效 1；B 第一次按下：input = (A 已按 ? 3 : 2)、音效 1（每顆鍵只算一次） | 0x3ed |
| 0x3ed | input == cmd.answer（沒按 = 4）→ 播 0xa9e（開心，音效 17）、音效 52、cmd+1、分數+1 → 0x3ee；否則播 0xa9f（難過，音效 13）＋音效 13 → 0x3ef | |
| 0x3ee | 等 20 f；cmd<10 → 0x3e9；否則 → 0x3f2 | |
| 0x3ef | 等 20 f | 0x3f2 |
| 0x3f2 | 播 0xaa2，同時**把音量設為 0**（`FUN_10030bb4(0)`，所以這段沒有聲音） | 2000 |
| 0x3f3（搶按） | 播 0xa9f | 0x3f4 → 等 30 f → 0x7d1 |
| 2000 | 等 20 f | 0x7d1 |
| 0x7d1 | 恢復音量（設定值 `0x18000bec` bits 3–4）；分數 ≥10 → anim 0xaa0、mission 4、+10；≥8 → 0xaa1 +5；≥1 → 0xaa2 +1；0 → 0xaa3；等 30 f | 0x7d2 → 淡出 → 0x7fe0 |

- C 鍵：state 100..1999 期間有效。
- **資料**（`together01.json`）：`0x100a10c2` = 3 個序列 × 10 題 × `{u16 feint, u16 answer}`（序列 = `0x18001678` bits 7–8）。假動作 anim `A[v]` = 0x100a00a0+2v：v=1 → 0xa96（30 f，只是上下彈 3 次）、2 → 0xa97（50 f，2 次快速揮旗）、3 → 0xa98（90 f）、4 → 0xa99（110 f，揮 5 次，每次音效 10）。答案 anim `B[v]` = 0x100a00a8+2v：1 → 0xa9a（左舉右放）、2 → 0xa9b（左放右舉）、3 → 0xa9c（兩旗都舉）、4 → 0xa9d（兩旗都放）。門檻 `0x100a13d8` = 10/8/1；結果 anim `0x100a13c0` = 0xaa0..0xaa3。
- **音效**：10（揮旗／答案出現）、1（按鍵）、52（答對）、13（答錯）、17（開心腳本）、59/52/52/48（結果）、2（離開）。

### 2.4 game 06「CATCH」接球（scene 0x6a，init `FUN_10026fb4`，tick `FUN_100802f4`）

**玩法**：寵物丟球 3 次；畫面下方的計量條上，三角形指標在 x=9..119 間來回掃動；按 A 或 B 停下，停的位置決定接球品質（0–3 區）。

**畫面** [確認]
- 背景 C2；寵物 actor（`ctx[8]`）的 anim 都播在 (64,88)（elem 0x59e/0x59f/0x5a6/0x5aa/0x5ac..0x5af 都在 (64,88)）。
- 計量條 elem 0x5a0 sprite 0x8c3（128×32：藍｜黃｜橘｜粉紅 四段）於 (64,112)；指標 elem 0x5a1 sprite 0x8c2（16×16 粉紅倒三角形，2 palettes）於 (9,104)，掃動範圍 x = elem 0x5a1 (9) ↔ elem 0x5a5 (119)。
- 區域（以指標的絕對 x 判斷，`FUN_10017cf8`）：x ≥ 109（elem 0x5a4）→ 3；≥ 89（0x5a3）→ 2；≥ 57（0x5a2）→ 1；否則 0。
- 球 sprite 0x8c1（6 frames，大→小，表示遠近），球的軌跡全在動畫腳本中。

**狀態機**（`ctx` = `DAT_18014254`，主 state + substep `ctx+8`）[確認]

| state.sub | 動作 |
|---|---|
| 1000.0 | 允許 C 離開（`ctx+0x3c bit0`）；開場 anim 0xb04（90 f，寵物拿球＋丟接球示範）於 (64,88)；背景；淡入 → sub 1 |
| 1000.1 | 開場結束 → 淡出 → 2000 |
| 2000.0 | 清空 layer；建背景 group（`FUN_10070fa0`）與計量條（`FUN_10017c94`）；淡入 → sub 1000 |
| 2000.1000（每球） | 寵物播 0xb0a（pose 1 待機，20 f 循環）；指標回到起點；隱藏計量條；等 10 f → 0x3e9 |
| 2000.0x3e9 | 顯示計量條：縮放 2.0→1.0、alpha 0→1，各 0.5 s（曲線 1）；等 20 f → 2000 |
| 2000.2000 | 開始掃動（方向 = 向右）；idle 重設 → 0x7d1 |
| 2000.0x7d1 | 等指標停止 → 3000 |
| 2000.3000 | zone = 區域；`count[zone]++`；隱藏計量條；寵物播 `0x10093e92[zone]`：0 → 0xb18、1 → 0xb14、2 → 0xb10、3 → 0xb0b（都在第 80 frame 完成） |
| 2000.0xbb9 | 完成 → 播 0xb1b（zone≥1，45 f）或 0xb1d（zone 0，寵物去撿球，75 f） |
| 2000.0xbba | 完成 → 球數 +1；<3 → sub 1000；否則關閉 C、→ 3000 |
| 3000.0 | 等 10 f → sub 1000 |
| 3000.1000 | 清空 layer；評等（`FUN_1007aee8`）：`count[3] ≥ 3` → 3（0xb21、mission 4、+10）；否則若 `count[3] > 0 且 count[0] == 0` → 2（0xb22，+5）；否則若 `count[0] < 2` → 1（0xb23，+1）；否則 0（0xb24）。重建背景 |
| 3000.0x3e9 | 結果動畫完成 → 淡出 → 9000 → 回選單 |

**計量條指標**（node callback `FUN_1007a65c`，每 frame）[確認]
- 啟動中且沒有在閃爍時：
  - 按 A 或 B（mask 3）→ 音效 1、指標停在目前位置（凍結 tween）、閃爍計數 = 16。
  - 否則若 `idle_seconds() > 10`（10 秒沒有任何輸入）→ 音效 1、指標**回到起點 x=9**（→ zone 0）並停止。
  - 否則若目前這一趟 tween 已結束 → 開始下一趟：線性從一端到另一端，時間 = `0x10092dc0[球數]` = 0xa666／0x8000／0x6666 = **0.65 s／0.5 s／0.4 s（13／10／8 frames）**；方向每趟反轉。
- 閃爍中：每 frame −1，palette = `((cnt&7) > 3)`（每 4 frames 切換一次）；歸零 → 停止（啟動旗標清 0）。
- 雖然有 C 鍵離開，但在淡入淡出期間（`FUN_1007b488`）無效。
- **音效**：1（停止）、腳本內：0xb0b = 29,29,17（漂亮接住）、0xb10 = 29,29,21、0xb14 = 11,50、0xb18 = 14,2,0（漏接）、0xb1b = 1,10、0xb1d = 10,50,10,23；結果 59/52/52/48；2（離開）。

### 2.5 game 07「SHIROTAMAS」（scene 0x6b，init `FUN_10027180`，tick `FUN_1005a9b4`）

**玩法**：俯視場地（18×19 格，每格 32 px），寵物以固定速度 2 px/frame 自動往前走，**轉盤決定前進方向**（絕對位置，每格 15°）。碰到 5 隻 Shirotama（T）就收集：牠們會排成一列跟在寵物後面（貪食蛇式）。碰到 G（黃色小生物）會被撞停 2 秒並記一次「撞到」。60 秒限時。

**畫面** [確認]
- 地圖 = `0x1009c7a8 + map*0x156`（18 字元 × 19 列，map = `0x18001678` bits 14–15）；'w' = 樹牆、'S' = 起點、'T' = Shirotama（每張 5 隻）、'G' = 撞擊生物（每張 2 隻）、' ' = 地面。3 張地圖都是 12×13 的空地，外圍 3 層牆（見 `together07.json`）。
- field 外觀表 `0x1009474a + fieldtype*8`（4 × u16）：{開場 anim 0xaeb，地面 sprite（0x8b7/0x8b8/0x8b9，128×128 草地等），G 元件（0x595/0x596/0x597 → sprite 0x8bb/0x8bc/0x8bd，32×32 黃色小雞類），牆 sprite（0x8be/0x8bf/0x8c0，64×64 椰子樹等，oy −24）}。
- 地面：4 張 elem 0x57d（地面 sprite）組成 2×2、間隔 128 px 的平鋪 group；group callback `FUN_1006c0d8` 每 frame 依移動量捲動（mod 128，方向與寵物移動相反）→ 無限捲動的地面。
- 牆：7×7 個 elem 0x598 節點（相對位置 (i·32, j·32)，i,j = −3..3）；group 位置 = (80 − x%32, 104 − y%32)；每個節點 callback（0x1007ffe0）查看「寵物所在格 + (i,j)」的字元：'w' → 顯示牆 sprite，其他 → 隱藏；依螢幕 y 排序深度。
- 物件（T、G）：放在 `ctx[0xb]` group 中的 (cx·32+16, cy·32+16)；group 位置 = (64 − px, 64 − py)，所以寵物所在點永遠畫在 (64,64)。T = elem 0x58d（anim 0xaf8，sprite 0x8ba 白色 Shirotama），G = 依 field 的 G 元件。
- 寵物：actor 在 elem 0x584 (64,80)，依前進方向播 4 種走路動畫（小圖 pose）：

| 方向 f（= heading 圈數，−0.5..0.5） | anim | 意義 |
|---|---|---|
| −0.211 ≤ f ≤ 0.2 | 0xaf1（pose 47/48） | 往上（背面） |
| 0.2 < f ≤ 0.414 | 0xaf2（pose 42/43） | 往左 |
| −0.417 ≤ f < −0.211 | 0xaf3（同 0xaf2 但水平翻轉） | 往右 |
| 其他（f > 0.414 或 f < −0.417） | 0xaf0（pose 34/35） | 往下（正面） |

  門檻值是韌體中的浮點常數（0.2、−0.2110、0.4140、−0.4170，**不對稱**）[確認]。只在方向改變且沒被撞停時才換動畫（`FUN_10006fe4`）。
- 跟隨者（收集到的 Shirotama）：elem 0x58f，依自己的方向播 0xafa–0xafd（sprite 0x8ba frames 2–7；四個方向門檻 ±0.125、±0.375，`FUN_10062258`）。
- HUD：左上 elem 0x580（sprite 0x80e）＋0x581 文字 = 已收集數；右上倒數框 elem 0x582（sprite 0x80f）＋0x583 文字，**只在最後 9 秒顯示**：值 = `floor(clamp(60 − t, 0, 9))`。

**移動與碰撞**（寵物 node callback `FUN_10081768`）[確認]
- `heading` = 寵物 node 的旋轉值 r（256 = 一圈），初始 r = 128（往下）。主迴圈中，若本 frame `dial_delta ≠ 0` 且沒被撞停：r = `(0x8000 − dial_angle) · 256 >> 16`，即 **f = 0.5 − pos/24**（pos = 轉盤絕對位置 0..23，進入場地時歸零）。
- 速度 v = `ctx+0x7c`（16.16）：起跑前 0，起跑後 2.0 px/frame；被撞停或收集時 0。每 frame：`dx = −v·sin(2πf)`、`dy = −v·cos(2πf)`（f=0 → 往上，f=0.5 → 往下）。
- 牆壁（以「格內偏移」判斷，格子 32 px）：dx<0 且左邊格是 'w' 且格內 x < 10 → dx=0；dx>0 且右邊格是 'w' 且格內 x > 22 → dx=0；dy 同理（上方格／格內 y < 10，下方格／> 22）。被擋住時不會反彈，只會沿另一軸滑動。
- 位置超出 0..576 / 0..608 時繞回（實際上被牆擋住，不會發生）。
- 走路音效：移動中每 20 frames 播一次音效 29。
- 軌跡：移動中每 frame 把 (x, y, heading) 推進 16 筆歷史紀錄。第 1 隻跟隨者讀寵物歷史的第 13 筆（13 frames 前 = 26 px），之後每隻讀前一隻歷史的第 7 筆（14 px）；位置換算成相對螢幕座標，並處理地圖繞回。
- 碰撞（T/G 的 node callback `FUN_1006d424`）：物件的螢幕位置與點 (64,84) 的距離平方 < 676（**半徑 26 px**），且目前沒有在撞停／收集中：
  - **G**（每隻只觸發一次）：速度 0、撞停旗標、撞到次數 `ctx+0xa4` +1、G 的 alpha 在 1 秒內淡出；`FUN_10007330`：寵物播 0xaf4（pose 33/36）＋音效 16，**2 秒**後恢復速度 2.0 並重選方向動畫。
  - **T**（已收集 < 5）：速度 0、收集旗標、暫停計時顯示（`0x95`）；寵物播 0xaf5（pose 33）＋音效 52，2 秒後恢復。若已收集 < 4：建立隱藏的跟隨者（掛在寵物或上一隻跟隨者下）；下一 frame 在 T 的位置產生特效 elem 0x58e（anim 0xaf9）並刪除 T；特效 1 秒後用曲線 7 在 1 秒內飛到跟隨者位置，再過 1 秒後消失、跟隨者顯示、計時旗標清除。第 5 隻（已收集 = 4 時）不產生跟隨者，只記錄它的 index。已收集 +1。
  - 注意：計時暫停旗標只停止「顯示與逾時判斷」，`t = now − start` 仍包含暫停的時間 → **實際上沒有暫停** [確認]。

**狀態機**（`ctx` = `DAT_18014258`）[確認]

| state | 動作 |
|---|---|
| 0 | idle flag → 1000 |
| 1000 | 開場 anim `0x1009474a[ft]` = 0xaeb（90 f，示範 Shirotama，sprite 0x714/0x8ba）於 elem 0x57c (64,88)；背景；淡入 → 0x3e9 |
| 0x3e9 | 開場結束 → 白色淡出、等 10 f → 2000。C → 離開 |
| 2000 | 清空 layer；建地面（依 field）、`dial_reset`、牆節點、T/G 物件、寵物 node（在 S 格中心，r=128）、寵物 anim 0xaef（pose 30 站立）、分數 HUD；白色淡入；記錄起跑時間 → 0x7d1 |
| 0x7d1 | 1 秒後：速度 = 2.0 → 3000。C → 離開 |
| 3000 (0xbb8) | 寵物播 0xaf0；等 5 f → 0xbb9 |
| 0xbb9 | 建倒數框（callback `FUN_1008e364`，起始時間 = 現在）→ 0xbba |
| 0xbba（主迴圈） | 撞停／收集處理；若已收集 < 5 且未逾時：產生收集特效、轉盤 → 方向、C → 離開、方向改變 → 換走路動畫、跟隨者方向動畫。否則 → 速度 0，若收集滿 5 隻則第 5 隻播 0xaf9 → 4000 |
| 倒數 callback | t > 51 s：顯示倒數；t > 60 s：速度 0、逾時旗標；若已收集 < 2 → 0x1004（失敗）|
| 4000 (0xfa0) | 等 1 秒 → 0xfa1：寵物播 0xaf6（開心，音效 45，39 f 時完成），跟隨者（最多 4 隻）播 0xafe，第 5 隻也播 0xafe |
| 0xfa2 | 寵物動畫完成 → 白色淡出 |
| 0xfa3 | 淡出結束：清空；hits = clamp(撞到次數, 0, 2)；結果 r：**已收集 5 且 hits==0 → 0**；否則已收集 ≥4 → 1；否則（2–3）→ 2。心情 `0x100a0c70[r]` = +10／+5／+1；r==0 → mission 4。結果 anim：r0 = 0xb00 於 elem 0x59a、r1 = 0xb01 於 0x59b、r2 = 0xb02 於 0x59c（都在 (64,88)）；重建背景；白色淡入 |
| 0xfa4/0xfa5 | 淡入完成後等 2 秒 → 0x7fe0 |
| 0x1004（失敗：逾時且已收集 < 2） | 寵物播 0xaf7（難過，音效 13），跟隨者播 0xaff → 等完成 → 白色淡出 → 0x1006：重建背景、播 0xb03 於 elem 0x59d（音效 48）→ 淡入 → 2 秒 → 0x7fe0。**沒有獎勵** |

- 注意：已收集 5 隻但撞過 G → r=1（+5）；逾時時已收集 4 隻 → r=1；2–3 隻 → r=2（+1）；0–1 隻 → 失敗（0）。
- **音效**：29（走路，每 20 f）、16（撞到 G）、52（收集）、45／13（成功／失敗動畫）、59/52/52/48（結果）、2（離開）。

---

### 2.6 game 00「ARROW」猜左右（scene 0x65，init `FUN_100266d4`，tick `FUN_10052e7c`；**無入口**）

**玩法**：「こっちむいてホイ」。5 回合；每回合玩家按 A（左）或 B（右），200 frames 沒按就自動選左；接著寵物把頭轉向固定答案的方向；方向相同 = 得分。

**畫面** [確認]
- 背景 C2。開場音效 76，開場 anim 0xa8b（80 f）於 elem 0x52b (64,88)。
- 寵物 anim 0xa8c（待機）於 elem 0x52e (64,88)。
- 箭頭：elem 0x52f sprite 0x89c（48×48）於 (24,104)，elem 0x530 同一 sprite 水平翻轉於 (104,104)。每回合重建。
- 分數框 elem 0x52c (15,11)＋0x52d 文字。
- 結算畫面元件（一開始 alpha 0）：elem 0x535 sprite 0x89b（128×48 面板）於 (64,104)、0x538 sprite 0x899 於 (48,96)、0x539 sprite 0x89a 於 (80,96)、文字 0x536 (24,104) = 得分、0x537 (104,104) = 失誤數。

**狀態機**（`ctx` = `DAT_18014200`）[確認]

| state | 動作 |
|---|---|
| 0 → 100 | 音效 76；開場 0xa8b；背景；淡入 → 0x65 等完成 → 1000 |
| 1000 | 停止所有 node 動畫；建 HUD 與第 1 回合（`FUN_1006784c`：箭頭、寵物 0xa8c、音效 0、回合 +1）；播音效 77（189 ms 短音，經 `FUN_10023f54` 先停止音樂通道再播放）→ 0x3ea |
| 0x3e9 | 下一回合（同上）；音效 77 → 0x3ea |
| 0x3ea | 計數 +1：第 200 frame → 選 0（左）＋音效 1；A → 0；B → 1（音效 1） |
| 0x3eb | 沒選的那個箭頭在 0.2 s 內淡出 |
| 0x3ec | 寵物播 `0x100a009a[ans]`（ans = `0x100a113a[idx]`；0 → 0xa8d，1 → 0xa8e，pose 11/12） |
| 0x3ed | 等 20 f |
| 0x3ee | ans == 選擇 → 0xa8f（開心）、分數 +1、音效 52；否則 0xa90（難過）、失誤 +1、音效 51 |
| 0x3ef | idx = idx<17 ? idx+1 : 0；等 30 f；回合 < 5 → 0x3e9；否則刪箭頭 → 2000 |
| 2000 | 寵物 0xa8c；建結算面板 0x535；等 10 f |
| 0x7d1 | 0.3 s 內：面板、0x538/0x539、兩個文字淡入；分數框與分數文字淡出；等 30 f |
| 0x7d2 | 分數 ≥5 → 0xa91、+10、mission 4、音效 59；≥3 → 0xa92 +5 音效 52；≥1 → 0xa93 +1 音效 52；0 → 0xa94 音效 48（都在 elem 0x531）；等 50 f → 0x7fe0 |
| 0x7fe0 | 把 idx（夾在 0..17）寫回 `0x18001678` bits 2–6 → 淡出 |

- 答案表 `0x100a113a` u16[18] = 0,0,1,0,1,0,1,1,0,0,1,0,1,0,1,1,1,0；門檻 `0x100a13d0` = 5/3/1。05-content 提到的「`&3` 只有 3 種版面」bug 屬於 Lab game 00，與這個遊戲無關。

### 2.7 game 02「3 UFOS」三杯猜球（scene 0x67，init `FUN_100269dc`，tick `FUN_10054364`；**無入口**）

**玩法**：3 個 UFO，獎品在 UFO 2（一開始在中間）。3 回合，每回合依固定序列交換位置 N 次後，玩家選位置；選中獎品 UFO 得 1 分。

**畫面** [確認]
- 背景 C2；開場 anim 0xab8（90 f）於 elem 0x542 (64,64)。
- UFO：elem 0x543/0x544/0x545 sprite 0x89e（48×24）於 (22,32)/(64,32)/(106,32)。
- 游標：`0x10091cb8[c]` = elem 0x549/0x54a/0x54b（anim 0xac6，sprite 0x8a0）於 (22/64/106, 48)。
- 寵物動畫位置 `0x10091f9e[pos]` = elem 0x546/0x547/0x548（(22/64/106, 64)）。

**狀態機**（`ctx` = `DAT_18014208`）[確認]

| state | 動作 |
|---|---|
| 100 | 開場 0xab8；背景；淡入 → 1000 等 30 f |
| 0x3e9 | 寵物播 0xabd 於 elem 0x547（寵物躲進中間的 UFO？[推測]），音效 61；建 3 個 UFO |
| 0x3ea | 等完成；step = 1、count = 1 |
| 0x3eb | 讀 `e = layouts[set][step]`（e[pos] = 站在 pos 的 UFO id）；每個 UFO 若目標 x 改變：x 線性移動，時間 = `0x1009c598[round]` 秒（1.0／0.6／0.3）；同時 y 以一半時間偏移 20 px 形成弧線：移到位置 2 → 往上（y−20）；移到位置 0 → 往下（y+20）；移到位置 1 → 從 0 來往上、從 2 來往下（等於「往右移動走上弧、往左移動走下弧」；第 0 步的「上一步」取該序列第 19 筆） |
| 0x3ec | y 移動結束 → y 在一半時間內回到原高度 |
| 0x3ed | x 移動結束：count < `swaps[set][round]` → step = (step+1)%20、count+1 → 0x3eb；否則 → 2000 |
| 2000/0x7d1 | 游標（初始 0）：A → 下一個（循環）、音效 0、計時重設；B 或 200 frames → 確定（音效 1） |
| 0x7d2–0x7d4 | 刪游標；等 20 f；選中位置上是 UFO 2 → 寵物播 0xac1 於該位置、分數+1；否則播 0xac4 於 UFO 2 的位置；音效 61 |
| 0x7d5 | 動畫完成：答對 → 25 frames 後音效 52；答錯 → 立刻音效 13；之後等 30 f → round < 2 → 3000，否則 → 4000 |
| 3000/0xbb9 | 寵物在 UFO 2 的位置播 0xabd（躲回去）、音效 61 → 完成後 round+1、count = 0 → 等 10 f → 0x3eb |
| 4000/0xfa1 | 淡出；心情 `0x100a0c60[分數]` = 0/1/5/10；分數 3 → mission 4；結果 anim `0x10095738[分數]` = 0xaca/0xac9/0xac8/0xac7 於 elem 0x54c；刪 UFO；淡入 → 完成 → 0x7fe0 |

- 資料：版面 `0x1009a710` u16[5][20][3]（set = `0x18001678` bits 9–11）、交換次數 `0x1009a6fe` u16[3][3] = {10,16,20},{12,14,19},{11,15,18}（組 = bits 12–13）、移動時間 float[3] = 1.0/0.6/0.3 秒。step 指標**跨回合延續**（第 0 回合從 step 1 開始；之後每回合第一次 0x3eb 是原地不動）。
- 音效：61、0、1、52、13、2；結果動畫內 59/52/52/48。

### 2.8 game 03「WALKING」轉盤迷宮（scene 0x68，init `FUN_10026bbc`，tick `FUN_10055f30`；**無入口**）

**玩法**：12×14 格迷宮（每格 48 px），寵物自動前進（最快 2 px/frame），轉動轉盤讓整個迷宮旋轉（寵物方向隨之改變），在 60 秒內走到 G 區域；碰到移動障礙物會被撞停 2 秒。

**畫面與規則** [確認，除非另外標記]
- 地圖 = `0x1009c5b0 + map*0xa8`（12 字元 × 14 列；map = `0x18001678` bits 7–8，**與 game 01 共用**）。'w' 牆、'S' 起點、'G' 終點（每張 3×2 格）、'H'／'V' 障礙物（三角波 ±48 px、週期 3 秒，H 水平、V 垂直）、'-' 障礙物的軌道（不影響邏輯）。
- 外觀表 `0x1009472c + ft*10`（5 × u16）：{開場 anim 0xacb/0xacc/0xace，地面 sprite 0x8a3/0x8a4/0x8af，障礙物 sprite 0x8a9/0x8aa/0x8b1，牆 sprite 0x8a6/0x8a7/0x8b2（64×64，oy −28），終點 sprite 0x8ac/0x8ad/0x8b0}。
- 世界 group `ctx[0xc]` 置於 (64,64)（反組譯確認 x、y 都是 0x400000），旋轉 = `dial_angle >> 8`（整個迷宮隨轉盤旋轉）；牆節點 5×5（elem 0x55b，間隔 48 px，callback `FUN_1007fe08`：顯示牆或終點 sprite）；障礙物 elem 0x55d。
- 寵物 node：旋轉 r = `(0x8000 − dial_angle)·256 >> 16`；速度每 frame +0.2（0x3333）直到 2.0；`dx = +v·sin(θ)`、`dy = −v·cos(θ)`；牆壁判斷同 game 07，但格子 48 px、門檻 10 / 38 px（0xa0000 / 0x260000）。地面捲動依移動方向相對於旋轉角，所以畫面上寵物看起來一直往同一個方向走 [推測]。寵物 anim 0xad9（pose 34/35，走路音效 29）。
- 開場：0x550 開場 anim（90 f）→ 白色淡出 → 建場地；鏡頭 node 從終點附近平移到起點（3 秒，0.5 秒後開始，`FUN_10080274`）；平移結束 → 速度 = 2.0 → state 3000（0xad9）→ 建倒數框（elem 0x557 sprite 0x80f ＋ 0x558 數字 sprite 0x13）。
- 障礙物：與 (64,64) 的距離平方 < 1024（半徑 32 px）且沒有在無敵中 → 撞到次數 `ctx+0x60` +1、撞停：寵物 0xada、音效 16、速度 0；2 秒後恢復 0xad9 並開始閃爍（方波 0.5 s），速度重新加速；再過 2 秒解除無敵。
- 終點：任何顯示中的 'G' 牆節點的螢幕位置落在 48<x<96、52<y<100 → 音效 15 → 4000。
- 倒數：t > 51 s 顯示數字 frame = `ceil(60 − t)`（9..1）；t > 60 s → 音效 74 → 0x1004 失敗。
- 結果（0xfa2）：hits = clamp(撞到次數, 0, 2)；心情 `0x100a0c68[hits]` = +10／+5／+1；hits 0 → mission 4；anim 0xadd/0xade/0xadf 於 elem 0x55f/0x560/0x561。寵物先播 0xadb（開心，音效 45）。失敗：0xadc（音效 13）→ 0xae0 於 elem 0x562，沒有獎勵。

---

### 2.9 EGG HUNT（PSID_OSEWA_HUNT 0x26，init `FUN_100274fc`，tick `FUN_10041df8`）

**入口條件**（圈選單，08-care-ui §2.3）：寵物睡著 → 0x69；成長階段 < 3 → 0x6b；planet rank 0 → 0xf2。init 時 `scene_set_next(0x1c)`，結束回 HOME。`ctx+0x15c` = `&0x18000ce8[fieldtype]`（當天的使用 mask）。

**狀態機**（`ctx` = `DAT_18014180`）[確認]

| state | 畫面／動作 | 下一步 |
|---|---|---|
| 2000 | 種類選單：背景 elem 0x176（sprite 0x7e 128×128 風景：天空、山、沙地）、`listmenu_create(style 0, flags 0x2000 = 不畫清單背景, 2 項 0x10094170)`：**0x42 VOLCANO（ポコポコかざん）→ kind 0、0x43 SPRING（ポコポコいずみ）→ kind 1**；elem 0x177 anim 0xb4（藍色巴士 sprite 0x748）；游標記在 `ctx+0x150`（夾在 0..1）；淡入 | 2001 |
| 2001 | 確定 → 淡出 → 3000；C → `0x18001c6a |= 4` → 0x7fe0（回 HOME 並打開圈選單） | |
| 3000 | 時段不是 0–2（06:00–18:59 以外）→ 6000。建名單（見下）；空的名單位置 < 3 → 0x1964；否則開啟隊員選擇 UI（`FUN_1002e120`），淡入 | 0xbb9 |
| 0xbb9 | 選滿 3 位 → 把 3 筆 `{chara, slot, flags}` 存到 `ctx[0x24..0x2c]` → 淡出 → 4000；取消 → 0xbba（5 frames 後回 2000） | |
| 4000 | 確認畫面：背景 elem 0x179（sprite 0x7e）、`choice_create(…, flags 0x2000, 文字 0x9a「BRING THEM?」)`（YES/NO）；3 位隊員各用一個 actor 播 anim 0xba（pose 30 小圖站立）於 elem 0x17b/0x17c/0x17d = (24,125)/(64,125)/(104,125)；淡入 | 0xfa1 |
| 0xfa1 | YES（index 0）→ 淡出 → 0x1004；NO → 淡出 → 3000 | |
| 0x1004 | 記錄使用：slot 0（自己）與 slot 9–12（NPC）在**三個 field** 的 mask 都設位元；slot 1–8（放牧 tama）只在目前 field 設位元。播出發 anim 0x7ab（巴士開走，200 f，第 100 frame 送出完成訊號，音效 52、29×3）於 (64,64)，角色 role 0/1/2 = 3 位隊員；淡入 | 0x1005 → 完成後淡出 → 0x1006 |
| 0x1006 | 決定結果 r（見下）；播旅程 anim `0x10093920[(kind + ft*2)*4 + r]`，於 (64,64)；播音樂 80（`FUN_10024f60(0x50, 0xc)`，[推測] 0xc = 重複次數）；淡入 | 0x1007 → 完成後淡出 → 0x1008 |
| 0x1008 | 停止音樂；播回程 anim `0x10095778[ft*4 + r]` 於 (64,64)；淡入 | 0x1009 |
| 0x1009 | 完成：r < 2 → 淡出 → 5000（開蛋）；否則 → 0x7fe0（沒有收穫） | |
| 5000 | 開蛋畫面：背景 elem 0x17e（sprite 0x7e）＋ elem 0x17f（sprite 0x85 128×128 奶油色面板）；n = r==0 ? 9 : rand(7,8)；第 i 顆（i<n）：actor[9+i] 播蛋動畫 `0x10095790[ft]`（ft0 0xb2 / ft1 0xb3 / ft2 0xb1，蛋 sprite 0x8b / 0x8c / 0x8a，64×64，3 frames 破殼，80 f，音效 23,23,24）於 elem 0x189+i；物品 sprite 元件 elem 0x180+i（縮放 0.9，alpha 0），動畫送出訊號（C=0）時才顯示（callback `FUN_1001198c`）。**所有蛋同時開**。3×3 格位置：x = 28/64/100、y = 32/64/96（i = 0..8 依列排列） | 5001 |
| 5000（每顆蛋的內容） | `idx = ft*8 + kind*4 + rand(0,3)`；sprite = `0x100a0600[idx].sprite`；**立刻** `item_give(0x100a0600[idx].item, 1)`；若 mission 17 位元 `0x100a0600[idx].bit` 尚未設定就設定（`0x18000ccc` bitset，3 bytes） | |
| 5001 (0x1389) | 計數 +1：滿 100 frames 後，到 200 frames 或任何按鍵（A/B/C/轉盤按下）→ r==0 → 0x138a；否則 → 0x7fe0 | |
| 0x138a | 所有物品 alpha 在 1 秒內淡出（曲線 3）；等 20 f | |
| 0x138b | 刪除物品；在 elem 0x184 (64,64) 放大蛋 sprite `0x10093382[ft]`（0x8b/0x8c/0x8a），alpha 0 | |
| 0x138c | 大蛋 1 秒內淡入；等 20 f | |
| 0x138d | actor[9] 在 elem 0x18d (64,64) 播蛋動畫；刪除靜態大蛋；elem 0x184 換成 sprite 0x706（GOLDEN PEACH，32×32），alpha 0，訊號時顯示；`item_give(6048, 1)` | |
| 0x138e | 同 5001 的等待 → 0x7fe0 | |
| 6000 / 0x1964 | 訊息畫面：elem 0x9d（sprite 0x36 128×128 背景）＋文字 elem 0x9e：6000 → 0x9b「CAN'T BRING NOW」，0x1964 → 0x9c「NO MORE TODAY」。任何鍵或 200 frames → 音效 1 → 淡出 → 2000 | |

**名單（state 3000）** [確認]
- 名單項目 12 B `{u32 chara, u32 slot, u8 flags}`，存在 `ctx[0x2d + k*3]`：
  - k=0：自己（chara 60000）。
  - 接著 n = `grazing_count(fieldtype)`（`FUN_100188d8`）隻放牧 tama：`grazing_id(field, j)`，j 從 `0x18000ddc[ft*4] >> 4` 開始往下（−1，<0 時繞回 7）。
  - 接著 NPC filler（TamaID 0x2336 = 9014，chr 181）：rank < 6 → 2 隻；6–9 → 3 隻；≥10 → 4 隻（rank = `0x18000c6c & 0xf`）。
- slot（每日使用 mask 的位元）：自己 = 0；放牧 tama = 1..n；NPC = 從 `9 + (rank<6 ? 2 : 6–9 ? 1 : 0)` 起連續（所以 NPC 用的是 11–12、10–12 或 9–12）。flags bit0 = bit4 = 「今天已使用」。
- 空位數 = 上述 slot 中位元為 0 的數量；< 3 → 0x1964。**bug**：計數迴圈只處理 n ≤ 2 的跳躍，放牧 tama ≥3 隻時會把不存在的 slot（n+1..8）也算成空位 [確認]。
- 隊員選擇 UI（`FUN_1002e120` style 0，callback `FUN_1002ea78`，樣式表 `0x10093ec6`）：每頁 4 張卡（2×2）。背景 elem 0x165（sprite 0x7e）；卡框 elem 0x167–0x16a（sprite 0x83，56×56，palette 1 = 灰色＝已使用或已選）於 (30,34)/(87,34)/(30,92)/(87,92)；角色 elem 0x16b–0x16e（type 7 角色元件）於 (31,53)/(88,53)/(31,111)/(88,111)；選擇順序數字 elem 0x16f–0x172（sprite 0x7f，8×16，frame = 第幾個被選）於 (46,19)/(103,19)/(46,77)/(103,77)；游標 elem 0x166（sprite 0x80，64×64 框）；捲軸 elem 0x173–0x175（sprite 0x2b/0x2c，x=120）。
  - A 或轉盤正轉 → 下一張，轉盤反轉 → 上一張（循環）；換頁時整頁在 0.5 s 內上下滑動 128 px（曲線 6），同頁內游標 0.1 s 滑動；音效 0。
  - B 或轉盤按下 → 選擇：已使用或已選 → 音效 11；否則標記、卡片變灰、顯示順序數字；未滿 3 位 → 音效 1，滿 3 位 → 完成。
  - C → 已選 > 0：取消最後一位（音效 2）；否則取消整個 UI（音效 2）。
  - **不一定要選自己**；但結果判定只在自己有出發時才看心情。

**結果判定（state 0x1006）** [確認]

```
if 自己 (60000) 在 3 位隊員中:
    s = rand(1,100)
    if gokigen >= 20:  r = 0 if s <= 50 else 1     # r==0 → mission 16 計數 0x18000cca = min(+1, 50)
    elif gokigen > 0:  r = 1
    else:              r = 3 if s <= 10 else 2     # r==3 → req_injury_if_ok()（受傷請求）
else:
    r = 1
```

- r=0「大成功」：9 顆蛋 ＋ GOLDEN PEACH；r=1：7–8 顆蛋；r=2：沒有收穫；r=3：沒有收穫並且受傷。
- 旅程動畫（`0x10093920`，[ft][kind][r]）：ft0 kind0 = 0x754/0x756/0x758/0x75a，kind1 = 0x755/0x757/0x759/0x75a；ft1 = 0x75d/0x75f/0x761/0x763、0x75e/0x760/0x762/0x763；ft2 = 0x78e/0x790/0x792/0x794、0x78f/0x791/0x793/0x794（長度 520/515/385/290 f，完成訊號在 420/415/285/190 f）。回程（`0x10095778`，[ft][r]）：ft0 0x75b/0x75c、ft1 0x764/0x765、ft2 0x795/0x796、r2 = 0x7c7、r3 = 0x7c8（250/250/220/260 f）。
- `FUN_1001810c` 讀取時，若 `ctx+0x150` > 1 會先把它清成 0。
- **音效**：清單／選擇 UI 的 0/1/2/11；訊息 1；出發 52,29；旅程動畫內 29、15、52、14、11；音樂 80；開蛋 23,23,24。

**收穫表 `0x100a0600`**（6 B `{u16 sprite, u16 item, u16 mission17_bit}`，見 `hunt.json`）[確認]

| field | kind 0 VOLCANO（rand 0..3） | kind 1 SPRING |
|---|---|---|
| 0 TROPICAL | 5066 TROPICAL STEAK、5067 TROPICAL PATTY、5070 BUTTERFLY SNACKS、5071 MEAL CRITTERS | 5068 TROPICAL FRUITS、5069 BANANA、5070、5071 |
| 1 FROZEN | 5076 FROZEN MEAT、5077 CANNED MEAT、5080 BEET、5081 RED BERRIES | 5078 NORTHEN SEAFOOD、5079 SALMON ROE、5080、5081 |
| 2 MORI | 5056 CHASHU PORK、5057 PEKING MEAT、5060 PERSIMMON、5061 POMEGRANATE | 5058 BAMBOO GRASS、5059 SESAME LEAVES、5060、5061 |

sprite = item id − 3335（例 5066 → 1731）；mission 17 位元 0..17 每種食材一個；第 25 筆（index 24）= {0x706, 6048 GOLDEN PEACH, 0}（程式直接寫死，不查表）。

---

## 3. 資料表

| 表 | 位址 | 格式 | 內容 / 抽出 |
|---|---|---|---|
| TOGETHER 選單 | 0x10094140 | 4 × 12 B `{u32 text, u16 scene, pad}` | `together_menu.json` |
| TOGETHER rank 門檻 | 0x1009396e | u16[4] | 0,0,0,0（後面接 Lab 的 1,4,4,1…） |
| 鎖定訊息 | 0x10093fe0 | u16[需求 rank] | 0, 0xf2, 0xf3, 0xf4 |
| game 00 答案 | 0x100a113a | u16[18] | `together00.json` |
| game 00 動畫 | 0x100a009a / 9e / a0 | u16 | 0xa8d,0xa8e / 0xa8f / 0xa90 |
| 門檻 | 0x100a13c8 / 13d0 / 13d8 | u16[3]+pad | 10,8,1（Lab 00）/ 5,3,1（game 00）/ 10,8,1（game 01） |
| game 01 題目 | 0x100a10c2 | u16[3][10][2] {feint, answer} | `together01.json` |
| game 01 假動作／答案 anim | 0x100a00a0 / 0x100a00a8 | u16，以值 1..4 為索引 | 0xa96–0xa99 / 0xa9a–0xa9d |
| game 01 結果 anim | 0x100a13c0 | u16[4] | 0xaa0–0xaa3 |
| game 02 版面 | 0x1009a710 | u16[5][20][3] | `together02.json` |
| game 02 交換次數 | 0x1009a6fe | u16[3][3] | 10,16,20 / 12,14,19 / 11,15,18 |
| game 02 移動時間 | 0x1009c598 | float[3] | 1.0, 0.6, 0.3 秒 |
| game 02 心情 / 結果 anim | 0x100a0c60 / 0x10095738 | s16[4] / u16[4] | 0,1,5,10 / 0xaca,0xac9,0xac8,0xac7 |
| game 03 迷宮 | 0x1009c5b0 | char[3][14][12] | `together03.json` |
| game 03 外觀 | 0x1009472c | u16[3][5] | |
| game 03 心情 | 0x100a0c68 | s16[4] | 10,5,1,0 |
| game 04 牌值 | 0x10091ece | u16[10][3] | `together04.json` |
| game 04 圖案 frame | 0x10091f10 | u16[3][2] | (1,2),(3,4),(5,6) |
| game 04 寵物位置 | 0x10091f94 | u16[5] | 1,2,0,2,1 |
| game 04 元件 | 0x10091f0a / 0x10091f8e / 0x10091cbe | u16[3] | 牌 0x567–0x569 / 寵物 0x570–0x572 / 游標 0x56c–0x56e |
| game 06 掃動時間 | 0x10092dc0 | u32[3]（16.16 秒） | 0xa666, 0x8000, 0x6666 |
| game 06 區域 anim | 0x10093e92 | u16[4] | 0xb18, 0xb14, 0xb10, 0xb0b |
| game 07 場地 | 0x1009c7a8 | char[3][19][18] | `together07.json`（與 game 03 的字串相連） |
| game 07 外觀 | 0x1009474a | u16[3][4] | |
| game 07 心情 | 0x100a0c70 | s16[4] | 10,5,1,0 |
| HUNT 種類選單 | 0x10094170 | 2 × 12 B | 0x42 VOLCANO、0x43 SPRING |
| HUNT 訊息 | 0x10094364 | u16[2] | 0x9b, 0x9c |
| HUNT 收穫 | 0x100a0600 | 25 × 6 B | `hunt.json` |
| HUNT 旅程 anim | 0x10093920 | u16[3 ft][2 kind][4 r] | |
| HUNT 回程 anim | 0x10095778 | u16[3 ft][4 r] | |
| HUNT 蛋 anim / 大蛋 sprite | 0x10095790 / 0x10093382 | u16[3] | 0xb2,0xb3,0xb1 / 0x8b,0x8c,0x8a |
| 隊員選擇 UI 樣式 | 0x10093ec6 | 0x22 B / 樣式 | style 0：bg 0x165、游標 0x166、捲軸 0x173–0x175、卡框 0x167–0x16a、角色 0x16b–0x16e、數字 0x16f–0x172 |
| HUD 數字格式 | 0x100a4128 | char[] | `"\x9a%d"` |

**主要 sprite**（已用 `extract_sprites.py` 輸出 PNG 檢查）：0x8b3 紅色圓桌 128×64；0x8b4 牌 32×32×7（背面＋桃子、盆栽、柿子、蓮花、桔梗、老虎…）；0x8b5 牌框 32×48；0x8b6 粉紅箭頭 24×24；0x89d 旗子 32×32×4；0x8c3 計量條 128×32；0x8c2 指標 16×16；0x8c1 球 ×6；0x8ba Shirotama 32×32×8；0x8bb 黃色生物（G）；0x8be 椰子樹牆 64×64；0x8b7 草地 128×128；0x7e 探險風景 128×128；0x748 巴士 ×2；0x8b 彩色蛋 64×64×3（破殼）；0x85 奶油色面板；0x706 GOLDEN PEACH；0x80e 分數框 32×24；0x80f 倒數框 16×24。

---

## 4. 新命名

```
10040ff4 scene_osewa_together f
10027aa8 together_init f
10026d70 osewagame04_init f
10059600 osewagame04_tick f
1002683c osewagame01_init f
100539dc osewagame01_tick f
10070d54 osewagame01_setup f
1008223c osewagame01_next_cmd f
10067960 osewagame01_play_pose f
10078aec osewagame01_input f
10026fb4 osewagame06_init f
100802f4 osewagame06_tick f
100803ac osewagame06_intro f
100804e0 osewagame06_throws f
10080858 osewagame06_result f
1007aee8 osewagame06_grade f
10070fa0 osewagame06_build_bg f
10017c94 gauge_create f
1007a65c gauge_update_cb f
10017dd8 gauge_start f
10017dc0 gauge_running f
10017cf8 gauge_zone f
10017d84 gauge_reset f
10027180 osewagame07_init f
1005a9b4 osewagame07_tick f
10081768 osewagame07_pet_move_cb f
1006d424 osewagame07_object_cb f
10016fc0 osewagame07_follower_cb f
1000b788 osewagame07_capture_fx_cb f
10006fe4 osewagame07_pick_walk_anim f
10062258 osewagame07_follower_anims f
10007330 osewagame07_stun_update f
10025928 osewagame07_spawn_capture_fx f
1006c0d8 osewagame07_ground_scroll_cb f
1008e364 osewagame07_timer_cb f
1002c440 osewagame07_score_cb f
100266d4 osewagame00_init f
10052e7c osewagame00_tick f
10070bc8 osewagame00_setup f
1006784c osewagame00_new_round f
10078a38 osewagame00_input f
100269dc osewagame02_init f
10054364 osewagame02_tick f
10026bbc osewagame03_init f
10055f30 osewagame03_tick f
1008121c osewagame03_pet_move_cb f
1008e21c osewagame03_timer_cb f
1007fe08 osewagame03_tile_cb f
1006bfc0 osewagame03_ground_scroll_cb f
10080274 osewagame03_camera_cb f
100274fc hunt_init f
1001810c hunt_trip_anim f
1001198c hunt_reveal_item_cb f
1002e120 charapicker_create f
1002ea78 charapicker_update_cb f
1002e48c charapicker_build_page f
1002e37c charapicker_result f
1002e2c4 charapicker_get_pick f
1002d5c4 listmenu_create2 f
1002f1c8 choice_create_noicon f
10066b34 tween_set f
10066abc tween_set_now f
100668e4 tween16_set_now f
10066aec osc_set f
10066b84 tween_freeze f
1008ebec tween_eval f
1008ea08 osc_eval f
10023f20 snd_play f
10023f54 music_play f
10024f60 music_play_loop f
10024fd8 music_stop f
10030bb4 snd_set_volume_level f
1000278c idle_exceeds f
10002764 idle_seconds f
10006110 dial_reset f
10006104 dial_angle f
10009b84 cos16 f
10031b74 sin16 f
100694ac bit_test f
100694d4 bit_set f
10018214 field_bg_elem_now f
10000854 actor_play f
1000093c actor_stop f
100008ec actor_play_elem f
1007eea0 ui_element_create f
1006f290 ui_group_create f
1000a268 layer_update f
10009ff4 layer_advance_time f
10023a74 ui_elem_x f
10023ab4 ui_elem_y f
10064f1c text_printf f
10064f94 text_set f
100a10c2 g_flags_commands l
10091ece g_matching_cards l
1009c5b0 g_maze_maps l
1009c7a8 g_shirotama_maps l
1009a710 g_ufo_layouts l
100a0600 g_hunt_rewards l
10093920 g_hunt_trip_anims l
```

---

## 5. 未解問題

1. game 04：`0x1800167a` 的 p、q 兩個計數器在出廠／新遊戲時是否都為 0（若是，則永遠走在全部可贏的路徑上，見 §2.2）。
2. 曲線 2–7 的公式建立在 `FUN_10031b74` = sin（0x10000 = 360°）的假設上；`FUN_10009b84` 已確認是用 sin 表 +90° 的 cos。
3. 背景 2× 縮放時 512×512 sprite 的取樣中心（是否以 sprite 中心對齊 (64,64)）需要實機比對。
4. game 03 的地面捲動與迷宮旋轉方向（寵物在螢幕上固定朝哪個方向走）只從公式推得，未經實機確認。
5. HUD 文字 `"\x9a%d"` 中 0x9a 字元的字形；game 01 左旗 palette 1 的實際顏色（推測白色）。
6. `FUN_10024f60(0x50, 0xc)` 的第三個參數（重複次數？）；`FUN_10025cb4` 設定的 `0x18001cbb bit3` 的完整作用。
7. game 07 G 被撞後掛上的 callback `0x1000b93c`（推測是淡出後刪除）。
8. EGG HUNT 放牧 tama ≥3 隻時空位計數的 bug 是否會造成「選不滿 3 位卻進入選擇畫面」（需要實際測試：UI 會以 flags 擋住已使用的卡片，可能卡在無法完成、只能按 C 取消）。

---

## 6. 摘要（≤15 行）

1. 選單 4 項 = 0x69 MATCHING（game 04 翻牌，純猜）、0x66 FLAGS（game 01 舉旗反應）、0x6a CATCH（game 06 接球計量條）、0x6b SHIROTAMAS（game 07 轉盤控制方向、收集 5 隻）；rank 門檻全 0。05-content §2.11 的「猜箱子／傾斜滾球」說法需更正。
2. 無入口：0x65 ARROW（猜左右）、0x67 3 UFOS（三杯）、0x68 WALKING（轉盤旋轉迷宮）；7 個遊戲的狀態機、時間、資料都已寫出。
3. 全部 20 fps；時間 16.16 秒（0xccc/frame）；內容由存檔循環計數器決定，沒有亂數；game 01 與 game 03 共用同一個計數器。
4. 獎勵只有心情 0/+1/+5/+10，完美時 mission 4 +1；C 鍵離開沒有獎勵。
5. FLAGS：假動作後 20 frames 內照最終姿勢按鍵（A、B、A+B、不按），搶按或錯一次就結束，10/8/1 門檻。
6. CATCH：指標在 x=9↔119 來回掃，每趟 13/10/8 frames；區域邊界 57/89/109；10 秒沒輸入自動判 0 區。
7. SHIROTAMAS：2 px/frame 自動前進，方向 f = 0.5 − 轉盤位置/24；牆壁用格內 10/22 px 判斷；碰撞半徑 26 px；60 秒；計時「暫停」其實沒有暫停。
8. MATCHING：寵物位置與牌組每回合輪替；翻牌前看不到牌面，正常路徑上每回合 50% 猜中（部分組合必輸，但計數器同步時遇不到）。
9. EGG HUNT：選 kind → 名單（自己、放牧 tama、NPC 9014 ×2–4）選 3 位 → 結果依心情與 rand(1,100)；r0 = 9 顆蛋＋GOLDEN PEACH、r1 = 7–8 顆、r2 = 無、r3 = 無＋受傷；每顆蛋 = `0x100a0600[ft*8+kind*4+rand(0,3)]`，物品立即入袋並設 mission 17 位元。
10. 所有固定資料由 `tools/extract_minigames_c.py` 抽出到 `data/minigames/together*.json`、`hunt.json`（含 assert 自我檢查）。
