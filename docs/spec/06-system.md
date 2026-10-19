# 06 — 系統架構、主迴圈、輸入、場景狀態機

範圍：開機流程、PRAM/XIP 分工、DPD 映像、主迴圈與時基、中斷、輸入（A/B/C + 轉盤）、場景（PSID）狀態機與選單、UART monitor、電源管理、工廠 JIG/TESTMODE、MMIO 對照。
所有位址皆可直接在 `re/decomp-all/{ram,XIP}.c` 以 `// ==== <addr>` 搜尋。

## 1. 摘要

- **沒有 RTOS**，是單執行緒 super-loop。`Reset_Handler`(PRAM 0x2f4) → `SystemInit`(0x4438) → ARM C runtime（scatter-load）→ `main`（XIP 0x1007f81c）= `sys_init`(0x1006ac00) + `main_loop`(0x1006b0b4)，`main_loop` 永不返回。
- 三個時基：**SysTick 1 ms**（ms 計數器 + 1 kHz 取樣轉盤 + 音效 sequencer）；**畫面 frame = LCD TE 腳 3 個脈衝**（名目 20 fps，見 §2.3）；**遊戲邏輯 = RTC 秒**（`gametime_update` 0x10012c90 每 frame 比對 RTC，把經過的秒數以 ≤60 秒為一塊推進）。
- 每 frame 依序：餵狗 → 讀鍵 → 電池 ADC 濾波 → 遊戲時間推進 → 清 framebuffer → `scene_update`（場景邏輯 + 繪圖）→ 淡入淡出 → UART1 通訊 → UART0 monitor → `rand()` 攪一次 → 等 TE → SPI DMA 送 128×128 RGB565 framebuffer。
- 場景系統：137 個場景描述子（`PSID_*`，表在 0x10094830，28 bytes/筆，`init/update/draw/exit/flags/name`），由 `scene_update`(0x1006b200) 以 3 相（init→run→exit）驅動；轉場只靠「下一場景 ID」變數 0x18001cb6，再由 `scene_resolve_next`(0x10002844) 覆寫（電池低 → 0x76、閒置 → 0x75 關機、閒置回家 → 0x1c、debug → 0）。
- 輸入：4 顆 GPIO 鍵（bit0=A P0.9、bit1=B P0.11、bit2=C P0.10、bit3=轉盤按壓 P0.8，低電位有效），轉盤正交編碼器 P2.0/P2.1 由 SysTick 每 1 ms 取樣，每 3 個計數產生一個 bit6(DIAL DOWN, −)/bit7(DIAL UP, +) 單 frame 脈衝。
- 電源：閒置 200 frames(≈10 s) 回 HOME／關圈選單，600 frames(≈30 s) 進 `PSID_SYSTEM_POWERDOWN` → 計算下一個遊戲事件時間（最多 3600 s 後）→ 設 RTC alarm → **Deep Power Down**；喚醒（按鍵或 RTC）都是整機重開機，靠 reset cause 判斷要補算時間後再睡、還是點亮螢幕。
- 工廠模式：冷開機時按住 **A+C**（B、轉盤不按）→ 透過 UART1(prongs, 460800) 跟治具做 0xAA/0x55 交握 → 成功才進 `PSID_TESTMODE`(0x87) → 退出進 `PSID_DEBUGMENU`(0)。沒有治具就逾時、正常開機。
- UART monitor：**UART0**(0x4000a000, P0.0 TX / P0.1 RX=TP48)，**460800 8N1**，未解鎖時完全靜默；輸入 `mY6$O$xw` + CR 解鎖後才註冊 `d` 指令（→ `mon_dispatch` 0x10076350）。

## 2. 詳細規則

### 2.1 開機（Boot）

1. **[確認]** PRAM 向量表（`dumps/pram.bin` 0x0）：SP=0x1801ef18、Reset=0x2f5、NMI=0x10025a55、HardFault=0x2bc5、SysTick=0x432d。完整中斷表見 §2.4。
2. **[確認]** `Reset_Handler` 0x2f4：`SP = *(VTOR)`，呼叫 `*(0x33c)=0x4439`(`SystemInit`) 再跳 `*(0x340)=0x141`(`__main`)。
3. **[確認]** `SystemInit` 0x4438：依 `*(0x10004)==Reset` 決定先餵 WDT0 或 WDT1（寫 0x5afa55aa 到 +0xc），再把兩個 WDT 關掉（`0x40008000/0x40009000 = 0x5afa0000`）；`FUN_000085a0` 開時脈：`0x45000000|=0x10/0xc000`，`0x4500000c` 選 mode 4（PLL 192 MHz/2 = **96 MHz**，見 `clk_get_sysclk` 0x43dc），`0x45000100=0x157eb7`（周邊時脈閘）；`FUN_00005850` 若 RTC alarm 已過且 `0x45000234 & 0xa00` → 設 `0x4500002c |= 2`（標記「從 DPD 被 RTC 叫醒」）；`FUN_00008714` 設 SPI flash controller(0x40022000)；`FUN_0000867c` 驗證 XIP（0x4002f000）；`FUN_000084e4(0)` 依表 0xe1c0/0xe248/0xe1b4 設 NVIC 優先權。
4. **[確認]** `__scatterload` 0x148，region 表在 0xe554：
   - 0xe6a0 → 0x18000000，長度 0xb30，用 `__decompress1`(0x1a4, ARM RLE/LZ) 解壓 → 初始化 RW data（已用 Python 還原，見 §3.5）。
   - 0x18000b30 起 0x1e3e8 bytes 清零（`__scatterload_zeroinit` 0x204）。
   - Heap/stack：`0x1801cb18–0x1801ef18`（0x330/0x344/0x348 常數）。
5. **[確認]** `__rt_entry` 0x2d4：`__user_setup_stackheap`(0x1c0c) → `0x2b0`(lib init) → **`bl 0xb420`（veneer → XIP 0x1007f81c = `main`）** → `exit`(0x1dbc)。
6. **[確認]** `main` 0x1007f81c：`sys_init()`(0x1006ac00)，`main_loop()`(0x1006b0b4)。
7. **[確認]** `sys_init` 0x1006ac00 順序（重點）：
   - 讀 RTC 秒 `rtc_get_seconds`(0x3394 = `*0x45000304`)、wake flag(0x33a0 = `!(0x45000300 bit15)`)、reset cause(0x43a8 = `0x45000014 | (0x4500002c&2)<<2`)。
   - `systick_config(sysclk/1000)`(0x100603ec)：`SYST_RVR = 96000-1`，CSR=7 → **1 ms SysTick**。
   - heap 初始化 `FUN_10018ec0(0x18005e30, 0x8000)`（32 KB heap）。
   - 電池 ADC 初始化 `FUN_1000495c`，開機電壓檢查 `FUN_10004754`（<2.1 V 時再取 10 次，仍低 → 回 0）。
   - `srand(...)`(0x1002a6d8)；SPI flash status 檢查（0x3678，QE bit 0x40）。
   - `save_load`(PRAM 0x8b5c)：讀主/備份存檔（flash 0xefe000/0xeff000），兩份都壞 → 初始化。
   - 若開機電壓過低且 `0x18000be8` bit3（低電旗標）→ `pm_power_down_lowbat`(0x1002a1d8) 直接 DPD。
   - RTC 有效性檢查（0x3280 = `0x45000200 bit7`）；無效 → `0x18001be8 |= 1`（時間未設定）。
   - `rtc_init`(0x1002a20c)，`srand(adc + rtc_seconds)`。
   - `asset_init`(0x10003160, flash 0x111000 主資產)，`boot_wake_handler(resetcause, wake, rtc)`(0x10012a0c，§2.7)。
   - GPIO 全部配置（`gpio_config` 0x291c，pins 0..0x23）、timer3 重設/啟動（0x4658/0x45e8，TE 時間戳用）。
   - framebuffer/LCD/flash 初始化：`FUN_1007b5f0`（fb 指標 0x1801438a, 0x8000 bytes）、`FUN_1007b7e0`（P1.10=TE 設輸入）、`FUN_1007b6ac`（flash ID：只接受 0xC8xxxx/0x1Cxxxx/0xC2xxxx，否則死迴圈印 "unsupport chip"；接著 `lcd_init` 0x1001c4e4）。
   - 亮度（0x18000c6a bits 9–12，1..9）、`input_init`(0x10005d50)、音效、語言、monitor 配置/初始化（0x10076f24/0x10076eb8）、UART1 關閉、`asset_init_additional`(0x10003238, flash 0xd2e000 起 40 個 16 KB 擴充槽)。
   - 存檔區驗證印出 `Tama Ghost / Item Storage / Grazing Tama : Verify %s`。
   - `0x40018034=0x15555155; 0x40019034=0x55540140`（P0/P1 pull 設定）。
   - **初始場景 `g_scene_cur = 0x6c`（PSID_STARTUP）**。
8. **[確認] PRAM vs XIP 分工**：PRAM（64 KB，加密存放於 flash 0x1000，開機由 boot ROM 解密到 0x0）放：向量表與 ISR、C runtime、`SystemInit`、所有低階 HAL（GPIO/UART/SPI/SPIFC/RTC/WDT/Timer/ADC/DPD）、**存檔讀寫（flash erase/program 必須在 RAM 執行）**、monitor 密碼檢查、libc（printf/strtol/memset/memcpy/softfloat）、`TamaData/TamaCare` getter。XIP（0x10000000，flash 0x11000 明文）放全部遊戲邏輯、UI、場景、monitor 指令。XIP 透過 0xb420 起的 veneer 表被 PRAM 呼叫，XIP 用 `thunk_EXT_FUN_0000xxxx` 呼叫 PRAM。
9. **[推測] DPD 映像（flash 0x110000，4 KB，實際用 0x60c bytes）**：自己的向量表（SP 0x18001108, Reset 0x1dd），獨立的 ARM C runtime（scatter：0x568→0x18000000 長 0xa4，0x180000a4 清 0x1064）。`pram 0x1fbc rtc_set_alarm_dpd(t)` 設 `0x45000230 = t-1` 後呼叫 `dpd_set_wake_image(0x60110000, 0x1000)`(0x229c)：`0x45000234 = ..|0x5aa50100`、`0x4500023c = flash addr | (頁數-1)<<4` → 告訴 boot ROM「DPD 喚醒時從 flash 0x110000 載入這個小程式」。DPD 程式 main(0x518)：
   - `0x4500002c |= 2`；`0x308` 用 `0x45000204/208/200` 喚醒遮罩與 `0x45000210..218` 鎖存值找出哪一隻喚醒腳觸發（0..0x23，0x24=無）。
   - 有按鍵喚醒 → 跳到 SRAM 0x18000000（=映像 0x568 的函式）：設時脈，用 SPIFC DMA(0x40022100..) 把 flash 0x60001000（PRAM 映像）64 KB 讀到 0x18010000，複製到 0x0，`AIRCR` 重置 → 正常開機。
   - 否則 `0x2d4`：若 `RTC(0x45000304) >= alarm(0x45000230)` → `0x45000234 = (x & ~0x100) | 0xa00`，然後 `0x3f0(1)` 進睡眠/重置 → 主韌體 `FUN_00005850` 看到 0xa00 → 標記 RTC 喚醒。
10. **[推測]** `pram_reload_and_reset` 0x1007f7ac：把一段資料（每個 word XOR `FUN_1009104e()` 回傳的 key）複製到 0x0..0xffff，然後 `NVIC_SystemReset`。唯一呼叫者 `FUN_10082268`：來源是 flash 0x60e66010、長度 `*0x60e66004` words——這是透過 UART1 下載 RAM 程式（Paramix 的 dumper 走這條）的入口。

### 2.2 主迴圈 `main_loop` 0x1006b0b4

**[確認]** 前置：`FUN_1006b454`（frame 計時歸零）、`FUN_100772a8`（monitor UART0 開，`g_mon+0x1a8 |= 1`）、`FUN_10064110`（P1.10 TE 腳設上升緣中斷）。之後每圈：

| 順序 | 函式 | 作用 |
|---|---|---|
| 1 | `t0 = g_ms_tick`(0x1801c3a8) | frame 開始時間 |
| 2 | `wdt_feed` 0x10068370 → PRAM 0x4c64(0) | WDT0 寫 0x5afa55aa |
| 3 | `input_poll` 0x10006170 | §2.5 |
| 4 | `battery_update` 0x100049cc | ADC 濾波，§2.8 |
| 5 | `gametime_update` 0x10012c90 | RTC 秒 → 遊戲邏輯，§2.3 |
| 6 | `g_rtc_cache`(0x18014360) = RTC 秒 | |
| 7 | `FUN_1007bee0` / `FUN_1007bc50` | debug 文字數歸零；**framebuffer 清 0x8000 bytes** |
| 8 | `scene_update` 0x1006b200 | 場景邏輯+繪圖，§2.6 |
| 9 | `FUN_1007bef0` | 疊畫 debug 文字（`FUN_1007bdc8` 收集的） |
| 10 | `FUN_1007b518` / `FUN_1007bca0` | 淡入淡出（`0x1801c3b8/3be`：bit0 啟動、bit1 方向、bits4–9 目標、bits10–15 目前值、`0x1801c3ba/3c0` 速度，每 frame −0x40）；全黑/全白時直接以單色填滿 fb |
| 11 | `comm_poll` 0x10060d34 | UART1 封包（TCP/jig），逾時 2000/5000 ms |
| 12 | `mon_poll` 0x10076f3c | UART0 monitor 命令列 |
| 13 | `rand()` 0x1002a55c | **每 frame 多抽一次亂數** |
| 14 | frame 時間統計 `0x1801c394/398` | |
| 15 | `do { lcd_wait_te(); } while (g_te_count < 3 && !(0x18001cbc & 2))` | `lcd_wait_te` 0x1007b808 等 P1.10 先低後高 |
| 16 | `0x1801c390 = 80 / g_te_count; g_te_count = 0` | 只寫不讀（debug FPS） |
| 17 | `lcd_blit(0,0,127,127,fb)` 0x1007b7ec→0x1001f308；`while(lcd_busy())` | SPI DMA 送整張 |
| 18 | `g_frame_count++`(0x18005de0) | |

- **[確認]** `0x18001cbc` bit1 = 「不鎖 3 個 TE」：設定者為 TCP 開發場景（0x1000d77c/0x1000d7cc）等，一般遊戲不設。
- **[確認]** RNG（0x1002a55c）：`seed = seed*0x41c64e6d + 0x3039; return (seed>>16) & 0x7fff`（seed 存於 0x1801cae4）。`rand_range(lo,hi)` = 0x1002a584。種子 = 電池 ADC 原始值 + RTC 秒。因第 13 步每 frame 都消耗一次，任何機率結果都跟 frame 數有關——remake 若要「可重現」，需把這個每 frame 的空抽也模擬，或明說不追求位元級一致。

### 2.3 時基與 tick

- **[確認] SysTick = 1 ms**：`systick_config(clk/1000)`；`SysTick_Handler` 0x432c：`g_ms_tick++`；`rotenc_sample()`(0x1008fe74)；`FUN_10090c00`：每 `(0x1800ed9a)+1` ms 呼叫一次音效 sequencer `FUN_10090a80`（tempo 由 `FUN_1007ace0` 設，上限 0x20）。
- **[確認] frame = 3 個 LCD TE 脈衝**。TE 由 `GPIO1_IRQHandler` 0x2558 計數（`0x4001901c` bit10 → 清 `0x40019020=0x400`，`g_te_count++`，順便記 timer3 計數到 0x1801c3a0）。
- **[推測，強] 名目 20 fps（50 ms/frame）**：證據 `FUN_1003b8f4`(0x1003b8f4 附近 XIP.c:38545)把「`idle_ms > 30 s`」與「per-frame 計數器 > 599」並列為同一個離開條件，即 600 frames ≈ 30 s。LCD 初始化 FRMCTR1/2/3 = `08 3b 3b`（ST7735S 系列），以 ST7735S 公式約 54–61 Hz → 18–20 fps，一致。remake 建議固定 **20 Hz** frame tick。
- **[確認] 遊戲邏輯 tick = 1 秒（RTC）**：`gametime_update` 0x10012c90：
  - 只在 `0x18000be8 bit0`（有存檔/遊戲已開始）且 `!(0x18000be8 bit7)`（時間待校正）時執行；`0x18001be8 bit6` 時暫停（另記 bit7 待處理）。
  - `now = rtc_to_seconds(&g_now)`（`FUN_10006970` 讀 RTC 成年月日時分秒於 0x18001bec，`FUN_10006be0` 轉秒），`last = 0x18000c00` 的已處理時間；若 `now < last` 則 `last = now`（時鐘倒退不補算）；`now == last` 直接 return（同一秒內所有 frame 不做事）。
  - 處理 EM_REQ_WAKE/SLEEP（依 `FUN_10010c18()` 時段）、生日/過年 DODODO 事件（12/13 13:17 等日期條件），再把差值 `d` 以 **60 秒一塊**呼叫 `FUN_100134bc(start, n)`：`d > 60` 時迴圈最多 0x5a0(=1440) 塊，之後再補一塊 `min(d,60)`。**單次最多補算約 24 h + 60 s，超過的部分直接丟棄**（`0x18000c00` 直接設為 now）。
  - 日期變更 → `FUN_10013234`（換日處理）。結束時若 `0x18001be8 bit2` → `FUN_10083984` + `save_commit(1)`。
- **[確認]** `ms_now()` = 0x10018e2c 回傳 `g_ms_tick`。

### 2.4 中斷表（PRAM 向量表）

| 向量 | IRQ | 位址 | 用途 |
|---|---|---|---|
| 2 NMI | – | 0x10025a54 | WDT NMI：印 `=== WDT Non Mask IRQ Issue \| SystemReset ===`，`sys_panic_reset` |
| 3 HardFault | – | 0x2bc4 | 傾印 R0–R3/R12/LR/PC/xPSR、"Stack Overflow"，經 SDK printf，死迴圈 |
| 4,5,6,11,12,14 | – | 0x320..0x32a | 死迴圈 |
| 15 SysTick | – | 0x432c | 1 ms，見上 |
| 18 | IRQ2 | 0x33c8→0x3290 | **RTC**：`0x45000300 |= 0x10` 清旗標 |
| 19 | IRQ3 | 0x30a | **WKP（喚醒腳）**：`0x45000300 |= 0xc0` |
| 25 | IRQ9 | 0x1008ff68 | **ADC 完成**：`g_adc_raw(0x1801425c) = (0x4003a008 & 0xffff)>>6`，旗標 0x18014364 |
| 43 | IRQ27 | 0x2558 | **GPIO1**（P1.10 LCD TE） |
| 52 | IRQ36 | 0x10066bbc | **UART0 RX**（monitor，0x4000a000） |
| 53 | IRQ37 | 0x10066cdc | **UART1 RX**（prongs：TCP/Paramix/jig，0x4000b000） |
| 其餘 | | 0x32e | 死迴圈 |

**[確認]** 沒有 RTOS/task 切換；PendSV/SVC 只是死迴圈。Timer（0x40000000+n·0x1000，n=0..7，`timer_base` 0x8800）：timer3 = TE 時間戳；timer2/timer7 = 蜂鳴器 PWM（`FUN_10028dec(7/2)` 停止，`FUN_000031f0`）**[推測]**。

### 2.5 輸入

**接腳 [確認]**（`key_pin_table` = RW data 0x18000406 = `09 0b 0a 08`，與 GMMan testpads.txt 一致）：

| bit | 鍵 | GPIO | pin# |
|---|---|---|---|
| 0x01 | A (SW3) | P0.9 | 9 |
| 0x02 | B (SW4) | P0.11 | 11 |
| 0x04 | C (SW5) | P0.10 | 10 |
| 0x08 | 轉盤按壓 (DIAL PUSH) | P0.8 | 8 |
| 0x40 | DIAL DOWN（逆向，計數 −） | 編碼器 P2.0/P2.1 | 0x20/0x21 |
| 0x80 | DIAL UP（正向，計數 +） | 同上 | |

pin 編號 = port×16 + bit（`gpio_read` 0x25c4、`gpio_config` 0x291c）。鍵為低電位有效（讀值取反）。

**編碼器取樣 `rotenc_sample` 0x1008fe74 [確認]**（SysTick，每 1 ms）：`s = P2.0 | P2.1<<1`。狀態機以 00 與 11 為停駐點：從「停在 00」看到 11 時，若上一個樣本是 2(10) → `raw--`，是 1(01) → `raw++`；從「停在 11」看到 00 時，上一個樣本是 1 → `raw--`，是 2 → `raw++`。`raw` = `g_rotenc_raw`(0x1800efc0, s8)。

**`input_poll` 0x10006170 [確認]**（每 frame）：
- `held = ~pins & 0x0f`（清掉 bit6/7 再加轉盤脈衝）；`prev = 上一 frame held`。
- 轉盤：`delta = raw; raw = 0`（`dial_delta()` 0x10006140 回傳本 frame 的 `delta`，存 0x1800efbf）。若上一 frame 沒有轉動，累加器 `acc`(0x1800efc1) 先設成 `±3`（使第一格立即觸發），否則 `acc += delta`；`acc >= 3` → `held |= 0x80, acc -= 3`；`acc <= -3` → `held |= 0x40, acc += 3`。也就是**每 3 個原始計數產生一個單 frame 的 UP/DOWN 脈衝**。
- 轉盤絕對位置 `g_dial_pos24`(0x1800efbc) = `(pos + delta) mod 24`，`0x1800efb8 = pos*65536/24`（0..1 定點角度）。累計轉動量寫進存檔 `0x18000bf8`(正) / `0x18000bfc`(負)。
- `down`(0x18001ce8) = `held & ~prev`；`up`(0x18001cec) = `prev & ~held`。
- 每顆實體鍵 i（0..3）：
  - 按住時：`hold_cnt[i]`(0x18001cd0+i) 從 1 起每 frame +1（飽和 0xff）；`repeat_cd[i]`(0x18001cd4+i) 每 frame −1，歸零時 `repeat |= bit` 並重設為 3 → **自動連發：按下 18 frames 後每 3 frames 一次**（放開時重設為 0x12=18）。
  - 剛按下那個 frame 若 `rel_cnt[i]`(0x18001cd8+i, 放開後經過的 frames) 在 3..10 → `double |= bit`（雙擊）；接著 `rel_cnt = 0`。
  - 放開時：若 `hold_cnt > 13` 或 `== 0` → 從 `up` 清掉該位元（**長按不算 tap 放開**）；`hold_cnt = 1`；`rel_cnt++`（飽和）。
- `g_no_key_frames`(0x1800eda4)：沒有任何鍵按著時 +1，否則 0。
- 查詢 API：`key_pressed(m)`=0x10005ea8（down）、`key_held(m)`=0x10006150、`key_released(m)`=0x10005e50（`prev & ~held`，**未過濾長按**）、`key_pressed_or_repeat(m)`=0x10005e7c、`key_double(m)`=0x10005ec8、`key_long_held(m)`=0x10005de0（held 且 `hold_cnt > 13`，約 0.65 s）。
- 沒有事件佇列：每個場景每 frame 直接查上述 bitmask（edge 只維持一個 frame）。`input_reset`(XIP.c:4185 一帶) 清全部狀態。

**常見操作語意 [確認]**（各選單共用）：
- LABOMENU(0x10045db0)：A 或轉盤 → 移動（0x40 → −1，否則 +1，循環 6 項）；B 或轉盤按壓（mask 0x0a）→ 確定；C → 回 HOME。
- DEBUGMENU(0x10032770)：A(含連發)/DIAL DOWN → 上一項；C(含連發)/DIAL UP → 下一項；B/轉盤按壓 → 進入。
- HOME 畫面（0x10039eb8 等）：A → 開該畫面的圈選單（ring menu）；**長按轉盤按壓（>13 frames）→ LABOMENU(0x2a)**；按任一鍵確認事件。
- 音效 id（`FUN_10024c78(id,100)` / `FUN_10024c60(id)`）：0=游標移動、1=確定、2=取消、0xb=不可、3=通知。

### 2.6 場景（PSID）狀態機

**描述子表 [確認]**：0x10094830，每筆 28 bytes：

| off | 型別 | 欄位 |
|---|---|---|
| +0x00 | s16 | scene id |
| +0x02 | u16 | pad |
| +0x04 | fn | `init()` → 回傳 2 = 立即結束（不跑 update） |
| +0x08 | fn | `update()` → 回傳非 0 = 結束 |
| +0x0c | fn | `draw()`（update 回 0 時才呼叫） |
| +0x10 | fn | `exit()` |
| +0x14 | u32 | flags |
| +0x18 | char* | 名稱（"PSID_..."） |

共 0x89 筆（0..0x88，0x88 = `PSID_INIT_DUMMY` 全 0）。`scene_get_desc(id)` 0x10018260 線性搜尋，找不到回傳第 2 筆（0x1009484c）。全表輸出於 `docs/spec/data/scenes.csv`。

**flags [確認，語意部分推測]**：
- 0x01：開發/系統場景（DEBUG/DEVELOP/VIEWER/TESTMODE）——不做閒置計時、不做低電檢查。
- 0x02：不做閒置計時（STARTUP、INITIAL_LOGO、EVENT_DYING、EVENT_CHANGESTATUS、EVENT_UFO、SYSTEM_POWERDOWN）。
- 0x04：「家/設定」類（HOME_SPACE/FIELD/TAMA/CELL/FIELD2、HELP_SITTER、EVENT_DEAD、DELETE、CORRECTTIME、LANGUAGE、DATETIME、BIRTHDAY、PLANETNAME）——閒置只會關機不會跳回 HOME；電壓 ≤2.2 V 即觸發低電。
- 0x08：電源場景（POWERDOWN 0xa、BATTERY 0x1a）——不做低電檢查。
- 0x10：進入時存檔（HOME、OSEWA_*、LABOMENU*、BATTERY）——除非前一場景是 STARTUP(0x6c)。

**系統變數 [確認]**：`g_scene_cur` 0x18001cb4(u16)、`g_scene_next` 0x18001cb6(u16, 0xffff=未指定)、`g_scene_prev` 0x18001cb8(u16)、`g_scene_flags` 0x18001cba(u8：bits0–2 phase、bit3 debug-return、bit6 閒置關機、bit7 低電)、`g_scene_flags2` 0x18001cbb（bit0 閒置回 HOME、bit1 暫停 update/draw、bit2 閒置計時重設中、bit3 閒置計時暫停、bit4 閒置 200 frames 已到、bit7 由各場景 init 設/清）。

**`scene_update` 0x1006b200 [確認]**（同一 frame 內循環直到 phase==1 處理完）：
- phase 0（進場）：`flags &= 0x37`、`flags2 &= ~1`；`idle_reset()`；清 `0x18001be8 bit6`；淡入值歸 0；`g_scene_next = 0xffff`；若 `prev != 0x6c && flag 0x10` → `save_commit(1)`；`init == NULL` → panic；`r = init()`；`r == 2` → phase 2，否則 phase 1 並重設 frame 計時。
- phase 1（執行）：`flags2 bit1` 未設時 `done = update()`（NULL 視為 1）；`done == 0` → `draw()`。`done` → phase 2。
- phase 2（離場）：`exit()`；`prev = cur`；`cur = scene_resolve_next(g_scene_next)`；phase 0。
- 每 frame 結尾（phase 1）：`idle_update()`(0x10025a78)；若場景沒有 flag 1 與 flag 8：`battery_is_low()`（EMA < 2.1 V）→ `flags |= 0x80`；否則若 EMA ≤ 2.2 V 且 flag 4 → `flags |= 0x80`。

**`scene_resolve_next` 0x10002844 [確認]**（優先序）：`flags bit7` → **0x76 SYSTEM_BATTERY**；`flags bit6` → **0x75 SYSTEM_POWERDOWN**；`flags2 bit0` → **0x1c HOME**；`flags bit3` → **0 DEBUGMENU**；否則 `g_scene_next`。（bit3 在 XIP 中找不到設定者，可能只能由 debug 工具設。）

**場景內子狀態機 [確認]**：幾乎所有場景的 context 開頭是共用「task」結構：`+0 state`、`+8 substep`、`+0xc done`、`+0x10 wait frames`、`+0x14 flags`（bit0 可執行、bit1 要求離開、bit2 允許被系統強制離開）、`+0x18 UI layer`。`task_init` 0x10069590；`task_set_state(ctx,s)` 0x1006953c（同時清 substep）；`task_wait(ctx,n)` 0x10069558（等 n frames）；`task_pre_update` 0x100695b8：倒數 wait、並在 `scene_should_exit()`(0x100027e4：flags bit6/bit7 或 flags2 bit0) 為真時強制 `state = 0x7fe0`（淡出）。慣例狀態值：`0`=進入、`1000`=主狀態、`0x7fe0/0x7fe1`=開始淡出、`0x7ff0`=設 `done=1`。`init` 用 `scene_set_next(x)`(0x100028cc) 預設「返回目標」。

**主要轉場圖 [確認]**（`docs/spec/data/scenes.csv` 有完整 init/update 位址）：
- 開機：`STARTUP(0x6c)` → `INITIAL_LOGO(0x6d)`（無存檔）/ `CORRECTTIME(0x70)`（時間無效）/ `SYSTEM_BATTERY(0x76)`（低電旗標）/ `TESTMODE(0x87)`（JIG）/ `HOME(0x1c)`。新遊戲流程（0x10038250/0x100383ac/0x100387dc/0x10038d24/0x10039034）：`ROM_VER(0x6e)` → `DELETE(0x6f)`/`LANGUAGE(0x71)` → `DATETIME(0x72)` → `BIRTHDAY(0x73)` → `PLANETNAME(0x74)` → `EVENT_BIRTH(0x48)` → HOME。
- `HOME(0x1c)` 是**路由場景**（init 回傳 2）：依 `0x18001c6a & 3` 選 `HOME_SPACE(0x1d)/HOME_FIELD(0x1e)/HOME_TAMA(0x1f)/HOME_CELL(0x20)`，再依事件覆寫：死亡 `EVENT_DEAD(0x4a)`、垂死 `EVENT_DYING(0x49)`、星球升級 `TAMASPACE_RANK(0x7d)`、任務 `INFO_MISSION(0x39)`、幼體期 `HELP_SITTER(0x41)`、狀態變化 `EVENT_CHANGESTATUS(0x4e)`、隨機事件（用 `rand_range(0,99)`：METEOR 30%→`LABOGAME_05_EVENT(0x63)`、SHOOTINGSTAR 10%→0x57、UFO→0x4f）。
- 各 HOME 畫面按 A 開**圈選單**（`FUN_1002af28`，表 0x10094120，§3.3），選中項目直接 `scene_set_next(entry.scene)`；不能用時跳 `NOTICE_MSG(0x1b)`。
- `LABOMENU(0x2a)`（長按轉盤）：6 項 → `INFO(0x2b)`、`GAME(0x2c)`、`SHOP(0x34)`、`HELP(0x35)`、`BOOK(0x36)`、`SETTING(0x37)`（§3.2）；C 回 HOME。`0x18001c6b bit3` 為「只能進 SETTING」模式。
- `EVENT_*`、`OSEWA_*` 都以 HOME 為返回目標；`LABOGAME_xx` → `LABOGAME_RESULT(0x5c)` → `LBMENU_GAMExx`；`OSEWAGAME_xx` → `OSEWA_TOGETHER(0x24)`。

### 2.7 電源管理

**閒置計時 `idle_update` 0x10025a78 [確認]**（每 frame，僅在無 flag 1/2 的場景）：
- 任何 `key_pressed(0xf)`、`key_released(0xf)`、`dial_delta()!=0` 或 `flags2 bit2` → 計數歸零（`flags2 &= ~0x14`）並記錄時間。
- 否則（`flags2 bit3` 未設時）`g_idle_frames`(0x18001cbe)++；`g_idle_ms`(0x18001ccc) = 距最後輸入的 ms（`idle_seconds()` 0x10002764 = /1000）。
- 門檻（`idle_reset` 0x10025c6c 設定）：`g_idle_home = 200`(0x18001cc4)、`g_idle_off = 600`(0x18001cc2)；特定流程改 0x4b0=1200（0x10039034 蛋選擇）。
  - 非 flag 4 場景：沒存檔（`0x18000be8 bit0==0`）→ `≥600` 時 `flags |= 0x40`（關機）；有存檔 → `≥200` 時 `flags2 |= 0x11`（回 HOME + 通知）。
  - flag 4 場景：`≥600` 且成長階段 ≠ 1（蛋）→ 關機；場景 0x6f/0x70 `≥600` → 關機；`≥200` → `flags2 |= 0x10`（關閉圈選單等）。
  - 換算：200 frames ≈ **10 s**、600 ≈ **30 s**、1200 ≈ 60 s（以 20 fps）。
- 有些場景另用 `idle_seconds() > N`（`FUN_1000278c(N)`，N=5/10/30 s）。

**`PSID_SYSTEM_POWERDOWN`(0x75) [確認]**：state 1000 印 `[SYSTEM] Deep Power Down`，呼叫 `pm_sleep_until_next_event` 0x10011b18：
- 設 `0x18000be8 |= 2`（睡眠中）；沒有存檔或 `be8 bit7` → 直接 `pm_power_down(1)`（只能按鍵喚醒）。
- 否則計算**下一個需要喚醒的時間** `wake = min(now+3600, 下一個整點日界, 各種事件)`：EVENT_DODODO、成長計時（`0x3840`=4 h、`0x15180`=24 h、`0x189c0`=28 h − age_timer）、睡/醒時刻、onaka/gokigen 計時歸零時刻（`def.Hungry*60 - timer`）、`onaka_timer<900`、cold 等；每項以 `mon_puts("%4d %02d:%02d:%02d Request Reason %s")` 記錄。結果存 `0x18000c18`，`save_commit(1)`，然後 `pm_dpd_until(wake)`(0x1002a2f0)/0x1002a268。
- **[確認]** `pm_dpd_until(t)`：A/B/C/轉盤（P0.8–0.11）設上拉輸入；`pm_wait_keys_release`(0x1008fd34：四鍵連續 10 ms 放開)並設為**喚醒腳**（`gpio_set_wake_pin` 0x23e4，寫 `0x45000208`/`0x45000204`）；關 ADC、UART、LCD（`lcd_sleep` 0x1001f26c：0x28 Display Off、20 ms、0x10 Sleep In）；`timer3` 停；RTC IRQ；`t = max(t, now+2)`；`rtc_set_alarm_dpd(t)`；所有腳設低功耗；`pm_enter_dpd`(0x3268)：`FUN_00002310`（SPIFC 停、P1 腳位調整）、`FUN_00002020`（鎖存 GPIO 到 0x45000210..218）、`pm_sleep(1)`(0x20d4：關 SysTick、清 `0x4500002c bit1` 與 reset cause、關所有 NVIC 但保留 IRQ2/3，WFI 迴圈)。
- 變體：`pm_power_down(waitkeys)` 0x1002a178（無 RTC alarm）、`pm_dpd_seconds(n)` 0x1002a398（n 秒後，WDT 測試畫面用 60 s）、`pm_power_down_lowbat` 0x1002a1d8。

**喚醒 = 重開機，`boot_wake_handler` 0x10012a0c [確認]**（`rc` = reset cause）：
- `rc bit1` → `0x18001be8 &= ~1`，非冷開機（推測為 WDT/軟體重置）。
- `rc bit3`（`0x4500002c bit1`，DPD 被喚醒）→ `bVar1 = (wake_flag == 0)` 即**RTC alarm 喚醒**：若 `rtc < 0x18000c18` → 太早，立刻 `pm_dpd_until(0x18000c18)` 再睡；否則若 `be8 bit1`（睡眠中）→ `gametime_update()` 補算，然後若仍在睡 → `pm_sleep_until_next_event()` 再睡（**不亮螢幕**）；若產生了需要顯示的請求（`0x18000d84` 原因碼 1..0x23，EM_REQ_EVOLUTION/WAKE/SLEEP/...）就繼續開機到 HOME。
- 其他 → **冷開機**：`0x18001be8 |= 8`（允許 JIG 檢查）；`rc bit4` → 清 `be8 bit3`。
- 電壓 `< 2.2 V`(`FUN_100047f4`，10 次重試) → `be8 = (be8 & ~2) | 8`（**低電旗標**）。
- 結束：`gametime_update()`、`be8 = (be8 & ~2) | 4`、`save_commit(1)`、`0x18001be8 |= 2`。

**低電量 [確認]**：
- ADC：`adc_read` 0x3470 = `(0x4003a008 & 0xffff) >> 6`（10-bit）；`V_now = raw * 0xd33300 >> 16`（16.16 定點，raw=1023 → 3.30 V）；EMA `V += (V_now − V) * 0xccc / 65536`（α≈0.05），初值 3.0 V（0x30000）。
- 門檻：`battery_is_low` 0x10004860：EMA < **2.1 V**(0x21999)；`battery_ok` 0x100048e4：EMA > **2.2 V**(0x23332)。`0x18005d20` = 量測已穩定（啟動 ≥1000 ms 且非低電持續 >200 ms）。
- 觸發後進 `PSID_SYSTEM_BATTERY`(0x76, 0x10037724)：`be8 |= 8`，顯示電池圖示（sprite 0xf1）淡入，等 **200 frames(≈10 s)**，印 `[SYSTEM] Low Battery(%d)`；若此時已不低 → 清 `be8` bit1/2、存檔、`pm_power_down(1)`；仍低 → `pm_power_down(0)`；最後 `NVIC_SystemReset`。下次開機在 STARTUP 看到 `be8 bit3` → 印 `** LOW BATTERY FLAG DETECTED **` 又進 0x76。冷開機時電壓 <2.1 V 且旗標在 → `sys_init` 直接 DPD。

**WDT [確認]**：WDT0 0x40008000、WDT1 0x40009000，所有寫入需 0x5afa 鑰匙。`wdt_feed`：+0xc = 0x5afa55aa（主迴圈每 frame、長迴圈內也會呼叫）。`sys_panic_reset` 0x10068388：`wdt_set_timeout(0,0x20)` + `wdt_enable(0)` + 死迴圈（任何 assert 失敗都走這裡）。**正常執行時 WDT0 是啟用的**：`sys_init` 第一步 `FUN_1006837c`(0x4d38(0)，重設) 後 `wdt_start` 0x100683a4 = `wdt_set_timeout(0,0xff)` + `wdt_enable(0)`；`mon_poll` 的 movie 模式結束時也重新呼叫它。timeout 0xff 的時間單位未知。

### 2.8 工廠 JIG / TESTMODE

**進入條件 [確認]**（`startup_state` 0x100378d0，PSID_STARTUP state 1000）：`0x18001be8 bit3`（本次為冷開機）且 `key_held(A)` 且 `key_held(C)` 且 `!key_held(B)` 且 `!key_held(DIAL PUSH)` → state 2000：
1. `comm_open(0xf, cb=0x1008d644)`(0x10060888)：UART1 0x4000b000，96 MHz 時脈、**460800**、8N1，IRQ37。
2. `comm_send_pkt(0x100)`(0x10060c70)：送 `PKT 256`，回呼 type 1 以 0xAA 填滿 256 bytes。
3. 等待（`comm_poll`：2000 ms 無回應重送，最多 3 次 → `CAN` → 失敗碼 1）。
4. 治具回傳 256 bytes，回呼 type 4 檢查**全部為 'U'(0x55)**，否則錯誤 3。
5. `comm_close`；結果 0 → `scene_set_next(0x87 TESTMODE)`，否則回到 state 0x3e9（正常開機）。
- 沒有治具時按 A+C 開機只會延遲約 3×2 s 後正常開機。

**`PSID_TESTMODE`(0x87) [確認]**：init 0x100641a0 讀 flash 0x60d29000（版本區）：word0 = 版別（1 RIKU、2 KAI、3 KOUZAN、4 MORI、5 TROPIC、**6 FROZEN = 本機 White Glacier**），+0xff4/+0xff6 = 版本號；畫面 `REV.%d V%d.%02d`。11 個測試項（表 0x100a0b84，20 bytes/筆：名稱[16] + handler）：

| # | 名稱 | handler |
|---|---|---|
| 0 | SHIPPING | 0x10061490 |
| 1 | LCD | 0x10061304 |
| 2 | RTC | 0x100613e4 |
| 3 | ROM | 0x10061340 |
| 4 | SAVE | 0x10061440 |
| 5 | DIAL | 0x10061284（DIAL UP/DOWN 各要 24 計數 + 按壓，`FUN_1000e9b4/eac8/eb84`） |
| 6 | COM | 0x10061160 |
| 7 | BATTERY | 0x10061134 |
| 8 | SOUND | 0x100614a4（1kHz SOUND） |
| 9 | CURRENT | 0x100611e0 |
| 10 | DPD | 0x10061248（"ENTER DPD"，`pm_power_down(0)`） |

操作（`testmode_state` 0x10039618）：A 移動游標、B 執行；`0x1801c39c != 0` 時自動依序執行全部。離開 → `PSID_DEBUGMENU(0)`。**這是零售機進入 debug 選單的唯一靜態可見路徑。**

**`PSID_DEBUGMENU`(0) [確認]**：頁表 0x1009317c（11 頁 × 12 bytes：`id, name*, items*`；item 8 bytes：`label*`（0 → 用場景名稱+5 跳過 "PSID_"）、`value`（bit30 = 跳頁，低 8 bit = 頁號；否則場景 id；-1 結束））。內容見 §3.4。目前頁/游標存 0x18001be4/0x18001be6。另有 `PSID_DEVELOP_WDTTEST`(0x0e)：A → `pm_power_down(1)`+reset、B → 忙等 2.5 s 觸發 WDT、C → `pm_dpd_seconds(60)`、轉盤按壓 → 離開，畫面顯示 EMA/NOW 電壓、ADC 原始值與 RTC 時間。

### 2.9 UART monitor（UART0）

- **[確認] 硬體**：UART0 = 0x4000a000（`FUN_1008eee8` index 0），**TX = P0.0（無測試點）、RX = P0.1 = TP48（下拉）**。**460800 baud**（`0x1801caf0 = 0x70800`，時脈 96 MHz），8N1，RX 中斷 IRQ36。不是 prongs（prongs = UART1）。可用 monitor 指令 `baudrate` 改（`FUN_10077434`）。
- **[確認] 解鎖**：`mon_printf`/`mon_puts` 在 `g_mon+0x1a8 bit3` 未設時**完全不輸出**（連 banner `** UART Monitor Service Ver.1.00 **` 都不會出現）。RX ISR 把字元收進 `g_mon` 緩衝（`\b` 退格、`\n` 忽略、`\r` 也存入並設 bit5；最長 63 字）。`mon_poll` 收到一行時：
  - 若為 `help` → 列出已註冊指令（仍受 bit3 限制，未解鎖時看不到）。
  - 未解鎖：`mon_check_password(len, buf)`(PRAM 0x1e9c)：**len 必須 = 9**，`buf[i] ^ key[i] == table[0xe18c+4i]`，解出字串 **`mY6$O$xw`**（第 9 字為 `\r`）。成功 → `mon_register("d", mon_dispatch)`(0x10076d74)，設 bit3。
  - 已解鎖：以空白切 argv（最多 8 個，`FUN_100665d4`），找已註冊指令（16 槽 × 0x14 bytes，於 `g_mon+0x60`）並呼叫 `handler(argc, argv)`。
- **語法**：`d <cmd> [args]`（`mon_dispatch` 用 `argv[1]` 比對）。系統類指令：`ver`、`wdreset`（死迴圈等 WDT）、`sysreset`、`wdtimer`（忙等 2.5 s）、`pdown`、`sleep`（`flags |= 0x40` → 0x75）、`savedata`、`advance`、`iomap`、`rotenc`、`clrdata`、`rtcint`、`language`、`lowbattery`（印電壓並 `flags |= 0x80`）、`fieldtype`、`weather`、`romtype`、`baudrate`，以及 `docs/re-notes.md` 列的遊戲狀態指令。
- **[確認]** 另有「movie」模式（`g_mon+0x1a8 bit1`）：每 frame 印 `BINARY 32768 %d` 後以 UART0 原始送出整張 framebuffer，等 `ACK`/`NAK`（5 s 逾時），`0x1a6` 張後 `MOVIESTOP`。
- **[確認]** SDK 的 `printf`（0x10081be4，如開機 `[example]flash id:%x`）走 UART index `*0x18000a26 = 2` → **UART1**（prongs）。

## 3. 資料表

### 3.1 場景表 — 0x10094830，28 B × 0x89

完整欄位（id, name, init, update, draw, exit, flags）：`docs/spec/data/scenes.csv`。分組：
0x00 DEBUGMENU；0x01–0x17 DEVELOP_*；0x18–0x1a VIEWER_*；0x1b NOTICE_MSG；0x1c HOME；0x1d–0x21 HOME_*；0x22–0x28 OSEWA_*（GOHAN, OTEIRE, TOGETHER, FIELDSWITCH, HUNT, SETSUBI, POOPCLEANING）；0x29 CELL_STATUS；0x2a–0x37 LABOMENU/LBMENU_GAME00–06；0x38–0x47 INFO/SHOP/BOOK/HELP/SETTING 子畫面；0x48–0x5b EVENT_*；0x5c–0x64 LABOGAME_*；0x65–0x6b OSEWAGAME_*；0x6c STARTUP；0x6d INITIAL_LOGO；0x6e ROM_VER；0x6f DELETE；0x70 CORRECTTIME；0x71 LANGUAGE；0x72 DATETIME；0x73 BIRTHDAY；0x74 PLANETNAME；0x75 SYSTEM_POWERDOWN；0x76 SYSTEM_BATTERY；0x77–0x86 TAMASPACE_*；0x87 TESTMODE；0x88 INIT_DUMMY。

### 3.2 LABOMENU 項目表 — 0x10094188，8 B × 6（`u16 text_id, u16 0, u16 scene, u16 0`）

| idx | text_id | scene |
|---|---|---|
| 0 | 0 | 0x2b LABOMENU_INFO |
| 1 | 2 | 0x2c LABOMENU_GAME |
| 2 | 3 | 0x34 LABOMENU_SHOP |
| 3 | 5 | 0x35 LABOMENU_HELP |
| 4 | 1 | 0x36 LABOMENU_BOOK |
| 5 | 6 | 0x37 LABOMENU_SETTING |

### 3.3 HOME 圈選單 — 0x10094120，3 × `{u32 items*, u32 count}`；item 12 B：`u32 icon_id, u32 scene, u32 flags(bit0=灰)`

| 表 | 用於 | 項目（icon → scene） |
|---|---|---|
| 0（0x10094254, 5） | HOME_SPACE | 0x22→0x77 TUSHIN、0x23→0x7f LOOKSTARS、0x24→0x80 DECO、0x25→0x7e TRAVEL、0x26→0x7d RANK |
| 1（0x10094224, 4） | HOME_FIELD | 0x37→0x27 SETSUBI、0x38→0x26 HUNT、0x39→0x28 POOPCLEANING、0x3a→0x25 FIELDSWITCH |
| 2（0x10094290, 3） | HOME_TAMA | 0x44→0x22 GOHAN、0x47→0x24 TOGETHER、0x48→0x23 OTEIRE |

各項有前置條件（`0x10039eb8` state 5000）：不符時設 `0x18001c70` 訊息 id（0x69/0x6b/0xf2/0xf3/0xf5）→ NOTICE_MSG(0x1b)。

### 3.4 DEBUGMENU 頁表 — 0x1009317c

- 頁 0 DEBUGMENU：[STARTUP]→6、[HOME]→7、[LABO]→9、[DEVELOP]→2、[VIEWER]→3、[UART]→5、[MINIGAME]→4、[TAMA SPACE]→10、[SETTING]→8、CELL STATUS 0x29、ROTARY ENCTEST 0x06、BRIGHTNESS 0x07、TEST OSEWA 0x08、TEST LAB 0x09、SAMPLE1–3 0x0a–0x0c、SOUND TEST 0x0d、WDT TEST 0x0e、TEXT VIEW 0x0f、TESTMODE 0x87、POWER DOWN 0x14
- 頁 2 DEVELOP：GHOST TEST 5、ACTION TEST 3、CHARA TEST 4、ANIM TEST 2、COMMON CTRL 1、UNIQUE ID 0x15、TEST QR 0x16、TCP 0x17
- 頁 3 VIEWER：キャラ 0x18、イベント 0x19、ゴースト 0x1a
- 頁 4 MINIGAME：LABO_GAME00–06（0x5d–0x62, 0x64）、OSEWAGAMELIST 0x24、OSEWAGAME03/06/07（0x68/0x6a/0x6b）
- 頁 5 UART：UART TEST 0x10、UART AGEING 0x11、UART BYTE 0x12
- 頁 6 STARTUP：STARTUP 0x6c、DELETE 0x6f、LANGUAGE 0x71、DATETIME 0x72、BIRTHDAY 0x73、PLANT NAME 0x74
- 頁 7 HOME：TAMA HOME 0x1c、TAMA SPACE 0x1d、TAMA FIELD 0x1e
- 頁 8 SETTING：せっていメニュー 0x37、にちじひょうじ 0x44
- 頁 9 LABO：ラボメニュー 0x2a
- 頁 10 TAMA SPACE：TUSHIN 0x77、ARCADE 0x83、ARCADE GAME01/02 0x85/0x86、CREDIT 0x81
- 每頁最後 RETURN → 頁 0。頁 1 "SYSTEM" 只有 RETURN。

### 3.5 RW 初始資料（0x18000000，0xb30 B）

由 PRAM 0xe6a0 以 ARM `__decompress1` 解出（腳本邏輯：token 低 3 bit = 字面長度+1（0 → 下一 byte），高 4 bit = 匹配長度（0 → 下一 byte），bit3 = 回溯複製（offset 1 byte，長度+2）否則補零）。已確認位置：0x18000406 = 按鍵腳位表 `09 0b 0a 08`；0x18000a26 = SDK printf UART index = 2。

### 3.6 MMIO 對照

| 位址 | 周邊 | 證據 |
|---|---|---|
| 0x40000000 + n·0x1000 (n=0..7) | Timer0–7（timer3 TE 時戳；timer2/7 蜂鳴器 PWM [推測]） | `timer_base` 0x8800、0x4588/0x45e8/0x4658/0x31f0 |
| 0x40008000 / 0x40009000 | WDT0 / WDT1（+0 ctrl、+8 timeout、+0xc feed；鑰匙 0x5afa） | `wdt_base` 0x8884、0x4c64/0x4cdc/0x4d74 |
| 0x4000a000 | UART0（monitor），+0 THR，+4 ctrl(bit0 RX IRQ)，+0x14 status | 0x10076e3c、0x1008eee8 |
| 0x4000b000 | UART1（prongs：TCP/Paramix/jig；SDK printf） | 0x10060888 |
| 0x4000e000, 0x4000f000/f100/f120 | SPI0（LCD，P1.5 CS/P1.6 SCLK/P1.8 MOSI）+ 其 DMA | `spi_init` 0x3b84；`lcd_init` 0x1001c4e4 |
| 0x4002b000 | 第二種 SPI/串列模式（spi_init mode 1） | 0x3b84 |
| 0x40018000 / 0x40019000 / 0x4001a000 | GPIO P0 / P1 / P2（+0 data、+4 dir、+8 mode 2bit/pin、+0x1c IRQ status、+0x20 IRQ clear、+0x34 pull、+0x3c 保持值） | 0x2310、0x2020、0x23e4、0x2558 |
| 0x40020000/10, 0x40022100, 0x40024100, 0x40029100, 0x40030100, 0x40032100, 0x4002b100 | 各周邊 DMA 通道 | 0x845c |
| 0x40022000 | SPI flash controller（SPIFC，QSPI P0.15/P1.0–1.4）；+0x100.. DMA | 0x8714、0x35e4/0x3678/0x3754、DPD stub |
| 0x4002f000 | XIP/解密相關（SystemInit 驗證 0x10000000） [推測] | 0x867c |
| 0x40038000 | CRC 引擎（存檔 CRC） | 0x100065f0、0x88c0 |
| 0x4003a000 | ADC（+8 資料，P0.6 電池） | 0x3470、0x34b0 |
| 0x45000000 | 系統控制：+0 時脈源、+0x0c 時脈選擇、+0x10 除頻、+0x14 reset cause、+0x2c DPD 喚醒旗標、+0x100 周邊時脈閘 | 0x85a0、0x43a8、0x43dc |
| 0x45000200–0x4500023c | RTC/喚醒：+0x200 ctrl(bit7 valid)、+0x204 喚醒電平、+0x208 喚醒遮罩、+0x210..218 GPIO 保持、+0x230 alarm、+0x234 alarm ctrl、+0x23c DPD 映像位址 | 0x1fbc、0x229c、0x23e4、0x33e0 |
| 0x45000300 / 0x45000304 | 電源控制（bit0–2 睡眠模式、bit4 清 RTC IRQ、bit6–7 清 WKP、bit15 狀態）/ **RTC 秒計數器** | 0x20d4、0x3394 |
| 0x60000000 + off | flash 資料視窗（0x60d29000 版本、0x60d2e000 擴充資產、0x60111000 主資產、0x60e66000 下載區） | |
| 0xE000E010.. | SysTick；0xE000E100/180/200/280 NVIC；0xE000ED0C AIRCR 重置 | 0x100603ec、0x10069c30 |

LCD：128×128、RGB565（COLMOD 0x3a=0x05）、MADCTL 0x08、FRMCTR1–3 `08 3b 3b`；P0.4 RESET、P0.5 D/C、P1.9 電源、P0.7 背光 PWM、P1.10 TE（GMMan testpads）。

## 4. 新命名

```
00000148 __scatterload f
000001a4 __decompress1 f
00000204 __scatterload_zeroinit f
000002d4 __rt_entry f
00001dbc exit f
00004438 SystemInit f
000085a0 clk_init f
000043dc clk_get_sysclk f
000043a8 rst_get_cause f
00004358 delay_loop f
0000432c SysTick_Handler f
00002558 GPIO1_IRQHandler f
000033c8 RTC_IRQHandler f
0000030a WKP_IRQHandler f
00002bc4 HardFault_Handler f
00001e9c mon_check_password f
00003394 rtc_get_seconds f
00001f98 rtc_alarm_disable f
00001fbc rtc_set_alarm_dpd f
0000229c dpd_set_wake_image f
00003268 pm_enter_dpd f
000020d4 pm_sleep f
000023e4 gpio_set_wake_pin f
0000291c gpio_config f
000025c4 gpio_read f
00004c64 wdt_feed_hw f
00004cdc wdt_enable f
00004d74 wdt_set_timeout f
00008884 wdt_base f
00008800 timer_base f
00004588 timer_read f
00003470 adc_read f
00008b5c save_load f
00008bd8 save_commit f
00008908 save_crc f
00003b84 spi_init f
0000845c dma_base f
1007f81c main f
1006ac00 sys_init f
1006b0b4 main_loop f
1006b200 scene_update f
10018260 scene_get_desc f
100182b0 scene_has_flag f
10002844 scene_resolve_next f
100028cc scene_set_next f
1000274c scene_get_cur f
10002758 scene_get_next f
10002780 scene_get_prev f
100027e4 scene_should_exit f
10025a78 idle_update f
10025c6c idle_reset f
10002764 idle_seconds f
1000278c idle_exceeds f
10006170 input_poll f
10005d50 input_init f
1008fe74 rotenc_sample f
10005ea8 key_pressed f
10006150 key_held f
10005e50 key_released f
10005e7c key_pressed_or_repeat f
10005ec8 key_double f
10005de0 key_long_held f
10006140 dial_delta f
10006128 dial_pos24 f
100049cc battery_update f
100048cc battery_get_ema f
100048d8 battery_get_now f
10004860 battery_is_low f
100048e4 battery_ok f
10004754 battery_boot_check f
100047f4 battery_check_2v2 f
10012c90 gametime_update f
10012a0c boot_wake_handler f
10011b18 pm_sleep_until_next_event f
1002a178 pm_power_down f
1002a398 pm_dpd_seconds f
1002a2f0 pm_dpd_until f
1002a1d8 pm_power_down_lowbat f
1008fd34 pm_wait_keys_release f
10068370 wdt_feed f
10068388 sys_panic_reset f
100683a4 wdt_start f
10069c30 sys_reset f
1002a55c rand f
1002a6d8 srand f
1002a584 rand_range f
10018e2c ms_now f
100603ec systick_config f
10076f3c mon_poll f
10076e3c mon_uart_init f
10076eb8 mon_init f
10076d74 mon_unlock f
10077404 mon_register f
10066bbc UART0_IRQHandler f
10066cdc UART1_IRQHandler f
1008ff68 ADC_IRQHandler f
10025a54 NMI_Handler f
10060888 comm_open f
100607f4 comm_close f
10060c70 comm_send_pkt f
10060d34 comm_poll f
1001c4e4 lcd_init f
1001f308 lcd_blit f
1001f6f0 lcd_busy f
1001f26c lcd_sleep f
1007b808 lcd_wait_te f
1007b40c fade_start f
1006953c task_set_state f
10069558 task_wait f
10069590 task_init f
100695b8 task_pre_update f
10003160 asset_init f
10003238 asset_init_additional f
1007f7ac pram_reload_and_reset f
10082268 run_downloaded_pram f
100378d0 startup_state f
10037848 powerdown_state f
10037724 battery_scene_state f
10039618 testmode_state f
10032770 debugmenu_state f
10045db0 labomenu_state f
1001abd0 home_router_init f
1002af28 ringmenu_create f
10094830 scene_table l
1009317c debugmenu_pages l
10094188 labomenu_items l
10094120 ringmenu_tables l
100a0b84 testmode_items l
18000406 key_pin_table l
18001cb4 g_scene_cur l
18001cb6 g_scene_next l
18001cb8 g_scene_prev l
18001cba g_scene_flags l
18001cbb g_scene_flags2 l
18001cbe g_idle_frames l
18001cc2 g_idle_off_frames l
18001cc4 g_idle_home_frames l
18001ccc g_idle_ms l
18001cdc g_key_held l
18001ce0 g_key_prev l
18001ce4 g_key_repeat l
18001ce8 g_key_down l
18001cec g_key_up l
18001cf0 g_key_double l
1800efc0 g_rotenc_raw l
1800efbf g_dial_delta l
1800efbc g_dial_pos24 l
1801c3a8 g_ms_tick l
1801c3a4 g_te_count l
18005de0 g_frame_count l
1801cae4 g_rand_seed l
18014078 g_mon l
1801407c g_comm l
18005d24 g_batt_ema l
18001be8 g_sys_flags l
18000be8 g_save_flags l
18000c18 g_next_wake_time l
```

## 5. 未解問題

1. **TE 實際頻率**：只能由 600 frames≈30 s 推出 20 fps。要確認可用 Capybara 跑韌體量 `g_ms_tick` 差（`0x1801c394`），或用示波器量 TP6(P1.10)。
2. **`g_scene_flags` bit3（回 DEBUGMENU）的設定者**：XIP 內沒有直接寫入；可能由 TCP 開發指令或 PRAM 設定。可在 `xip.dis` 搜 `strb` 到 0x18001cba 的間接寫法。
3. **DPD stub 細節**：0x45000234 的 0x100/0xa00 位元、0x20000f00 暫存器、`0x3f0(1)` 的確切行為（睡眠或立即重置）只是推測；boot ROM（GMMan snc7330-hidden-rom）可確認。
4. **WDT timeout 單位**：`wdt_set_timeout(0,0xff)`（0x4d74 寫 +8）對應幾 ms 未知；panic 用 0x20。
5. **TESTMODE 各測試項 handler**（0x10061134..0x100614a4）的具體判定與 SHIPPING 動作（可能清存檔/設出貨狀態）未逐一追。
6. **`pm_sleep_until_next_event` 0x10011b18 的完整事件清單與優先序**：本文只列出主要項；屬於照顧/時間子系統，需與 01-care.md / 02-evolution.md 對齊。
7. **`FUN_100134bc`（每 ≤60 s 一塊的遊戲推進）**內容屬照顧子系統，未在此展開。
8. 新遊戲流程 0x10038250..0x10039034 各狀態的確切分支（語言/日期/生日/星球名順序）只看了轉場目標。
