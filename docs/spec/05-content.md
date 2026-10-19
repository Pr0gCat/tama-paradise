# 05 遊戲內容與進度（Content & Progression）

來源：`re/decomp-all/XIP.c`、`ram.c`、`dumps/xip.bin`、全 flash dump。所有位址為 vaddr（XIP 0x10xxxxxx，PRAM 0x0000xxxx，SRAM 0x18xxxxxx，flash 視窗 0x60xxxxxx）。
標記：**[確認]** 直接讀 code／資料；**[推測]** 由上下文推得。

---

## 1. 摘要

- 遊戲以「scene」驅動：場景表 `0x10094830`（137 筆 × 28 bytes，含 `PSID_*` 名稱字串）定義所有畫面；每個 scene 的 update wrapper 呼叫一個大型 state machine（state 1000/2000/…/0x7fe0 結束）。
- 主畫面有 4 個 zoom 層級（`DAT_18001c6a & 3`）：0 = 星球（HOME_SPACE）、1 = 場地（HOME_FIELD）、2 = 寵物（HOME_TAMA）、3 = 細胞（HOME_CELL）；每層有自己的選單表（`0x10094120`）。
- 世界：一顆星球有 3 個 field（`DAT_18000dd7`：0 TROPICAL、1 FROZEN、2 MORI），每個 field 可花 10000 點解鎖第二區域（HOME_FIELD2，`DAT_18000c6d` bit）。天氣每日依月份機率表抽 6 種 pattern 之一。
- 進度軸：**Planet rank**（`DAT_18000c6c` 低 nibble，0–10）由進化、送去放牧／結婚、首次切換 field 推進；rank 解鎖商店分頁、選單、物品、探險隊伍人數、DODODO 事件。另有 **22 個 mission**（每個完成 +1000 點）。
- 經濟：Gotchi 點數 `DAT_18000ca4`（上限 99999）。主要來源：進化（+300/+500/+800）、mission（+1000）、新年（+1000）、小遊戲（10–800）、每日一次 +50、arcade、賣東西。支出：買物品（週期性半價）、解鎖區域 10000、保母 300。
- 物品：flash `0x60125972` 有 343 筆 × 46 bytes 的物品表（food/snack/玩具/帽子/眼鏡/飾品/可拜訪星球），已抽成 `docs/spec/data/items.csv`。背包 640 格，每格數量 4-bit，依類型上限（食物 10、帽子 5…）。商店每日依 `(月+日)%5+1` 輪替。115 組 8 位數 shop code 抽成 `data/shopcodes.csv`。
- 事件：日曆事件（生日、新年、每月 9/19/29 流星、7/14/21/28 流星雨、11/23 UFO、12 月 Santa/Eve/Xmas、每月 13 日 17 時 DODODO）由 `FUN_10083984` 每日排程，再於回到 home 時依時段與 `rand(0,99)` 觸發。
- 社交：12 位朋友、紅外／TCP playdate（玩、打架、吃掉、被吃、結婚）、放牧（3 field × 8 格 ghost）、結婚（連線或離線媒合）、死亡 ghost 5 格、DLC 40 格。

---

## 2. 詳細規則

### 2.1 場景表與 zoom

| 項目 | 規則 | 證據 |
|---|---|---|
| 場景表 | `0x10094830`，137 筆，每筆 `{u32 id, fn_a, fn_update, fn_c, fn_d, u32 flags, char* name}`；0x88 = `PSID_INIT_DUMMY` 結尾 **[確認]** | xip.bin 直接解析；`FUN_100028cc(id)` 切換 scene |
| Zoom 層級 | `DAT_18001c6a & 3`：0 HOME_SPACE(0x1d)、1 HOME_FIELD(0x1e)、2 HOME_TAMA(0x1f)、3 HOME_CELL(0x20) **[確認]** | `FUN_1001abd0`（HOME init）依此值選 scene |
| HOME_FIELD/TAMA state machine | `FUN_10039eb8`；state 0xfa0 → 縮小到 HOME_SPACE，state 0x1004 → 放大到 HOME_CELL **[確認]**；dial 轉動與按鍵映射細節 **[推測]** | `FUN_10039eb8` |
| HOME_SPACE | `FUN_1003aff0`；HOME_CELL `FUN_100398c8`；HOME_FIELD2 `FUN_1003b8f4` **[確認]** | wrapper 0x1001b688 / 0x10019f5c / 0x1001a400 |
| 進 FIELD2 | 在 field 畫面按鍵，且 `FUN_10018358(field)`（該 field 區域已解鎖）為真才進 0x21 **[確認]** | `FUN_10039eb8` L138–158 |
| 各層選單 | 表 `0x10094120`（`{ptr,count}` × 4，項目 12 bytes，+4 = 目標 scene）**[確認]**：<br>0 星球：TUSHIN(0x77)、LOOKSTARS(0x7f)、DECO(0x80)、TRAVEL(0x7e)、RANK(0x7d)<br>1 場地：SETSUBI(0x27)、HUNT(0x26)、POOPCLEANING(0x28)、FIELDSWITCH(0x25)<br>2 寵物：GOHAN(0x22)、TOGETHER(0x24)、OTEIRE(0x23)<br>3 細胞：CELL_STATUS(0x29) | `FUN_1001abb4` |
| 星球選單限制 | 睡覺中（`FUN_10010c80`）時 0、3 項不可（msg 0x69）；DECO 需 rank≥3（msg 0xf4）；TRAVEL 需成長階段≥3（msg 0x6b）且 rank≥3 **[確認]** | `FUN_1003aff0` L148–167 |
| 場地選單限制 | 睡覺中 0–3 項全部不可；SETSUBI 需 rank≥2（0xf3）；HUNT 需階段≥3（0x6b）且 rank≥1（0xf2）；FIELDSWITCH 需 rank≥4（0xf5）**[確認]** | `FUN_10039eb8` L388–415 |
| Rank 不足訊息 | msg 0xf2/0xf3/0xf4/0xf5 = 需要 rank 1/2/3/4 **[推測]**（依呼叫條件對照） | 同上、`FUN_1004a438` |
| 回到 home 的優先序 | `FUN_1001abd0`：完成 HOMETRG/UNCHI request → 若狀態 `DAT_18000d08`==6 進 EVENT_DEAD(0x4a)、==5 進 EVENT_DYING(0x49) → 若 rank 上升（高 nibble < 低 nibble）進 TAMASPACE_RANK(0x7d) → 若有 mission 剛達成（`FUN_10025368`）進 INFO_MISSION(0x39) → 保母中進 HELP_SITTER(0x41) → 有 COLOR/LOVE/IMMUNITY/HAPPY/REDU_CM/DOSSARI/NAP request 進 CHANGESTATUS(0x4e) → 隨機事件（§2.9）**[確認]** | `FUN_1001abd0` |

### 2.2 Gotchi 點數

| 規則 | 數值 | 證據 |
|---|---|---|
| 餘額 | `DAT_18000ca4`（u32），`points_get` 0x10010ab4 **[確認]** | |
| 加點 | `points_add(n)`：餘額 = min(餘額+n, 99999)；n≠0 時另累加「歷史總賺取」`DAT_18000cc4`（mission 14，上限 99999）。注意：用 unsigned 比較，負數 n 會變成極大值而被夾到 99999，所以扣點不用這個函式 **[確認]** | `points_add` 0x1000f170 |
| 扣點 | `points_spend(n)` 0x10010e30：若 n ≤ 餘額則扣除並回傳 true，否則不變、回傳 false **[確認]** | |
| 進化獎勵 | 嬰→幼 +300；幼→少 +500；→成（一般）+800；特殊路線直接到 stage 5 也 +800 **[確認]** | `FUN_1000fc98` case 2/3/4 |
| Mission 獎勵 | 每個 +1000（表 `0x1009430c`，22 × u16 全為 1000）**[確認]** | `FUN_10048bd0` state 4000 |
| 新年事件 | +1000 **[確認]** | `FUN_1003f1f0` state 0xbba |
| Lab 小遊戲結果 | `0x10091bec[result + level*4]`：level0 = 10/100/150/250，level1 = 10/200/300/500，level2 = 10/300/450/800（result 0..3）**[確認]** | `FUN_100228c4`（LABOGAME_RESULT） |
| 每日一次 +50 | **MICROSCOPE 的 LAB CODE**（§2.4a）：輸入的是表中「NO DATA」的有效碼（結果 4），訊息 0xe7 之後若 `DAT_18000be9` bit0 未設 → 畫面 0xe2「LOOKING FORWARD TO SEEING YOU AGAIN」+50 並設 bit0；bit0 每日在 `FUN_10013234` 清除 **[確認]** | `FUN_1005e61c` state 0xca、`FUN_1005cc78` |
| Arcade 連線遊戲 | 加上分數欄位（ghost +0x1c）**[確認]** | `FUN_100883e0` |
| 賣物品 | +售價 × 數量 | `FUN_1004af0c` |
| Shop demo | 一次 +99999（展示模式，debug `shopdemo`）**[確認]** | `FUN_10013158` |
| Debug `point n` | 設定餘額 = n（透過 `points_add(n - 目前)`） | `cmd_point` |

### 2.3 物品

#### 表格與欄位
- 物品表在資產區（flash `0x60125972`–`0x60129714`，343 筆 × 46 bytes，23 × u16）。執行時透過 bank 表 `DAT_1800defc + bank*0x5c`（`[begin,end)` 指標）查詢，bank 0 = 內建，其他 = DLC。`item_find(id)` 0x10022ff0 以 0x17 個 u16 為步距線性搜尋 **[確認]**。
- 欄位（u16 index / byte offset）：

| idx | off | 名稱 | 意義 | 證據 |
|---|---|---|---|---|
| 0 | 0x00 | id | 5xxx food、6xxx snack、7xxx 玩具、8xxx 星球裝扮、99xx 星球 | |
| 1 | 0x02 | idx | 連續索引（圖鑑？）**[推測]** | |
| 2,3 | 0x04 | name_txt | 文字 id | |
| 4,5 | 0x08 | spr | 場地物件 sprite id（玩具） **[推測]** | |
| 11 | 0x16 | type | 1 Food、2 Snack、3 小型戶外玩具、4 大型戶外玩具、5 帽子、6 眼鏡、7 飾品、8 可拜訪星球 **[確認]**（filter `FUN_1007d730`；與 GMMan 清單一致） | |
| 12 | 0x18 | buy | 買價（0xffff 不可買） **[確認]** | `FUN_1004a438` |
| 13 | 0x1a | sell | 賣價 **[確認]** | `FUN_1004af0c` |
| 14 | 0x1c | food_group | 食物類別 1–18（寫入飲食紀錄 `DAT_18000d4b[group]`） **[確認]** | `FUN_1001021c` |
| 15 | 0x1e | ver | 內容版本 1/2/3 **[推測]** | |
| 16 | 0x20 | req_rank | 商店顯示所需 rank（0 = 不限） **[確認]** | `FUN_1007d730` |
| 17 | 0x22 | like_tag | 與角色定義 +0xc 相同 → 心情 +1；與 +0x1a 相同 → −1 **[確認]** | `FUN_100171fc` |
| 18 | 0x24 | snack_flag | snack 且角色 stage==5 時 +1 心情 **[確認]** | `FUN_100171fc` |
| 19 | 0x26 | onaka | 吃下時 `care_add_onaka(x)` **[確認]** | `FUN_1001021c` |
| 20 | 0x28 | gokigen | snack：心情加值；食物：飲食紀錄加值 **[確認]** | 同上 |
| 21 | 0x2a | day_group | 1–5：只有 `(月+日)%5+1 == day_group` 那天出現在商店；0 = 每天 **[確認]** | `FUN_1007d730` |
| 22 | 0x2c | flags | bit0 需要 shop code 解鎖、bit1 在商店販售、bit2 可賣、bit3 系統物品（不可送／不可賣） **[確認 bit0/bit1；bit2/bit3 推測，與 GMMan 欄位對照 100% 吻合]** | |

- 分類統計：Food 85、Snack 51、小玩具 57、大玩具 30、帽子 52、眼鏡 8、飾品 48、星球 10、空白 2（id 5086、6052）。

#### 背包
- 640 格（0x280）：物品 id `DAT_18000e54[i]`（u16），數量為 4-bit nibble `DAT_18001354[i>>3] >> ((i&7)*4)` **[確認]**（`FUN_1007e760`/`1007e71c`/`1007e834`/`1007e884`）。
- `item_give(id, n)` 0x1001e0d8：數量 = clamp(現有+n, 0, cap[type])；cap 表 `0x10093d2c` = `[0,10,10,10,10,5,5,5,1,2,8,9]`（type 1–4 為 10，5–7 為 5，8 為 1）**[確認]**。
- 解鎖狀態 bitset `DAT_18000e04`，用物品在表中的序號索引（`FUN_10023084` → `FUN_1007ddd0`）**[確認]**。

#### 吃東西效果（`FUN_1001021c`，EM_REQ_GOHAN 完成時）**[確認]**
1. `care_add_onaka(item.onaka)`。
2. 若角色 GrowthStage < 4 且 food_group 屬於該角色 Attribute 對應的 field 範圍（attr 類別 1 → group 13–15 TROPICAL、2 → 16–18 FROZEN、3 → 10–12 MORI；`FUN_1001dd2c`）：`d4b[group] += item.gokigen`；超過 4 時減 5，並把 `group-1` 推入最近 4 筆的飲食歷史 `DAT_18000d27[]`（計數在 `DAT_18000d26`）。少年期進化時由 `FUN_10081d70` 取該 field 3 種食物中吃最多的那種決定分支（平手 → 3）。
3. bonus = `FUN_100171fc`（喜好 ±1；GrowthStage>4 吃 PELLETS 5002 −1；stage 5 吃 snack_flag 的 snack +1）。
4. type 1（餐點）：`care_add_gokigen(bonus)`，mission1 `DAT_18000ca8`++（上限 500）。type 2（snack）：`care_add_gokigen(item.gokigen + bonus)`，mission2 `DAT_18000caa`++（上限 500）。
5. 特殊 snack：

| id | 名稱 | 效果 |
|---|---|---|
| 6002–6007 | BE RED/BLUE/GREEN/PINK/SKY BLUE/PURPLE JUICE | 顏色設為 0/4/6/9/11/10，並發出 EM_REQ_COLOR |
| 6008 | BE VIVID JUICE | 顏色 = rand 0–8（不同於目前） |
| 6009 | BE PASTEL JUICE | 顏色 = rand 9–15（不同於目前） |
| 6010 | HAPPY PANCAKES | +20 心情，EM_REQ_HAPPY |
| 6011 | LOVE-LOVE ICE CREAM | 只在 stage 5 有效：EM_REQ_LOVE |
| 6012 | CHEERFUL POPCORN | EM_REQ_IMMUNITY |
| 6013 | LARGE SOFT SERVE | EM_REQ_DOSSARI |
| 6014 | SLEEPY HERBAL TEA | 清除狀態 bit，NAP 計時器歸零，EM_REQ_NAP（午睡 60 秒） |
| 6015 / 6047 | RANDOM JELLY / RANDOM BOBA | `rand(0, stage==5?2:1)`：0 → +20 心情、HAPPY；1 → IMMUNITY；2 → LOVE |
| 6016 | DANCING COTTON CANDY | status = DANCE(0x20) |
| 6017 | SPARKLING DRINK | status = KIRAKIRA(0x40) |
| 6046 | BE DARK GREEN JUICE | 顏色 6 |
| 6048 | GOLDEN PEACH | EM_REQ_REDU_CM（照顧失誤 −1，`FUN_10068aa0` case 0x1b） |
| 6049 / 6050 | SUNSUN ORANGE / WHITE SLUSHY | 顏色 8 / 14 |

- 吃飯拒絕條件（`FUN_1003ff80` state 0x4b0）：onaka == 6 時拒絕餐點（snack 仍可吃）；狀態 `DAT_18000d08` 為 2–6（生病／瀕死等）時拒絕 **[確認]**。

#### 料理（PSID_SHOP_COOKING → `FUN_1004b624`）**[確認]**
- 12 道食譜，表 `0x10091fbc`（`{u16 成品, u16 材料}`）：5062←5056、5063←5058、5064←5059、5065←5061、5072←5067、5073←5068、5074←5069、5075←5070、5082←5076、5083←5079、5084←5080、5085←5081。
- 條件：材料 ≥ 3 且 onaka ≠ 6；消耗 3 個材料，立刻吃成品（`FUN_10068aa0(4, 成品)`）；mission 18 bit[recipe] 設起（12 bits）。

### 2.4 商店（`PSID_SHOP_*`）

| 規則 | 數值 | 證據 |
|---|---|---|
| 買（`FUN_1004a438`） | 4 個分頁：0 Food（cat 1）、1 Snack（cat 2）、2 玩具（cat 8 = type 3+4）、3 裝扮（cat 9 = type 5–7）；表 `0x10093d34` **[確認]** | |
| 分頁 rank 限制 | 分頁 0/1 需 rank ≥ 1，2 需 ≥ 2，3 需 ≥ 3 **[確認]** | 同上 state 1000 |
| 商品清單 | `FUN_1001e7e0(0, cat)`：type 符合、rank ≥ req_rank、flags bit1、day_group 符合當日、bit0 項目須已用 code 解鎖；最多 256 項；以 id 排序 **[確認]** | `FUN_1007d730` |
| 半價日 | 日期個位數 = 5 → Food/Snack 半價（`DAT_18000be8` bit6）；= 0 → 玩具／裝扮半價（bit5）；價格 = buy >> 1 **[確認]** | `FUN_10083984`、`FUN_1004a438` |
| 購買 | 總價 = 單價 × 數量，數量上限 = cap − 持有；`points_spend` 後 `item_give` **[確認]** | state 0x514 case 3 |
| 賣（`FUN_1004af0c`） | 可賣數 = 持有 − 已擺放（`FUN_1001e628`）；得到 sell × 數量；`item_give(id, −n)` **[確認]** | |
| Shop code（`FUN_1004bcf4`、`FUN_1006da10`） | 輸入 8 位數字；與表 `0x100a1a20`（109 筆 `{u16 item, char* code}`）＋ PRAM 表 `0x0000e25c`（6 筆，筆數在 `0x0000e1b0`）比對；命中 → 解鎖該物品（之後才會在商店出現）**[確認]**。清單：`data/shopcodes.csv` | |
| 區域解鎖（`FUN_1004c1f8`） | 3 個 field 各一個擴充區，價格表 `0x10093d14` = 10000 × 3；點數不足 → state 5000（錯誤訊息）；成功：`DAT_18000c6d |= 1<<field`，扣 10000 **[確認]** | |
| 保母（HELP_SITTER → `FUN_1004d304`） | 費用 300。條件：成長階段 ≥ 3（msg 0x6b）、時間 06:00–18:58（0xd6）、onaka 與 gokigen 皆 > 0（0xd7）、點數 ≥ 300（0xd8）、未睡覺（0xd9）。成功：`DAT_18000d08 = 1`（託管中，即 cond 1）**[確認]**；19 點時段開始時若仍託管：結束並 **−5 心情** **[確認]**（`FUN_10012c90`）。託管動畫 0x831 來、80% 0x832 / 20% 0x833、0x834 走；託管中 HOME 一律路由到這個畫面，按鍵可提前結束（0xd4，預設 NO，不扣心情）**[確認]**（08-care-ui §2.18） | |

#### 2.4a MICROSCOPE（LABOMENU→BOOK→MICROSCOPE，PSID_BOOK_CELL 0x40，`FUN_1004f360`）[確認]

- BOOK（0x36，`FUN_100464c4`）兩項：TAMA FILE（0x09 → 0x3f）、MICROSCOPE（0x0a → 0x40）。
- MICROSCOPE 選單（`FUN_1005ce28`）：LAB CODE（0x10）、COLLECTION（0x11，`0x18001684` 沒有任何位元時反灰）。
- LAB CODE（`FUN_1005c9f8`）：訊息 0xe3 → 文字鍵盤（`FUN_10065034`，最多 16 字）→ SEARCHING…（0xe1，40 frames，`FUN_1005e61c`）→ 比對：長度 ≠ 8 → 0xe4；表外 → 0xe5；已收集 → 0xe6；表中 chr = 0 → 0xe7「NO DATA」（+ 每日 +50，見 §2.2）；否則 → 結果畫面（`FUN_1005c0b0`：UI 0x272 背景、0x273 角色、0x274 NEW!、0x275 名字），並 `FUN_1001f8a8(index)` 記錄收集。
- 碼表 XIP `0x100a1660`：60 筆 × 16 B `{u16 chr, u16, char* code8, ..}`（`FUN_1006db1c`；英數 8 碼）。0–28 有角色（ChrID 0x40–0xa1，都是成體），29–59 是 NO DATA 碼。NO DATA 碼不會被標成已用，所以每天都能換 +50。重製：`shop::LAB_CODES`、`Pet::lab_code`。
- 未做：COLLECTION 檢視（0xdac `FUN_1005ead0`）、結果畫面的放大（dial 上，0x1194）。

### 2.5 世界：field、區域、天氣

| 規則 | 數值 | 證據 |
|---|---|---|
| Field | `DAT_18000dd7` ∈ {0,1,2}；debug 名稱表 `0x10091650`：TROPICAL、FROZEN、MORI **[確認]**；對照：探險、料理食材與飲食分支都依此順序（0 熱帶食物 5066–5071、1 冰凍 5076–5081、2 中式／森林 5056–5061）**[確認]** | `FUN_100727a0`、`FUN_10081d70` |
| 換 field | `set_fieldtype(f)` 0x10010cf4：若 rank == 4 且 f ≠ 目前 → rank 升到 5；換 field 時清除每 field 停留計時 `DAT_18000d46..4a`；若 stage==3 且有 EVOLUTION request 則清除它 **[確認]** | |
| FIELDSWITCH 場景 | `FUN_1004140c`（需 rank ≥ 4），內含 METAMORPHO 處理 **[確認]** | |
| Attribute → field | attr 1,5 → 0；2,6 → 1；3,4 → 2（`FUN_100039d4`）；`FUN_10003974` 同樣映射但從 1 起算 **[確認]** | |
| 區域解鎖 | `DAT_18000c6d` bit0–2（每 field 一個）；查詢 `FUN_10018358`，設定 `FUN_10016520`；debug `areaunlock 0..2` **[確認]** | |
| 天氣 pattern | 每日（`FUN_10013234`）`DAT_18000dd8 = FUN_1008fdec()`：r = rand(0,99)，依月份表 `0x10091b5c`（12 × 6 u16，只用前 5 欄做累加）取第一個累加 > r 的 index；都沒有 → 5 **[確認]** | |
| 月份機率（pattern 0..4，剩下為 5） | 1,2,12 月：30/0/10/60/0；3,4,5,7,10,11 月：60/10/10/0/10（5 = 10%）；6 月：10/30/20/0/30（5 = 10%）；8,9 月：30/10/10/0/10（5 = 40%） **[確認]** | |
| 時段 | `FUN_1008e704(hour)`：<6 → 4、6–9 → 0、10–15 → 1、16–18 → 2、19–21 → 3、≥22 → 4 **[確認]** | |
| 當下天氣 | `DAT_18001bf9 = 0x1009138c[pattern*5 + 時段]`；pattern 表：0 = 00000、1 = 22222、2 = 11111、3 = 00022、4 = 22000、5 = 00200 **[確認]**；背景 layout = `FUN_1000aeac(field)[period*3 + weather + 1]`（`FUN_10015d90`），weather == 2 時另開粒子效果 flag，field 1（FROZEN）用不同的粒子 callback（0x1001613d，其他 0x10015ef1）**[確認]**；所以 0 = 晴、1 = 陰、2 = 降水（FROZEN 是雪、其他是雨）**[推測]** | `FUN_10010c2c`、`FUN_10010e10` |

### 2.6 Planet rank

- `DAT_18000c6c`：低 nibble = 目前 rank（0–10），高 nibble = 上次看過的 rank；home 發現高 < 低 時播放 TAMASPACE_RANK（0x7d，`FUN_100570bc`）再同步 **[確認]**。
- 升級條件（皆為 `rank = rank+1`，舊值移入高 nibble）**[確認]**：

| 從 → 到 | 觸發 | 函式 |
|---|---|---|
| 0 → 1 | 第一次進化成幼年期 | `FUN_1000fc98` case 2 |
| 1 → 2 | 進化成少年期 | case 3 |
| 2 → 3 | 進化成成年期 | case 4 |
| 3 → 4 | 世代交替：送去放牧（`FUN_1004dcd0`）、離線媒合結婚（`FUN_1004e1ec`）、連線結婚（`FUN_1008c430` result 4） | |
| 4 → 5 | 首次切換到不同 field（`set_fieldtype`、FIELDSWITCH `FUN_1004140c`） | |
| 5..9 → +1 | 每次世代交替（同 3→4 的三種方式） | |
| 上限 10 | rank 10 播放片尾（ENDCREDIT / RANKUP_END 0x81/0x82）**[推測]** | `FUN_100570bc` 只在 <10 時顯示下一級 |

- Rank 解鎖：商店分頁（1/2/3）、選單（SETSUBI 2、HUNT 1、DECO/TRAVEL 3、FIELDSWITCH 4）、物品 req_rank、探險隊伍人數（§2.8）、DODODO 事件（≥6）。
- Debug：`planetrankup n` 與 `prank n` 指向同一個 handler 0x10073d8c（`c6c = (n | n<<4) - 0x10`，n 夾在 1..10）**[確認]**（BL 解碼確認兩者都呼叫 0x10073d8c）。**「prank」是 planet rank 的縮寫，不是惡作劇功能。**

### 2.7 Mission（22 個）

- 達成檢查 `FUN_10025368`（每次回到 home 時）：counter ≥ 目標 → 設「已達成」bitmask `DAT_18000cdc`（`FUN_10025868`）；畫面 INFO_MISSION（`FUN_10048bd0`）領獎：+1000，設「已領」`DAT_18000ce0`（`FUN_10025808`）**[確認]**。
- 目標表 `0x100942b4`（u32 × 22）；文字 id 表 `0x100a4ce0`（169..190）**[確認]**。

| # | 計數器 | 目標 | 增加時機 | 證據 |
|---|---|---|---|---|
| 1 | `ca8` u16 | 500 | 吃餐點 | `FUN_1001021c` |
| 2 | `caa` | 500 | 吃 snack | 同上 |
| 3 | `cac` | 100 | 完成 OTEIRE（清潔），+1 心情 | `FUN_10068aa0` case 5 |
| 4 | `cae` | 100 | 「一起玩」小遊戲拿到完美結果 | `10052e7c` 等 OSEWAGAME |
| 5 | `cb0` | 100 | 清便便的次數 | `FUN_100108d4` |
| 6 | `cb2` | 1000 | 清掉的便便總數（含放牧 tama 便便加權） | 同上 |
| 7 | `cb4` | 10 | 完成 SOWASOWA（想上廁所 → 帶去） | `FUN_10068aa0` case 0xe |
| 8 | `cb6` 7 bits | 7 | 旅行去過的 7 個目的地 | `FUN_1006666c`（TRAVEL 獎勵） |
| 9 | `cb8` | 50 | SLEEP request 時關燈哄睡 | `FUN_1003ecd4` |
| 10 | `cba` | 50 | WAKE request 時叫醒 | 同上 |
| 11 | `cbc` | 100 | 連線 playdate／arcade 次數（輸贏都算） | `FUN_100883e0`、`FUN_1008c430` |
| 12 | `cbe` | 50 | 連線交換物品 | `FUN_100899c0` |
| 13 | `cc0` | 100 | Lab 小遊戲拿到最高結果（result 3） | `FUN_100228c4` |
| 14 | `cc4` u32 | 99999 | 歷史總賺取點數 | `points_add` |
| 15 | `cc8` | 50 | 結婚次數 | `FUN_1004e1ec` |
| 16 | `cca` | 50 | 探險大成功 | `FUN_10041df8` |
| 17 | `ccc` 18 bits | 18 | 探險發現的 18 種食材 | `FUN_10041df8` |
| 18 | `ccf` 12 bits | 12 | 12 道料理 | `FUN_1004b624` |
| 19 | `cd1` 16 bits | 16 | field 0 成年角色圖鑑 | `FUN_1000fc98` case 4 |
| 20 | `cd3` 16 bits | 16 | field 1 成年角色 | 同上 |
| 21 | `cd5` 16 bits | 16 | field 2 成年角色 | 同上 |
| 22 | `cd7` 16 bits | 16 | 顏色（0–15）收集 | `FUN_1000f708`、`FUN_10068aa0` case 0x15 |

- Debug：`missionqualifynum [n]` 印出、`setmissionnum n v`、`qualifymission n`、`allqualifymission` 把計數直接設到目標 **[確認]**。

### 2.8 探險（PSID_OSEWA_HUNT → `FUN_10041df8`）

- 只能在時段 0–2（06:00–18:59）**[確認]**。
- 隊伍：自己（標記 60000）＋目前 field 的放牧 tama（`FUN_100188d8` 數量）＋填充 NPC 0x2336：rank < 6 → 2 隻，6–9 → 3 隻，≥10 → 4 隻 **[確認]**。每個 field 每日有 13 格的使用 mask `DAT_18000ce8[field]`，空格 < 3 時拒絕（state 0x1964）；每日與進化時重置 **[確認]**。
- 結果 r = rand(1,100) **[確認]**：
  - gokigen ≥ 20：r ≤ 50 → 大成功（result 0，mission 16 +1）；否則 result 1。
  - 0 < gokigen < 20 → result 1。
  - gokigen == 0：r ≤ 10 → result 3（受傷 EM_REQ_INJURY）；否則 result 2。
- 收穫數：result 0 → 9 個，其他 → rand(7,8) 個。每個 = 表 `0x100a0600[(field*8 + kind*4 + rand(0,3))]`（6 bytes：sprite、item id、mission17 bit），kind = 0/1 **[確認]**。大成功額外得到 GOLDEN PEACH 6048 ×1（state 0x138d）**[確認]**。

### 2.9 事件與 request（EM_REQ_*）

#### Request enum（`FUN_100378d0` 字串對照）**[確認]**
0 NONE、1 EVOLUTION、2 WAKE、3 SLEEP、4 GOHAN、5 OTEIRE、6 BIRTHDAY、7 NEWYEAR、8 DEATH、9 DEATH_DELAY、10 COLD、11 INJURY、12 DYING、13 DEAD、14 SOWASOWA、15 HOMETRG、16 UNCHI、17 ONAKA、18 GOKIGEN、19 EVOL_SUPPORT、20 METAMORPHO、21 COLOR、22 LOVE、23 IMMUNITY、24 HAPPY、25 DOSSARI、26 NAP、27 REDU_CM、28 EVENT_DODODO、29 EVENT_DODODO_IGNORE、30 EVENT_METEOR、31 EVENT_SHOOTINGSTAR、32 EVENT_UFO、33 EVENT_SANTA、34 EVENT_EVE、35 EVENT_XMAS。

- API：`req_raise(id)` 0x10069218、`req_clear(id)` 0x10068a50、`req_pending(id)` 0x10010c68（`FUN_1006916c(id,1)`）、`req_done(id,arg)` 0x10068aa0（執行完成效果並設「今日已完成」bitset `DAT_18000cf3`）**[確認]**。

#### 時間推進（`FUN_10012c90`）**[確認]**
- 比較 RTC `DAT_18001bec`（年 u16、月 u16、日 u16、時 u16、分 u16、秒 u16）與上次時間 `DAT_18000c00`；以 60 秒為單位呼叫 `FUN_100134bc(t, 60)`，最多 1440 塊（離線補算上限 24 小時），再補餘數。
- 跨日 → `FUN_10013234`（每日處理）＋ `FUN_10083984`（日曆事件）。
- 成長階段 ≥3 且非午睡：時段 4（22–05 點）時完成 WAKE/SLEEP；其他時段發 WAKE，時段 ≥3 發 SLEEP。

#### 每日處理（`FUN_10013234`）**[確認]**
- 若狀態 `d08` < 6 且非 OTEIRE 中：髒污計數 `yogore_count` +1（上限 3）。
- 遊玩天數 `DAT_18000c98` +1（上限 999）。
- 清除 WAKE/SLEEP/OTEIRE/BIRTHDAY/NEWYEAR/所有事件 request。
- rank < 6 或 stage < 3：DODODO 直接 raise 並 done（等於跳過）。
- 抽今日天氣；重置媒合次數 `c9c/c9e` 與探險 mask `ce8`；呼叫日曆事件。

#### 日曆事件（`FUN_10083984`，跨日時）**[確認]**

| 事件 | 條件 |
|---|---|
| BIRTHDAY(6) | 今天 = 使用者生日（`DAT_18000c6a`：bits0–3 月、bits4–8 日；2/29 在非閏年改成 3/1）`FUN_1006d904` |
| NEWYEAR(7) | 1/1 |
| METEOR(30) | 日 % 10 == 9（9、19、29 日） |
| SHOOTINGSTAR(31) | 日 % 7 == 0（7、14、21、28 日） |
| UFO(32) | 11/23 |
| SANTA(33) | 12/1–12/23 |
| EVE(34) | 12/24 |
| XMAS(35) | 12/25 |
| 半價 | 日 % 10 == 0 → 玩具／裝扮；== 5 → 食物／snack |

#### 觸發到畫面（回到 home 時，`FUN_1001abd0`）**[確認]**
前提：stage ≥ 3、非睡覺狀態（`TamaData+0x10 == 0`）、旗標 `DAT_18001be8` bit5。r = rand(0,99)，p = 時段。若 ONAKA/GOKIGEN/SOWASOWA 都沒有 pending：
- METEOR pending 且 p == 1（10–15 點）且 r < 30 → scene 99 `PSID_LABOGAME_05_EVENT`（流星小遊戲）。
- SHOOTINGSTAR pending 且 p == 3（19–21 點）且醒著且 r < 10 → scene 0x57。
- UFO pending 且醒著 → 0x4f（100%）。
- SANTA pending 且 p ≥ 3 且 r < 10 → 0x4f。
- XMAS pending 且醒著 → 0x4f。
另外 EVE pending 且**睡著**且時 > 18 → 0x4f（不受前提中的 request 限制）。

#### 事件效果
| 事件 | 效果 | 證據 |
|---|---|---|
| NEWYEAR | +1000 點 | `FUN_1003f1f0` 0xbba |
| BIRTHDAY | 動畫（sprite 0x6b2；有放牧 tama 時一起出現） | 0xfa2 |
| SHOOTINGSTAR | 動畫 0x6ac | 2000 |
| UFO / SANTA / EVE | 動畫 0x6ae / 0x6af / 0x6b0 | `FUN_1003efc4` |
| XMAS | +20 心情，動畫 0x6b1 | 同上 |
| METEOR（小遊戲） | 成功 +10 心情；失敗 −10 心情、EM_REQ_INJURY（`d08` < 4 時）、3 個 field 各 2 種擺放物被清掉且數量 −1（`FUN_10016900` → `FUN_1001e178(id,−1)`；解讀成「隕石砸壞擺設」為 **[推測]**） | `FUN_10058b98` |
| DODODO | rank ≥ 6、stage ≥ 3、非午睡，在**每月 13 日 17 點**觸發（午睡結束時也會檢查）；看完 +10 心情。17:15 後仍未處理 → DODODO_IGNORE：−10 心情、INJURY、目前 field 的擺放物被清掉並 −1（`FUN_10016900`） | `FUN_10012c90`、`FUN_100134bc`、`FUN_1003f1f0` 0x138d/0x1771 |

#### 其他 request 來源（`FUN_100134bc`，每 60 秒一次）
UNCHI(0x10)、ONAKA(0x11)、GOKIGEN(0x12)、EVOL_SUPPORT(0x13)、METAMORPHO(0x14)、SOWASOWA(0xe)、DYING(0xc)、DEATH(8/9)、COLD(10) 都在這裡發出，屬照顧／進化子系統。內容相關部分：
- METAMORPHO：stage 3 時，在**非出生 field**累計停留 > 3600 秒（每 field 計時 `DAT_18000d46[field]`）→ 發 METAMORPHO，並把進化紀錄全設為這個 field **[確認]**。
- stage 2 時，每 field 停留滿 900 秒 → 把 field 推入 `DAT_18000d1a[]` 紀錄（最多 11 筆）**[確認]**。

### 2.10 社交

#### 朋友（`friends_*`）**[確認]**
- 12 格：有效 mask `DAT_180015d4`，最近順序 `DAT_180015d6[12]`，紀錄 `DAT_180015e4 + slot*12` = `{u32 planet_id, u32 tama_uid, u16 friendship 1..4, u16 last_result 0..5}`。
- flash：每位朋友 2 個 0x5000 block，從 0x60e86000 開始（星球截圖＋ghost）。
- 新增／更新 `FUN_10017588`（滿了回傳 0）、刪除 `FUN_100172d4`、查 friendship `FUN_100173c4`。

#### Playdate（scene 0x78 → dispatcher `FUN_1008ada4`）**[確認]**
- 交換 0x14 bytes：romtype、planet id、uid、friendship、onaka、gokigen、LOVE 狀態、result、可結婚 flag。
- 本局 friendship：雙方記錄任一為 0 → 0，否則取較大；基礎值（`FUN_1006cbf4`）：任一方 SpeciesRank==5 → 4；任一為 0 → 1；否則 4 − |rank 差|；實際 = max(記錄, 基礎)。
- HOW PLAY（由 host 決定，依序判斷）：
  - 5：我方可吃人（flags bit0）、對方可被吃（bit1）、我方 onaka ≤ 5 → 我吃掉對方。
  - 6：反過來，對方 onaka < 6 → 我被吃。
  - 4：雙方都（onaka < 3 或 gokigen < 6）→ 打架。
  - 0..3：普通玩耍，值 = friendship − 1。
  - 動畫表 `0x10093912`。
- RESULT：0 玩耍、1 打架（`d08` < 4 時 EM_REQ_INJURY）、2 吃掉對方、3 被吃（便便量 +6，`FUN_1006706c`）、4 結婚、5 arcade。
- 友好度成長（`FUN_1006cb04`）：首次見面或 result ≠ 0 → +0；否則 +1，一方 LOVE → +2，雙方 LOVE → +3；上限 4。arcade 用另一條規則 `FUN_1006cb9c`（HOCKEY +1、SHOOTING 結果 ≥ 1 +2／否則 +1、首次見面 +0，見 09-arcade §1.1）。
- 每次 playdate／arcade：mission 11 +1。playdate 本身**不給點數、不加心情**；只有 arcade 會給點數。
- 交換物品（`FUN_100899c0`）：送出 −1、收到 +1，mission 12 +1。

#### 結婚／世代交替 **[確認]**
- **連線結婚**：雙方 stage 5、儲存的 friendship == 4、HOW 0..3、雙方都沒有「不可結婚」flag（bit2）；沒有性別檢查；guest 回答兩次確認（msg 0x6f、0xea）。結果：親代複製進放牧（依 attribute 對應的 field）、rank +1（3 或 5..9 時）、`new_generation_egg(60200)` → 蛋 chara 1000、世代 `DAT_18000d06` +1（≤ 99）、外型 id = 對方 ghost、顏色 50% 自己／50% 對方。
- **離線媒合**（HELP_BREED `FUN_1004e1ec`，對象是內建 NPC，不是放牧 tama）：每日 3 次（`DAT_18000c9e`，每日在 `FUN_10013234` 歸零），超過 → msg 0xde。候選 = 每 field 16 個 NPC（id 基準：field 0 0x1000、field 1 0x1016、field 2 0xfea），優先未見過（`DAT_18000ca0` mask）且 SpeciesRank 差 ≤ 1，提供 4 位。
  - 成功率 = 基礎（rank 差 0/1/2/≥3 → 80/50/40/20；自己 rank 5 → 80）＋ onaka（≤2 −5、≥5 +5）＋ gokigen（≤5 −5、≥16 +5）＋ LOVE +100；roll rand(0,99) < 成功率。
  - 雙方 flags 同時含可吃人與可被吃，或任一方不可結婚 → 必失敗。
  - 成功：親代進放牧、`DAT_18000c74[attr]` +1、rank +1、mission 15 +1、`new_generation_egg(候選)`（外型／顏色各 50% 繼承 **[推測：參數在反組譯中遺失]**）。
- **送去放牧**（HELP_GRAZING `FUN_1004dcd0`）：兩次確認（0xda、0xea），親代進放牧（stage 5 時 `DAT_18000c74[field]` +1，≤ 99），rank +1，`new_generation_egg(0)` → 世代**重置為 1**、預設顏色。
- **new_generation_egg**（`FUN_1000f708`）：chara = 1000、天數 0、stage bits = 1；保留 BIRTHDAY/NEWYEAR/日曆事件 request；照顧資料 0x70 bytes 清零；把目前顏色記入 mission 22。

#### 放牧 tama **[確認]**
- 3 field × 8 格 ring；per-field 狀態 `DAT_18000ddc + field*4`（byte0 低 nibble 下一格、高 nibble 上一格；byte1 佔用 mask；byte2 顯示 mask）。
- ghost 存在 flash 槽 1–24（每格 0x4000）；虛擬 chara id = 60100 + field*8 + idx（`FUN_1001892c`）。
- 新增 `FUN_10018cc8`；每個 field 最多顯示 4 隻（`FUN_10018978`，不含最後加入的那隻）。
- 放牧 tama 的便便會計入清便便數（`FUN_100108d4`：3 個 field 的數量總和，上限 12；清理間隔 3600–7199 秒 → +2×數量，7200–14399 秒 → +1×數量）。

#### Ghost 槽 **[確認]**
| 槽 | 內容 |
|---|---|
| 0 | 目前 tama（0x4000 @ 0x60dce000） |
| 1–24 | 放牧 tama |
| 25–29 | 死亡 ghost（ring 5 格；`DAT_18000df0[5]`：bit0 有效、bits1–3 死因、bits4–11 年齡、bit12 年齡 > 29） |
| 30 | 對方完整 ghost（0x1b000 @ 0x60e46000） |
| 31 | 我方送出用／暫存 |

#### DLC（additional）**[確認]**
40 格 × 0x4000 @ 0x60d2e000；mask `DAT_18001694`；註冊 `FUN_10000b88`；每格最多 3 個 sub-bank（資料表、sprite、文字）；物品查詢會掃描所有 bank（`FUN_100030d8`／`FUN_10003104`）。

#### 旅行（TAMASPACE_TRAVEL → `FUN_10057904` / `FUN_1006666c`）**[確認]**
- 需 TamaData+0x1B 計量 ≥ 60，否則 msg 0x82；每次旅行扣 60（`FUN_10062748(-60)`）。
- 7 個目的地，各給 1 個紀念品：`item_give(DAT_180006e6[dest*2 + toggle], 1)`，toggle 為 `DAT_18001680` bit[dest]，每次去同一地點在 2 種物品間交替。
- 設 mission 8 bit[dest]（`DAT_18000cb6`），心情 +1。
- 該計量在 `FUN_1006706c` 的 log 叫「UNCHI METER」，playdate 被吃時 +6；為何同時當旅行燃料 **[推測]**。

#### Lab QR **[推測]**
`FUN_100452f0` 組出 `HTTPS://TMGCP.TYB.JP/` + 0x23 字元資料 + 64 字元 hash；payload 欄位尚未追。沒有找到「lab tama」的接收路徑。

### 2.11 小遊戲

共同規則 **[確認]**：小遊戲以 **20 fps** 計時（秒 × 20）；`btn_pressed_mask` 0x10005ea8、`btn_held_mask` 0x10006150（1/2/4/8 實體鍵，4 = 離開，0x40/0x80 = dial 一格）、`dial_delta` 0x10006140、`dial_angle` 0x10006104。除了 Arcade 02，**所有遊戲內容都是固定的**：依遊玩計數器 `DAT_18001674`／`DAT_18001678` 輪流選固定版面（每玩一次 +1），不使用亂數。

#### Lab 遊戲（Lab 選單 → Game；scene 0x5d–0x64）
- 選單 index 0..6 → scene 0x5d, 0x5e, 0x5f, 0x60, 0x64, 0x62, 0x61。解鎖：planet rank ≥ `0x10093976` = {1,4,4,1,4,1,1}；難度 0/1/2 需 rank ≥ {1,6,9}（`0x10093968`），選到的難度存在 `DAT_18001c69` **[確認]**（`FUN_10047508`、`FUN_1002ff88`、`FUN_1002f874`）。
- 每個遊戲把結果 0..3 寫到 `DAT_18001c68`，再進共用結果畫面 `FUN_100228c4`（scene 0x5c）：`points_add(0x10091bec[level*4 + rank])`（表見 §2.2）；rank 3 → mission 13 +1。不改心情、不給物品 **[確認]**。

| 遊戲 | 類型 | 規則 | 結果門檻（3/2/1） | 證據 |
|---|---|---|---|---|
| 00 | 打地鼠（6 洞） | 固定 500 frames（25 秒）；每 20 frames 從版面表讀一列 6 個 short，非 0 就冒出；冒出 20 → 停留 60 → 下降；打中顯示 50。鍵 1 移游標、鍵 2 打。打中 state 1/2 +1；**打空洞立刻結束**；10 分也結束 | ≥10 / ≥8 / ≥1 | `100202cc`、`100500d0`、`1007847c`、門檻 `0x100a13c8` |
| 01 | 往上跳（Doodle Jump 類） | 自動跳 14 px/f（彈簧 25），重力 1.4 px/f²；按住 1/2 左右 5.6 px/f（x 16..112），dial 也可；高度 6400、10 段；9 張地圖 `0x10091fec`（level*3 + 計數%3）；平台：一般／彈簧／碎裂／移動（3 px/f）；時限 {100,90,80} 秒（`0x1009572c`）；掉下去回到最高落腳平台（只損失時間） | 分數 = min(高度,6400)×3000/6400；≥3000 / ≥1800 / ≥300 | `100205dc`、`10020a30`、`100051dc`、`10021178` |
| 02 | 翻牌配對＋炸彈 | 5×3 共 15 張 = 7 對 + 1 炸彈；15 種固定版面 `0x1009436c`（`(計數>>4&7)%5 + level*5`）；開始時全部翻開 120 frames；時限 {50,40,30} 秒（`0x10093fd4`）；翻到炸彈 → 結果 0 | 7 對 / 6 / 1–5 | `10050954`、`1007073c`、`1007ad44` |
| 03 | 三線道收集 | 速度 3 → 5 → 7（距離 1152、2816 時），終點 4992（約 51 秒）；鍵 1/2 或 dial 換線道；6 種固定版面 `0x10093984`（level*2 + toggle），收集品 39/30/30；某物品吸附約 61 frames **[推測]** | ≥30 / ≥16 / ≥1 | `100514dc`、`1007adbc` |
| 04 | 打磚塊找獎品磚 | 球速 3.0 px/f，發射角表 `0x1009bb50`/`0x1009bb5c`；15 塊磚，獎品磚 index `0x10091d30[level*50 + q*5 + round]`；5 回合，每回合限時 {1200,1000,800} frames（`0x1009c5a4`）；漏球或超時結束。疑似 bug：板子速度上限用 −4/+5 原始值比較 fixed-point，按住會一直加速 **[推測]** | 過 5 回合 / 3–4 / 1–2 | `10052024`、`100788d0`、`1007ae24` |
| 05 | 三線道擋隕石 | 鍵 1/2/4 在 0/1/2 線舉盾 20 frames；每 20 frames 一列，共 40 列；落下時間 3.0 → 2.5 → 2.0 秒；生命 {3,2,1}；frame 880 或生命歸零結束；最高 28/30/34 分 | ≥25 / 20–24 / 6–19 | `10058b98`、`1005fbb0`、`1005f750`、`1006f3f8` |
| 05_EVENT | 流星事件（scene 99，§2.9） | 同 game 05 的 code，但生命 1、固定版面 `0x18000316`、不能離開、沒有教學；不進結果畫面、不給點數；存活 +10 心情，失敗 −10 心情＋受傷＋擺放物 −1 | — | 同上 |
| 06 | 橫向捲軸收金幣 | 按住鍵 1 上升（±0.24 px/f²）；捲動 4.0 px/f + 俯衝加成；終點 x = 6500，2000 frames 計時；金幣 50/45/41、障礙 5/10/20；撞到障礙只被擊退（損失時間），之後 20 frames 無敵 | ≥40 / ≥25 / ≥1 | `10022220`、`100179d0`、`1007ae84` |

- 已知 bug（game 00）**[確認]**：計數器 `&3` 輪 0..3，但每個難度只有 3 種版面，第 4 次會讀到下一難度的第一張；難度 2 的第 4 次讀到 NULL（從位址 0 讀版面）。難度 2 的版面也短於 25 列，會讀過頭。重製時要決定是否保留。
- `game05gaugestop`（`DAT_18001c34`）**[確認]**：只有 debug 指令會寫，韌體中沒有任何讀取；出貨版 game 05 沒有 gauge，這個指令是殘留的。

#### 一起玩（TOGETHER，scene 0x24 → `FUN_10040ff4`，選單表 `0x10094144`）
- 4 個項目 → scene 0x69, 0x66, 0x6a, 0x6b，永遠開放。0x65/0x67/0x68 在這版韌體中沒有入口 **[確認]**。
- **不給點數**；只呼叫 `care_add_gokigen` 0/+1/+5/+10；完美 → mission 4 +1（上限 100）；鍵 4 離開沒有獎勵 **[確認]**。

| 遊戲 | 類型 | 規則 | 完美（+10、M4+1） | +5 | +1 |
|---|---|---|---|---|---|
| 00 `10052e7c` | 猜左右 | 5 回合，答案固定表 `0x100a113a`；200 frames 未選自動選 | 5 對 | 3–4 | 1–2 |
| 01 `100539dc` | 舉旗反應 | 最多 10 個指令 `0x100a10c2`，輸入窗 20 frames；搶按或按錯即結束 | 10 | 8–9 | 1–7 |
| 02 `10054364` | 三杯猜球 | 3 回合，交換次數 {10,16,20}/{12,14,19}/{11,15,18}，每次 1.0/0.6/0.3 秒 | 3 對 | 2 | 1 |
| 03 `10055f30` | dial 走迷宮 | 3 張 12×14 地圖 `0x1009c5b0`（`w` 牆、`S` 起點、`G` 終點、`H`/`V` 移動障礙，48 px 格）；最快 2 px/f；60 秒；撞到停 2 秒 | 0 次撞 | 1 次 | ≥2 次（超時 0） |
| 04 `10059600` | 猜箱子 | 3 回合，內容固定（`0x10091ece`、`0x10091f94`）；200 frames 自動確定 | 3 | 2 | 1 |
| 06 `100802f4` | 時機停止 | 3 回合，掃過時間 0.65/0.5/0.4 秒；評分 `FUN_1007aee8`（區域門檻在版面資料） | 三次都在最高區 | 有最高區且沒有 0 區 | 0 區少於 2 次 |
| 07 `1005a9b4` | 傾斜場地滾球 | 3 張 18×19 場地（同一字串 offset 0x1f8 + map×0x156）；60 秒；收 5 個 T、避開 G | 5 T 且 0 撞 | ≥4 T | 2–3 T（時間到且 <2 → 0） |

#### Tama Space Arcade（兩台連線，scene 0x83–0x86；詳見 09-arcade.md）
- 入口 `FUN_1008cffc` → 0x83；成長階段 < 3 不可玩；免費 **[確認]**。
- 結果 `FUN_100883e0`：`points_add(DAT_18001c90)`，mission 11 +1（輸贏都算），新增／更新朋友，friendship +1–2（夾在 1..4）**[確認]**。
- Arcade 01（對打 Pong，`100860f8`）：2400 ticks（1 tick = 1 frame，每 frame 同步交換一次，見 09-arcade **[確認]**），板子每 tick 6 px，可蓄力；結果碼 0/1/2 → 500/100/250 點（`FUN_1008709c`）；0/1/2 = 勝／負／平 **[推測]**。
- Arcade 02（合作射擊，`1008764c`）：1200 ticks；兩人共用準心（一人控 X、一人控 Y）；目標出現在隨機空洞 `rng_range(0, n−1)`（每邊 9 洞）——唯一有亂數的小遊戲；每次命中 1–3 分；總分 ≤10 → 100，11–50 → 250，≥51 → 500。

---

## 3. 資料表

| 表 | 位址 | 格式 | 筆數 | 抽出 |
|---|---|---|---|---|
| 場景表 | 0x10094830 | 7 × u32 `{id, fnA, fnUpdate, fnC, fnD, flags, name*}` | 137 | 上文 §2.1 |
| 選單表 | 0x10094120 | `{ptr,count}` ×4；項目 12 bytes（u16 text, u16 0, u16 scene,…） | 5/4/3/1 | §2.1 |
| 物品表 | flash 0x60125972 | 46 bytes（23 × u16） | 343 | `data/items.csv` |
| 背包 cap | 0x10093d2c | u8[12] | | `[0,10,10,10,10,5,5,5,1,2,8,9]` |
| 商店分頁 | 0x10093d34 | u8[4] | 4 | `[1,2,8,9]` |
| 區域價格 | 0x10093d14 | u32[3] | 3 | 10000 ×3 |
| Mission 目標 | 0x100942b4 | u32[22] | 22 | 500,500,100,100,100,1000,10,7,50,50,100,50,100,99999,50,50,18,12,16,16,16,16 |
| Mission 獎勵 | 0x1009430c | u16[22] | 22 | 全 1000 |
| Lab 遊戲點數 | 0x10091bec | u16[3][4] | 12 | 10/100/150/250, 10/200/300/500, 10/300/450/800 |
| 天氣月份 | 0x10091b5c | u16[12][6] | 12 | §2.5 |
| 天氣 pattern | 0x1009138c | u8[6][5] | 6 | §2.5 |
| 料理 | 0x10091fbc | `{u16 dish, u16 ingredient}` | 12 | §2.3 |
| 探險收穫 | 0x100a0600 | `{u16 sprite, u16 item, u16 m17bit}` | 24 + GOLDEN PEACH | §2.8 |
| Shop code | 0x100a1a20 / PRAM 0xe25c | `{u16 item, u16 pad, char* code8}` | 109 + 6 | `data/shopcodes.csv` |
| 進化 rank 文字 | 0x100a138e | u16[11] | | 132..136,135… |

`items.csv` 欄位：`flash_off,id,f1_idx,name_txt,name_txt2,spr_a,spr_b,f6..f10,type,buy,sell,food_group,f15_ver,req_rank,like_tag,f18,onaka,gokigen,day_group,flags,gm_name,gm_type`（最後兩欄取自 GMMan/tama-paradise-items-list，只是方便閱讀的名稱）。

---

## 4. 新命名

```
10010e30 points_spend f
10022ff0 item_find f
1001e0d8 item_give f
1001e7e0 item_list_by_category f
1007d730 item_filter f
1007e81c item_type_cap f
1007e760 inv_find_slot f
1007e71c inv_free_slot f
1007e834 inv_get_count f
1007e884 inv_set_count f
1001e97c item_is_unlocked f
1006da10 shopcode_lookup f
1001021c eat_item_effect f
100171fc item_like_bonus f
1001dd2c food_group_matches_attr f
10068aa0 req_done f
10069218 req_raise f
10068a50 req_clear f
10010c68 req_pending f
1000fc98 evolve_apply f
10010cf4 set_fieldtype f
10016520 area_unlock f
10018358 area_is_unlocked f
10010c2c weather_now f
10010e10 weather_update f
1008fdec weather_pick_pattern f
1008e704 time_period f
10010c18 time_period_now f
10012c90 time_advance f
100134bc tick_60s f
10013234 daily_rollover f
10083984 calendar_events f
1006d904 is_birthday f
1002a55c rng_next15 f
1002a584 rng_range f
1007a858 rng_range_except f
10025368 mission_check_all f
10025788 mission_is_qualified f
10025868 mission_set_qualified f
10025748 mission_is_unclaimed f
10025808 mission_set_claimed f
10048bd0 scene_info_mission f
100108d4 poop_clean_count f
10010c80 tama_is_asleep f
1001abd0 scene_home_init f
10039eb8 scene_home_field f
1003aff0 scene_home_space f
100398c8 scene_home_cell f
1003b8f4 scene_home_field2 f
1001abb4 menu_table_get f
1004a438 scene_shop_buy f
1004af0c scene_shop_sell f
1004b624 scene_shop_cooking f
1004bcf4 scene_shop_code f
1004c1f8 scene_shop_areaunlock f
1004d304 scene_help_sitter f
1004dcd0 scene_help_grazing f
1004e1ec scene_help_breed f
10041df8 scene_osewa_hunt f
1004140c scene_osewa_fieldswitch f
1003ff80 scene_osewa_gohan f
10040df4 scene_osewa_oteire f
10040ff4 scene_osewa_together f
10043a44 scene_osewa_setsubi f
10044f78 scene_osewa_poopcleaning f
1003f1f0 scene_event_calendar f
1003efc4 scene_event_ufo f
10058b98 labo05_tick f
100228c4 scene_labogame_result f
10022b90 labogame_result_update f
10047508 labomenu_game_tick f
1002ff88 labomenu_game_select f
1002f874 labomenu_level_select f
100202cc labo00_init f
100500d0 labo00_tick f
1007847c labo00_play f
100675ac labo00_holes_update f
100709d0 labo00_setup f
100205dc labo01_tick f
10020a30 labo01_play f
10021178 labo01_rank f
10050954 labo02_tick f
1007ad44 labo02_rank f
100514dc labo03_tick f
1007adbc labo03_rank f
10052024 labo04_tick f
100788d0 labo04_play_frame f
1007ae24 labo04_rank f
1006f3f8 labo05_rank f
10022220 labo06_dispatch f
1007ae84 labo06_rank f
10052e7c osewagame00_tick f
100539dc osewagame01_tick f
10054364 osewagame02_tick f
10055f30 osewagame03_tick f
10059600 osewagame04_tick f
100802f4 osewagame06_tick f
1007aee8 osewagame06_grade f
1005a9b4 osewagame07_tick f
100883e0 arcade_result f
100860f8 arcade_pong_play f
1008764c arcade_shoot_play f
10005ea8 btn_pressed_mask f
10006150 btn_held_mask f
10006140 dial_delta f
10006104 dial_angle f
100028cc scene_set_next f
10002780 scene_get_prev f
1000274c scene_get_current f
100570bc scene_tamaspace_rank f
10057904 scene_tamaspace_travel f
10057e54 scene_tamaspace_lookstars f
100582cc scene_tamaspace_deco f
1000f708 new_generation_egg f
10013158 shopdemo_setup f
10073d8c cmd_planetrankup f
100760f0 cmd_weather f
100727a0 cmd_fieldtype f
10071f20 cmd_areaunlock f
1007282c cmd_game05gaugestop f
10071d84 cmd_allqualifymission f
10073ec4 cmd_qualifymission f
1007312c cmd_missionqualifynum f
100751f4 cmd_setmissionnum f
10072ce4 cmd_item f
18000ca4 g_points l
18000cc4 g_points_total l
18000c6c g_planet_rank l
18000c6d g_area_unlock l
18000dd7 g_fieldtype l
18000dd8 g_weather_pattern l
18001bf9 g_weather l
18001bec g_rtc_now l
18000c00 g_rtc_last l
18000c6a g_birthday l
18001c6a g_home_zoom l
18000cdc g_mission_qualified l
18000ce0 g_mission_claimed l
18000ca8 g_mission_counters l
18000e54 g_inv_ids l
18001354 g_inv_counts l
18000e04 g_item_unlock_bits l
18000be8 g_sys_flags l
18000d08 g_tama_state l
18000d06 g_generation l
18000c98 g_play_days l
10094830 g_scene_table l
100942b4 g_mission_targets l
1009430c g_mission_rewards l
10093d2c g_item_type_cap l
10091b5c g_weather_month_prob l
1009138c g_weather_pattern_tbl l
10091fbc g_cooking_recipes l
100a0600 g_hunt_rewards l
100a1a20 g_shopcodes l
```
（社交相關命名另見：friends_register 10017588、friends_delete 100172d4、friends_get_friendship 100173c4、playdate_dispatch 1008ada4、playdate_handshake 1008b53c、playdate_breed 1008aeec、playdate_result 1008c430、arcade_result 100883e0、item_trade 100899c0、grazing_add 10018cc8、grazing_id 1001892c、grazing_count 100188d8、grazing_pick_visible 10018978、dead_ghost_add 1000affc、ghost_slot_addr 100186a8、additional_register 10000b88、Asset_InitAdditional 10003238、attr_to_fieldtype 100039d4、req_injury_if_ok 10010de4。）

---

## 5. 未解問題

1. 天氣值 0/1 的實際外觀（看 `FUN_1000aeac` 背景表對應的 sprite）。
2. Zoom 的 dial／按鍵映射與轉場動畫：`FUN_10039eb8` 的 state 1000 輸入處理（`FUN_10005ea8` 按鍵、`FUN_1008e604` dial）。
3. 玩具（SETSUBI 擺放）有沒有效果：`FUN_10043a44` 沒有直接加值；TOGETHER 的效果只來自小遊戲（§2.11）。
5. ~~每日 +50 點屬於哪個畫面~~ 已解：MICROSCOPE 的 NO DATA lab code（§2.4a）。
7. 可拜訪星球（type 8 物品，9901–9910）與 TRAVEL 目的地的關係；TamaData+0x1B 計量（旅行 −60、被吃 +6、標為 UNCHI METER）的增加來源。
8. 離線媒合 `new_generation_egg` 的實際參數（反組譯遺失）。
9. Lab QR payload 格式。
10. 物品欄位 1、4–10、15 的用途。
11. OSEWAGAME 06 的區域門檻（在版面資料裡）、Arcade guest 端的結果碼、game 00 版面讀過頭的實際行為。
