# 02 成長階段、進化與角色定義表

資料來源：`re/decomp-all/XIP.c`、`dumps/paradise-glacier-20261019-113049.bin`、`dumps/xip.bin`。
抽出的資料：`docs/spec/data/characters.csv`（189 筆）、`docs/spec/data/tamaid_list.csv`（135 筆）。

## 1. 摘要

- 角色定義表不在 XIP 裡，而是在 **flash 資產 ARC2 封包**（flash 0x111000）的 entry 0「game data block」裡的第 11 號子表。開機時 `asset_init_main` (0x10003160) 把每個子表的指標登記到 SRAM `0x1800ded4 + bank*0x5c`。`chara_def_get` (0x10023e10) 在每個已登記的 bank 裡線性搜尋 TamaID。主封包有 **189 筆 × 30 bytes（15 × u16）**，位於 flash 0x129714–0x12ad3a。另有最多 40 個追加封包，位於 flash 0xd2e000 + i*0x4000，本 dump 裡全部為空。
- 成長階段存在 `g_tama_stage_bits` (0x18000d10) 的 bits 2–4：1 = 蛋、2 = 嬰兒、3 = 幼年、4 = 青年、5 = 成年。TamaID 的千位數也對應階段：1000 是蛋，1001 是嬰兒，2xxx 幼年，3xxx 青年，4xxx 成年。
- 年齡計時器 `g_care_age_timer` (0x18000d40) 以秒為單位，每個階段歸零，上限 1,209,600 秒（14 天）。秒數由 `tama_tick` (0x100134bc) 累加；RTC 補算時每次最多傳入 60 秒。
- 進化分兩步。先由 `tama_tick` 依年齡與條件發出 `EM_REQ_EVOLUTION` (1) 或 `EM_REQ_EVOL_SUPPORT` (0x13)；這個 request 被「完成」時，`req_done` (0x10068aa0) 再呼叫 `tama_evolve` (0x1000fc98)，由它選出下一個角色。
- 分支輸入如下：
  - **所在區域**：`g_field_cur` (0x18000dd7)，共 3 個區域，0/1/2。
  - **嬰兒期的區域停留紀錄**：每待滿 15 分鐘記一筆。
  - **餵食分類紀錄**：只計算原生區域的食物，取最近 4 筆，算出分支 0–2；平手或沒有紀錄時為 3。
  - **照顧失誤數**：0x18000d2b，加上最大愛心數，算出品質 0–3。
  - **通訊紀錄**：青年期和兩台不同 ROM type 的機器連線過，用於稀有進化。
  - **幼年期年齡是否逾時**：逾時會變成 4017「bbmarutchi」。
- 亂數**不參與**進化，只用在出生時的顏色與 id2 繼承（各 50%）。
- 角色表欄位 `Evolution` 和 `EvolBranch` 的名字有誤導性。`Evolution` 沒有被進化程式讀取，`EvolBranch` 實際上是「討厭的食物群組」。真正的進化路徑寫死在 XIP 的兩張表裡：0x10091314 與 0x1009132c。
- 出生由 `tama_birth` (0x1000f708) 處理，會把 0x18000d14 開始的 0x70 bytes 照顧資料全部清零。世代數存在 0x18000d06，上限 99。下一代的顏色和 id2 各有 50% 機率繼承父方（伴侶）。死亡紀錄會存進墓碑表；`tama_days` ≥ 30 時標記為 daiojo（壽終正寢）。

## 2. 詳細規則

### 2.1 角色定義表的位置

| 規則 | 證據 | 標記 |
|---|---|---|
| ARC2 格式：magic `ARC2`(0x32435241)，+4 checksum（從 +8 起的位元組和），+8 長度，+0xC 條目數，條目從 +0x10 開始，每筆 16 bytes `{?, offset, size, size2}`，offset 相對於封包起點 | `arc2_check` 0x1000299c、`arc2_entry` 0x100028e4、`arc2_get_file` 0x10002954 | [確認] |
| 主封包在 flash 0x111000（vaddr 0x60111000），3 個條目：0 = game data（0x111040，750,140 B），1 = 圖像（0x1b8a7c），2 = 文字（0xc02e24） | `asset_init_main` 0x10003160 | [確認] |
| game data block 開頭是 23 個 u32 偏移（以 u16 為單位，相對於 block 起點）；`asset_bank_register` 0x10023f8c 把 `base + off[j]*2` 存到 `0x1800ded4 + j*4 + bank*0x5c`。子表 11 → 12 是角色表（`chara_def_get`）；子表 10 → 11 是道具表，每筆 23 × u16 共 343 筆（`item_def_get` 0x10022ff0） | 0x10023f8c、0x10023e10 讀 `0x1800df00/0x1800df04`（= ded4 + 0x2c/0x30） | [確認] |
| 角色表：flash 0x129714，30 B/筆，**189 筆**。搜尋順序是依 bank 註冊順序（`asset_bank_id` 0x10003104），找到第一個 TamaID 相符的就回傳 | 0x10023e10 | [確認] |
| `chara_def_by_chrid` 0x10023d7c：用欄位 1（ChrID）搜尋同一張表 | 0x10023d7c | [確認] |
| 追加封包：flash 0xd2e000 + i*0x4000，i = 0..39，註冊為 bank i+1。本 dump 全部無效（沒有 ARC2 magic） | `asset_init_additional` 0x10003238，Python 檢查 | [確認] |

### 2.2 欄位意義（record 位元組偏移）

| idx | 偏移 | 欄位 | 意義 | 證據 | 標記 |
|---|---|---|---|---|---|
| 0 | +0x00 | TamaID | 主鍵。千位數 = 階段；`TamaID % 1000` = 顯示用的 `TAMA_C%04d` | mon_tamaid | [確認] |
| 1 | +0x02 | ChrID | 圖像/角色資源 ID，傳給 `FUN_100006a8/FUN_10000674` 繪圖 | 0x10006fe4 等 | [確認] |
| 2 | +0x04 | NameID | 文字表索引（`text_get` 0x10065ea0） | 0x100452f0 | [確認] |
| 3 | +0x06 | GrowthStage | 1–5，複製到 stage bits | mon_tamaid、0x1001021c | [確認] |
| 4 | +0x08 | Species | 屬種。成年/青年 = 區域×100 + 系列；幼年 = 1..6 | 0x10018394 | [確認]（意義 [推測]） |
| 5 | +0x0A | SpeciesRank | 成年 1–4 一般、5 = 稀有；配對相容度用 | 0x1006cc98 | [確認] |
| 6 | +0x0C | ItemGrp | **喜歡的食物群組**：餐點群組與它相同時心情 +1 | `item_mood_bonus` 0x100171fc | [確認] |
| 7 | +0x0E | Attribute | 原生區域/版本 1..6：1 land、2 water、3 sky、4 forest、5 tropical、6 frozen | `attr_to_field` 0x100039d4 | [確認] |
| 8 | +0x10 | Gender | 0/1/2，4017 為 4 | 未追蹤 | [推測] |
| 9 | +0x12 | Hungry | 飢餓愛心每掉 1 顆需要的**分鐘數**（程式裡乘 60），0 表示不掉 | 0x100134bc | [確認] |
| 10 | +0x14 | Happy | 心情愛心每掉 1 顆需要的分鐘數 | 0x100134bc | [確認] |
| 11 | +0x16 | Evolution | 嬰兒 60、其他 1440（分鐘）。**程式沒有讀取**，實際門檻寫死在 0x100913ac | 搜尋所有 chara_def_get 呼叫者 | [推測] |
| 12 | +0x18 | Disease | 每次新增照顧失誤時，生病的機率（%） | 0x100134bc `OSEWAMISS CHECK` | [確認] |
| 13 | +0x1A | EvolBranch | **討厭的食物群組**：餐點群組與它相同時心情 −1（名字是開發者取的，與進化無關） | 0x100171fc | [確認] |
| 14 | +0x1C | Option | 預設顏色 0–15（`g_tama_color` 低 5 bits；0x1f = 未設定） | 0x1000f708、0x1000fc98 | [確認] |

特殊紀錄：
- index 0 的 TamaID 為 0xFFFF（哨兵）。
- 9000–9021：ChrID 167–187，其他欄位為 0，推測是 NPC 或訪客。
- 60000、60100–60128、60200、60300：ChrID 189–220，推測是第 2 代以後的 id2 外觀。

### 2.3 成長階段與年齡門檻

`g_tama_stage_bits` 的 bits 2–4 = stage，讀法是 `(b & 0x1f) >> 2`。寫入時用 `(b & 0xe3) + stage*4`。

年齡門檻表 `evo_stage_age_threshold` 位於 0x100913ac，共 6 個 u32，以 stage 為索引：`{0, 60, 3600, 86400, 86400, 1209600}` 秒。

| 階段 | TamaID | 何時發出 EVOLUTION request | 證據 | 標記 |
|---|---|---|---|---|
| 1 蛋 | 1000 | age ≥ **60 s**（通用分支：stage ∉ {2,3,5} 且 `age >= thr[stage]`） | 0x100134bc | [確認] |
| 2 嬰兒 | 1001 | 見 2.4。age < **14400 s（4 h）** 時要等某區域累積 ≥ 4 筆紀錄（≥ 60 分鐘）；age ≥ 14400 時強制進化。`thr[2]=3600` 只是名目值，不會用到 | 0x100134bc、規劃器 0x10011b18 用 0x3840 | [確認] |
| 3 幼年 | 2xxx | **86400 ≤ age < 100800**，且目前區域 == 紀錄[0]（原生區域）時才發出；不在原生區域時會清除 request。age ≥ **100800（28 h）** 時強制發出，結果變成 4017 | 0x100134bc（`thr[3]` + 0x3840） | [確認] |
| 4 青年 | 3xxx | age ≥ **86400 s（24 h）** | 0x100134bc | [確認] |
| 5 成年 | 4xxx | 不再進化 | `uVar7 != 5` | [確認] |

年齡累加規則（0x100134bc 開頭）：
- 只在 life_state (0x18000d08) < 6 時累加。上限 0x127500 = 1,209,600 秒。
- 例外：stage 3 且 age ≥ 86400 時，只在「沒在午睡（flag 0x1a）也沒在睡覺時段」（`FUN_10010c80`==0）且 life_state ≠ 1 時才累加。所以幼年 24–28 h 的 4 小時窗口只算**醒著的時間**。[確認]
- 呼叫端 `tama_rtc_catchup` (0x10012c90) 把 RTC 經過的秒數切成 ≤ 60 s 的段落，最多 1440 段（24 h），每段呼叫一次 `tama_tick`。[確認]
- 每次進化（`tama_evolve`）都會把 age 歸零。[確認]

`cmd_evolve` (0x10072600) 的作弊行為，可用來對照真實規則：
- stage 1：+60 s（剛好達到蛋的門檻）。
- stage 2：往區域紀錄塞入 4 筆目前區域（讓 dominant field 成立）。
- stage 3 和 4：+86400 s。

「TamaAge」在 monitor 裡印的其實是 stage 值。

### 2.4 嬰兒期：區域紀錄與 EVOL_SUPPORT

| 規則 | 證據 | 標記 |
|---|---|---|
| 區域計時器 `g_field_timer[3]`（u16，0x18000d46 + 2*field）在 stage 2 每 tick 加 dt；≥ 900 s 時歸零，並在 `g_field_log`（0x18000d1a，筆數存在 0x18000d19，最多 11 筆）附加目前區域編號 | 0x100134bc | [確認] |
| 換區域時（`field_set_current` 0x10010cf4）三個區域計時器全部歸零 | 0x10010cf4 | [確認] |
| `evo_dominant_field` 0x10081c68：統計紀錄中各區域筆數，某區域 ≥ 4 筆就回傳它；多個成立時取**編號最大**的；都不成立回傳 3 | 0x10081c68 | [確認] |
| age < 14400 且 dominant ≠ 3 時：如果 `g_evol_support_miss`（0x18000d17）< 2，就發出 **EM_REQ_EVOL_SUPPORT**（0x13），並把 support 計時器 0x18000d44 歸零；否則直接發出 EM_REQ_EVOLUTION | 0x100134bc | [確認] |
| EVOL_SUPPORT 存在期間，0x18000d44 累加 dt；≥ 900 s（15 分鐘）沒完成就逾時：呼叫 `evo_drop_dominant_field` 0x10082118（從紀錄刪掉所有 dominant 區域的筆數），清除 request，miss + 1 | 0x100134bc、0x10082118 | [確認] |
| EVOL_SUPPORT 被「完成」時（`req_done(0x13)`），與 EVOLUTION 一樣呼叫 `tama_evolve` | 0x10068aa0 case 1/0x13 | [確認] |
| age ≥ 14400：如果 EVOL_SUPPORT 還在，先 drop dominant 再清除，然後強制發出 EVOLUTION | 0x100134bc | [確認] |
| 玩家如何「完成」support（推測是小遊戲 "delivering vitamin!"，文字 #804） | — | [推測] |

### 2.5 `tama_evolve` (0x1000fc98) 的完整分支

前提：對應的 request（1 或 0x13）處於 pending 狀態（`req_is_pending` 0x1006916c）。

`field = g_field_cur`（0x18000dd7），值為 0 / 1 / 2。

- **stage 1 → 2**：新 ID = 1001。[確認]
- **stage 2 →**：`d = evo_dominant_field()`
  - `d == 3`（沒有任何區域累積到 1 小時）→ 直接成為**成年 4017 bbmarutchi**（stage 5），+800 點，紀錄筆數歸零。[確認]
  - 否則 → 幼年（stage 3），+300 點，新 ID 取自 `{2091, 2113, 2069}[d]`，紀錄改成 4 筆 d，筆數設為 4。[確認]
- **stage 3 →**：
  - age < 100800 → 青年（stage 4），+500 點，新 ID = `evo_teen_table[field*4 + food_branch()]`（0x10091314）。[確認]
  - age ≥ 100800 → 成年 4017，+800 點。[確認]
- **stage 4 →**：成年（stage 5），+800 點。
  - 品質 q：
    - `care_miss_log_count`(0x18000d2b) == 0 且 `onaka_max` > 4 且 `gokigen_max` > 4 → **q=0**
    - 否則 count < 2 → **q=1**
    - 否則 count < 6 → **q=2**
    - 否則 → **q=3**
  - [確認]
  - `b = food_branch()`；新 ID = `evo_adult_table[field*16 + b*4 + q]`（0x1009132c）。[確認]
  - **RARE EVOLUTION**：以下三個條件同時成立時，新 ID = `{4112, 4134, 4090}[field]`，並印出 `RARE EVOLUTION !!`。[確認]
    - `comm_romtype0`(0x18000d60) ≠ 0 且 `comm_romtype1`(0x18000d64) ≠ 0
    - `romtype_to_field(g_romtype)` == field
    - b ≠ 3
  - 非稀有進化時，在 `g_adult_branch_bits[field]`（0x18000cd1/cd3/cd5，各 16 bit）設 bit `b*4+q`。[確認]
- 所有進化共通的後續處理：[確認]
  - 清除 request、METAMORPHO(0x14)、DODODO_IGNORE(0x1d)。
  - `g_tama_status &= 0x10`（只保留 DIRTY）。
  - `chara_id` = 新 ID。
  - `chara_id2` = 世代 < 2 時等於新 ID，否則為 60000。
  - age 歸零，cold 歸零，0x18000ce8[0..2] 歸零。
  - 如果 color == 0x1f，就在顏色收集 bitmap 0x18000cd7 設 bit Option。
  - 呼叫 `FUN_1000580c(newID)`，設定圖鑑 bit 0x18000c78[list]。
  - 存檔。

**食物分支 `food_branch` 0x10081d70**：
- 讀取 `g_food_log`（0x18000d27，最多 4 筆，筆數存在 0x18000d26），統計 18 種分類（值 0..17）的次數。
- 依區域取 3 個分類比較：field 0 → 分類 12/13/14；field 1 → 15/16/17；field 2 → 9/10/11。
- 次數**嚴格最大**的那一個回傳 0/1/2；平手或全為 0 時回傳 3。[確認]

**食物紀錄的寫入**（`tama_eat_item` 0x1001021c）：
- 道具的分類 cat（item+0x1c）必須屬於角色 Attribute 的原生範圍（`item_cat_not_native` 0x1001dd2c）：
  - 群組 1（attr 1/5）：cat 13–15
  - 群組 2（attr 2/6）：cat 16–18
  - 群組 3（attr 3/4）：cat 10–12
- 並且角色 GrowthStage < 4（只限蛋、嬰兒、幼年）。
- 符合時，`g_food_cat_pts[cat]`（0x18000d4b + cat − 1）加上 item+0x28。值 > 4 時減 5，並把 `cat−1` 推入 4 格 FIFO。
- 因此青年期餵食**不影響**成年分支，成年和青年用同一個 b。[確認]

**幼年變形 METAMORPHO**（request 0x14）：
- stage 3 時，在**非原生區域**（≠ `field_log[0]`）累積 3600 s（區域計時器 ≥ 0xe10）就發出 request。
- 同時把紀錄改寫成 4 筆目前區域，食物紀錄筆數歸零。
- 完成時（`req_done` case 0x14），角色換成 `{2091, 2113, 2069}[field_log[0]]`，也就是變成另一個區域的幼年。
- 另外，stage 3 時換區域會清除 pending 的 EVOLUTION（0x10010cf4）。
- [確認]

**通訊登記**（`comm_register_romtype` 0x1002ace4）：
- 只在 stage 4（青年）且對方 romtype ≠ 自己時登記。
- 第一台存進 0x18000d60；第二台必須與第一台不同，存進 0x18000d64。
- 所以稀有進化需要青年期和**兩種不同版本**的機器連線過。[確認]

**ROM type 與區域**：
- `g_romtype`（0x18001cb0）在開機時從 flash 0xd29000 讀取。本機的值是 **6**（White Glacier）。
- `romtype_to_field` 0x1002b314 的對應：1,5 → 0；2,6 → 1；3,4 → 2；其他 → −1。開機時把 `g_field_cur` 設成這個值（PRAM 0x89a8）。[確認]
- 因此本機的原生區域是 field 1，屬種 6（frozen）。

### 2.6 進化樹（本韌體可達的路徑）

蛋 1000 →（60 s）嬰兒 1001 babymarutchi →（≥ 1 h 停留於某區域，最晚 4 h）

各區域的幼年 → 青年對照：

| field | 幼年 | 青年 b=0 | b=1 | b=2 | b=3（平手/無） |
|---|---|---|---|---|---|
| 0 tropical | 2091 tropical kid | 3092 roar young | 3093 toddle young | 3094 lick young | 3095 sprout young |
| 1 frozen | 2113 frozen kid | 3114 roar young | 3116 glide young | 3115 toddle young | 3117 float young |
| 2 forest | 2069 forest kid | 3070 roar young | 3071 toddle young | 3072 chirp young | 3073 sprout young |

成年 = `evo_adult_table[field][b][q]`，q = 0 最好，q = 3 最差：

| field / b | q=0 | q=1 | q=2 | q=3 |
|---|---|---|---|---|
| 0/0 | 4096 meowtchi | 4097 madillotchi | 4098 tarantytchi | 4099 crocotchi |
| 0/1 | 4100 mametchi | 4101 parorotchi | 4102 tenatchi | 4103 namakemotchi |
| 0/2 | 4104 kuikuitchi | 4105 poizutchi | 4106 chameleotchi | 4107 iguanatchi |
| 0/3 | 4108 yashikitchi | 4109 potsunentchi | 4110 ananatchi | 4111 avovotchi |
| 1/0 | 4119 okojotchi | 4118 wolftchi | 4120 polakumatchi | 4121 azaratchi |
| 1/1 | 4122 irukatchi | 4123 ginjirotchi | 4125 rakkotchi | 4124 hoppentchi |
| 1/2 | 4127 lemmingtchi | 4126 moosetchi | 4128 raichotchi | 4129 yukiusatchi |
| 1/3 | 4130 nerinetchi | 4131 dangouotchi | 4132 yaytytchi | 4133 yukiraratchi |
| 2/0 | 4074 horhotchi | 4075 konkotchi | 4076 tigaotchi | 4077 tanoontchi |
| 2/1 | 4078 lessapantchi | 4079 kanokotchi | 4080 suigyutchi | 4081 panbootchi |
| 2/2 | 4082 kachitchi | 4083 tokipatchi | 4084 kuchipatchi | 4085 sparrotchi |
| 2/3 | 4086 shiitaketchi | 4087 peatchi | 4088 nappatchi | 4089 rushraditchi |

稀有進化：field 0 → 4112 manapatchi、field 1 → 4134 hobohorntchi、field 2 → 4090 tatsutchi。
失敗分支：4017 bbmarutchi（嬰兒沒有區域、或幼年逾時）。

**注意：**
- field 1 的表順序不是單調的（例如 q=0 是 4118 之後的 4119）。表內容照實抄錄，未修改。
- 第一代版本（attr 1–3：land/water/sky，TamaID 2002–2004、3005–3016、4018–4068，稀有 4066–4068）在本韌體的進化程式中**無法到達**。推測只用於顯示來訪或通訊對象。

### 2.7 出生、世代、死亡

**`tama_birth(partner)` 0x1000f708**：

- **partner ≠ 0 時（養育下一代）**：先查詢目前角色（父/母），然後：
  - 如果 color == 0x1f，就固定成它的 Option 顏色。
  - 用父/母的 Attribute 設定區域：`field_set_current(attr_to_field(Attribute))`。
  - [確認]
- **共通處理**：
  - chara_id = 1000，tama_days = 0，day 秒數（0x18000d00）= 0，stage = 1。[確認]
- **partner == 0（新遊戲或死亡後重來）**：id2 = 1000，世代 = 1，color = 0x1f。[確認]
- **partner < 60000**：
  - 世代 + 1（上限 99）。
  - id2：`rand(0..99) < 50` 時保留舊 id2，否則 = partner。
  - 顏色：50% 保留目前顏色，50% 取 partner 的 Option（上限 15）。
  - 在顏色收集設 bit。
  - [確認]
- **partner ≥ 60000**：
  - 世代 + 1，id2 = partner。
  - 顏色：50% 保留，50% 取 `FUN_10018558` 回傳紀錄的 +0x10（為 0xff 時改用該紀錄 +0xc 角色的 Option）。
  - [確認]
- **後續**：
  - 照顧區 0x18000d14..0x18000d83（0x70 B）全部清零，包含 age、miss、紀錄、通訊。
  - request bitmap 0x18000cee/0x18000cf3 各 5 B 清零；但會保留 BIRTHDAY、NEWYEAR、METEOR..XMAS 這些事件 request，並重新發出。
  - life_state = 0，status = 0，重新取亂數種子（`g_tama`）。
  - [確認]
- **呼叫者**：
  - `cmd_birth` 0x10071fec：partner = 0。
  - 0x1003ccb0（event 0xbbb）：partner = 0，死亡後的新蛋。
  - 0x1004dcd0：partner = 0，「new egg」送走成年寵物。成年時 `g_adults_raised[group]`（0x18000c74）+1。
  - 0x1004e1ec（event 0x518）：partner = `chara_def_by_chrid(伴侶 ChrID)` 的 TamaID，即「breed」配對。成年時同樣 +1，另外 0x18000cc8 + 1（上限 50）。
  - [推測：0x1004e1ec 的參數由 r0 傳遞，Ghidra 沒有顯示]

**死亡流程**（細節屬於 care 子系統，這裡只列和世代有關的部分）：
- EM_REQ_DEATH(8)：life_state = 4，0x18000d78 = 3540 s。
- DYING(0xc)：life_state = 5，0x18000d78 = 60 s。
- DEAD(0xd)：呼叫 `grave_record_add(cause, tama_days)`（0x1000affc），life_state = 6。[確認]
- 墓碑表在 0x18000df0，共 5 筆環狀。每筆格式：bit0 = valid、bits1–3 = cause（3 = 一般、1 = 延遲死亡旗標）、bits4–11 = tama_days、**bit12 = daiojo = (tama_days > 29)**。[確認]
- `cmd_dying 1` 把 tama_days 設成 30 來模擬 daiojo。程式裡沒有找到「年老自然死亡」的計時器。[推測]

### 2.8 Tama days

- 只在 life_state < 4 時累加：0x18000d00（u32 秒）+= dt；≥ 86400 時 `g_tama_days`（0x18000d04）+1（上限 999），並扣掉 86400。[確認] 0x100134bc
- 出生時歸零。tama_days 和 age timer 是**兩個獨立的計時器**：age 每個階段都歸零，tama_days 是一生的天數。

## 3. 資料表

| 位址 | 大小 | 內容 |
|---|---|---|
| flash 0x111000 | ARC2，3 條目 | 主資產封包 |
| flash 0x111040 | 23 × u32 | game data 子表偏移（u16 單位）：`0x2e,0x33,0x62a5,0x6959,0x6a29,0x7083,0x9147,0xa15f,0xa498,0xa499,0xa499,0xc36a,0xce7d,...` |
| flash 0x125972 | 343 × 46 B | 道具表（子表 10；+0x16 類型 1 = 餐點 / 2 = 零食，+0x1c 分類，+0x22 群組，+0x26 飢餓值，+0x28 分數） |
| flash 0x129714 | **189 × 30 B** | 角色表（子表 11）→ `data/characters.csv` |
| XIP 0x100919da | 135 × u16 | `tamaid_list`：monitor `tamaid n` 用的 ID 清單（1000, 1001, 2002…4134，不含 4017 與 9xxx/6xxxx）→ `data/tamaid_list.csv` |
| XIP 0x10091314 | 12 × u16 | `evo_teen_table[field*4+b]` |
| XIP 0x1009132c | 48 × u16 | `evo_adult_table[field*16+b*4+q]` |
| XIP 0x100913ac | 6 × u32 | `evo_stage_age_threshold` = {0, 60, 3600, 86400, 86400, 1209600} |
| 程式內常數 | 3 × u16 | 幼年 `{2091, 2113, 2069}`、稀有 `{4112, 4134, 4090}` |
| flash 0xc02e24 | ARC2，9 條目 | 文字封包。條目 `lang−1`：0 = 日文，1–6 = 拉丁語系（名稱相同），7、8 = CJK。格式為 `u16 count, u16 off[count]`（u16 單位），字串是 u16 glyph、以 0 結尾 |

拉丁 glyph 對應（已驗證）：305 = 空白、306–315 = '0'–'9'、317 = '-'、318–343 = 'a'–'z'、61440 = 換行。CSV 的 `name_en` 取自文字條目 1。

`characters.csv` 欄位：`index, flash_offset, TamaID…Option（15 個 u16，以無號數輸出）, name_en`。抽取腳本的邏輯：讀取主 ARC2 entry 0 → 子表 11/12 → 每筆 30 B，用 `struct '<15H'` 解析。

## 4. 新命名

```
10003160 asset_init_main f
10003238 asset_init_additional f
1000299c arc2_check f
100028e4 arc2_entry f
10002954 arc2_get_file f
100030d8 asset_bank_count f
10003104 asset_bank_id f
10023f8c asset_bank_register f
10022ff0 item_def_get f
10023d7c chara_def_by_chrid f
10022d6c chara_tamaid_by_chrid f
10065ea0 text_get f
1000f708 tama_birth f
1000fc98 tama_evolve f
100134bc tama_tick f
10012c90 tama_rtc_catchup f
10011b18 tama_plan_wakeup f
10013234 tama_new_day f
10013158 tama_debug_preset f
10081c68 evo_dominant_field f
10081d70 evo_food_branch f
10082118 evo_drop_dominant_field f
1002b314 romtype_to_field f
100039d4 attr_to_field f
10003974 attr_to_group f
1001dd2c item_cat_not_native f
1001021c tama_eat_item f
100171fc item_mood_bonus f
1002ace4 comm_register_romtype f
10010cf4 field_set_current f
10010aa4 tama_life_state_get f
10010c80 tama_is_resting f
1007dc1c tama_is_sleep_time f
1006916c req_is_pending f
10069218 req_set f
10068a50 req_clear f
10068aa0 req_done f
1000affc grave_record_add f
1000580c tamafile_register f
100694d4 bitmap_set f
18000d00 g_tama_day_secs l
18000d06 g_generation l
18000d08 g_tama_life_state l
18000d17 g_evol_support_miss l
18000d19 g_field_log_count l
18000d1a g_field_log l
18000d26 g_food_log_count l
18000d27 g_food_log l
18000d2b g_care_miss_log_count l
18000d44 g_evol_support_timer l
18000d46 g_field_timer l
18000d60 g_comm_romtype0 l
18000d64 g_comm_romtype1 l
18000dd7 g_field_cur l
18001cb0 g_romtype l
18000cd1 g_adult_branch_bits l
18000cd7 g_color_collection l
18000c74 g_adults_raised l
18000c78 g_tamafile_bits l
1800ded4 g_asset_bank_tables l
10091314 evo_teen_table l
1009132c evo_adult_table l
100913ac evo_stage_age_threshold l
```

## 5. 未解問題

- **EVOL_SUPPORT 的玩家操作**：哪個 UI 或小遊戲會呼叫 `req_done(0x13)`？可以搜尋 `FUN_10068aa0(0x13` 或間接呼叫。
- **g_tama_chara_id2 的意義**：第 2 代以後固定為 60000，出生時 50% 取伴侶 ID。推測是「外觀繼承」或遺傳特徵用的 ID；60000-series 的紀錄（ChrID 189–220）需要對照 sprite 確認。
- **`onaka_max`/`gokigen_max` > 4 的成長來源**：XIP.c 第 10711 行附近的函式有 +1，需要追呼叫者，可能是訓練。
- **request 0x1b**：會讓 `care_miss_log_count` −1，需要找出是哪個事件（可能是讚美或道具）。
- **Gender 欄位、Evolution 欄位**：在本韌體沒有找到讀取者。可能用於配對（0x1004e1ec）或通訊封包，需要搜尋 `+ 0x10)` 與 chara_def 指標的組合。
- **第一代角色（attr 1–3）**：是否能透過追加封包（flash 0xd2e000）或 romtype 1–3 機器的通訊取得。
- **年老死亡**：程式裡沒有看到「成年 N 天後自動死亡」的規則。墓碑的 daiojo 只看 tama_days ≥ 30。死亡觸發（0x18000d7c/0x18000d80 = 129600 s 倒數）請參照 care 子系統文件。
- **life_state 1 的意義**：會影響 age 與飢餓計時，推測是睡眠中。
