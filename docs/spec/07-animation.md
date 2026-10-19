# 07 角色動畫（姿勢 / motion script / HOME 寵物行為）

範圍：角色姿勢（data 表 18/16/15）如何隨時間播放、motion script（data 表 13/14）格式與直譯器、actor 角色替換、HOME 寵物動作選擇、部件型別 16/17 的顯示條件。
工具：`docs/spec/tools/decode_motion.py`；資料：`docs/spec/data/motions.csv`（2979 支 script 摘要）。

## 1. 摘要

- **一個 anim number = 一個靜態姿勢**（表 18 的一筆 `{first_kf, part_count}`，每部件一個 keyframe，沒有時間軸）。`FUN_100075c4` 每幀只畫 `inst[3]` 指定的那一個姿勢。
- 時間軸在另一層：**motion script**（data 表 13 = 索引、表 14 = 指令流，共 2979 支）。每支 script 掛在一個 scene node（0x138 B）上，由 `FUN_100016d0` 每幀直譯：指令可「換姿勢（anim）」「換眼/嘴幀」「位移/縮放/旋轉/透明度 tween」「翻轉」「放音效」「生子 script（特效）」，每條指令帶 `W` = 之後等幾幀（20 fps）。
- 寫 `inst[3]`（anim）的只有 script 指令 0x1c（設角色+anim）與 0x1d（設 anim），另有兩個特例（`FUN_10007550`、測試畫面）。
- Script 裡的角色 ID 是**佔位符**（0 / 0x1f，配 B=role），由 actor（`FUN_1000053c`，0xc8 B）的替換表換成真正角色；0xbd–0xdc 是「寵物槽」虛擬 ID（0xbd = 自己的寵物）。
- 每個角色模型有兩組姿勢：**anim 1–20 = 大圖（64×64，HOME_TAMA 特寫）**，**anim 30–53 = 小圖（32×32，HOME_FIELD 場地）**；anim n 的 body 幀 = n−1（大）或 n−30（小）。
- HOME 寵物行為**不是**逐幀 AI，而是進入 HOME_FIELD/TAMA 時（`FUN_10078c94`）依睡眠/cond/待處理呼叫/狀態旗標/飢餓/心情/隨機挑一個「HOME 狀態」（0–31），查表 `0x1009dbd8` 得 3 支 script（場地 / 特寫 / 進 CELL），然後**無限循環**播放。走動、轉身全是 script 內的 x tween + flip，相對固定錨點。
- 部件型別 17（髒污 sprite 221/220，α 51）只在「被畫的是自己的寵物（0xbd）且 `g_tama_status & DIRTY(0x10)`」時顯示；型別 16 **永遠不畫**。

## 2. 詳細規則

### 2.1 姿勢與角色 instance

| # | 規則 | 證據 | 標記 |
|---|---|---|---|
| P1 | Chara instance（0x28 B，`FUN_1007efe8` 初始化）：`[0]` chr、`[1]`?、`[2]` palette、`[3]` anim、`[4]` 眼睛幀覆寫、`[5]` 眼睛來源角色、`[6]` 嘴（型別 0xf）幀覆寫、`[7]` 型別 0x11 顯示開關、`[8..10]` 裝備道具；`+0x16` bit0 自訂寵物、bit1 = chr 是寵物槽、bits2–7 槽號；`+0x17` bit0 = `[5]` 是槽號；`+0x18` bit0 = 回報繪製位置；`+0x1c/+0x20` 繪製時寫回螢幕 x/y、`+0x24` bit0 有效 | `FUN_1007efe8`、`FUN_100075c4`、`FUN_10000164` case 5 | [確認] |
| P2 | 預設值：`[4]=[5]=[6]=0x7fff`（不覆寫）、`[7..10]=0x7ffe`（隱藏）。0x7fff = 用 keyframe 值，0x7ffe = 不畫該部件 | `FUN_1007efe8`、`FUN_10007c14` | [確認] |
| P3 | `FUN_100075c4` 只畫 `inst[3]` 這一個姿勢：逐部件 `FUN_10023af4` 解 keyframe，依型別：0 body（palette=`inst[2]`）；1 eyes（`inst[5]==0x7ffe` 不畫；`inst[4]` 覆寫幀；`inst[5]` 為另一角色時取該角色**同 anim** 的 part 1 sprite）；4/6/8 道具（`inst[8/9/10]`，0x7ffe 不畫）；0xf 嘴（`inst[6]==0x7ffe` 不畫，否則 `inst[6]` 覆寫幀）；**0x10 一律跳過**；0x11 只在 `inst[7]!=0x7ffe` 時畫 | `FUN_100075c4` switch | [確認] |
| P4 | 姿勢內沒有時間資訊：無 duration、無下一幀指標；動畫完全靠外部改 `inst[3]/[4]/[6]` | `FUN_100075c4`、`FUN_10023d64`/`FUN_10023d54` | [確認] |
| P5 | 模型版面（以 ChrID 38 驗證）：anim 0 與 21–29 空；1–20 用 body sprite（64×64）幀 0–19、eyes/mouth 64×32；30–53 用小 body（32×32）幀 0–23、小 eyes/mouth。座標小圖約為大圖一半 | `kfall.py 38` 實測 | [確認] |
| P6 | 座標：keyframe x/y 以 `<<16` 為 16.16，flip 由 inst flags 與 keyframe `[9]` XOR；旋轉 `[4]<<4`、縮放 `[3]<<4`；角度 0x10000 = 360°（sin 表 `0x1009a968` 1024 筆，`FUN_10031b74`） | `FUN_100075c4`、`FUN_10024a78`、`FUN_10031b74` | [確認] |

### 2.2 Motion script 格式（data 表 13/14）

| # | 規則 | 證據 | 標記 |
|---|---|---|---|
| S1 | 表 13 = `u16 off[N+1]`（N = 2979），單位 **u32**；script 位址 = `T14 + off[i]*4`，長度（u16 字數）= `(off[i+1]-off[i])*2`。ID 經 A7 bank 解析（DLC 也可帶 script） | `FUN_10022d98`、`FUN_10022dd8`、`FUN_1006abd4`（`DAT_1800df08` = 表 13、`DAT_1800df0c` = 表 14） | [確認] |
| S2 | word0 = `(指令數 << 1) | loop`；bit0 = 播完是否循環 | `FUN_100016d0`（`*(byte*)script & 1`）；指令數吻合 9/9 個抽樣 | loop [確認]、指令數 [推測] |
| S3 | 指令 = `u16 op`：低 8 bits = opcode；bit15→A、bit14→B、bit13→C、bit11→D、bit10→E、bit9→W 依序跟隨 u16 參數（bit12 不用）；缺的參數 = 0。`0xFCBD` = END | `FUN_100016d0` | [確認] |
| S4 | 每幀（20 fps）每個 node：先清「新生」旗標；若 wait≠0 且 ≠0x7fff 則 −1；**wait==0 時連續執行指令**，每條執行後 `wait = W`。END 或讀到尾：loop 位元 → 跳回 loop 點（初值 0，op 0x1b 設定）並在同一幀繼續；否則 node+5 bit0「完成」（`FUN_100792a0`），由 `FUN_10002090` 回收。單幀超過 1000 條 → assert | `FUN_100016d0`、`FUN_10002090` | [確認] |
| S5 | W = 0x7fff → 永遠停住（不遞減） | `FUN_100016d0` | [確認] |
| S6 | 1 幀 = 1/20 s：tween 時長 `E<<16 / 20`、node tick 的 dt = `frames<<16 / 20` | `FUN_1008ed84`、`FUN_10001398` | [確認] |
| S7 | 同一幀新生的 node（op 0x10 生出）會在該幀內被補 tick | `FUN_10002090` 的 do-while | [確認] |

**Opcode 表**（`FUN_100016d0` switch；名稱為本文件所取）

| op | 名稱 | 參數 / 效果 | 標記 |
|---|---|---|---|
| 0x00 | nop | 只等 W | [確認] |
| 0x01 | sprite | node sprite = A（經 actor 替換表 kind 0）、幀 = C&0x1ff | [確認] |
| 0x02 | frame | 幀 = C&0x1ff | [確認] |
| 0x03 | palette | palette = C | [確認] |
| 0x04/05/06 | tw_x / tw_y / tw_xy | 曲線 B（0–6），起點 C（0x7fff=目前）、終點 D（像素，相對 node 錨點）、時長 E 幀；06 = x→C、y→D 同時（起點皆目前）。E=0 = 立即設定 | [確認] |
| 0x07 | tw_scale | C→D，256 = 1.0 | [確認] |
| 0x08 | tw_rot | C>>4 → D>>4，套用時 `<<8`；故 script 4096 = 360° | [確認] |
| 0x09 | tw_scale_rot | scale→C、rot→D | [確認] |
| 0x0a | tw_alpha | C>>4→D>>4，0x100 = 不透明（script 4096） | [確認] |
| 0x0b | tw_fade | 第二個 0..0x100 值（node+0xd0 → sprite 混色欄位） | 欄位 [確認]，視覺意義 [推測] |
| 0x0c | tw_alpha_fade | 兩者同時 | [確認] |
| 0x0d | flip | node+4 bits6–7 = C（bit0 水平翻轉、bit1 垂直） | [確認] |
| 0x0e | sound | A = 音效 ID，C = 音量（0–100）；C=0 → 若在播就停 | [確認] |
| 0x0f | spawn_obj | 生一般 sprite 物件 A 於 (C,D)，E 覆寫幀，B = z 偏移（±10000） | [確認] |
| 0x10 | spawn_script | 生子 node 跑 script A 於 (C,D)，B = z 偏移；繼承 actor callback | [確認] |
| 0x11 | xform_mode | node+5 bits 4/2/3（是否繼承父的平移/縮放/透明度） | [推測] |
| 0x12 | scene_fx | 寫 scene +0xc..+0xf（色/淡入出參數） | [推測] |
| 0x13 | shake | 寫 scene +0x10..+0x13、清 +0x18/+0x1a（畫面偏移，`FUN_1000108c` 使用） | [推測] |
| 0x14 | visible | node+5 bit1 = C&1 | [確認] |
| 0x16 | layer | node 優先層 bits30–31 = C | [確認] |
| 0x17 | blend | node+4 bits2–3 | [確認] |
| 0x19 | actor_obj | callback msg 4 → actor：建/換場地附屬物件（C=field，−1=目前，−2=只刪除；D<<8 = alpha） | [確認] |
| 0x1a | signal | callback msg 2 → actor：C=−1 設「完成」旗標（`FUN_10000584`）；C=4 設旗標 bit1；並記錄 C/D | [確認] |
| 0x1b | loop_here | loop 點 = 目前位置 | [確認] |
| 0x1c | chr | `inst[0]=A`（再經替換）、`inst[3]=C`（anim）、`inst[4]=D`（眼睛幀）、`inst[6]=0x7fff`；callback msg 5 | [確認] |
| 0x1d | anim | `inst[3]=C`、`inst[4]=D` | [確認] |
| 0x1e | eyes | `inst[4]=C` | [確認] |
| 0x20 | kill_children | 刪除所有子 node | [確認] |
| 0x24 | mouth | `inst[6]=C`（0x7fff 還原） | [確認] |
| 0x15/0x18/0x1f | — | 無動作 | [確認] |
| 0x21–0x23 / 其他 | — | assert | [確認] |

### 2.3 Actor 與角色替換

| # | 規則 | 證據 | 標記 |
|---|---|---|---|
| A1 | Actor（0xc8 B，`FUN_1000053c`）= 一個「可播 script 的角色槽」。`FUN_10000854(actor, script, x, y, z)` 清替換表、建 node（`FUN_100024c0`）、掛 callback `FUN_10000164` | `FUN_10000854` | [確認] |
| A2 | 替換表：kind 0 = sprite、kind 1 = chr、kind 4 = 眼睛來源 chr；每筆 `{from, to, role}`（role = 比對 script 指令的 B 參數，可省略）。`FUN_10000674(a, from, role, to)` 加 chr 替換 | `FUN_100006dc`、`FUN_100005d8` | [確認] |
| A3 | `FUN_10000028(a, script, x, y, z)` = 播放 + 預設替換：chr 0x1f(role 0)→0xbd、chr 0(role 0)→0xbd、眼睛 1→0xbd。也就是 script 中 chr 0/0x1f 代表「主角寵物」 | `FUN_10000028` 反組譯 | [確認] |
| A4 | chr 0xbd–0xdc = 寵物槽 0–31（`FUN_10018730`；槽 = id−0xbd）；0xbd = 自己的寵物、0xdb（槽 30）= 進化/換色前的舊外觀快照 | `FUN_10018730`、`FUN_1003dab8`、`FUN_1003ff80`（COLOR） | 範圍 [確認]，0xdb 意義 [推測] |
| A5 | 替換後 chr == 0x1f → `inst[5]=inst[6]=0x7ffe`：**不畫眼睛與嘴**（蛋、剪影）。HOME 蛋 script 0x538/0x539 用 chr 0x1f | `FUN_10000164` case 5 | [確認] |
| A6 | chr == 2 → 改用 `FUN_10007c68`（由 `UNK_100a1020[DAT_18000c6c&0xf][FUN_1002b314()]` 選角色 + 3 個裝備 `DAT_18000d88/8a/8c`） | `FUN_10000164` case 5、`FUN_10007c68` | [確認]（角色意義未知） |
| A7 | actor `+0x9c` bit0 = 覆寫模式：chr/眼睛直接用 `+0xa0/+0xa2`，`+0xa4` bit0 決定型別 17 | `FUN_10000164` case 5 | [確認] |
| A8 | 每次繪製把 pet 的螢幕座標回寫到 actor `+0xb8..+0xc0`（`FUN_10000000`）；HOME 由場地切到特寫時用它當特寫錨點：`x*2-64, y*2-64` | `FUN_10000164` case 0xc、`FUN_10084df8`、`FUN_1008467c` | [確認] |

### 2.4 型別 16 / 17 部件（Q3）

| # | 規則 | 證據 | 標記 |
|---|---|---|---|
| K1 | 型別 0x10（例：ChrID 38 anim 5 的 sprite 219）：渲染器 `case 0x10: break` → **永不顯示** | `FUN_100075c4` | [確認] |
| K2 | 型別 0x11 = 髒污斑點（sprite 221 大 / 220 小，α 51/64 ≈ 80%）。顯示條件：`inst[7] != 0x7ffe`；`inst[7]` 只在 op 0x1c 的 callback 設定：替換後 chr == **0xbd**（自己的寵物）且 `tama_stat_test(0x10)`（`g_tama_status` DIRTY）→ 0x7fff 顯示，否則 0x7ffe。chr 不是 0xbd（訪客、舊外觀 0xdb、其他角色）一律不顯示 | `FUN_10000164` case 5 | [確認] |
| K3 | DIRTY 由 yogore>2 在起床時設、OTEIRE 清（見 01-care §2.6）；進化事件 `FUN_1003dab8` 也會清 DIRTY | `FUN_1003dab8` | [確認] |
| K4 | 型別 0xf = 嘴、1 = 眼睛（預設部件表 `{0,1,0xf,0x10,0x11}` 對應 body/eyes/mouth/未用/髒污） | `FUN_100075c4`、ChrID 38 解碼 | [確認] |

### 2.5 HOME 寵物動作選擇（Q2、Q4）

`FUN_10078c94`（HOME_FIELD/TAMA init `FUN_1001a61c` 呼叫一次）設定 `home+0xbc`（home = `DAT_18014128`）= HOME 狀態，z（`+0xc0`）= 蛋 200、其他 400：[確認]

1. stage==1（蛋）→ **0**。
2. 否則預設 8；若睡著或午睡（`FUN_10010c80`：`req_is_done(NAP)` 或 `tama_is_sleeping_at`）→ cond==COLD **3**、INJURY **4**、其他 **2**。
3. 醒著且 cond==0：看最高優先待處理呼叫 `req_top_priority`（`FUN_10010b54`）：SOWASOWA(0xe)→**0x11**、ONAKA(0x11)→**0x1b**、GOKIGEN(0x12)→**0x1c**、EVOL_SUPPORT(0x13)→**0x1a**、DODODO(0x1c)→**0x1d**、DODODO_IGNORE(0x1d)→**0x1e**；SLEEP(3) 設 home+0x17c bit6 後走 4；其餘（含無）走 4。
4. 心情路徑：LOVE→**0x12**、IMMUNITY→**0x13**、HAPPY→**0x14**；否則 onaka≤2→**0xe**；否則 gokigen≤5→**rand(11,12)**；否則 DANCE 且 `0x18001c6b` bit1→**0x15**、KIRAKIRA 且 bit2→**0x16**；仍未定則隨機。
5. 隨機：`idx = DAT_18000dd6 % 27`（`DAT_18001bfe` 已設則改用上次存的 `DAT_18001bfc`），取 `0x100a05b4[idx]`，`idx++`，依候選檢查，不合格就下一個，直到選定：1/5 → 場地有朋友（`FUN_1001654c(field,0)`，且不是從 CELL 回來）→ **1**；2 → onaka≥5 → **0xd**；3 → gokigen 6–15 → **rand(8,10)**；4 → gokigen≥16 → **rand(5,7)**；6 → 時段 0 或 3 → **0xf**；7 → `bf0+2`==13 且 0<`bf4`<30 → **0x10**；8 → DANCE → **0x15**；9 → KIRAKIRA → **0x16**。
6. cond 2/3/4 醒著 → **0x17/0x18/0x19**；cond 1、5、6、7 → 保持 8。

播放：`FUN_1008467c(on, slot)` 在 actor `home+0x60` 用 `FUN_10000028` 播 `0x1009dbd8[state*3+slot]`（只有換 ID 時重播）；slot 0 = 場地（以場地角色清單 type 6/7 進入 `FUN_10016a18`，錨點來自 `0x100933ae[field*0x54]`），slot 1 = HOME_TAMA 特寫（zoom 值 0x10000–0x30000），slot 2 = 放大到 CELL（>0x30000）；slot 2 == 0 時禁止進 CELL（home+0x17c bit0）。所有 HOME script 的 loop 位元 = 1 → **永久循環**，HOME 不檢查完成旗標。[確認：`FUN_1008467c`、`FUN_100687dc`、`FUN_10084df8` 反組譯參數]

**HOME 狀態表**（`0x1009dbd8`，32×3 u16；姿勢序列見 `motions.csv`）

| 狀態 | 條件 | 場地 / 特寫 / CELL | 內容摘要（場地 anim ↔ 特寫 anim） |
|---|---|---|---|
| 0 | 蛋 | 0x538 / 0x539 / – | chr 0x1f（無五官）；場地：滾動（rot 90° 步進 + x±32）；特寫：左右晃 ±4 |
| 1 | 隨機：和場地朋友玩 | – / 0x563 / 0xe3 | 特寫：1/2 呼吸、4 跳兩下、8 側身、愛心特效 0x51f |
| 2 | 睡覺 | 0x581 / 0x583 / 0x583 | 50 ↔ 15（閉眼）上下 1px/20 幀 + Zzz 0x4f3/0x4f2 |
| 3 | 睡覺+感冒 | 0x585 / 0x586 / 0x583 | 同上 + 感冒符號 0x4fd/0x4fc |
| 4 | 睡覺+受傷 | 0x587 / 0x588 / 0x583 | 同上 + 0x4e4/0x4e5 |
| 5–7 | gokigen≥16（開心） | 0x560/0x561/0x562 / 0x563 | 30 呼吸、33 跳、34/35 交替（踏步）、42/43 側走、47/48 |
| 8–10 | gokigen 6–15（普通 idle） | 0x564/0x565/0x566 / 0x567 | 30 呼吸 + 38/39 側看、42/43 側走 x±16–20 並 flip；特寫 1 呼吸、眨眼（eyes 1↔0）、e8 |
| 11–12 | gokigen≤5（無聊/不開心） | 0x568/0x569 / 0x56a | 32（低頭）、41 側跳、45、36；特寫 1/3/13/4 |
| 0xd | onaka≥5（飽） | 0x56b / 0x56c | 36 + 嘴 1/3/4；特寫 1 + 嘴 8/9 |
| 0xe | onaka≤2（餓） | 0x56d / 0x56e | 45 側身 + 眼 13/12/9；特寫 3 + 嘴 4/5 |
| 0xf | 早上/晚上時段 | 0x56f / 0x567 | 30、40（眼 12）+ 0x4ea |
| 0x10 | 13:01–13:29 | 0x570 / 0x567 | 36 + 嘴 |
| 0x11 | SOWASOWA（快大便） | 0x571 / 0x572 | 32 / 3，x 抖 1px（2 幀）+ 汗 0x4e8/0x4e9 |
| 0x12–0x14 | LOVE / IMMUNITY / HAPPY | 0x589,0x58d,0x591 / 0x58b,0x58f,0x593 | 30 / 1 + 嘴 6，加各自圖示特效 |
| 0x15 | DANCE | 0x5a1 / 0x5a3 | 30/31/33 ↔ 1/2/4 以 5 幀交替上下跳 4px、38/46 轉身 |
| 0x16 | KIRAKIRA | 0x5a5 / 0x5a8 | 同開心 + 閃光 0x503/0x50b |
| 0x17 | 醒著感冒 | 0x5aa / 0x5ac / 0x5ac | 32 ↔ 3，上下 1px/20 幀 + 0x4fd/0x4fc |
| 0x18 | 醒著受傷 | 0x5bc / 0x5be / 0x5be | 52 ↔ 19 |
| 0x19 | DEATH 倒數中 | 0x5ca / 0x5cc / 0x5cc | 32 ↔ 3 + 0x5cb/0x5cd |
| 0x1a | EVOL_SUPPORT 呼叫 | 0x545 / 0x546 | 30,39,32 ↔ 1,9,3 |
| 0x1b | ONAKA 呼叫 | 0x646 / 0x647 | 32→33 ↔ 3→4，嘴 3/2/4 |
| 0x1c | GOKIGEN 呼叫 | 0x648 / 0x649 | 49/45 ↔ 14/10/11 |
| 0x1d | DODODO 事件 | 0x64a / 0x64b | 39,41,34/35 ↔ 9,11,1/4 + 0x4ef/0x4f1 |
| 0x1e | DODODO_IGNORE | 0x564 / 0x567 | 同普通 idle |
| 0x1f | （哨兵，隨機用） | 0x538 / 0x539 | 不會停在此值 |

### 2.6 其他場景的 motion（Q2）

| 動作 | 場景 / 函式 | Script | 標記 |
|---|---|---|---|
| 吃：看到食物（依喜好 0 普通 / 1 喜歡 / 2 討厭） | OSEWA_GOHAN `FUN_1003ff80` state 3000 case 0；表 `0x10093388` | 0x60c / 0x60f（anim 4、愛心 0x4d8、snd 52） / 0x611（anim 3、snd 13）；食物 sprite 佔位 0x682 → 道具 sprite | [確認] |
| 吃：咀嚼 | 同上 case 1：道具 `+0x14` bit0=0 → `0x10093394[pref*2 + bit1]`；=1 → `0x1009338e[pref]` | 0x5fa/0x600（anim 5→6↔7 咀嚼、snd 20 ×3、尾 snd 21）、0x5fd/0x603、0x606/0x609；另一類 0x619/0x61c/0x61f（anim 3↔1 + 嘴 4/0） | [確認]（bit 意義 [推測]：+0x14 bit0 = 點心類；bit1 = 已不餓或零食選單） |
| 吃完變飽 | case 2，onaka 吃到 6 | 0x613（anim 17、snd 17） | [確認] |
| 拒絕（飽） | onaka==6 且非選單 2 → `0x100933a2[0]` | 0x615（anim 17 眼 15 嘴 12、snd 11、左右抖 2px） | [確認] |
| 拒絕（生病/受傷/垂死） | cond 2–6 → `0x100933a2[1]` | 0x617（anim 10 嘴 12、snd 48、每 10 幀 flip = 搖頭） | [確認] |
| 狀態效果 | GOHAN 後 / EVENT_CHANGESTATUS `FUN_1003e07c` | COLOR 0x622（舊 0xdb → 新 0xbd）、LOVE 0x623、IMMUNITY 0x624、HAPPY 0x625、REDU_CM 0x626；0x59f、0x599 | [確認] |
| 孵化（stage 2） | EVENT_EVOLUTION `FUN_1003dab8` | 0x53c（單）/ 0x541（雙，role 1 = `FUN_1001887c` 夥伴）→ 0x53e / 0x543：chr 0x1f 蛋抖（x±2、snd 23）、縮放淡出，寵物 anim 1 從 y−32 落下、anim 2 跳 | [確認] |
| 進化（其他 stage） | 同上 | 0x549：role 0 = 0xdb（舊）、role 1 = 0xbd（新）；舊的 1/4/8 轉圈 + tw_fade，新的 anim 4 跳出 | [確認] |
| 大便 | EVENT_UNCHI `FUN_1003e824` | 0x574：anim 3 下蹲（y+4）→ 1（嘴 5、snd 19）→ 4 | [確認] |
| 清大便 | EVENT_UNCHI_KAISHU `FUN_1003e90c` | 0x575 | [確認] |
| 治感冒 / 治傷 | `FUN_1003e4a4` / `FUN_1003e664` | 0x5ae、0x5bb；0x5c0（anim 19 → 1）、0x5c8 | [確認] |
| 垂死 / 救援 / 升天 | EVENT_DYING `FUN_1003d474` | 0x5db（anim 16 倒地抖）、0x5ed、0x5e0、0x5e7（anim 3 翻轉後 y→−150 上升）、0x5ef、0x5f4（anim 46 上升+縮小） | [確認] |
| 死亡 | EVENT_DEAD `FUN_1003ccb0` | layout 物件（`FUN_100000bc`）+ 0x672 | [確認] |
| 睡覺 / 起床 | EVENT_TO_SLEEP/TO_WAKE `FUN_1003ecd4` | **無 motion**，只有對話框 0x67/0x66；外觀由 HOME 狀態 2 提供 | [確認] |

**姿勢語意（由使用情境推得）** [推測]：1 正面 idle、2 idle 交替、3 低頭/難過/生病、4 開心跳、5–7 咀嚼、8/9 側身、10 側身搖頭（拒絕/生氣）、11 側跳、13 側身（垂死）、14 背面（無臉）、15 睡覺（閉眼）、16 倒地、17 滿足/拒絕（側身）、19 受傷；場地版 30 idle、31 交替、32 低頭/生病、33 開心跳、34/35 踏步、36 坐、38/39 側看、40、41 側跳、42/43 側走兩幀、44 側跳、45 側身（餓）、46 背面、47/48 無臉、49 無臉（呼叫）、50 睡覺、52 受傷。

### 2.7 HOME 移動（Q4）

- 沒有自由漫遊 AI：寵物錨點固定（場地 = `0x100933ae` 的 layout 座標；特寫 = 場地回寫座標 ×2−64），所有移動都是 script 的 `tw_x/tw_y`（相對錨點、整數像素、可帶曲線）。[確認]
- 走路 = 42/43 每 15 幀交替 + `tw_x` 16–64 px / 60–120 幀；轉向 = op 0x0d flip（bit0 水平）。例：0x565 先 `y→30,x→16 (60 幀)` 後 flip1 走，`x→−20 (90 幀)` flip0 走回，再 `x→0 (120 幀)`。[確認]
- 呼吸 = `tw_y` 0↔1 px（E=0 立即）每 15 幀；睡覺每 20 幀。[確認]
- 進入場地時先空跑 48 次 `FUN_10002090`（`0x30`）讓場地角色就位。[確認 `FUN_10084df8`]

## 3. 資料表

| 位址 | 格式 | 內容 |
|---|---|---|
| data 表 13 @0x12bf12 | u16[2980]，u32 單位偏移 | motion script 索引（2979 支） |
| data 表 14 @0x12d65a | u16 流，224432 B | script 本體（S2/S3） |
| data 表 18 / 16 / 15 | 見 04-assets C6/C7 | 每 ChrID 54 個姿勢；222 筆（47952/0xd8） |
| 0x1009dbd8 | u16[32][3] | HOME 狀態 → {場地, 特寫, CELL} script（§2.5） |
| 0x100a05b4 | u8[27] | HOME 隨機候選序列：5,1,2,6,4,9,7,3,8,9,2,7,1,8,5,6,3,4,2,5,3,4,1,8,7,9,6 |
| 0x10093388 | u16[3] | 看到食物 by 喜好：0x60c,0x60f,0x611 |
| 0x1009338e | u16[3] | 咀嚼（道具 +0x14 bit0=1）：0x619,0x61c,0x61f |
| 0x10093394 | u16[3][2] | 咀嚼（bit0=0）：{0x5fa,0x600},{0x5fd,0x603},{0x606,0x609} |
| 0x100933a0 | u16 | 吃飽：0x613 |
| 0x100933a2 | u16[2] | 拒絕：飽 0x615、生病 0x617 |
| 0x100933ae | 0x54 B × field | 場地 layout（角色錨點 layout ID @+0xc+type*4、z @+0xe；+6 = 寵物錨點） |
| 0x1009a968 | i16[1024] | sin 表，4096 = 1.0，1024 筆 = 360° |

`docs/spec/data/motions.csv`：`id, hex, loop, ncmds, total_frames(含 W 總和，inf = 有 0x7fff), chr_ids(op 0x1c 的 A), anim:frames`（每次換姿勢與其停留幀數）。963 支 script 會改角色姿勢。

## 4. 新命名

```
100075c4 chara_draw_pose f
10022f00 chara_model_get f
10023d64 chara_model_anim f
10023d54 chara_anim_part_count f
10023af4 chara_keyframe_decode f
10023cd4 chara_anim_has_part_kind f
1007efe8 layout_new_chara_node f
10007c14 chara_inst_set_chr f
10007c38 chara_inst_set_pet_slot f
10018730 chr_is_pet_slot f
10018720 pet_slot_to_chr f
10018528 chr_to_pet_slot f
10022d98 motion_script_ptr f
10022dd8 motion_script_len f
100024c0 motion_node_new f
100016d0 motion_script_step f
10001398 motion_node_tick f
10001414 motion_node_update_xform f
10002090 scene_nodes_tick f
100792a0 motion_node_mark_done f
1008ed84 motion_tween_pos f
1008ee20 motion_tween_xy f
1008e820 motion_tween_q4 f
1008e8c4 motion_tween_q4_pair f
1008e980 motion_tween_scale f
10066968 tween_start f
1000053c actor_new f
100005a4 actor_free f
10000854 actor_play f
10000028 actor_play_pet f
100000bc actor_play_layout f
10000154 actor_stop f
10000164 actor_node_callback f
10000584 actor_is_done f
10000594 actor_flag2 f
10000674 actor_map_chr f
100006a8 actor_map f
100006dc actor_map_set f
100005d8 actor_map_find f
10000000 actor_pet_pos f
10078c94 home_pet_pick_state f
1008461c home_pet_motion_id f
1008467c home_pet_play f
10084df8 home_field_populate f
10016a18 field_char_play f
10010b0c onaka_level f
10010ac4 gokigen_level f
10010c80 tama_is_resting f
1003ff80 osewa_gohan_update f
1003dab8 event_evolution_update f
1003ecd4 event_wake_sleep_update f
1009dbd8 g_home_motion_tbl d
100a05b4 g_home_random_seq d
10093388 g_gohan_motion_tbl d
1009a968 g_sin_tbl d
18000dd6 g_home_random_idx l
```

## 5. 未解問題

1. Tween 曲線 0–6 的公式：`FUN_10066b34`/`FUN_10066968` 存參數，求值在 `FUN_100669e8`/`FUN_10066800`（未讀）。
2. op 0x0b（node+0xd0 → sprite 旗標 `(v&0x1f8)<<1`）、0x11、0x12、0x13、0x17 的視覺效果需追 PRAM blitter（`FUN_100322c4`）。
3. 姿勢語意（§2.6 末段）是由 script 使用情境推得；應以 sprite 圖逐一核對（`dumps/*_sprites.zip` body 幀 0–19 / 小圖 0–23）。
4. 場地（HOME_FIELD）其他角色的選擇與 script（`FUN_10016c64`、`FUN_1001669c`、`FUN_10016388`、`FUN_10018978`、0x18001c02 快取）屬場地內容，未展開。
5. 狀態 0x10 的時間欄位：`DAT_18001bec` 結構中 `+6`（推測 hour）、`+8`（推測 minute）需對照 RTC 結構確認。
6. `FUN_10007c68` 的 chr 2 角色表 `UNK_100a1020`（11×3）用途（推測星球居民/訪客）。
7. 型別 16（sprite 219）資料存在但從不繪製：是否為未完成功能或由他處另行使用，未確認。
8. 自訂/DLC 寵物（`inst+0x16` bit0，`FUN_10018634` 每 anim 5 部件 × 0x16 B 嵌在寵物資料 +0x600）的姿勢格式未展開。
