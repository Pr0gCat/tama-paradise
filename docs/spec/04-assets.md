# 04 — 資產、顯示與聲音（assets / display / sound）

對象：White Glacier dump `dumps/paradise-glacier-20261019-113049.bin`。所有位址：XIP vaddr（0x10xxxxxx）、PRAM（0x0000xxxx）、SRAM（0x18xxxxxx）、flash offset（0x00xxxxxx，對應 vaddr 0x60xxxxxx）。

工具（stdlib only，皆已在本 dump 上跑過）：

| 檔案 | 用途 | 驗證 |
| --- | --- | --- |
| `docs/spec/tools/extract_sprites.py` | 從 raw flash 解碼 sprite → PNG；`--check` 與 Paramix 匯出逐像素比對 | **2284/2284 張與 `*_sprites.zip` 完全一致** |
| `docs/spec/tools/extract_strings.py` | 9 語言字串表 → `docs/spec/data/strings.csv` | 內建 assert（ko `정보`、zh `信息`） |
| `docs/spec/tools/extract_sounds.py` | 87 首蜂鳴器旋律 → `docs/spec/data/sounds.csv`（可選 WAV） | 87 首全部解析，終止符正確 |

---

## 1. 摘要

- **LCD**：128×128、RGB565，單一 32 KiB framebuffer 在 SRAM `0x1801438a`。每幀：清 FB → 場景邏輯畫 sprite（軟體 blitter，PRAM）→ 等 LCD TE 腳（GPIO 0x1a）累計 ≥3 次 → 以 SPI0（`0x4000e000`）+ DMA（`0x4000f000`）把整張 FB 以 16-bit 推出（DCS `0x2C`）。**無 double buffering**：下一幀開始前輪詢 DMA 完成。
- **資產**：flash `0x111000` 是一個 `ARC2` 封包（校驗和 OK），內含 3 個檔：data pack（23 張表）、sprite pack（2284 個 sprite）、strings（另一個 ARC2，9 語言 × 824 字串）。DLC 以同格式存在 `0xd2e000 + slot*0x4000`（40 槽）。
- **資產 ID**：16-bit。bit15=0 → 內建包（bank 0），index = id；bit15=1 → DLC，bits 7–14 = 虛擬 bank V，bits 0–6 = 槽內 index（0–127）。
- **Sprite 格式**：與 GMMan `sprites.md` 完全相符。本 dump 全部是 2/4/8bpp 調色盤、全部 XOR 0x53 加密、絕大多數 bytewise RLE；無 wordwise、無 16bpp。
- **文字**：字串為 u16 glyph code 陣列（非 Unicode）。code 1–0x1E0 = 日文假名/全形/Latin（font 表帶 SJIS/ASCII 對照），≥0x1E1 = `cjk_charset[code-0x1E1]`（韓文 2350 + 注音 + 漢字）。0xF000 換行、0xF001 換頁、0xF002–0xF00B / 0xF016–0xF017 為控制/插入碼。
- **聲音**：**不是 PCM、也不是 Sonix Audio32**。是單音 PWM 蜂鳴器旋律：87 首（id 0–0x56），每音符 `{steps, period_us, duty}`，由 1 kHz SysTick 推進。音量 = PWM duty。
- **角色組合**：每個角色 6 張 sprite（大 body/eyes/mouth、小 body/eyes/mouth），執行時把多個部件（body、eyes、mouth、道具）用動畫 keyframe 疊合；eyes 可來自另一角色（基因/配對），顏色由 16 組調色盤之一決定。

---

## 2. 詳細規則

### 2.1 LCD / 顯示

| # | 規則 | 證據 | 標記 |
| --- | --- | --- | --- |
| D1 | 解析度 128×128。blitter clip 預設 `0..0x80`，FB row stride `0x100` bytes（=128 px × 2） | `FUN_00008e3c`（PRAM）：`local_3c=local_44=0x80`、`DAT_1801c7f8 + y*0x100 + x*2` | [確認] |
| D2 | 色深 RGB565（16 bpp）。清畫面 `0x8000` bytes = 128×128×2；填色 `0x2000` words（每 word 兩像素） | `FUN_1007bc50` memset `0x8000`；`FUN_1007bc68` 填 0x2000 u32；RGB888→565 `FUN_1007bd58` | [確認] |
| D3 | FB 位址 `0x1801438a`（MOVW/MOVT 解出，2-byte 對齊），指標存於 `DAT_1801c7f8` | `FUN_1007b5f0` @0x1007b604/0x1007b60c | [確認] |
| D4 | LCD 介面：SPI0（regs `0x4000e000`），D/C 腳 = GPIO 5（0=命令,1=資料，`thunk_FUN_0000291c(5,1,2,v)`）。命令 8-bit、像素 16-bit（`DAT_1801cabd+1` = 8 或 0x10 後重設 SPI） | `FUN_1001f308`、`FUN_1001f37c`、PRAM `FUN_00003b84`、`FUN_00003cc0` | [確認] |
| D5 | 控制器指令：`0x2A` CASET（x+2）、`0x2B` RASET（y+1）、`0x2C` RAMWR、`0x28` DISPOFF、`0x10` SLPIN（中間 delay 20000）。偏移 (2,1) 屬 ST7735S 類 1.44" 128×128 面板 | `FUN_1001f3f4`（set window）、`FUN_1001f26c`（off+sleep） | [確認]（型號 [推測]） |
| D6 | 像素以 DMA 推送：DMA ctrl `0x4000f000`、channel `0x4000f100/0x4000f120`，來源 = FB，目的 = SPI0 data reg（`base+0x1c`），長度 = (x1-x0+1)*(y1-y0+1) = 16384 個 16-bit 傳輸 | PRAM `FUN_00003fc8`；呼叫 `FUN_1001f308(0,0,0x7f,0x7f,fb)` 經 `FUN_1007b7ec` | [確認] |
| D7 | 主迴圈（`FUN_1006b0b4`）每幀順序：清 debug 文字佇列 `FUN_1007bee0` → 清 FB `FUN_1007bc50` → 場景 `FUN_1006b200` → debug 文字 `FUN_1007bef0` → `FUN_1007b518` → 淡入淡出全屏填色 `FUN_1007bca0` → … → 等 TE → 推 FB `FUN_1007b7ec` → 輪詢 DMA 完成 `FUN_1007b7d8` | `FUN_1006b0b4` | [確認] |
| D8 | **幀率**：每次 TE 上升沿（GPIO 0x1a，GPIO1 bit10 中斷，vector 43 = PRAM `FUN_00002558`）`DAT_1801c3a4++`；主迴圈在 `DAT_1801c3a4 < 3` 時持續等 TE（除非 `DAT_18001cbc` bit1），之後歸零。⇒ 每幀 = 3 個 TE 週期。`DAT_1801c390 = 80 / count` 只寫不讀（debug 用）。**統一結論：名目 20 fps**（見 06-system §2.3：600 frames ≈ 30 s、面板 FRMCTR ≈ 60 Hz ÷ 3；小遊戲亦以 20 fps 設計） | `FUN_1006b0b4`、`FUN_1007b808`、`FUN_00002558` | [確認]（20 fps 為 [推測，強]） |
| D9 | 無 double buffer：同一 FB 被 DMA 讀出時不重畫；下一幀前 `do{}while(FUN_1007b7d8())` 等 SPI 狀態回 1 | `FUN_1006b0b4` 尾端、`FUN_1001f6f0` | [確認] |
| D10 | 全域相機偏移：`DAT_1801c3cc/DAT_1801c3d0` 的高 16 bit 加到 sprite 座標（sprite 旗標 bit7 可忽略相機）；縮放預設 `0x10000`（16.16） | `FUN_00008e3c`、`FUN_1007b5f0/FUN_1007bd18` | [確認] |
| D11 | Sprite alpha：0–0x40（0x40 = 不透明），<0x40 時逐通道 `dst + (src-dst)*a>>6` 混色 | `FUN_00008e3c` 16bpp 路徑 | [確認] |
| D12 | 翻轉：instance flag bit0 = 水平翻轉、bit1 = 垂直翻轉；anchor 以 sprite 中心 + `offset_x/offset_y` | `FUN_00008e3c` | [確認] |
| D13 | 調色盤特效：palette-RAM 模式（`DAT_1800eda8&1`，去重快取）或逐 sprite 混色：模式 1/2 在兩組 palette 間插值（係數 0–32）、模式 3 加色（飽和至 31/63/31） | PRAM `FUN_0000a408` | [確認] |
| D14 | 全螢幕淡色：`FUN_1007b3b0(?, color, ?, speed, target)` 設 `DAT_1801c3be/c3c0`，每幀 ±0x40 步進，顏色表 `0x10093522`（u16 RGB565） | `FUN_1007b3b0`、`FUN_1007bca0`、`FUN_1007bd8c` | [確認] |

### 2.2 資產容器（ARC2）

| # | 規則 | 證據 | 標記 |
| --- | --- | --- | --- |
| A1 | 頭 16 bytes：`magic 'ARC2'(0x32435241)`、`checksum`、`length`（不含頭）、`num_files`；其後 `num_files × {flags, offset(相對頭), aligned_len, real_len}`。checksum = 從 offset 8 起 `length+8` 個 byte 之和 | `FUN_1000299c`（`arc_validate`，第二參數非 0 才算 checksum） | [確認] |
| A2 | 取檔：`FUN_100028e4(arc,i)` 回 entry 指標（越界即 `FUN_10068388` 斷言）；`FUN_10002954(arc,i)` 回 `arc + entry.offset`（offset 0 → NULL） | 同上 | [確認] |
| A3 | 內建資產：root `0x111000`，len `0xb22b94`，3 檔，checksum `0x4f5ded07`（實算相符）。資產區結束 `0xc33ba4` | `arc.py` 實測；`FUN_10003160` 以 `&DAT_60111000` 呼叫 | [確認] |
| A4 | root[0] data pack @`0x111040`（0xb723a B）、root[1] sprite pack @`0x1c827c`（0xa3aba8 B）、root[2] strings ARC2 @`0xc02e24`（0x30d80 B，9 檔） | `FUN_10003160`（`Asset_Init`）依序註冊到 `FUN_10023f8c`/`FUN_100323dc`/`FUN_10065f6c` | [確認] |
| A5 | Data pack = 23 個 u32「半字（u16）偏移」接資料；表 i 位於 `pack + off[i]*2`。註冊時把 23 個絕對指標存到 `0x1800ded4 + bank*0x5c + i*4` | `FUN_10023f8c` | [確認] |
| A6 | Bank 表：最多 41 個 bank（0 = 內建，1..40 = DLC）。`DAT_18000b96[k*2]` = 虛擬 bank V（DLC 的 item ID），`DAT_18000b97[k*2]` = bit7 有效 + 實槽 R；`DAT_18000b94` = 已註冊數 | `FUN_1000353c`（"Regist Bank V:%d R:%d"）、`FUN_100035bc`、`FUN_10003668` | [確認] |
| A7 | 資產 ID 解析 `FUN_1002ba44`：bit15=0 → bank 0，index 不變；bit15=1 → V=(id>>7)&0xff，index=id&0x7f，再查 bank 表得 R | `FUN_1002ba44`、`FUN_1002baa4`、`FUN_10023ef4` | [確認] |
| A8 | DLC：40 槽，各 0x4000（16 KiB），flash `0xd2e000 + slot*0x4000`（= `0x60d2e000`）；暫存區 `0xd2a000`。槽有效位元圖 `DAT_18001694`。啟動時 `Asset_InitAdditional`（`FUN_10003238`）逐槽 validate（含 checksum）後註冊 data/sprites/strings；DLC 的 V = data 表 19 的第一個 s16（`FUN_10023ea4`） | `FUN_10003238`、`FUN_10003384`、`FUN_10000b88` | [確認]（與 GMMan 的 0xd4e000 不同，Glacier 位移 -0x20000） |
| A9 | 從 flash 讀超過 `0x60000000` 的資料時不走 memory-map，而是關中斷呼叫 SPI flash 讀取（PRAM `FUN_00003754`）複製到 SRAM 緩衝（`DAT_18001d18`, 0x4000 B） | `FUN_1007b828`、`FUN_1007b97c` 等 | [確認] |

### 2.3 Sprite 格式（已以解碼器驗證）

Sprite pack：u32 偏移表（數量 = 第一個偏移/4 = **2284**），偏移相對 pack 起點。取得：`FUN_1007bd48(pack, idx)` = `pack + offs[idx]`；sprite instance 初始化 `FUN_100322e8(inst, id, reset)`（id 經 A7 解析，-1 → 0 且標記隱藏）。

24-byte header（LE）：

| off | 型別 | 欄位 | 本 dump 觀察 |
| --- | --- | --- | --- |
| 0 | u32 | data_length | |
| 4 | u8 | flags：bit0/1 恆 1；bit2 透明；bit5 bytewise RLE；bit6 wordwise RLE；bit7 加密 | 0xa7×2006、0xa3×222、0x87×49、0x83×7（**全部加密**、無 wordwise） |
| 5 | u8 | bpp：0=1bpp,1=2bpp,2=4bpp,3=8bpp,0x10–0x12 直接色 | 1:89、2:2123、3:72（無 1bpp、無直接色） |
| 6 | u16 | num_sprites（cell 數） | 最大 9642（CJK 字型） |
| 8 | u8×2 | cell 寬、高 | 8–128 |
| 10 | s8×2 | offset_x（右正）、offset_y（**上正**）錨點 | |
| 12 | u8×2 | 每幀 cell 寬數、高數（幀 = iw×ih 個 cell 拼成） | |
| 14 | u8 | palette_bpp（恆 0x11） | |
| 15 | u8 | num_palette_sets | 1–20 |
| 16 | u16 | 透明色 index | 2283 個為 0；sprite 37 為 1 |
| 18 | u16 | palette_offset | 24 |
| 20 | u16 | pixel_data_offset | |
| 22 | u16 | padding | |

規則：
- S1 [確認] 幀數 = num_sprites / (iw×ih)；cell 依左→右、上→下組成一幀，幀依序存放（`FUN_00008e3c` 以 `frame * iw * ih` 起算 cell）。
- S2 [確認] Palette：每組 `2^bits` 個 RGB565 u16；選用第 `palette_set` 組（blit 參數 `param_5`）。
- S3 [確認] 像素為 LSB-first bitstream、無 stride。
- S4 [確認] 壓縮時 `pixel_data_offset` 處為 `num_sprites × {u32 offset, u32 len}`；offset bit31=1 表示該 cell 未壓縮。Bytewise RLE：控制 byte 0=結束；bit7=1 → 後面 `c&0x7f` 個字面值；否則下一 byte 重複 c 次。**加密 = 每個資料 byte（含控制 byte）XOR 0x53**（`FUN_1007b9f4` 解密+解壓同時進行；key 由 `FUN_1007bd70` 回傳 `0x53535353` 存於 `DAT_18005d18`）。
- S5 [確認] 解碼分派在 PRAM blit 前置（`caseD_2` 附近 ram.c:8180）：flags&0x60==0 → 直接取 `cell * w*h*bits/8`，加密時 `FUN_1007b388` 原地 XOR；bytewise → `FUN_1007b9f4`（加密）/`FUN_1007b97c`；wordwise → `FUN_1007b8dc`（加密）/`FUN_1007b858`。解碼到 SRAM `DAT_18010074`。
- S6 [確認] 每 bpp 一個 blitter：跳表 PRAM `0x91f4` → `FUN_000059c4/0000612c/000068a4/0000701c`；16bpp 路徑內嵌於 `FUN_00008e3c`。
- S7 [確認] Paramix 的 PNG 匯出 = 所有幀左→右排列、每列寬度上限 1280 px 換行；RGB565→888 用 `c*255//31`（綠 `//63`）取整。我們的解碼器以同規則輸出，2284/2284 逐像素相符。

### 2.4 字型與文字

| # | 規則 | 證據 | 標記 |
| --- | --- | --- | --- |
| T1 | 字型表 = data pack 表 0（u16 索引，5 個字型）+ 表 1（字型資料）。取字型 `FUN_10022f70(id)` | `FUN_10022f70` | [確認] |
| T2 | 字型記錄：`u16 sprite_id, u16 advance_base, u16 line_height, s16 glyph_count-1`，接 `glyph_count` 個 `{u16 char_code, s16 advance_adj}`（code c 在 `font + 4 + c*4`）。glyph 圖 = sprite 的第 c 幀 | `FUN_10064d90`（讀 `puVar2[0..2]`）、`FUN_10064900`（`font + c*4 + 6`） | [確認] |
| T3 | 5 個字型：#0 sprite 0（8×8，96 字，debug ASCII，16 組 palette）；#1 sprite 17（16×16 cell，2830 字：假名/Latin/韓文）；#2 sprite 18（9642 字：再加漢字）；#3 sprite 19（16 字，數字）；#4 sprite 2060（32×32，7 字，大數字） | `font.py` 實測 @0x1110a6/0x11122e/0x113e6e/0x11d51e/0x11d566 | [確認] |
| T4 | 字寬 = advance_base + 字距 + adj；行首字只算一半寬（置中排版用），對齊模式由 text box `+0x12` 位元選（左/中/右、上/中/下） | `FUN_10064900`、`FUN_10064ac4`、`FUN_10064be4` | [確認] |
| T5 | Glyph code → 字元：1–0x1E0 用 font #1 的 char_code（SJIS 或 ASCII；Latin 帶重音字只記底字母）；≥0x1E1 → `cjk_charset.txt[code-0x1E1]`（GMMan，2350 韓文 → 注音 → ㈠… → 漢字） | `extract_strings.py` 自檢；韓/中解碼結果語意正確（例：「연구 기록 / 研究记录」=LAB LOG） | [確認] |
| T6 | 控制碼：0xF000 換行、0xF001 換頁（`FUN_10064a28` 以 0xF001 計頁）、0xF002–0xF00B 與 0xF016–0xF017 為插入碼（用 callback 替換，`FUN_10064cd8`）、其餘 0xF002–0xF015 為不佔寬的格式碼 | `FUN_10064cd8`、`FUN_10064be4` | [確認]（各插入碼語意 [未知]） |
| T7 | 字串表：strings ARC2 有 9 檔 = 語言 1..9：ja,en,fr,de,pt,es,it,zh,ko（名稱表 `0x10091774`，9×12 bytes）。每表 `u16 count(=824)`、`u16 offset[count+1]`（半字、相對表頭，最後一個 = 結尾），字串以 0 結尾 | `FUN_10065ea0`、`FUN_10073010` | [確認] |
| T8 | 取字串 `FUN_10065ea0(id, lang)`：id 經 A7 解析 bank → `DAT_1800efcc[bank]` → 第 `lang-1` 個表；`FUN_10065e80(id)` 用目前語言 `DAT_18001d16`（1..9，`FUN_10065f34` 設定，越界斷言） | 同上 | [確認] |
| T9 | 語言存於設定 `DAT_18000c6a` bits 9–12（值 = 語言 index+1） | `FUN_10073010`（UART `language n`） | [確認] |
| T10 | Debug 文字（UART/測試畫面用）：`FUN_1007bdc8/FUN_1007bf74` 用 XIP 內建 8×8 sprite `0x1009df4c`，ASCII `c-0x20` → 幀；`E3 xx xx`（UTF-8 假名）另行對應；最多 256 字/幀佇列 `0x1801c3dc` | `FUN_1007bdc8`、`FUN_1007bef0` | [確認] |

### 2.5 聲音

| # | 規則 | 證據 | 標記 |
| --- | --- | --- | --- |
| SND1 | 硬體：單聲道 PWM 方波（timer 通道 7，設定結構 `0x1801c904 + 7*0x34`），**無 PCM**。GMMan 的 aud32-decoder 是 SNC7320 用的 Audio32（G.722.1）解碼器，與本機無關 | `FUN_10090a80`、`FUN_10028d38/FUN_10028d98`、PRAM `FUN_00002ff4` | [確認] |
| SND2 | 音效表 `0x100a0a28`：0x57 = 87 個 u32 指標（`FUN_10018180` 回 0x57） | `FUN_10024c78` | [確認] |
| SND3 | 旋律 = `{u16 steps, u16 period_us, u16 duty}` 序列，steps=0 結束；period=0 為休止。PWM reload = `clk/1e6 * period_us`（故 period 單位 µs，例：955 → 1047 Hz = C6） | `FUN_10090a80`、PRAM `FUN_00002ff4` | [確認] |
| SND4 | 一個 step = (divider+1) 個 SysTick（1 ms）。divider 由 `FUN_1007ace0(id)` 查表 `0x10091b00`：{78:13, 79:18, 80:12, 82:11, 85:29}，其餘 8（→ 9 ms/step），上限 0x20 | `FUN_10090c00`、`FUN_1007ace0`；SysTick reload = clk/1000（`FUN_1006ac00` → `FUN_100603ec`），vector 15 = PRAM `FUN_0000432c` | [確認] |
| SND5 | 音量：duty% = note.duty × vol/100 × master/100（PWM 比較值 = (100-duty)*(reload/100)）。本 dump 所有音符 duty=10。vol 0–100（API 參數），master `DAT_18000704` 由設定 0/1/2/3 → 0/30/60/100 | `FUN_10090a80`（double 運算）、`FUN_10030bb4`、`FUN_10030b74` | [確認]（double 運算順序細節 [推測]） |
| SND6 | API：`FUN_10024c60(id)` = 播放 vol 100；`FUN_10024c78(id,vol)` 播放（先停前一首，單音、無混音）；`FUN_10024e08` = 循環播放；`FUN_10024e30/FUN_10024f80(id,vol,div)` 指定 divider（+循環）；`FUN_10024fd8` 停止；`FUN_10024db4` 回目前 id（未播 0xffff）；`FUN_10024c4c` 是否靜止；`FUN_10024de4(vol)` = 播 0x56 並凍結（1 kHz 測試音） | 各函式 | [確認] |
| SND7 | 輸出致能：`FUN_10024fb0(b)` 設 `DAT_1800ed9c.b2` bit0，播放時寫入通道 config byte +0x18 = (~b&1)<<2（4 = 通道關閉）。場景切換時 `FUN_1006b200` 呼叫 (1)…(0) | `FUN_10024fb0`、`FUN_10024c78` | [確認]（行為意義 [推測]） |
| SND8 | 常用 ID：1 = 決定（按鈕確認，57 處）、2 = 取消/返回、0 = 游標移動、0x0b = 不可選（錯誤嗶）、0x56 = 1 kHz 測試音。長曲（62–72、78–85，最長 #84 約 36 s）為音樂/BGM；#80 以 divider 12 循環播放（`FUN_10024f60(0x50,0xc)`） | 呼叫點統計（XIP.c 29359/29364、30837/30841 等） | ID 0/2 語意 [推測]，1/0xb/0x56 [確認] |
| SND9 | Sound test 畫面（`FUN_10033b48`）：id 0–0x56、vol 0–100，模式 0/1=播放、2=1 kHz、3=循環 | `FUN_10033b48`、`FUN_1000d10c` | [確認] |

### 2.6 角色組合

| # | 規則 | 證據 | 標記 |
| --- | --- | --- | --- |
| C1 | 每角色 6 張連續 sprite：+0 body 64×64×20 幀、+1 eyes 64×32×20、+2 mouth 64×32×18、+3 小 body 32×32×24、+4 小 eyes 32×16×14、+5 小 mouth 32×16×14（2bpp，1 palette） | sprites 268–273（圖片目視確認） | [確認] |
| C2 | Paramix `figuren` 的編號 = body sprite index（207、268、274…1066，步距 6，共 135 = `tamaid` 的 135 個角色） | zip 檔名 vs sprite header | [確認] |
| C3 | sprite 206 = 64×64 4bpp、**16 組 palette** → 寵物 16 色（`g_tama_color` 0–15）的調色盤來源。驗證：2113 body（sprite 304）自帶調色盤 = 206 第 14 組，其預設色 Option = 14；1001 body（268）= 第 7 組，Option = 7。蛋（207）用自己的調色盤 | sprite header、`tama-extract` | [確認] |
| C4 | 角色繪製 `FUN_100075c4`：`FUN_10022f00(chr)` 取 data 表 18 的 0xd8-byte 角色模型記錄 → `FUN_10023d64(rec, anim)` 取動畫 → 逐部件（預設 5 個，部件型別表 `{0,1,0xf,0x10,0x11}`）用 `FUN_10023af4` 解 keyframe → 以 16.16 仿射矩陣（平移 `FUN_10024bc0`、縮放 `FUN_10024b38`、旋轉 `FUN_10024a78`）組合 → `FUN_100322e8` 建 sprite instance | `FUN_100075c4` | [確認] |
| C5 | 部件型別 1（eyes）可由 `inst[5]`（≠0x7fff）指定**另一角色**的 eyes sprite（配對/基因）；型別 4/6/8 = 裝備道具 `inst[8..10]`（`FUN_10022ff0(item)+4` 的 sprite）；型別 0 = body，palette 用 `inst[2]` | `FUN_100075c4` switch | [推測]（欄位語意） |
| C6 | Keyframe（data 表 15，以表 16 的 u32 半字偏移索引）：`u16 flags, u16 [1]=x, u16 [2]=y` + 依旗標選填（`x * 0x20000000 < 0` 測的是 bit2）：bit2→[6] 幀、bit3→[7]、bit4→[4] 旋轉、bit5→[3] 縮放（預設 0x1000 = 1.0）、bit6→[5] alpha（預設 0x40 = 不透明）、bit7→[8]、bit8→[9] 翻轉、bit9→[10] 部件型別、bit10→[0] sprite | `FUN_10023af4`；`tama-extract` 解出的姿勢與畫面一致 | [確認] |
| C7 | 角色模型（data 表 18）每個 ChrID 一筆 0xd8 B = 54 個動畫 × `{i16 第一個 keyframe, i16 部件數}`。anim 0 空；**anim 1 = idle**（2113：body 304 (0,−32)、eyes 305 (1,−16)、mouth 306 (0,−16)、型別 17 sprite 221 α51 灰塵效果）；anim 2 = idle 交替幀（body 幀 1、mouth 幀 4）。座標 = sprite 中心相對角色底部中心 | `FUN_10022f00`、`FUN_10023d54`、`FUN_100075c4` | [確認]（型別 16/17 的顯示條件 [未知]） |

---

## 3. 資料表

| 名稱 | 位置（flash / vaddr） | 單筆 | 數量 | 欄位 |
| --- | --- | --- | --- | --- |
| Root ARC2 | 0x111000 | 16 + 16/檔 | 3 | 見 A1/A4 |
| Data pack 表頭 | 0x111040 | u32 半字偏移 | 23 | 表 i 起點 = 0x111040 + off[i]*2 |
| Sprite pack | 0x1c827c | u32 偏移 + 記錄 | 2284 | 見 2.3 |
| Strings ARC2 | 0xc02e24 | 9 檔 | 9×824 | 見 T7；輸出 `docs/spec/data/strings.csv` |
| 字型 | 表 0 @0x11109c、表 1 @0x1110a6 | 6 B 頭 + 4 B/字 | 5 字型 | 見 T2/T3 |
| 角色定義 | 表 11 @0x129714 | 15×u16 | 189 | TamaID, ChrID, NameID…（見 `chara_def_get` 0x10023e10；其他文件詳述） |
| 角色模型 | 表 18 @0x1bc726 | 0xd8 B | ~222（0xbb50/0xd8 非整數，尾端可能有 padding） | `FUN_10022f00` |
| 動畫 keyframe | 表 15 @0x16430a / 表 16 @0x1a0a3e（u32 索引） | 可變 | — | 見 C6 |
| 表 6 | @0x1232ce | 5×u16 | 824 | 首欄 = sprite id（多為恆等），其餘 0（用途未知） |
| 音效指標表 | XIP 0x100a0a28 | u32 | 87 | 見 SND3；輸出 `docs/spec/data/sounds.csv` |
| 音效 divider 表 | XIP 0x10091b00 | {s16 id, s16 div} | 5 | (78,13)(79,18)(80,12)(82,11)(85,29) |
| 淡色色表 | XIP 0x10093522 | u16 RGB565 | ? | `FUN_1007bd8c` |
| 語言名 | XIP 0x10091774 | char[12] | 9 | Japanese…Korean |
| Debug 字型 sprite | XIP 0x1009df4c | sprite | 1 | 8×8 ASCII |

Data pack 23 表大小（bytes）：0:0xa 1:0xc4e4 2:0xd68 3:0x1a0 4:0xcb4 5:0x4188 6:0x2030 7:0x672 8:0x2 9:0 10:0x3da2 11:0x1626 12:0x11d8 13:0x1748 14:0x36cb0 15:0x3c734 16:0x15a50 17:0x6298 18:0xbb50 19:0x2 20:0x2 21:0 22:0。已知：0/1 字型、4/5 UI 元件定義（`FUN_10023124`，8 種型別 0x0000–0x8000）、11 角色定義、15/16 動畫、18 角色模型、19 DLC item ID、21/22 由 `FUN_10022e18/e98` 讀（DLC 用）。

---

## 4. 新命名

```
1000299c arc_validate f
100028e4 arc_entry f
10002954 arc_get_file f
10003160 asset_init f
10003238 asset_init_additional f
10003384 asset_dlc_slot_valid f
1000353c asset_bank_register f
100035bc asset_bank_unregister f
10003668 asset_bank_virtual_to_real f
1002ba44 asset_id_resolve f
10023f8c datapack_register f
100323dc spritepack_register f
10065f6c strings_register f
10022f70 font_get f
10023124 ui_element_def_get f
10023af4 anim_keyframe_decode f
10022f00 chara_model_get f
100075c4 chara_draw_parts f
100322e8 sprite_inst_init f
1007bd48 spritepack_get f
1007b388 sprite_xor_decrypt f
1007b858 rle_word_decode f
1007b8dc rle_word_decode_xor f
1007b97c rle_byte_decode f
1007b9f4 rle_byte_decode_xor f
1007b828 flash_fetch_to_sram f
1007bd70 sprite_xor_key f
1007bd58 rgb888_to_565 f
1007b5f0 gfx_init f
1007bc50 fb_clear f
1007bc68 fb_fill f
1007bca0 fb_fade_fill f
1007b3b0 fade_start f
1007bdc8 dbg_text_queue f
1007bd9c dbg_printf f
1007bf74 dbg_text_draw_now f
1007bef0 dbg_text_flush f
1007b808 lcd_wait_te f
1007b7ec lcd_push_frame f
1007b7d8 lcd_busy f
1001f308 lcd_write_window f
1001f37c lcd_cmd_ramwr f
1001f3f4 lcd_set_window f
1001f26c lcd_sleep f
1006b0b4 main_loop f
1006b200 scene_run f
10065ea0 str_get_lang f
10065e80 str_get f
10065f34 str_set_lang f
10064900 text_measure_width f
10064d90 textbox_init f
10024c60 snd_play f
10024c78 snd_play_vol f
10024e08 snd_play_loop f
10024e30 snd_play_div f
10024f80 snd_play_div_loop f
10024fd8 snd_stop f
10024db4 snd_current f
10024c4c snd_idle f
10024de4 snd_test_tone f
10024fb0 snd_output_enable f
1007ace0 snd_divider_get f
10090a80 snd_sequencer_step f
10090c00 snd_tick f
10018180 snd_count f
10030bb4 snd_master_level_set f
10030b74 snd_master_set f
10033b48 scene_soundtest_update f
00008dac blit_sprite f
00008e3c blit_sprite_inst f
0000a408 blit_palette_prep f
0000432c systick_handler f
00002558 lcd_te_isr f
00003fc8 spi_dma_write f
00003cc0 spi_write f
00003b84 spi_config f
00003754 flash_read f
1801438a g_framebuffer l
1801c7f8 g_fb_ptr l
1801c3a4 g_te_count l
1801c3a8 g_systick_ms l
18005d18 g_sprite_xor_key l
18005d34 g_spritepack_by_bank l
1800efcc g_strings_by_bank l
1800ded4 g_datapack_tables l
18000b90 g_asset_root l
18001d16 g_language l
18000704 g_snd_master l
1800ed90 g_snd_melody_ptr l
18010074 g_sprite_decode_buf l
18001d18 g_flash_read_buf l
100a0a28 g_snd_table l
10091774 g_language_names l
```

---

## 5. 未解問題

1. **TE 實際頻率**（決定 fps）：程式假設 80 Hz（26.7 fps）。需實機示波或 Capybara 量 GPIO 0x1a；ST7735 初始化序列（frame rate 暫存器 0xB1）在哪裡尚未找到（可能在 PRAM 或 `FUN_1001ee44` 附近）。
2. **重音 Latin 字**：font 表只存底字母；精確的 é/ñ/ç 等需比對 sprite 17 的字形（`extract_sprites.py` 輸出 `sprite_0017.png`，code c = 第 c 幀）。`strings.csv` 目前以「字母 + U+0301」標示為帶重音。
3. **插入碼 0xF002–0xF00B/0xF016–0xF017** 的語意（寵物名、數字、玩家名…）：看 `FUN_10064cd8` 的 callback 實作者。
4. **寵物顏色**：sprite 206 的 16 組 palette 是否就是 `g_tama_color` 來源、如何替換 body palette（`inst[2]` → blit `param_5`）需追 `FUN_100322c4`/`FUN_00008dac` 的 palette 參數。
5. **角色模型（表 18，0xd8 B）與動畫（表 13–16）的完整欄位**、部件型別 0xf/0x10/0x11 的意義。
6. 表 6（824×10 B）、表 2/3/7/10/12/17 的用途（多半屬遊戲資料，見其他子系統文件）。
7. SND8 中 0 / 2 的確切語意與其餘 80 個 ID 的觸發場景：多數 ID 經由資料表間接傳入（`FUN_10024c60(puVar3[1])` 等），需追資料表。
8. `FUN_00003754` flash 讀取與 XIP 同時存取的衝突處理（為何關中斷）— 重製版不需要，但說明 DLC/ghost 資料皆以 SPI 指令讀出。
