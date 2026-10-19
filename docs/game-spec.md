# Tamagotchi Paradise 遊戲規格書（重製用）

2026-10-19 · 依據：White Glacier 版實機 flash dump（ROM type 6，ROM 3.01）

這份文件說明原廠韌體實際上怎麼運作，作為 Rust 重製的依據。每條規則都能追到韌體位址；細節與證據放在 `docs/spec/0N-*.md`，這裡是整理後的總覽。

- **[確認]**：直接讀反編譯碼或資料得到。**[推測]**：由上下文推得，重製前最好再驗證。
- 位址慣例：PRAM `0x0000xxxx`、XIP `0x10xxxxxx`、SRAM `0x18xxxxxx`、flash 視窗 `0x60xxxxxx`（= flash offset）。
- 抽查過的關鍵事實：PRAM 解密（getter 位址吻合）、存檔 CRC（自算 0xb522 = 存檔值）、飢餓遞減公式、進化年齡表、角色表、sprite 解碼（2284 張逐像素一致）、monitor 密碼。

## 目錄

1. 硬體與韌體總覽
2. 執行架構：開機、主迴圈、時基
3. 寵物狀態模型
4. 時間模擬與離線補算
5. 照顧規則
6. 成長與進化
7. 世界、內容與進度
8. 素材：圖像、文字、聲音
9. 存檔
10. 重製設計對照
11. 未解問題
12. 檔案與工具索引

---

## 1. 硬體與韌體總覽

| 項目 | 內容 |
| --- | --- |
| SoC | Sonix SNC73410，ARM Cortex-M3，96 MHz（PLL 192 MHz/2） |
| 螢幕 | 128×128 RGB565，ST7735S 類 SPI 面板（[推測] 型號），單一 framebuffer 32 KiB |
| 輸入 | A、B、C 三鍵 + 轉盤（旋轉 + 按壓） |
| 聲音 | 單音 PWM 蜂鳴器（不是 PCM） |
| Flash | 16 MiB SPI NOR（KH25L12833F 類） |
| 通訊 | UART1 = 機身接腳（460800 baud）；UART0 = debug monitor（無對外接點，RX 在 TP48） |
| RTC | 32-bit 秒計數器，DPD 中持續走 |

### 韌體組成

| 區塊 | 位置 | 內容 |
| --- | --- | --- |
| Load table | flash 0x0 | `SONIXDEV`，V3（AES-256 CBC），指向 PRAM 映像 |
| PRAM | flash 0x1000，64 KB，**加密**；開機解密到位址 0x0 | 向量表、ISR、C runtime、低階 HAL、存檔讀寫（flash 寫入必須在 RAM 執行）、`TamaData()`/`TamaCare()` getter |
| XIP | flash 0x11000–0xbffff，**明文**；執行位址 0x10000000 | 全部遊戲邏輯、UI、場景、monitor 指令 |
| DPD 映像 | flash 0x110000，4 KB | Deep Power Down 喚醒小程式 |
| 資產 | flash 0x111000–0xc33ba3 | ARC2 封包：資料表、2284 張 sprite、9 語言字串 |
| 使用者資料 | flash 0xd29000 起 | 版本資訊、DLC、ghost、截圖、存檔（詳見 §9） |


---

## 2. 執行架構

細節：[06-system.md](spec/06-system.md)

### 開機

`Reset_Handler`(PRAM 0x2f4) → `SystemInit`(0x4438：時脈、關看門狗、NVIC) → C runtime（解壓初始資料到 0x18000000）→ `main`(XIP 0x1007f81c) → `sys_init`(0x1006ac00) → `main_loop`(0x1006b0b4)，永不返回。

`sys_init` 依序做：讀 RTC 與喚醒原因、1 ms SysTick、heap、電池檢查、載入存檔、RTC 有效性檢查、`srand(adc + rtc)`、載入資產、`boot_wake_handler`（補算時間）、LCD 與輸入初始化、驗證 ghost/道具/放牧資料，最後進入場景 `PSID_STARTUP`(0x6c)。

### 主迴圈與時基 [確認]

沒有 RTOS，單執行緒 super-loop。三個時基：

| 時基 | 頻率 | 用途 |
| --- | --- | --- |
| SysTick | 1 ms | 毫秒計數、轉盤取樣、音效 sequencer |
| Frame | 名目 **20 fps**（LCD TE 腳 3 個脈衝） | 輸入、場景邏輯、繪圖、動畫 |
| RTC 秒 | 1 Hz | **全部寵物模擬** |

每個 frame 依序：餵狗 → 讀鍵 → 電池 ADC 濾波 → **遊戲時間推進** → 清 framebuffer → `scene_update`（邏輯 + 繪圖）→ 淡入淡出 → UART1 通訊 → UART0 monitor → `rand()` 一次 → 等 TE → DMA 推 framebuffer。

20 fps 是 [推測，強]：600 frames 與 30 秒被當成同一個逾時條件，面板設定約 60 Hz ÷ 3，小遊戲也以 20 fps 設計。

### 輸入 [確認]

- 按鍵 bitmask：bit0 A、bit1 B、bit2 C、bit3 轉盤按壓（低電位有效）。
- 轉盤：每 1 ms 取樣，每 3 個計數產生單 frame 的 bit7（DIAL UP）或 bit6（DIAL DOWN）。
- 自動連發 18 frames 後每 3 frames；長按 >13 frames；雙擊 = 放開後 3–10 frames 內再按。
- 沒有事件佇列，各場景每 frame 直接讀 bitmask。

### 場景系統 [確認]

- 場景表 0x10094830：137 筆 `PSID_*`，每筆 `{id, init, update, draw, exit, flags, name}`（28 bytes）。完整清單：`docs/spec/data/scenes.csv`。
- `scene_update`(0x1006b200) 以 init → run → exit 三階段驅動；轉場只靠「下一個場景 ID」變數。
- 全域覆寫：低電量 → 0x76、閒置關機 → 0x75、閒置回家 → HOME(0x1c)、debug 旗標 → 0。
- 帶旗標 0x10 的場景（HOME、照顧畫面、Labo 選單）進入時會存檔。

### 電源 [確認]

- 閒置 200 frames（約 10 秒）回 HOME，600 frames（約 30 秒）關機。
- 關機前算出下一個遊戲事件時間（最多 3600 秒後）設成 RTC alarm、強制存檔、進 Deep Power Down。
- **每次喚醒都是完整重開機**。由 reset cause 判斷：RTC 喚醒就補算時間後再睡，按鍵喚醒就點亮螢幕。
- 低電量：低於 2.1 V（HOME/設定畫面 2.2 V）顯示低電量畫面 200 frames 後關機。

### Debug 與工廠模式 [確認]

- **UART monitor**：UART0、460800 8N1。輸入 `mY6$O$xw` + CR 解鎖（密碼 XOR 混淆存在 PRAM 0xe18c），之後用 `d <cmd> [args]`。指令表見 [re-notes](re-notes.md)，可直接讀寫寵物狀態，是實機驗證規則的工具。
- **工廠測試**：冷開機按住 A+C，經 UART1 與治具做 0xAA/0x55 交握後進 `PSID_TESTMODE`（11 項測試）。

---

## 3. 寵物狀態模型

細節：[re-notes.md](re-notes.md)、[01-care.md](spec/01-care.md)、[03-save.md](spec/03-save.md)

全部持久狀態是 SRAM `0x18000be8` 起的 **0xffc bytes**（存檔映像）。寵物本體在裡面：

- `TamaData()` = 0x18000cf8（映像 +0x110）
- `TamaCare()` = 0x18000d14（映像 +0x12c）

### 核心欄位 [確認]

| 位址 | 型別 | 欄位 | 範圍 |
| --- | --- | --- | --- |
| 0x18000cfc | u16 | 角色 ID（TamaID） | 見角色表 |
| 0x18000cfe | u16 | 第二 ID（外觀變體；第 2 代起進化時設為 60000） | |
| 0x18000d04 | u16 | Tama days | 0–999 |
| 0x18000d06 | u8 | 世代 | ≤99 |
| 0x18000d08 | u8 | cond：0 正常、1 未命名、2 感冒、3 受傷、4 DEATH、5 DYING、6 DEAD | |
| 0x18000d0c | u32 | 狀態旗標：0x01 LOVE、0x02 IMMUNITY、0x04 HAPPY、0x10 DIRTY、0x20 DANCE、0x40 KIRAKIRA | |
| 0x18000d10 | u8 | bits 2–4：成長階段 | 1–5 |
| 0x18000d11 | u8 | bits 0–4：顏色（0x1f = 未設定） | 0–15 |
| 0x18000d14 | u8 | onaka 飢餓 | 0–6 |
| 0x18000d15 | u8 | gokigen 心情 | 0–20 |
| 0x18000d16 | u8 | 照顧失誤累計 | ≤99 |
| 0x18000d18 | u8 | 感冒次數 | 0–4 |
| 0x18000d26–2a | | 食物分類紀錄（FIFO 4 筆） | |
| 0x18000d2b / 2c | u8 / u8[6] | 失誤紀錄索引 / 紀錄（種類 0x11 飢餓、0x12 心情） | 0–6 |
| 0x18000d32 / 33 | u8 | onaka_max / gokigen_max：幼年、青年期補滿的次數 | ≤99 |
| 0x18000d34 | u8 | 大便數 | 0–6 |
| 0x18000d36 | u8 | 髒污計數 | 0–3 |
| 0x18000d38 / 3a | u16 | 飢餓 / 心情計時器（秒，上數） | |
| 0x18000d40 | u32 | 年齡計時器（秒，每階段歸零） | ≤1,209,600 |
| 0x18000d68 | u8 | 飢餓遞減次數（每 2 次觸發便意） | |
| 0x18000d6c | u16[6] | 每坨大便的年齡 | ≤3600 |
| 0x18000d78 / 7c / 80 | | 死亡倒數：DeathRemain / DeathByCold / DeathByHungry | |
| 0x18000dd7 | u8 | 目前所在區域（field 0–2） | |

### 請求系統（EM_REQ）[確認]

遊戲用「請求」表示寵物的需求與事件，共 0x00–0x23 個 ID。兩組 40-bit 集合：raised（0x18000cee）與 done（0x18000cf3）。`req_raise` 發出、`req_done`(0x10068aa0) 完成並套用效果。完整 ID 表見 [01-care §2.10](spec/01-care.md)。常用的：

| ID | 名稱 | 意義 |
| --- | --- | --- |
| 0x01 | EVOLUTION | 可以進化 |
| 0x02 / 0x03 | WAKE / SLEEP | 起床 / 睡覺 |
| 0x04 | 餵食 | 吃道具 |
| 0x05 | OTEIRE | 打理（清髒污） |
| 0x08 / 0x09 | DEATH / DEATH_DELAY | 死亡流程 |
| 0x0a / 0x0b | COLD / INJURY | 感冒 / 受傷 |
| 0x0c | DYING | 臨終 |
| 0x0e / 0x10 | SOWASOWA / UNCHI | 想大便 / 大便了 |
| 0x0f | HOMETRG | 叫玩家回 HOME |
| 0x11 / 0x12 | ONAKA / GOKIGEN | 肚子餓 / 心情差的呼叫 |
| 0x13 / 0x14 | EVOL_SUPPORT / METAMORPHO | 嬰兒進化支援 / 幼年換區域變形 |
| 0x1a | NAP | 午睡 |
| 0x1b | REDU_CM | 減少失誤紀錄 |
| 0x1d | EVENT_DODODO | 特殊事件 |

---

## 4. 時間模擬與離線補算

細節：[01-care §2.1–2.2](spec/01-care.md)、[03-save §2.4](spec/03-save.md)

### 時鐘 [確認]

現在時刻 = `rtc_base_day`（2025-01-01 起的日數，映像 +0x08）+ `rtc_base_sec`（映像 +0x0c）+ RTC 計數器。設定時鐘時計數器歸零、存新 base。存檔全壞時預設 2026-01-01 12:00。

### 推進方式 [確認]

`care_time_update`(0x10012c90) 每 frame 執行：

1. 讀現在時刻 `now`，與「上次處理到的時刻」`last`（映像 +0x18）比較。
2. `now == last` 就返回，所以**模擬是每 RTC 秒一次**。
3. `now < last`（時鐘倒退）：`last = now`，不補時間。
4. 否則 `dt = now - last`，切成 ≤60 秒的段落呼叫 `care_tick(t, dt)`(0x100134bc)，**最多 1440 段 + 1 段，約 24 小時；超過的時間直接丟掉**。
5. 日期有變就呼叫 `care_day_change`(0x10013234) **一次**（跨多天也只一次）。
6. `last = now`。

線上與離線走同一條路徑。`care_tick` 用每段的起始時刻判斷是否在睡，所以離線補算也會套用作息。手動設定時鐘時 `last` 直接設成新時間，不補算。RTC 停止（拔電池）時設「時鐘失效」旗標，直到玩家設定時間前都不補算。

### `care_tick(t, dt)` 執行順序 [確認]

1. 年齡 `+= dt`（上限 14 天）。幼年期超過 24 小時後只算醒著的時間。
2. 清潔計時 `+= dt`（醒著才算，上限 14400）。
3. 午睡計時：滿 60 秒結束午睡。
4. DeathRemain `-= dt`，歸零時 DEATH → DYING。
5. 以下只在 cond < 4：
   - 天數秒數 `+= dt`，滿 86400 則 Tama days +1（模擬秒，不看日曆）。
   - DeathByCold / DeathByHungry 倒數，歸零時發 DEATH_DELAY。
   - **醒著且沒午睡時**：飢餓與心情計時、元素與進化計時、便意、大便老化、飢餓與心情遞減、失誤判定、嬰兒感冒、失誤後生病判定（詳見 §5）。

---

## 5. 照顧規則

細節：[01-care.md](spec/01-care.md)

### 飢餓與心情 [確認]

- onaka 0–6、gokigen 0–20。計時器只在**醒著、沒午睡**時累加。
- 計時器達到 `角色.Hungry × 60` 秒：計時歸零、onaka −1；達到 `角色.Happy × 60` 秒：gokigen −1。值為 0 的角色不遞減。
- 典型值（分鐘，Hungry/Happy）：蛋 0/0、嬰兒 3/5、幼年 45/20、青年 60/30、成年依角色 5–90。
- 降到 0 時發呼叫（ONAKA / GOKIGEN）+ HOMETRG。
- 呼叫 **900 秒**沒處理：清掉呼叫，幼年起算一次照顧失誤。
- HAPPY 狀態下 gokigen 不下降。
- 吃東西加 onaka 時：飢餓計時與 DeathByHungry 歸零。
- onaka_max / gokigen_max：幼年、青年期從未滿補到全滿就 +1，用於進化品質判定。

### 餵食 [確認]

- `onaka += item.onaka`。
- 喜好修正 `pref`：食物群組等於角色 `ItemGrp`（喜歡）+1，等於 `EvolBranch`（實為討厭）−1。
- 正餐：`gokigen += pref`。點心：`gokigen += item.gokigen + pref`。
- 蛋、嬰兒、幼年期吃原生區域食物時累計食物分類，影響進化（§6）。
- 特殊道具（0x1772–0x17a2）：改顏色、HAPPY/LOVE/IMMUNITY、午睡、DANCE/KIRAKIRA、減少失誤紀錄等。道具表 `docs/spec/data/items.csv`。

### 大便與髒污 [確認]

- 每 2 次飢餓遞減進入便意（SOWASOWA），**20 秒**後大便 +1（上限 6）。
- 便意期間帶去廁所：不產生大便，gokigen +1。
- 每坨大便各自計齡（上限 3600 秒）。**6 坨都滿 3600 秒**時感冒 +1。
- 清大便：大便歸零，依清潔計時給獎勵（3600–7200 秒間 +2k、7200–14400 秒間 +k）。
- 髒污：每次換日若當天沒打理就 +1（上限 3）；超過 2 時起床會設 DIRTY。打理（按 4 次）清除 DIRTY、計數歸零、gokigen +1。

### 生病、受傷、失誤 [確認]

- 感冒 +1 的來源：
  - 6 坨大便都滿 3600 秒。
  - 嬰兒年齡 600 秒時必定一次。
  - 幼年起每次新增失誤時：失誤總數為 6 的倍數，**或** `rand(0..99) < 角色.Disease`（多數 30–40）。
- IMMUNITY 狀態擋掉所有感冒增加；cond = 2（已在感冒）時也不再加。
- 感冒時：cond = 2，DeathByCold 設 **129600 秒（36 小時）**。吃藥一劑治好，gokigen +1。
- **感冒累計第 5 次直接死亡**（由大便路徑走 DEATH_DELAY，由失誤路徑直接 DEATH）。
- 受傷：cond = 3，沒有死亡倒數；治療 gokigen +1。
- 失誤紀錄最多 6 筆（索引 0x18000d2b），道具 REDU_CM 可減 1 筆索引，但不減總數。

### 睡眠作息 [確認]

| 時段 | 小時 | 行為 |
| --- | --- | --- |
| 0 | 06–09 | 06:00 發起床請求；未叫醒前算在睡 |
| 1 | 10–15 | 10:00 強制起床 |
| 2 | 16–18 | 醒著 |
| 3 | 19–21 | 19:00 發睡覺請求；已上床則算在睡 |
| 4 | 22–05 | 22:00 強制睡 |

- 蛋不睡；嬰兒在年齡 1800–2100 秒時午睡（自動）。
- 睡眠中**凍結**：飢餓、心情、大便、元素、失誤。**照走**：年齡（幼年 24 小時後除外）、死亡倒數、天數。
- 沒有關燈機制 [推測]。

### 死亡 [確認]

- 觸發：onaka = 0 起算 36 小時沒餵（DeathByHungry）、感冒 36 小時沒治（DeathByCold）、或第 5 次感冒。
- 流程：DEATH_DELAY →（醒著時）DEATH（cond 4，**3540 秒**急救窗口）→ DYING（cond 5，**60 秒**）→ DEAD（cond 6）。DYING 結束時若有大往生（≥30 天）的前代還沒救過人，它會把寵物救回（cond 0），每位前代一次。急救：寵物層按任意鍵；墓碑畫面按住 C 再按 A 領新蛋（08-care-ui §2.13–2.16）。
- 死亡紀錄寫入 5 格墓碑環形表（0x18000df0）：死因（1 疾病、3 飢餓）、天數。Tama days ≥ 30 標記為 daiojo（壽終正寢）。
- **沒有自然壽命**：程式裡沒有成年 N 天後自動死亡的規則。

---

## 6. 成長與進化

細節：[02-evolution.md](spec/02-evolution.md)；資料：`docs/spec/data/characters.csv`（189 筆）

### 角色定義表 [確認]

位於資產封包的資料表 #11（flash 0x129714），189 筆 × 15 個 u16。`chara_def_get(id)`(0x10023e10) 依 bank 註冊順序搜尋（bank 0 內建，1–40 DLC）。

| 欄位 | 真正意義 |
| --- | --- |
| TamaID | 主鍵；千位數 = 階段（1000 蛋、1001 嬰兒、2xxx 幼年、3xxx 青年、4xxx 成年） |
| ChrID | 圖像資源 ID |
| NameID | 名稱字串索引 |
| GrowthStage | 1–5 |
| Species / SpeciesRank | 屬種；成年 rank 5 = 稀有，用於配對相容度 |
| ItemGrp | **喜歡**的食物群組 |
| Attribute | 原生區域 1–6：land、water、sky、forest、tropical、frozen |
| Gender | 0/1/2（4017 為 4）；用途未明 |
| Hungry / Happy | 每顆愛心遞減的**分鐘數** |
| Evolution | **程式沒讀**（實際門檻寫死在 0x100913ac） |
| Disease | 每次新增失誤時的生病機率 % |
| EvolBranch | **討厭**的食物群組（名字有誤導性） |
| Option | 預設顏色 0–15 |

### 階段與門檻 [確認]

| 階段 | 進化條件 |
| --- | --- |
| 1 蛋 (1000) | 60 秒 |
| 2 嬰兒 (1001) | 在某區域累積 ≥60 分鐘（每 900 秒記一筆，需 4 筆）；最晚 4 小時強制 |
| 3 幼年 (2xxx) | 年齡 24–28 小時之間，且在原生區域時；24 小時後只算醒著的時間；超過 28 小時 → 4017 |
| 4 青年 (3xxx) | 24 小時 |
| 5 成年 (4xxx) | 不再進化 |

進化分兩步：`care_tick` 發出 EVOLUTION（或嬰兒期的 EVOL_SUPPORT），請求完成時 `tama_evolve`(0x1000fc98) 決定新角色。**進化不用亂數**。

### 分支輸入 [確認]

- **field**：目前區域 0 tropical、1 frozen、2 forest。
- **food_branch b**：蛋到幼年期吃原生區域食物的最近 4 筆分類中，嚴格最多的那一類 → 0/1/2；平手或沒有 → 3。青年期餵食不影響。
- **品質 q**：失誤紀錄 0 筆且 onaka_max > 4 且 gokigen_max > 4 → 0；<2 筆 → 1；<6 筆 → 2；否則 → 3。

### 分支規則 [確認]

- 蛋 → 嬰兒 1001。
- 嬰兒 → 幼年 `{2091, 2113, 2069}[停留最久的區域]`，+300 點；沒有任何區域滿 1 小時 → 成年 4017 bbmarutchi。
- 幼年 → 青年 `teen_table[field][b]`（0x10091314），+500 點；逾時 → 4017。
- 青年 → 成年 `adult_table[field][b][q]`（0x1009132c），+800 點。
- **稀有進化**：青年期與兩種不同 ROM type 的機器連線過 + 目前在本機原生區域 + b ≠ 3 → `{4112, 4134, 4090}[field]`。
- **幼年變形**：在非原生區域累積 3600 秒 → 變成該區域的幼年。
- 每次進化：年齡、感冒歸零；狀態旗標只保留 DIRTY；存檔。

完整進化樹（3 區域 × 4 食物分支 × 4 品質 = 48 個一般成年 + 3 稀有 + 1 失敗）見 [02-evolution §2.6](spec/02-evolution.md)。

### 出生與世代 [確認]

- `tama_birth`(0x1000f708)：ID 1000、階段 1、照顧資料 0x70 bytes 全清零。
- 世代 +1（上限 99）。配對出生時，顏色與第二 ID 各 50% 繼承伴侶。
- 本機 ROM type 6 → 原生區域 field 1（frozen）。

---

## 7. 世界、內容與進度

細節：[05-content.md](spec/05-content.md)；資料：`items.csv`、`shopcodes.csv`、`scenes.csv`

### 世界與縮放 [確認]

- 轉盤在 4 個縮放層級間切換（`0x18001c6a & 3`）：星球 → 場地 → 寵物 → 細胞。每層有自己的選單（表 0x10094120）。

| 層級 | 選單項目 |
| --- | --- |
| 星球 | 通訊、看星星、裝飾（rank≥3）、旅行（rank≥3 且幼年起）、Rank |
| 場地 | 設施（rank≥2）、探險（rank≥1 且幼年起）、清大便、切換場地（rank≥4） |
| 寵物 | 吃飯、一起玩、打理 |
| 細胞 | 狀態 |

- 3 個場地：TROPICAL、FROZEN、MORI，各可花 10000 點擴充第二區域。
- 天氣每天依月份機率表抽 6 種 pattern × 5 時段。

### 進度 [確認]

- **Planet rank**（0x18000c6c 低 4 bits，0–10）：rank 1–3 來自進化，4 來自第一次世代交替，5 來自第一次切換場地，6–10 來自之後的世代交替。Rank 解鎖選單、商店分頁、道具、事件。
- **22 個 mission**，各 +1000 點；計數器 0x18000ca8–cd8，目標表 0x100942b4。

### 經濟 [確認]

- Gotchi 點數 0x18000ca4，上限 99999。`points_add` 不能扣點（unsigned 比較）；扣點走 `points_spend`(0x10010e30)。
- 收入：進化 +300/+500/+800、mission +1000、新年 +1000、Lab 小遊戲 10–800、每日一次 +50（MICROSCOPE 的 NO DATA lab code）、賣道具。
- 支出：商店、擴充區域 10000、保母 300。
- 商店依 `(月+日) % 5 + 1` 每日輪替；尾數 5 的日子食物半價、尾數 0 的日子玩具與服飾半價。
- 道具表 343 筆 × 46 bytes（flash 0x125972）；背包 640 格，食物堆疊 10、服飾 5。12 個料理配方、115 組商店代碼。

### 事件 [確認]

日曆事件由 `FUN_10083984` 每日排程，回到 HOME 時依時段與 `rand(0..99)` 觸發：每月 9/19/29 流星（30%）、7/14/21/28 流星雨、11/23 UFO、12 月 Santa（10%）/Eve/Xmas、新年、生日、每月 13 日 17:00 DODODO（rank ≥ 6）。

### 小遊戲 [確認]

16 個：7 個 Lab 遊戲（依難度與結果給 10–800 點）、7 個「一起玩」（只加心情，其中 3 個沒有選單入口）、2 個連線 arcade。除一個 arcade 外都是固定配置、沒有亂數。各遊戲規則見 [05-content §2.11](spec/05-content.md)。

### 社交 [確認]

- 12 個好友位。每次 playdate 友好度 +1~+3。playdate 結果：玩、打架、吃、被吃。
- 結婚：雙方成年且友好度 4，沒有性別檢查。離線媒合每天 3 次，基礎機率 80/50/40/20%。
- 放牧：3 場地 × 8 格（存成 ghost）。死亡 ghost 5 格。DLC 40 格。
- 通訊協定：GMMan `tcp.md`、`playdate.md` 已有文件。

---

## 8. 素材：圖像、文字、聲音

細節：[04-assets.md](spec/04-assets.md)；工具：`docs/spec/tools/`

### 資產容器 ARC2 [確認]

- 標頭 `ARC2` magic、checksum（位元組和）、長度、檔案數；每個條目 `{flags, offset, aligned_len, real_len}`。
- 內建封包 flash 0x111000，3 個檔：資料包（23 張表）、sprite 包（2284 張）、字串包（9 語言）。
- 資產 ID 16-bit：bit15 = 0 內建；bit15 = 1 是 DLC（bits 7–14 bank、0–6 index）。

### Sprite [確認]

- 格式同 GMMan `sprites.md`。本版全部 XOR 0x53 加密、bytewise RLE、2/4/8 bpp 調色盤。
- `extract_sprites.py` 從原始 dump 解出的 2284 張與 Paramix 匯出逐像素一致。
- 繪圖由 PRAM 軟體 blitter 完成：相機偏移、16.16 縮放、alpha 0–0x40、水平與垂直翻轉、調色盤混色特效。
- **角色由部件組成**：每個角色 6 張 sprite（大小各一組：身體、眼睛、嘴巴），依動畫 keyframe 疊合（`chara_draw_parts` 0x100075c4）。角色模型（資料表 18）每個 ChrID 有 54 個動畫，anim 1 是 idle 姿勢；keyframe 格式見 04-assets C6/C7。身體顏色來自 sprite 206 的 16 組調色盤（第 n 組 = 顏色 n）[確認]。

### 文字 [確認]

- 9 語言（ja、en、fr、de、pt、es、it、zh、ko）× 824 字串，存成 u16 glyph code，不是 Unicode。
- code 1–0x1E0：假名、全形、Latin；≥0x1E1：`cjk_charset[code - 0x1E1]`。0xF000 換行、0xF001 換頁，其他為控制與插入碼。
- 字型：sprite 17（2830 字）、sprite 18（9642 字，含漢字）。
- 已抽成 `docs/spec/data/strings.csv`。歐語重音字母要比對 sprite 17 的字形才能還原。

### 聲音 [確認]

- 87 首旋律（表 0x100a0a28），每個音符 `{steps, period_us, duty}`，steps = 0 結束。
- 1 step = (divider + 1) ms，預設 9 ms。音量 = PWM duty × 音量 × 主音量（0/30/60/100%）。
- `snd_play_vol(id, vol)`(0x10024c78)。已抽成 `docs/spec/data/sounds.csv`，可輸出方波 WAV。

---

## 9. 存檔

細節：[03-save.md](spec/03-save.md)；工具：`docs/spec/tools/decode_save.py`

### 格式 [確認]

- 兩個 4 KiB slot：主 0xefe000、備份 0xeff000。內容 `u16 crc, u16 ~crc, image[0xffc]`，image 原樣 memcpy 到 0x18000be8。
- CRC-16/ARC（poly 0xA001 反射、init 0、xorout 0），硬體 CRC 引擎計算。本 dump 兩個 slot 都是 0xb522。
- 寫入：兩個 slot 依序 erase、program、讀回驗證。`save_commit(force)`：CRC 有變或 force 才寫。
- 載入：slot0 有效就載入，slot1 有效再覆蓋；一邊壞就用好的那邊；兩邊都壞就初始化預設值。

### 存檔時機 [確認]

進入旗標 0x10 的場景、進 DPD 前、每次喚醒處理完、換日、設定時鐘或語言、進化或出生、通訊註冊。**沒有定時自動存檔**。

### Flash 使用者區（本版）[確認]

| 區域 | 位址 |
| --- | --- |
| 版本資訊（ROM type 6、ROM 3.01） | 0xd29000 |
| DLC 下載暫存 | 0xd2a000 |
| DLC 槽 40 × 16 KiB | 0xd2e000 |
| Ghost 30 × 16 KiB（0 自己、1–24 放牧、25–29 死亡） | 0xdce000 |
| 接收 ghost 暫存 | 0xe46000 |
| 匯出 / patch 暫存 | 0xe66000 |
| 好友截圖 12 × 2 | 0xe86000 |
| 存檔主 / 備份 | 0xefe000 / 0xeff000 |

與 GMMan 的 Land 版文件相比，版本資訊、DLC、ghost 往前移了 0x20000。

### .tamasave（Paramix 備份格式）[確認]

0xc0 bytes 標頭 + 470 個 `{page, 0x1000, data}` chunk（涵蓋 flash 0xd2a000–0xefffff）+ `PMXE` 結尾。內容與 flash dump 逐位元組相同。

### 本機存檔內容（2026-10-19 dump 時）

青年 3071 toddle young（階段 4）；飢餓 0、心情 1、失誤 12、感冒 1、大便 1；遊玩 40 天、8070 點、planet rank 9、英文；10 個道具、5 隻放牧、3 個死亡 ghost。

---

## 10. 重製設計對照

這一節把原廠行為對應到 [重製計畫](../Tamagotchi%20Paradise%20Rust%20重製計畫.md) 的架構。標「建議」的是設計選擇，不是韌體事實。

### `tama-core` 的 tick

| 原廠 | 重製 |
| --- | --- |
| 寵物模擬每 RTC 秒一次，`care_tick(t, dt≤60)` | core 以 **1 秒**為模擬步長；`step(dt)` 內部切成 ≤60 秒段落，與原廠相同 |
| 畫面與輸入 20 fps | 平台層 20 Hz frame tick，處理輸入、動畫、場景 |
| 離線補算上限 24 小時，超過丟棄 | 建議保留，這是遊戲平衡的一部分；做成可設定的常數 |
| 換日只呼叫一次 `care_day_change` | 照做 |
| 時鐘倒退不補算 | 照做 |

目前 M0 的 `TICK_MS = 50`（20 Hz）與 frame tick 一致；寵物模擬應另外用秒計時，不要綁在 frame 上。

### 確定性與亂數

- 原廠每 frame 呼叫一次 `rand()`，所以亂數結果取決於 frame 數，無法逐位元重現。重製不需要逐位元一致。
- 建議：core 內放一個帶種子的 PRNG，存進存檔，保留確定性與重播能力。
- 用到亂數的地方只有：失誤後生病判定、出生時顏色與第二 ID 繼承、事件觸發、配對、天氣、特殊道具效果、一個 arcade 遊戲。**進化不用亂數**。

### 資料驅動

以下資料直接從 dump 抽出，轉成 RON/JSON，作為 core 的資料檔：

| 資料 | 來源 | 已抽出 |
| --- | --- | --- |
| 角色定義（189 筆） | flash 0x129714 | `characters.csv` |
| 進化表（幼年、青年、成年、稀有） | XIP 0x10091314、0x1009132c、常數 | 02-evolution §2.6 |
| 階段年齡門檻 | XIP 0x100913ac | `{0, 60, 3600, 86400, 86400, 1209600}` |
| 道具（343 筆） | flash 0x125972 | `items.csv` |
| 商店代碼（115 組） | | `shopcodes.csv` |
| 場景（137 個） | XIP 0x10094830 | `scenes.csv` |
| 字串（9 語言 × 824） | flash 0xc02e24 | `strings.csv` |
| 旋律（87 首） | XIP 0x100a0a28 | `sounds.csv` |
| Mission 目標、事件機率、天氣表 | XIP | 05-content |

寫死在程式裡、建議改成資料的常數：900 秒呼叫逾時、20 秒便意、3600 秒大便老化、36 小時死亡倒數、3540/60 秒死亡流程、作息時刻表、感冒 5 次致死、失誤 6 倍數必病。

### 狀態結構

建議 core 用語意化的 Rust struct，不照搬原廠 byte layout。存檔用 serde + 版本號。需要與實機互通時，另寫一個轉換器讀寫 0xffc 映像（`decode_save.py` 已有完整欄位對照）。

### 素材管線

`tama-extract` 可直接移植 `docs/spec/tools/` 的三個 Python 解碼器（sprite、字串、聲音），輸出 PNG、文字表、旋律資料。角色繪製需要部件疊合與動畫 keyframe 表，這部分還沒完全解出（見 §11）。

### 實作順序建議

1. 時間模擬 + 飢餓/心情 + 大便 + 作息 + 死亡（§4–5），用 monitor 指令在實機比對。
2. 成長與進化（§6），資料完全已知。
3. 存檔與離線補算（§4、§9）。
4. 場景系統與 HOME 縮放層級（§2、§7）。
5. 素材：靜態 sprite 先行，角色部件動畫後補。
6. 道具、商店、點數、mission、事件。
7. 小遊戲、社交（社交可先略過，或只做離線媒合）。

---

## 11. 未解問題

| # | 問題 | 影響 | 怎麼查 |
| --- | --- | --- | --- |
| 1 | 實際幀率（推定 20 fps） | 動畫與輸入手感 | 示波器量 TP6（TE 腳），或 monitor 讀 ms 計數 |
| 2 | 角色動畫：54 個動畫編號各自的用途（睡覺、吃飯、開心…）、動畫如何隨時間換幀、部件型別 16/17 的顯示條件（idle 姿勢已解出） | 角色動作 | `chara_draw_parts` 0x100075c4 的呼叫者 |
| 3 | ~~cond = 1 的意義~~ **已解 [確認]**：託給保母（HELP_SITTER `FUN_1004d304` 設 cond=1，300 點）；19:00 結束並 gokigen −5，或在保母畫面按鍵 → END TAMA SITTING? 提前結束 | 小 | 01-care §5、08-care-ui §2.18 |
| 4 | ~~玩家如何回應 EVOL_SUPPORT~~ **已解 [確認]**：細胞層（HOME 狀態 0x1a）任意鍵 → EVENT_EVOL_SUPPORT 0x4b（4 個 dominant 區域 icon 逐一吸收）→ EVENT_EVOLUTION 0x4c `req_done(0x13)` | 嬰兒期進化 | 02-evolution §2.4、08-care-ui §2.17 |
| 5 | ~~Gender 欄位的用途~~ 已解：def+0x10 是旗標，bit0 可吃別人、bit1 可被吃、bit2 不可結婚（`FUN_1008b53c` HOW PLAY 5/6 與結婚判斷）[確認]；第二 ID（`g_tama_chara_id2` 0x18000cfe）只在產生 Lab QR 時讀取（`FUN_10017f44`）：第 1 代 = 自己的 TamaID，之後的世代 = 60000（或配對時 50% 取伴侶），用來讓手機 App 判斷外觀來源；遊戲本身不讀 [確認] | 外觀、配對 | — |
| 6 | ~~急救、臨終畫面的觸發與互動~~ **已解 [確認]**：急救 0x54 = 寵物層狀態 0x19 任意鍵；臨終 0x49 / 墓碑 0x4a 由 HOME 路由自動進入；大往生的前代會救回臨終寵物一次；墓碑頁按住 C + A → NEW EGG。tween 曲線與急救暫停逾時秒數 [未解] | 死亡流程 UI | 08-care-ui §2.13–2.16 |
| 7 | ~~文字控制碼、歐語重音字形~~ **已解 [確認]**（`FUN_10064xxx` 文字繪製 0x10064540 一帶、`FUN_100647c8` 展開）：0xF000 換行、0xF001 結束／換頁、0xF002–0xF00B 與 0xF016–0xF017 由呼叫端代入（資料中只出現 0xF003 = 星球名）、0xF00C–0xF015 切換文字調色盤（資料中未使用）。重音字形 400–429 = ÀÁÂÃÄÆÇÉÈÊËÎÏÓÒÔÕÖŒÙÚÛÜŸÍÌÑ¿¡º（依字型像素判讀，`tools/extract_strings.py` 已更正）；431 起是方塊與圖示。重製版新遊戲加入 LANGUAGE（9 種語言） | 在地化 | — |
| 8 | ~~天氣值 0/1 的畫面、轉盤縮放~~ **已解**：0 = 晴、1 = 陰（場地背景的另一組圖）、2 = 雨／雪粒子；轉盤 = HOME 連續縮放（08-care-ui §2.2.1），重製版已實作並在瀏覽器確認 | UI | — |
| 9 | Lab QR payload（`FUN_10017f44` 0x10017f44，呼叫者 0x10042548 一帶）：`"%s%s%s"` = device_rand(0x18000c54)、QRUID1(0x1800168c，6 碼)、QRUID2(0x18001690，7 碼) 的 36 進位字串，再接 romtype(2 碼)、TamaID、第二 ID(4 碼)、顏色(1，0x1f→16)、planet rank(1)、朋友數(1)、亂數 % 0xb640(3)，全部 36 進位；之後用 flash 資產 1–3 的金鑰算 64 B 驗證值（`FUN_10069de0`）一起編成 QR [確認]。驗證演算法與金鑰用途未解，且需要 Tamagotchi Lab App 才能驗證——屬手機 App 互通，不是遊戲內容 | Lab App 互通 | `FUN_10069de0`、資產 1–3 |
| 10 | ~~多數音效 ID 的觸發場景~~ **已解 [確認]**：全部呼叫點對照見 [11-sounds.md](spec/11-sounds.md) 與 `data/sound_usage.csv`；87 個中 55 個有使用，其餘無呼叫點 | 音效 | — |
| 11 | ~~每日 +50 點屬於哪個畫面~~ **已解 [確認]**：LABOMENU→BOOK→MICROSCOPE→LAB CODE 輸入「NO DATA」碼（碼表 0x100a1660 的 29–59 筆）時每天一次 | 小 | 05-content §2.2、§2.4a |

規則類問題可在實機用 UART monitor 驗證（需接 UART0：RX 在 TP48，TX 為 P0.0 無測試點）。

---

## 12. 檔案與工具索引

| 路徑 | 內容 |
| --- | --- |
| `docs/re-notes.md` | 研究日誌：dump 步驟、位址對照、monitor 指令、狀態結構推導 |
| `docs/spec/01-care.md` | 照顧與時間模擬（含完整請求 ID 表） |
| `docs/spec/02-evolution.md` | 角色表、成長、進化樹、出生 |
| `docs/spec/03-save.md` | 存檔、RTC、DPD、flash 配置、.tamasave |
| `docs/spec/04-assets.md` | LCD、ARC2、sprite、文字、聲音、角色組成 |
| `docs/spec/05-content.md` | 場景、道具、商店、點數、世界、mission、事件、小遊戲、社交 |
| `docs/spec/06-system.md` | 開機、主迴圈、輸入、場景機、電源、MMIO、工廠模式 |
| `docs/spec/data/*.csv` | 抽出的資料表 |
| `docs/spec/tools/*.py` | sprite、字串、聲音、存檔解碼器（只用 stdlib，`python3 -I` 執行） |
| `re/sonix-decrypt/` | PRAM 解密與 device key 搜尋（Rust） |
| `re/scripts/*.java` | Ghidra headless 腳本（匯入、命名、反編譯匯出） |
| `re/names.txt` | 已確認的函式與變數命名 |
| `re/decomp-all/` | 全部反編譯 C（重新產生：見 re-notes） |

各子系統文件第 4 節列有數百個新命名，尚未套進 `re/names.txt`。
