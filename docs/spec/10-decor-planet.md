# 10 — 戶外玩具、星球裝扮、星球層（OUTSIDE TOY / DECO PLANET / HOME_SPACE / TRAVEL / TAMA STARS / PLANET LV.）

來源：`re/decomp-all/XIP.c`、`dumps/xip.bin`、全 flash dump、`dumps/pram.bin`（RW 初始資料）。位址皆為 XIP vaddr（0x10xxxxxx）或 SRAM（0x18xxxxxx）。
時間單位：frame（20 fps）。輸入記號沿用 08-care-ui：A=bit0、B=bit1、C=bit2、P=轉盤按壓 bit3、DN=轉盤下 bit6、UP=轉盤上 bit7；「確定」= `key_pressed(0xa)`（B 或 P）。
表格已抽到 `docs/spec/data/decor.json`（`tools/extract_decor.py DUMP [PRAM]`）。sprite 以 `tools/extract_sprites.py` 輸出 PNG 目視確認。

> **重要更正**：帽子／眼鏡／飾品（type 5/6/7）**不是戴在寵物身上，而是戴在「星球」身上**。星球本身是一個有臉的角色模型（ChrID 2、11–14、23–28），模型部件型別 4/6/8 = 帽子／眼鏡／飾品。裝備介面就是星球層選單的 **DECO PLANET（0x80）**。寵物模型沒有 4/6/8 部件。

---

## 1. 摘要

- **戶外玩具（SETSUBI 0x27）**：每個 field 有 2 個區域（area 0 = 主場地、area 1 = 花 10000 解鎖的擴充區 FIELD2），每區 6 個擺設位：0–3 小型（type 3），4–5 大型（type 4）；大型 4 佔用小型 0+1、大型 5 佔用 2+3。流程 PLACE/REMOVE 二選一 → 玩具清單 → 在場地預覽畫面用轉盤移游標選位置 → 「PLACE HERE?」→ 擋路的舊玩具飛走、新玩具從上方落下。
- 玩具本身不改變任何數值；它在 HOME 場地層播「展示動畫」（item+8），若有放牧中的 tama（或寵物本身，行為碼 1）被分配到該玩具則改播「遊玩動畫」（item+0xa，小型 1 隻、大型 2 隻角色）。空的小型位也可能有放牧 tama 在玩（idle 動畫 0x11e–0x120）。
- 「使用中」數量另存在 nibble 陣列 `0x18001494[640]`（save+0x8ac），擺出的玩具與星球正在戴的裝扮都會 +1；賣東西上限 = 持有 − 使用中。
- **星球層 HOME_SPACE（0x1d）**：星空背景（調色盤循環）+ 有臉的星球角色（依 romtype 與 planet rank 換外型，戴著 3 件裝扮）+ 最多 5 顆「死去 tama 的星星」。閒置 200 frames 顯示日期時鐘。A 開圈選單：CONNECTION、TAMA STARS、DECO PLANET、TRAVEL、PLANET LV.。
- **DECO PLANET**：先選類別（帽子／眼鏡／飾品），再在橫向轉盤選物品（即時在星球上預覽），確認後播閃光動畫 → 寫入 `0x18000d88/8a/8c`。戴上特定成套組合（7 組旅行紀念品）時，回 HOME 場地有 1/3 機率出現對應的訪客角色。
- **TRAVEL**：需 UNCHI METER（`0x18000d13`）= 60（滿）、rank ≥ 3、成長階段 ≥ 3。目的地依 rank 開放 2→7 個；旅行扣 60、心情 +1、得到該地 2 件紀念品之一（交替）、mission 8 記錄。
- **TAMA STARS**：列出死亡 ghost（最多 5）星星，選一顆看其 tama 與「名字／年齡／死因」。
- **PLANET LV.**：顯示目前 rank 與下一個目標；rank 上升時從 HOME 自動播放升級動畫（舊→新星球外型）；rank 10 進片尾。

---

## 2. 詳細規則

### 2.1 戶外玩具資料結構

| # | 規則 | 證據 | 標記 |
|---|---|---|---|
| T1 | 擺設陣列 `0x18000d8e`（save+0x1a6）：`u16 item_id[3 field][2 area][6 slot]`，位址 = `0x18000d8e + field*0x18 + area*0xc + slot*2`（**field 為外層**，03-save 寫的「+0x1a6/+0x1b2 兩層」其實是 field 0 的 area 0/1；整塊 0x48 bytes 到 0x18000dd5）。0 = 空 | `toy_slot_get` 0x10016638、`toy_slot_set` 0x10016840、`toy_slot_clear` 0x100168a4 | [確認] |
| T2 | 位 0–3 小型、4–5 大型；佔用關係：大型 `4+k` 覆蓋小型 `2k, 2k+1` | `toy_place_conflicts` 0x1006d804 | [確認] |
| T3 | 放置衝突：放小型到 p → 若大型 `4+p/2` 有東西，移除它；否則若小型 p 有東西，移除它。放大型到 4+k → 若大型本身有東西移除它；否則移除小型 2k、2k+1 中有東西的（0–2 件）。被移除的玩具回倉庫（使用中 −1） | 0x1006d804、`setsubi_remove_conflicts` 0x10083d54 | [確認] |
| T4 | 「使用中」計數：`0x18001494` u4[640]（與數量 nibble `0x18001354` 平行，同一倉庫 index），`item_inuse_add(id, ±1)` 0x1001e178：新值 > 持有數 → 失敗不變；< 0 → 0。擺玩具 +1、收回 −1；星球戴上 +1、換下 −1 | 0x1001e178、0x1007e85c/0x1007e8d0 | [確認] |
| T5 | 可擺數 = 持有 − 使用中（清單第 4 byte 低 nibble）；為 0 時該清單項目設 bit4（灰）[推測：清單元件以此顯示灰色/不可選] | `scene_osewa_setsubi` 0x10043a44 state 2000 | [確認]/灰色效果 [推測] |
| T6 | 賣出上限 = 持有 − 使用中（`scene_shop_sell` 0x1004af0c 用 `item_inuse_count` 0x1001e628），故正在擺／正在戴的不能賣 | 05 §2.4 | [確認] |
| T7 | 可擺的 field 清單（`setsubi_field_list_build` 0x100167d8，依 `field_is_open` 0x1006258c）：rank < 4 → 只有本機 romtype 的主場（romtype 5 → field 0、6 → 1、4 → 2）；rank 4–5 → field 0、1（romtype 4 時兩者皆否 —— 程式就是這樣寫）；rank ≥ 6 → 三個都可。起始游標 = 目前 field | 0x1006258c | [確認]（romtype 4 的行為看起來像 bug，[推測]） |
| T8 | area 1 只在該 field 的擴充區已解鎖（`DAT_18000c6d` bit field）時可去 | `FUN_10018358` | [確認] |
| T9 | 新遊戲送 NICE BOX(7998)，id 7999 BOX（小型，不可買）；GMMan 名稱見 items.csv | 08 §2.12 | [確認] |
| T10 | 存檔驗證：擺設位的 item 已不存在 → 清 0（`item_storage_verify` 0x1001e658） | 03-save | [確認] |

### 2.2 SETSUBI 畫面流程（`scene_osewa_setsubi` 0x10043a44，ctx `0x18014184`，0x5d0 B）

init `setsubi_init` 0x10027960：layer、`FUN_1007b4cc(0x20)`、建 field 清單（ctx+0x30 list、+0x33 數量、+0x34 目前）。返回目標 = HOME（0x1c）。

| state | 內容 | 輸入 | 標記 |
|---|---|---|---|
| 1000 | 2 選項選單（`FUN_1002d5c4`，項目在 RW 資料 `0x180009f0`：{text 59 PLACE}、{text 60 REMOVE}，12 B/項，+8 bit0 = 灰）。PLACE 灰 = 沒有任何 type 3/4 玩具（`item_list_collect(3,8)==0`）；REMOVE 灰 = 全部 field/area 都沒擺（`toy_count_total`==0）。游標記住上次選擇（ctx+0x173 bit0） | 確定 → PLACE 2000 / REMOVE 3000；C → `c6a|=4`（回 HOME 並重開場地圈選單）→ 0x1c | [確認] |
| 2000 | 玩具清單（`FUN_1002cc90` style 2）：所有持有的 type 3/4，依 id 排序；每項 `{u16 id, u8 可擺數|灰 bit4}`；游標停在上次選的 id | 確定 → 記 id、item ptr → 0x834；C → 1000 | [確認] |
| 0x834/0 | 候選位置清單：小型 → `[0,1,2,3]`（0x1009bb22）、大型 → `[4,5]`（0x1009ba90） | | [確認] |
| 0x834/100 | 畫場地（`setsubi_draw_area` 0x10070300）：`field_bg_create(field, 目前時段, 天氣, area)` 背景；已擺玩具以 **item+8 動畫**（展示動畫）放在位置表座標；每個候選位畫標記元件（小型 sprite 0x455 32×32、大型 0x456 64×64 的佔位框）；左右箭頭 0x2d6 (10,64)/0x2d7 (120,64)（anim 0xe/0xf，有多個 field 或 area 時才顯示 [推測：呼叫條件 >1]）；游標 0x2d8（anim 0x14）放在第一個候選 | | [確認] |
| 0x834/0x65 | A 或 UP → +1；DN → −1。游標補間到新位置（`FUN_10066b34` mode 1，長度 0x4000 ≈ 5 frames [推測換算]）。超出清單兩端：若該 field 有擴充區，在 area 0/1 間切換（往前從 area 0 進 area 1；往後從 area 1 回 area 0）；否則（或已到另一端）在 field 清單中 ±1 循環（field 清單 ≥2 時）。換 area/field → 淡出重畫（回 /100）。游標在補間中不接受輸入 | 確定 → 音效 1，記 field/位置 → /200；C → 音效 2 → 2000 | [確認] |
| 0x834/200 | 「PLACE HERE?」（text 0x98，`FUN_1002f1f8` YES/NO） | YES → 0x898；NO → /100 | [確認] |
| 0x898 | 重畫場地，鏡頭對準目標位置；算衝突（T3）。有衝突 → 舊玩具**往上飛走**（y → y−128，`FUN_10066b34` mode 1、長 0x10000），使用中 −1、清空該位，等 30 frames；然後新玩具**從 y−128 落下**到定位（mode 4 ease、長 0x20000），寫入擺設陣列、使用中 +1，等 50 frames → 2000 | | [確認]（長度單位換算 [推測]：0x10000 = 1 s = 20 frames） |
| 3000 | REMOVE：列出有擺東西的 field（從目前 field 開始），area 取第一個有東西的；位置走訪順序 `[0,4,1,2,5,3]`（0x1009bb26，只列有東西的位）；沒有任何東西 → 回 1000 | 同 0x65：A/UP/DN 移動，跨端切 area / field（跳過空的）；確定 → 「PUT AWAY?」（0x99）；C → 1000 | [確認] |
| 3000/300 | YES → 該玩具飛走（同上 30 frames）、使用中 −1、清位 → 回 3000/0 重算 | | [確認] |
| 鏡頭 | ctx+0x172 bit0：畫面偏移 = (64,64) − 目標座標；bit1：每 frame 追隨游標物件。位置表座標是**世界座標**（可為負或 >128），所以畫面會捲到目標處 | 0x10043a44 尾端 | [確認] |

### 2.3 位置表

SETSUBI 用 `0x1009ba92`（3 field × 2 area × 6 × `{u16 元件, s16 z}`）；HOME 場地用 `0x100933ae`（每 field 0x54 B：+0 3 筆背景元件、+0xc area 0 九筆、+0x30 area 1 九筆，`{u16 元件, s16 z}`；第 6 筆 = 寵物錨點、第 7 筆 = 蛋、第 8 筆 = 訪客 [推測]）。元件的 x,y = 世界座標（場地層倍率 1.0 時畫面中心 = 鏡頭焦點）。完整數值見 decor.json `toy_slots`。

| field/area | 位 0 | 位 1 | 位 2 | 位 3 | 大 4 | 大 5 |
|---|---|---|---|---|---|---|
| TROPICAL/0 | (−40,38) | (−32,84) | (152,54) | (144,100) | (−40,64) | (150,72) |
| TROPICAL/1 | (−4,63) | (25,101) | (179,74) | (140,94) | (−8,70) | (132,53) |
| FROZEN/0 | (−40,38) | (−32,84) | (152,54) | (144,100) | (−40,64) | (150,72) |
| FROZEN/1 | (3,69) | (−57,122) | (196,78) | (146,122) | (8,52) | (150,116) |
| MORI/0 | (−40,38) | (−32,84) | (132,55) | (142,100) | (−40,64) | (150,72) |
| MORI/1 | (0,77) | (8,127) | (101,78) | (124,112) | (−1,55) | (116,93) |

z（繪製順序）：位 0/2 = 100、1/3 = 300、4/5 = 200（MORI/1 略有不同，見 JSON）。寵物錨點 (64,80) z 400。[確認]

### 2.4 玩具在 HOME 的表現（`home_scene_build` 0x10084df8 → `home_toy_objects_build` 0x10016c64）

| # | 規則 | 證據 | 標記 |
|---|---|---|---|
| H1 | 每個有擺的位產生一筆 `{slot, play_anim = item+0xa, show_anim = item+8, guest0, guest1}`（若 item+0x14 bit0，play_anim 改用 item+0xa/+0xc/+0xe 依 field；本 dump 的玩具都沒設） | `home_toy_entries` 0x1001669c | [確認] |
| H2 | 空的小型位（且對應大型位也空）補「空位」項目：位 0/3 → anim 0x11e、1 → 0x11f、2 → 0x120，旗標 bit0 = 只在有訪客時顯示 | `home_toy_fillers` 0x10016388 | [確認] |
| H3 | 訪客 = 本 field 的放牧 tama（`FUN_10018978`，最多 4 隻）隨機洗牌，依序分配：小型玩具 1 隻、大型 2 隻（`home_toy_assign_visitors` 0x10016ebc）。分配結果存在 `0x18001c02 + field*0x10`，`0x18001c32` = 本次已抽（同一次開機/回 HOME 期間不重抽 [推測]） | 0x10016c64 | [確認] |
| H4 | 寵物行為碼 1（隨機待機 27 筆循環表裡的 1）且不在特殊狀態 → 寵物去玩玩具：隨機挑一個真玩具（容量：小 1、大 2 合計後 `rand % 容量`），把該位置的訪客換成 60000（= 目前寵物），原訪客移到空位。此時寵物不另外畫 | 0x10016c64 param 3、0x10084df8 | [確認]（無玩具時除以 0，[推測] 結果 index 0） |
| H5 | 顯示：最多 5 筆（`home_toy_compact` 0x10016b88）。有訪客 → 播 play_anim 並把動畫內 CHARA 佔位 role 0/1 換成訪客角色；無訪客 → 播 show_anim。動畫用 `petanim_play`，位置 = `0x100933ae` 表該位元件座標、z | `home_toy_anim_spawn` 0x10016a18 | [確認] |
| H6 | show_anim 範例：BOX 7999 anim 0x126 = 單張 sprite 0x4ba；TAMAVERSE OBJECT 7002 anim 0x2af = sprite 0x4bb 上下浮動 + 閃爍。play_anim 範例：0x127 = 玩具 + CHARA motion 38 跳動 80 frames；0x2b0 = 兩隻角色左右走動 180 frames。動畫結束（EVT −1）後由 HOME 重播 [推測] | `ui_tree.py` 解碼 | [確認] |
| H7 | 玩具**不呼叫任何照顧函式**（不加心情/飽足），只影響畫面與寵物待機動作 | SETSUBI 與 home 物件程式中無 care_* 呼叫 | [確認] |
| H8 | item 欄位（toys）：+2 名稱字串、+4 圖示 sprite（例 1210 = 紙箱 64×32）、+6 清單 sprite（多數同 +4）、+8 展示動畫、+0xa 遊玩動畫 | 0x1001669c、0x10083b94 | [確認] |

### 2.5 星球角色與裝扮（worn items）

| # | 規則 | 證據 | 標記 |
|---|---|---|---|
| W1 | 星球 instance 由 `planet_inst_build` 0x10007c68 填：`inst[0]` = ChrID = `planet_chr[class][rank]`；class = `romtype_field_class(romtype)` 0x1002b314（romtype 1/5 → 0、2/6 → 1、3/4 → 2；`0x18001cb0` = romtype，debug `romtype 4|5|6`）；rank > 10 夾 10 | 0x10007c68、`FUN_10074bd8` | [確認] |
| W2 | ChrID 表（`0x100a1020`，同 RANK 用的 0x10094786/0x100947b2/0x1009479c）：class 0 = 23,24,24,24,26,26,27,27,27,27,28；class 1 = 23,25,25,25,26,26,27,27,27,27,28；class 2 = 2,11,11,11,12,12,13,13,13,13,14（index = rank 0..10）。外型在 rank 1、4、6、10 變化 | xip 表 | [確認] |
| W3 | `inst[8]` = `0x18000d88`（帽子，部件型別 4）、`inst[9]` = `0x18000d8a`（眼鏡，型別 6）、`inst[10]` = `0x18000d8c`（飾品，型別 8）；值 0 → 0x7ffe（不畫） | 0x10007c68 | [確認] |
| W4 | 繪製（`chara_draw_parts` 0x100075c4）：型別 4/6/8 部件的 sprite = `item_find(inst[k])+4`，其餘（位置、幀、縮放、旋轉、翻轉、alpha）取該姿勢 keyframe。keyframe 裡寫的 sprite 只是佔位（1526/1488/1451 等） | 0x100075c4 | [確認] |
| W5 | 星球模型姿勢（每個 ChrID 都相同版面）：body 64×64（5 幀）於 (0,0)；帽子 (0,−20)、眼鏡 (0,−8)、飾品 (0,+10)，幀 0。姿勢 1 = 醒（body 幀 0）、2/3/4 = 表情（body 幀 3/4/2）、15 = 睡（body 幀 1）、19 = 幀 4 + 額外 16×16 sprite 231。座標 = sprite 中心相對角色原點，再加 sprite 標頭 origin（ox,oy，例 1450 = (−2,−5)） | `kf.py`（同 decor.json `planet_models`） | [確認] |
| W6 | 裝扮 sprite：+4 = 戴在星球上的 64×64 圖（例 8001 CREPE HAT → 1450），+6 = 32×32 清單圖示（1528） | sprite PNG 目視 | [確認] |
| W7 | type 對 slot：5 → 0、6 → 1、7 → 2（`deco_cat_types` 0x100a09b8 = {5,6,7}） | `deco_list_build` 0x1007ddf8 | [確認] |
| W8 | 成套組合（`equip_set_match` 0x1002820c，表 0x100947c8，7 × {hat, glasses, acc, guestA, guestB}，0 = 不限）：{8001,–,8002}、{8003,–,8004}、{8005,–,8006}、{8101,–,8102}、{8085,–,8086}、{8099,–,8100}、{8013,–,8014}（= 7 個旅行地的紀念品）。HOME 場地建構時若符合且本次未判定過（`0x18001bff`）：`rand%3==0` → 訪客 = 該組 guestA/guestB 隨機一個（9000–9021，`FUN_1002400c` 轉角色），用 anim 0x2f1 出現在場地上；結果存 `0x18001c00` 本次沿用 | `home_scene_build` 0x10084df8、`equip_set_guest` 0x100282f8 | [確認] |

### 2.6 HOME_SPACE（0x1d，`scene_home_space` 0x1003aff0，ctx `0x1801412c`）

| # | 規則 | 證據 | 標記 |
|---|---|---|---|
| S1 | 畫面（`space_view_build` 0x10088850）：元件 0x2c8 = anim 0xbf（sprite 0x450 128×128 星空，4 組調色盤每 10 frames 循環）、0x2c9 = anim 0xc0（0x451 前景星點，同樣循環）；死亡星星群組；星球元件 0x2ca（醒，anim 0xc2：CHARA 佔位 2 = 星球、姿勢 1）或 0x2cb（`tama_is_resting` 時，anim 0xc3 姿勢 15），位置 (90,92)，每 20 frames 上下浮動 1 px | 0x10088850、anim 解碼 | [確認] |
| S2 | 死亡星星（`space_stars_collect` 0x1006dcb0）：掃 ghost 槽 25–29（`0x18000df0[5]`），有效者依序放在元件 0x2d0–0x2d4：(30,28)、(58,56)、(92,26)、(22,68)、(44,104)，sprite 0x453（32×32，3 幀）；幀 = 1 若年齡 > 29 否則 0 | 0x1006dcb0、0x10088850 | [確認] |
| S3 | 縮放：同 HOME 的 zoom 物件（ctx+0x58），範圍 −16..64、4 [推測：停駐點參數]。從場地拉遠進來時播 zoom 動畫 + 白色淡入（`space_zoom_out_enter` 0x10088aac）。轉盤拉近到倍率 ≥ 2.0 → state 2000：反向 zoom + 白色淡出 + 音效 0x28 → `PSID_HOME_FIELD`。視差：星空背景、星點、星星群組分別以 0.1/0.3/0.4 的比例跟著縮放（`FUN_1000ecf0(k)`） | 0x1003aff0 state 1000/2000 尾端 | [確認]/視差公式 [推測] |
| S4 | 只有倍率正好 1.0 且不是蛋期才接受按鍵：A → 圈選單（表 0）；長按 P → LABOMENU(0x2a)。狀態 2/3/4（`FUN_10010aa4`，託管/睡覺等）時 A = 錯誤音 0xb，長按 P 只有 `c6b bit3` 時可 | 0x1003aff0 | [確認] |
| S5 | 閒置時鐘：任何鍵/轉盤歸零計數；> 200 frames 顯示時鐘覆蓋層（元件 0xfc 底 sprite 0x79 128×48 於 (64,30)、AM/PM 0xfe、時分數字 0xff–0x103、日期 0x104–0x10d，依 `0x18000bec` 的日期格式排列） | `space_idle_clock` 0x1008e604、`space_clock_update` 0x1006a074 | [確認] |
| S6 | 圈選單（表 0，style 1）：CONNECTION(34)→0x77、TAMA STARS(35)→0x7f、DECO PLANET(36)→0x80、TRAVEL(37)→0x7e、PLANET LV.(38)→0x7d。灰項：TAMA STARS 灰 = 沒有死亡星星；DECO 灰 = 沒有任何 type 5/6/7 物品。不可用（→ NOTICE_MSG，返回 0x1d）：睡覺中的 CONNECTION/TRAVEL → 0x69；DECO rank < 3 → 0xf4；TRAVEL 成長階段 < 3 → 0x6b、rank < 3 → 0xf4 | 0x1003aff0 state 5000 | [確認] |
| S7 | 從子場景回來（`home_space_init` 0x1001b3d4）依上一個 scene 把選單游標放回對應項（0x77→0、0x7f→1、0x80→2、0x7e→3、0x7d→4），子場景設 `c6a bit2` 時直接重開選單 | 0x1001b3d4 | [確認] |
| S8 | CONNECTION（0x77，`tushin_state` 0x1008cc54）是通訊子選單（PLAYDATE、GIFTS、PLANETS、DOWNLOAD、ADDITIONAL、ARCADE），規則見 05 §2.10 | | [確認] |
| S9 | 星球名：`0x18000c58` u16[9]（glyph），新遊戲「NAME YOUR PLANET」(100) 與設定「EDIT PLANET NAME」(12) 編輯；星球層畫面本身**不顯示**名字（用在通訊/好友資料） | 03-save、`setplanetname` | [確認]（顯示處 [推測]） |

### 2.7 DECO PLANET（0x80，`deco_state` 0x100582cc，ctx `0x1801422c`，0x590 B）

| state | 內容 | 輸入 | 標記 |
|---|---|---|---|
| init | 記錄 3 類各自是否擁有（0x588..0x58a = 沒有 → 灰） | | [確認] |
| 0 | 背景元件 0x10e（sprite 0x6d）、星球元件 0x110（anim 0xa8：CHARA 2 姿勢 1，(64,52)） | | [確認] |
| 1000 | 類別列（`deco_category_build` 0x1006cdb8）：底條 0x10f（(64,108)，sprite 0x3f）、圖示 0x112/0x113/0x114 於 x=24/64/104,y=108（sprite 0x6f 幀 0/1/2 = 帽子/眼鏡/飾品；沒擁有 → palette 1 灰）、游標 0x111（sprite 0x5d）。從下方滑入（10 frames） | A/UP → 下一類、DN → 上一類（循環 3，游標補間 2 frames）；確定：該類有物品 → 2000，否則錯誤音 0xb；C → 回 HOME_SPACE 並重開選單 | [確認] |
| 2000 | 物品轉盤（`deco_list_build` 0x1007ddf8）：該類所有持有物品（`item_list_collect(4,type)`，依 id 排序）+ 末尾一個 0（= 不戴），至少 3 格；初始游標 = 目前裝備（ctx+0x578）。橫向 5 格可見（中心 ±2，間距 42 px），每格框 0x11d + 物品 +6 圖示；名稱文字元件 0x117 = 物品 +2。**游標移動時即時把星球 `inst[8+類]` 換成該物品（0 → 不畫）預覽** | 轉盤/A 捲動（`listscroll`）；確定：選了 0 或與目前相同 → 目前沒戴 → 錯誤音 0xb，有戴 → 4000「REMOVE?」(0x7f)；選了不同物品 → 3000「IS THIS OKAY?」(0x7e)；C → 1000 | [確認] |
| 3000/4000 | YES/NO（`FUN_1002f1f8`） | YES → 5000；NO → 2000 | [確認] |
| 5000 | 星球改用 anim 0xc2（64,64），疊上閃光 anim 199（0xc7：一團 sprite 0x738/0x739 閃光縮放 80 frames，音效 0x28）；腳本發出 EVT 15 那一刻：`equip[類] = 新 id`，新物品使用中 +1、舊物品使用中 −1，重建星球 → 閃光散開（音效 0x21）；動畫結束 → 淡出 → state 0（回類別列） | | [確認] |

### 2.8 TRAVEL（0x7e，`scene_tamaspace_travel` 0x10057904，ctx `0x18014224`）

| # | 規則 | 證據 | 標記 |
|---|---|---|---|
| V1 | 標題畫面（`travel_title` 0x1006ed38）：UNCHI METER 量表（`FUN_10067204`）+ 元件 0x382「GO TRAVEL」按鈕（sprite 0x75，meter < 60 → palette 1 灰）+ 文字 0x381。B/P → meter ≥ 60 ? 目的地清單 : 訊息 0x82「COLLECT MORE POOPS BY CLEAN POOP」；C → 回 HOME_SPACE 選單 | 0x10057904 state 1000、0x5dc | [確認] |
| V2 | UNCHI METER = `0x18000d13`（TamaData+0x1b），`unchi_meter_add` 0x10062748 夾 0..60。來源：清大便時 + 清掉的數量（`FUN_1006706c`）；醒著時每 2 次 onaka 計時週期 +1（`FUN_100134bc`） | 0x10062748 呼叫者 | [確認] |
| V3 | 目的地清單（`travel_dest_list` 0x1006f4f0，清單 type 3 = 物品清單）：顯示順序與所需 rank：SWEET 9901 (3)、RELAX 9902 (3)、FOODIE 9903 (4)、MONSTER 9910 (5)、ICE 9908 (6)、ANGEL 9909 (7)、MUSIC 9907 (7)；清單長度依 rank：3 → 2、4 → 3、5 → 4、6 → 5、7–10 → 7（rank < 3 → 不可能，FUN_10068388 錯誤）。圖示 = 物品 +6（80×80 小星球 sprite 239–245/251/258/259） | 0x1006f4f0 | [確認] |
| V4 | 選定 → 「GO TO THIS PLANET?」(0x81) → YES：`travel_anim_start` 0x1006f810 播目的地動畫（0x866,0x867,0x868,0x86f,0x86d,0x86e,0x86c 依序對應上表），動畫內背景用物品 +4（128×128 目的地景色，例 2031）[推測：BG 指令 p=−1]；角色替換：role 0 = 寵物、role 1/2 = 本 field 前兩隻放牧 tama（`grazing_id(field,0/1)`）、role 3 = 當地 NPC（`0x10091634[dest*2 + toggle]`：0xa7/0xa8、0xa9/0xaa、0xab/0xac、0xba/0xbb、0xb6/0xb7、0xb8/0xb9、0xb3/0xb4） | 0x1006f810 | [確認] |
| V5 | 結算（`travel_reward` 0x1006666c，動畫開始同 frame）：meter −60；`item_give(souvenir[dest*2 + toggle], 1)`；toggle = `0x18001680` bit[dest]，之後翻轉；mission 8 bit[dest]（`0x18000cb6`）。動畫結束 → 心情 +1 → 顯示獲得的紀念品（`travel_souvenir_show` 0x1006ef30）→ 任意鍵結束回 HOME | 0x1006666c、0x10057904 state 0x514/0x578 | [確認] |
| V6 | 紀念品表（RW 資料 `0x180006e6`，7×2）：SWEET 8001 CREPE HAT / 8002 FRUIT BALLOONS；RELAX 8003 TURBAN / 8004 HOT SPRING SET；FOODIE 8005 BENTO BERET / 8006 BIG RICE PADDLE；MONSTER 8101 MONSTER HORNS / 8102 TRIDENT；ICE 8085 SNOW FLAKE TIARA / 8086 SNOW FLAKE WAND；ANGEL 8099 ASCENSION HAT / 8100 HOLY BELL；MUSIC 8013 KEYBOARD RIBBON / 8014 ACCORDION。第一次拿 A、第二次 B、交替 | pram.bin 解壓 | [確認] |
| V7 | 背包上限：裝扮 cap 5（`item_give` 夾住），滿了仍扣 meter [推測：沒看到檢查] | 05 §2.3 | [推測] |

### 2.9 TAMA STARS（0x7f，`lookstars_state` 0x10057e54，ctx `0x18014228`）

| # | 規則 | 證據 | 標記 |
|---|---|---|---|
| L1 | 畫面同星球層的星空 + 死亡星星（`lookstars_view_build` 0x1008898c），游標元件 0x2cc 放在選中星星的元件座標 | 0x1008898c、`lookstars_cursor_place` 0x100835c8 | [確認] |
| L2 | 星星 ≥ 2 時：A/UP → 下一顆、DN → 上一顆（循環），音效 0。確定 → 音效 1，在星星位置播 anim 0xca（拉近），結束後白色淡入第 1 頁 | state 1000/0x44c | [確認] |
| L3 | 第 1 頁（`lookstars_ghost_show` 0x10077558）：底元件 0x13f；若 ghost 資料可讀 → anim 0xa9（年齡 ≤ 29）或 0xaa（> 29）於 (64,96)，角色 = ghost 槽 0xd6+星星序號。A → 第 2 頁；C → 回星星清單 | | [確認] |
| L4 | 第 2 頁（`lookstars_profile_show` 0x1007f8c4）：名字（ghost 名；讀不到 → 「？？？」text 0x2cc）、年齡數字 + 「YEARS」(0xa0)、「CAUSE」(0x79) + 死因文字 0x79+cause（1 TAMA SICK、2 TAMA INJURY、3 HUNGER、4 LIFE SPAN）。A → 第 1 頁；C → 清單 | | [確認] |
| L5 | 清單畫面 C → 回 HOME 並重開選單 | | [確認] |

### 2.10 PLANET LV.（0x7d，`scene_tamaspace_rank` 0x100570bc，ctx `0x1801421c`）

| # | 規則 | 證據 | 標記 |
|---|---|---|---|
| R1 | 從星球選單進入（上一個 scene ≠ 0x1c）：第 1 頁元件 0x14e–0x153：標題「PLANET LV.」(38)、數字 = rank、底下文字 = rank < 10 ? 「GOAL」(131) : text 257 | state 1 | [確認]（257 的實際字串 [推測] 屬另一表） |
| R2 | 第 1 頁 確定：rank < 10 → 第 2 頁（元件 0x156–0x15a：「GOAL」+ 目標文字 `0x100a138e[rank]`：0 → 132 RAISE TO KID、1 → 133 TO YOUNG、2 → 134 TO ADULT、3 → 135 BREED OR GET NEW EGG、4 → 136 SWITCH FIELDS、5–9 → 135）；rank 10 → `PSID_TAMASPACE_ENDCREDIT`(0x81)。C → 回 HOME_SPACE 選單。第 2 頁 C → 第 1 頁 | state 1000/2000/0x7d1 | [確認] |
| R3 | 從 HOME 路由進入（rank 高 nibble < 低 nibble，05 §2.6）：anim 0xc4（星空 + 星球角色 2 跳動、音效 0x15/0x5 … 角色替換 role 0 = 舊 rank 外型、role 1 = 新 rank 外型，依 class 表）；同步 `c6c` 高 nibble = 低 nibble；結束 → 「CONGRATS! LEVEL n NOW!」（`0x100a13a2[rank]` = 142..151）；任意鍵或 200 frames → 結束；rank ≥ 10 → `PSID_TAMASPACE_RANKUP_END`(0x82) | state 3000/4000/0xfa1 | [確認] |

---

## 3. 資料表

| 位址 | 格式 | 內容 | 抽出 |
|---|---|---|---|
| 0x18000d8e（save+0x1a6） | u16[3][2][6] | 擺設位 item id（field 外層） | — |
| 0x18001494（save+0x8ac） | u4[640] | 倉庫「使用中」數（擺出/戴著） | — |
| 0x18000d88（save+0x1a0） | u16[3] | 星球裝備 帽/眼鏡/飾品 | — |
| 0x18000d13（save+0x12b） | u8 | UNCHI METER 0..60 | — |
| 0x18001680（save+0xa98） | u32 bit[0..6] | 旅行紀念品交替位 | — |
| 0x18001cb0 | u16 | romtype（4/5/6），決定星球 class 與主場 | — |
| 0x1009ba92 | 3×2×6×{u16 elem, s16 z} | SETSUBI 位置 | decor.json `toy_slots.setsubi_pos` |
| 0x100933ae | 3×0x54 B | HOME 場地錨點（+0xc area0 9 筆、+0x30 area1 9 筆） | `toy_slots.home_pos` |
| 0x1009bb22 / 0x1009ba90 / 0x1009bb26 | u8[4]/u8[2]/u8[6] | 小型候選 0..3、大型 4,5、REMOVE 順序 0,4,1,2,5,3 | `toy_slots` |
| 0x180009f0（RW） | 2 × 12 B | PLACE(59)/REMOVE(60) 選單 | — |
| 0x100a1020 / 0x10094786 / 0x100947b2 / 0x1009479c | u16[11] | 星球 ChrID by rank（class 0/1/2；a1020 為 4×0x16 B 同內容 + 第 4 列未用 [推測]） | `planet_chr_by_rank` |
| 0x100947c8 | 7 × {u16 hat, glasses, acc, guestA, guestB} | 成套組合 → 訪客 | `equip_sets` |
| 0x100a09b8 / 0x100a0084 | u8[3] / u16[3] | DECO 類別 type {5,6,7} / 圖示元件 0x112–0x114 | `deco` |
| 0x10091634 | u16[14] | 旅行地 NPC 角色（每地 2，依 toggle） | `travel.destinations[].npc_chara` |
| 0x180006e6（RW，PRAM 0xe6a0 解壓） | u16[14] | 旅行紀念品 | `travel.destinations[].souvenirs` |
| 0x100a138e / 0x100a13a2 | u16[10] / u16[11] | rank 目標文字 / 升級文字 | `rank` |
| item 表 type 3/4 | 46 B | +4 圖示、+6 清單圖、+8 展示 anim、+0xa 遊玩 anim | `toys`（87 筆） |
| item 表 type 5/6/7 | 46 B | +4 星球上的 sprite、+6 圖示 | `worn_items`（108 筆） |
| data 表 18 ChrID 2,11–14,23–28 | 0xd8 B | 星球模型，部件 kind 0/4/6/8 | `planet_models` |

UI 元件：0x2c8/0x2c9 星空、0x2ca/0x2cb 星球 (90,92)、0x2d0–0x2d4 星星、0x2cc 星星游標、0x2d6/0x2d7 左右箭頭、0x2d8 擺設游標、0x10e–0x114 DECO、0x117/0x118/0x119/0x11c/0x11d/0x120–0x124 DECO 轉盤、0x13f/0x144–0x14d 星星資料頁、0x14e–0x15a 等級頁、0x9d/0x9e 升級文字、0x381/0x382 旅行標題。

Sprite：0x450/0x451 星空（128×128，4 pal）、0x453 死亡星星（32×32×3）、0x455/0x456 擺設佔位框（32/64）、0x4ba 紙箱、0x4bb TAMAVERSE OBJECT、0xfc–0x101（252–257）/0xde（222）/0xf6–0xf9（246–249） 星球 body 64×64×5、0x5aa–0x5f7 帽/飾品 64×64、0x5f8.. 圖示 32×32、0x7ef–0x7f8（2031–2040）旅行地景 128×128、0xef–0xf5/0x102/0x103 小星球 80×80、0x738/0x739 DECO 閃光、0x6f DECO 類別圖示、0x79 時鐘底。

---

## 4. 新命名

```
10027960 setsubi_init f
100279f0 setsubi_update f
10027924 setsubi_exit f
1001654c toy_count_in_area f
100165d8 toy_count_total f
10016638 toy_slot_get f
10016840 toy_slot_set f
100168a4 toy_slot_clear f
10016900 toy_area_clear_all f
100167d8 setsubi_field_list_build f
1006258c field_is_open f
1006d804 toy_place_conflicts f
10083d54 setsubi_remove_conflicts f
10083b94 setsubi_drop_toy f
10070300 setsubi_draw_area f
10070ef0 setsubi_arrows_create f
10071624 setsubi_cursor_create f
10071864 setsubi_cursor_move f
10071818 setsubi_cursor_busy f
1007aaa4 setsubi_remove_fields_build f
1007ac64 setsubi_remove_slots_build f
1007171c setsubi_remove_cursor_create f
1001e178 item_inuse_add f
1001e628 item_inuse_count f
1001e5f8 item_owned_count f
1007e85c inv_inuse_get f
1007e8d0 inv_inuse_set f
1001e250 item_list_collect f
1007d968 item_list_filter f
1001669c home_toy_entries f
10016768 home_toy_capacity f
10016388 home_toy_fillers f
10016c64 home_toy_objects_build f
10016ebc home_toy_assign_visitors f
10016b88 home_toy_compact f
10016a18 home_toy_anim_spawn f
1002820c equip_set_match f
100282f8 equip_set_guest f
10007c68 planet_inst_build f
1002b314 romtype_field_class f
10062d94 deco_init f
10062e68 deco_update f
10062d44 deco_exit f
100582cc deco_state f
1006cdb8 deco_category_build f
1006cf1c deco_category_input f
1006d140 deco_category_hide f
1007ddf8 deco_list_build f
1007e1ac deco_list_cb f
1007e108 deco_list_cell_draw f
1001b3d4 home_space_init f
1001b688 home_space_update f
10088850 space_view_build f
1006dcb0 space_stars_collect f
1008e4d4 space_clock_build f
1006a074 space_clock_update f
1008e604 space_idle_clock f
1008e67c space_clock_show f
10088aac space_zoom_out_enter f
10088af4 space_zoom_in_leave f
100635f0 rank_init f
10063698 rank_update f
1006375c travel_init f
100637c8 travel_update f
1006ed38 travel_title f
1006f4f0 travel_dest_list f
1006eda8 travel_confirm f
1006f810 travel_anim_start f
1006666c travel_reward f
1006ef30 travel_souvenir_show f
10062748 unchi_meter_add f
100632fc lookstars_init f
10063370 lookstars_update f
10057e54 lookstars_state f
1006dd6c lookstars_collect f
1008898c lookstars_view_build f
100835c8 lookstars_cursor_place f
10077558 lookstars_ghost_show f
1007f8c4 lookstars_profile_show f
10063898 tushin_init f
100638f8 tushin_update f
1008cc54 tushin_state f
10074bd8 mon_romtype f
1009ba92 setsubi_pos_tbl l
100933ae home_field_layout_tbl l
1009bb26 setsubi_remove_order l
100947c8 equip_set_tbl l
100a1020 planet_chr_tbl l
10094786 planet_chr_tbl_c0 l
100947b2 planet_chr_tbl_c1 l
1009479c planet_chr_tbl_c2 l
100a138e rank_goal_txt l
100a13a2 rankup_txt l
100a09b8 deco_cat_types l
100a0084 deco_cat_elems l
10091634 travel_npc_tbl l
180006e6 travel_souvenir_tbl l
180009f0 setsubi_menu_items l
18000d8e g_toy_slots l
18001494 g_inv_inuse l
18000d13 g_unchi_meter l
18001680 g_travel_toggle l
18001cb0 g_romtype l
```

---

## 5. 未解問題

1. **tween 長度單位**：`FUN_10066b34` 的最後參數（0x4000/0x8000/0x10000/0x20000）本文假設 16.16 秒（0x10000 = 20 frames，依 08 U4 的 `(r<<16)/20`）；需讀 `FUN_10066b34` 確認。
2. **清單灰色項目**（可擺數 0 的玩具、bit4）在 `FUN_1002cc90` 清單元件內是否不可選：未讀清單元件程式。
3. **HOME 玩具動畫重播與訪客重抽時機**：`0x18001c32` 何時清除（換日？離開 HOME？）未追；遊玩動畫結束（EVT −1）後的處理在 home 物件 callback（`FUN_10016fc0`）未細讀。
4. **romtype 4 在 rank 4–5 的 field 清單**為空（`field_is_open`）——疑似原廠 bug 或 romtype 4 不存在於零售機；`0x18001cb0` 初值來源（flash 哪裡）未追。
5. **旅行動畫細節**：0x866–0x86f 的 BG 指令是否真的用物品 +4 的 128×128 景色、NPC 角色 id（0xa7…）對應哪些 TamaID，未逐一解碼；旅行時背包已滿是否仍扣 meter 未見檢查。
6. **PLANET LV. 第 1 頁 rank 10 的 text 257**（「LAB STAFF」）不合理，可能是另一個字串表（`FUN_10065e80` 參數）；需確認 `FUN_10065e80` 的 id 空間。
7. **HOME_SPACE zoom 參數**（`FUN_1000eff4(z, −16, 0x40, 4)` 第 4 參數）與視差函式 `FUN_1000ecf0` 公式未讀。
8. 成套訪客 9000–9021 對應的角色資料（`FUN_1002400c`）與 anim 0x2f1 出現的位置（`0x100933ae` 第 8 筆 0x3c6 (98,64)？）待確認。
9. `0x100a1020` 第 4 列（744..736）用途不明。

---

## 摘要（≤15 行）

1. 帽子／眼鏡／飾品戴在**星球**上，不是寵物：星球 = ChrID 2/11–14/23–28 的角色模型，部件 kind 4/6/8 = `0x18000d88/8a/8c`，sprite = item+4，位置固定 (0,−20)/(0,−8)/(0,+10)。
2. 星球外型 = `planet_chr[romtype class][rank]`（rank 1/4/6/10 換外型），裝備介面是星球選單 DECO PLANET（rank ≥ 3），轉盤選物品即時預覽，閃光動畫 EVT 15 時寫入。
3. 7 組旅行紀念品成套戴上 → 回 HOME 時 1/3 機率出現對應訪客角色（表 0x100947c8）。
4. 戶外玩具：`0x18000d8e[field][area][6]`（field 外層，area 1 = FIELD2）；位 0–3 小、4–5 大，大型 4+k 佔用小型 2k/2k+1，放置時擋路的自動收回。
5. 位置表 0x1009ba92（SETSUBI）/0x100933ae（HOME），世界座標，已抽到 decor.json。
6. 玩具 +8 = 展示動畫、+0xa = 遊玩動畫；放牧 tama（小 1／大 2）或寵物（行為碼 1）會在玩具上玩；玩具不影響數值。
7. 「使用中」nibble 陣列 0x18001494（save+0x8ac）= 擺出 + 戴著；可賣 = 持有 − 使用中。
8. SETSUBI 流程：PLACE/REMOVE → 清單 → 轉盤選位（跨端切 area/field）→ PLACE HERE?/PUT AWAY? → 飛走 30 f、落下 50 f。
9. HOME_SPACE：星空（調色盤循環）、星球 (90,92) 醒/睡姿勢、最多 5 顆死亡 tama 星星、閒置 200 f 顯示時鐘；拉近到 2.0 回場地。
10. TRAVEL：UNCHI METER（0x18000d13，0..60，清大便累積）滿 60 才能去，扣 60、心情 +1、紀念品 2 選 1 交替、mission 8；目的地依 rank 2→7 個。
11. TAMA STARS：死亡 ghost 槽 25–29，兩頁（tama 動畫／名字、年齡、死因）。
12. PLANET LV.：rank + 下一目標文字；升級時自動播舊→新星球動畫與「LEVEL n NOW!」，rank 10 進片尾。
13. 新工具 `tools/extract_decor.py`（stdlib，可選 pram.bin 取紀念品表，否則由成套表推得，兩者一致）。
