# 03 — 存檔、持久化、RTC/時鐘、Flash 配置（save / persistence / clock / flash layout）

來源：`re/decomp-all/ram.c`（PRAM，0x0000xxxx）、`re/decomp-all/XIP.c`（0x10xxxxxx）、`dumps/paradise-glacier-20261019-113049.bin`（flash）、`dumps/glacier-20261019-1923.tamasave`、`dumps/dpd.bin`。
解碼工具：`docs/spec/tools/decode_save.py`（stdlib；吃 16 MiB dump 或 .tamasave，驗 CRC 並印出欄位）。

## 1. 摘要

- 遊戲的全部持久狀態是 SRAM **0x18000be8–0x18001be3 這一塊 0xffc bytes**（下稱 `save` 影像，`save+N` = RAM `0x18000be8+N`）。g_tama（0x18000cf8 = save+0x110）、g_care（0x18000d14 = save+0x12c）都在裡面。緊接在後的 0x18001be4–0x18001cb3（0xd0 B）是每次開機清零的 runtime 區，不存檔。
- Flash 有兩個 4 KiB slot：主 **0xefe000**、備份 **0xeff000**（本機已確認）。格式 `u16 crc; u16 ~crc; u8 image[0xffc]`，CRC 是 **CRC-16/ARC**（poly 0xA001 反射、init 0、xorout 0），由硬體 CRC 引擎算。每次寫入都是「兩個 slot 依序 erase+program+讀回驗證」。
- 開機（每次 DPD 喚醒也是完整開機）由 PRAM `save_load_boot` 0x8b5c 從 flash 讀回 RAM：slot0 有效就載入，slot1 有效再覆蓋；只有一邊壞就用好的那邊補寫；兩邊都壞就初始化預設值並清 RTC。
- 寫入時機：進入帶旗標 0x10 的場景（HOME、所有照顧場景、Labo 選單）、進 DPD 前、每次喚醒處理完、日期變更、設定時鐘/語言、通訊註冊、死亡/進化等事件。**找不到週期性自動存檔**。
- 時鐘：RTC 只有一個 32-bit 秒計數器（MMIO 0x45000304），DPD 時持續走。現在時刻 = `rtc_base_day`（save+0x08，2025-01-01 起算的日數）+ `rtc_base_sec`（save+0x0c）+ RTC 計數。設定時鐘時把計數器歸零、存新的 base。
- 離線時間：所有狀態都用「上次處理到的時刻」`last_tick_time`（save+0x18，Y/M/D h:m:s）。開機/喚醒/主迴圈每次都把 now−last 以 ≤60 秒為單位補跑 `care_tick`，**最多補 24 小時（1440×60 s + 60 s），多的直接丟棄**。跨日只呼叫一次 `care_day_change`。手動設定時鐘時 last 直接設成新時間，不補跑。
- 待機：閒置 600 frame 進 `PSID_SYSTEM_POWERDOWN`，算出下一個事件時間（最多 +3600 s）設成 RTC alarm、強制存檔、進 DPD。DPD 喚醒 = 重開機；若還沒到 alarm 就直接再睡，到了就補跑後再決定要不要亮螢幕。
- Glacier 版 flash 配置跟 GMMan 文件（Land 版）不同：版本資訊在 **0xd29000**（不是 0xd49000），DLC staging/slot、ghost 區都往前移 0x20000，ghost 區擴大成 30 個 0x4000 slot。完整表見 §2.6。

## 2. 詳細規則

### 2.1 Save slot 格式與 CRC

| 規則 | 證據 | |
|---|---|---|
| Slot 位址 = `0x60efe000 + slot*0x1000`，slot ∈ {0,1}，其他值 → fault | PRAM `save_slot_addr` 0x8adc | [確認] |
| Slot 內容：`+0 u16 crc`、`+2 u16 ~crc`、`+4 image[0xffc]`；image 原樣 memcpy 到 0x18000be8 | PRAM `save_slot_load` 0xa8a8、`save_slot_write` 0xab40 | [確認] |
| 有效條件：`crc == ~ncrc` **且** `crc == CRC(image)` | 0xa8a8 | [確認] |
| CRC：硬體引擎 0x40038000，mode 0；mode 表 0x100a00bc（6×16 B：`{mode, poly, refin?, refout?}`：0:0xA001、1:0xA001/1、2:0xA6BC、3:0x8408、4:0xEDD1、5:0xEDB88320/1/1）。軟體等價為 CRC-16/ARC（init 0, xorout 0）；本 dump 兩 slot 存 0xb522，自算 0xb522 | `hw_crc16` PRAM 0x88c0 → `hw_crc_start` 0x10006670；`decode_save.py` | [確認] |
| CRC 引擎只接受 SRAM 0x18000000–0x18020000 的來源（否則回傳 3），所以 slot 驗證會先把 flash 讀進 heap 再算 | 0x10006670、`save_slot_crc` PRAM 0x8908 | [確認] |
| 寫入一個 slot：heap 4 KiB ← image；填 crc/~crc；關中斷；`spi_flash_erase4k`(PRAM 0x34f0)；`spi_flash_program`(PRAM 0x3968, 0x1000 B)；開中斷；讀回重算 CRC，不符 → fault（`FUN_10068388`，推測是 reset/assert）| 0xab40 | [確認] |
| `save_write(mode)` PRAM 0xaaa0：先 `FUN_10024fd8`，若 SPI status ≠ 0x40 且 runtime `0x18001be9 bit5`=0 就設 status 0x40（推測：解除寫保護），算一次 CRC，mode 0=只寫 slot0、1=只寫 slot1、2=兩個都寫（slot0 先）| 0xaaa0 | [確認] |
| `save_commit(force)` PRAM 0x8bd8：以目前 RAM 影像算 CRC；若 slot0 或 slot1 的 `crc/~crc` 跟它不同，或 `force≠0`，就 `save_write(2)`。所以 `force=0` 是「有變更才寫」，`force=1` 是無條件寫 | 0x8bd8 | [確認] |
| 讀 flash 一律走 `spi_flash_read`(PRAM 0x3754, mode 3) 而不是直接讀 0x60xxxxxx 視窗（少數只讀 header 的地方直接讀視窗）| 0xa8a8、0x10074c70 | [確認] |

### 2.2 開機載入與主/備選擇

| 規則 | 證據 | |
|---|---|---|
| `save_load_boot` PRAM 0x8b5c：清 runtime 區 0x18001be4（0xd0 B）→ `save_init_defaults` → `ok0=slot_load(0)`。<br>• ok0：`ok1=slot_load(1)`（有效就**覆蓋** RAM）；若 !ok1 → `slot_load(0)` 再 `save_write(1)` 修 slot1。<br>• !ok0：`ok1=slot_load(1)`；若 !ok1 → `save_init_defaults`、`rtc_restart`(0x1002a25c)、`save_write(2)`。slot0 壞而 slot1 好時**不會立刻**修 slot0（下一次 `save_commit` 因 crc 不符會兩邊都寫）| 0x8b5c | [確認] |
| `save_init_defaults` PRAM 0x89a8：image 全清 0；`rtc_base_day = date_to_days(2025→2026, 1, 1) = 365`（2026-01-01）；`rtc_base_sec = 0xa8c0 = 43200`（12:00:00）；`play_days(save+0xb0)=1`；從版本區 0x60d29000 讀 `+0 u32` → `g_rom_field_type`(0x18001cb0，0xffffffff 視為 1)、`+0xff4 u16` → 0x18001cb2；`settings(save+0x04) = (x & ~0x1f) | 0x11`；`FUN_1001e594(0x1f3e, ?)`（給一個初始道具，id 7998）；`0x18000dd7 = FUN_1002b314(field_type)`（初始區域）| 0x89a8，反組譯確認第 4 參數 r3=1 | [確認] |
| `save_mark_started` PRAM 0x8ab0：`save+0 |= 1`（遊戲已開始）、`device_rand(save+0x6c) = FUN_00004b70()`、`FUN_1001e558()`、`save_commit(1)`。由初始設定流程呼叫（0x10039034、0x10090f2c）| 0x8ab0 | [確認] |
| 開機 `system_boot` 0x1006ac00 順序：讀 RTC 計數、wake reason → 電量檢查 → `save_load_boot` → 若電量不足且 `save+0 bit3` → 直接進 DPD（0x1002a1d8）→ 若 RTC 計數==0 或 RTC 沒在跑 → runtime `0x18001be8 |= 1`（時鐘失效）→ `rtc_init` → 主資產 `asset_init_main`(0x10003160) → `wake_from_standby`(0x10012a0c) → … → `Asset_InitAdditional`(0x10003238，含 `Asset_CheckValidSaveData`) → 若 `save+0 bit0`：`Tama Ghost : Verify`(`ghost_slot_verify(0)`)，失敗設 `0x18001be9 bit4` → `Item Storage : Verify`(`item_storage_verify` 0x1001e658) → `Grazing Tama : Verify`(`grazing_verify` 0x10018d94) | 0x1006ac00 | [確認] |
| `Asset_CheckValidSaveData` 0x10003048：對 40 個 DLC slot，若 `dlc_mask`(save+0xaac) 有該 bit 但 `dlc_slot_valid` 失敗 → `dlc_slot_delete`（清 bit，印 `ADDITIONAL DELETE SLOT`）；之後 `item_storage_verify`、`save_commit(0)`。**它不是驗 flash 存檔，而是驗「存檔裡引用的 DLC 資源還在」** | 0x10003048、0x100009ac | [確認] |
| `item_storage_verify` 0x1001e658：640 個倉庫 slot 中 item 定義已不存在的 → 刪除（回傳 false）；裝備 u16[3]（save+0x1a0）無效則清 0；3 區域 × 6 擺設 × 2 層（save+0x1a6、+0x1b2）無效則清 0 | 0x1001e658 | [確認] |
| `grazing_verify` 0x10018d94：3 個 field，各自對 mask 內每隻放牧 tama 驗對應 ghost slot 的 checksum；任一失敗 → 該 field 的 `grazing[f]`(save+0x1f4+4f) 整個清 0 | 0x10018d94 | [確認] |
| `ghost_slot_verify(slot)` 0x100183fc：`total_length = *(u32*)(pkg+0x10c)`；需 ≥0x600 且 ≤ slot size（表 0x1009bc38）；以 `spi_flash_read` 分 4 KiB 讀，所有 u32 相加（含 checksum 與 complement 本身，二者相加為 0）== `pkg[0]` | 0x100183fc | [確認] |

### 2.3 存檔時機（誰呼叫 `save_commit`）

| 觸發 | force | 呼叫者 | |
|---|---|---|---|
| 進入場景且場景 flag 含 0x10，且上一個場景不是 0x6c（開機場景）。場景表 0x10094830（0x1c B/筆：`u32 id, init, update, draw, exit, u32 flags, char* name`，137 筆）。帶 0x10 的：PSID_HOME(0x1c)、PSID_OSEWA_GOHAN/OTEIRE/TOGETHER/FIELDSWITCH/HUNT/SETSUBI/POOPCLEANING(0x22–0x28)、PSID_LABOMENU 系列(0x2a–0x37)、PSID_SYSTEM_BATTERY(0x76，flags 0x1a) | 1 | `scene_run` 0x1006b200 | [確認] |
| 進 DPD 前（`care_schedule_alarm`）| 1 | 0x10011b18（兩處）| [確認] |
| 每次開機/喚醒處理結束 | 1 | `wake_from_standby` 0x10012a0c 結尾 | [確認] |
| 補跑後 runtime `0x18001be8 bit2`（日期變更待處理）時，`FUN_10083984` 後 | 1 | `care_time_update` 0x10012c90 | [確認] |
| 請求完成（如進化換 ID、Option 記錄）| 1 | `FUN_10068aa0` case …/0x15 | [確認] |
| 新生／進化（`g_tama` 亂數重抽、圖鑑）| 1 | 0x1000f708、0x1000fc98 | [確認] |
| 手動設定時鐘（設定場景）| 1 | 0x10038d24、0x1004f7d8 | [確認] |
| 語言設定（monitor `language`）、tamaid、tamacolor、shopdemo | 1 | 0x10073010 等 | [確認] |
| 其他 force=1：0x10034168（設 `next_alarm` 後存，SPI 狀態相關）、0x100899c0（mission 13 設 50 後）| 1 | | [確認] |
| DLC 註冊/刪除、好友註冊、QRUID 產生、通訊場景結束、Asset 檢查 | 0 | 0x10000b88、0x10088bd8、0x10017588、0x10006e14、0x100633f8、0x10003048 | [確認] |
| 低電量：清 `save+0 bit1,bit2` 後存 | (r0) | 0x10037724 | [確認] |
| 沒有找到計時器驅動的定期存檔；停在 HOME 而不切場景時，狀態只活在 RAM，直到下一次切場景/進 DPD | — | 全部 `save_commit` 呼叫點如上 | [推測：以呼叫點窮舉為據] |

### 2.4 RTC 與時間表示

| 規則 | 證據 | |
|---|---|---|
| RTC MMIO（base 0x45000000）：`+0x304` u32 秒計數（`rtc_counter_get` PRAM 0x3394）；`+0x200` 控制：bit7=運作中（`rtc_is_running` 0x3280 讀、`rtc_enable` 0x345c 設），bits0–2 中斷週期、bit3、bit5/6（`rtc_config` 0x33e0）；`+0x300 |= 0x10`：計數器清除（`rtc_reset` 0x32a4 先清控制位再設）[清除語意推測]；`+0x300 bit15` 由 0x33a0 讀；`+0x230` alarm 比較值（寫入 target−1）、`+0x234 bit8` alarm 致能、`+0x23c` DPD 開機映像位址 | PRAM 0x3394/0x3280/0x32a4/0x33e0/0x345c/0x1fbc/0x229c | [確認]（暫存器名稱為推測）|
| 計數器每秒 +1（`RTC_INT_1_SEC` 為預設；monitor `rtcint` 0..7 = 1s,16ms,63ms,250ms,5s,10s,30s,60s 只改中斷週期，存在 save+0x34）| 0x10074d84、字串 0x1009195a | [確認] |
| 日期序號：`date_to_days(Y,M,D)` 0x10006808 = Σ_{y=2025}^{Y-1}(365/366) + Σ_{m<M} days_in_month + D − 1。閏年判斷 `FUN_1007d3b0`；月天數表 0x10091190（`days_in_month` 0x10006a1c，2 月閏年 +1）| 0x10006808、0x10006a1c | [確認] |
| datetime 結構 = 6×u16：`year, month, day, hour, min, sec` | 0x10006970、0x10006be0 | [確認] |
| `datetime_to_secs` 0x10006be0 = days×86400 + h×3600 + m×60 + s（2025-01-01 00:00 = 0）；`secs_to_datetime` 0x10006c24 為反函數 | | [確認] |
| **現在時刻** `rtc_now_datetime` 0x10006970：`t = rtc_counter + rtc_base_sec`；`date = days_to_date(rtc_base_day + t/86400)`（`FUN_1000ada0(2025, n)`）；`h = t/3600 % 24`、`m = t/60 % 60`、`s = t % 60`。主迴圈每 frame 把結果寫進 runtime `g_rtc_now` 0x18001bec | 0x10006970 | [確認] |
| **設定時鐘** `rtc_set` 0x10006b2c(Y,M,D,h,m,s)：Y ≤ 2024 → 忽略；Y > 2079 → 夾到 2079；`rtc_base_day = date_to_days(Y,M,D)`；`rtc_base_sec = h*3600+m*60+s`；`rtc_restart`（0x1002a25c = `rtc_reset` + `rtc_init`：計數器歸零並重新啟動）| 0x10006b2c、0x1002a25c、0x1002a20c | [確認] |
| `rtc_rebase` 0x10006a7c：若時鐘有效（`save+0 bit0` 且 RTC 在跑）→ 讀 now 再 `rtc_set(now)`（把累積秒數折回 base、計數器歸零）；否則設 runtime 時鐘失效 bit。在 standby 補跑前與 `rtcint` 時呼叫 | 0x10006a7c | [確認] |
| 時鐘失效：開機時 RTC 計數 0 或 RTC 沒在跑 → runtime `0x18001be8 bit0`。wake reason 不含 bit1/bit3（冷開機）也設這個 bit。若遊戲已開始，`save+0 bit7 = 1`（時鐘遺失），`g_rtc_now` 強制成 2025-01-01 12:00:00，而且 `care_time_update` 在 bit7=1 時不補跑（只設 runtime bit7）。玩家在時鐘設定場景確定後才清 bit7 | 0x10012a0c、0x10012c90、0x1004f7d8 | [確認] |
| 設定場景確定時（0x1004f7d8）：`rtc_set(新時間)`，並把 `g_rtc_now` **和 `last_tick_time`(save+0x18) 都設成新時間** → 改時鐘不會補跑任何照顧時間（往前、往後都一樣）。再呼叫 `FUN_1000f508`，依 `day % 10`：0→清 save+0 bit6、5→清 bit5、其他→清 bit5|6；清 bit7 | 0x1004f7d8 | [確認] |
| ROM 版本區 `+0xff4 == 0xffff`（開發 ROM）時清時鐘失效旗標 | 0x10012a0c | [確認] |

### 2.5 離線/DPD 時間的計算與套用

| 規則 | 證據 | |
|---|---|---|
| 主迴圈 `main_loop` 0x1006b0b4 每 frame 呼叫 `care_time_update` 0x10012c90。條件：`save+0 bit0`=1、bit7=0、runtime `0x18001be8 bit6`=0、`save+0 bit3`=0（低電量時只更新 last，不跑照顧）。now = `datetime_to_secs(g_rtc_now)`、last = `datetime_to_secs(save+0x18)`；now<last → last=now（時鐘倒退不補）；now==last → return | 0x10012c90 | [確認] |
| 補跑：`dt=now−last`；dt>60 時 `for i<1440 { care_tick(t,60); t+=60; dt-=60; if dt<60 break }`，最後 `care_tick(t, min(dt,60))`。上限 86400+60 秒，**超出部分丟棄**（不會延後處理）| 0x10012c90 | [確認] |
| 若 `save+0x1c`（last 的 day 欄位，只比 u16 day，不比月/年）≠ 現在 day → `care_day_change` 0x10013234 **一次**；之後 `last_tick_time = g_rtc_now` | 0x10012c90 | [確認] |
| DPD 期間 RTC 計數器持續走，所以「斷電時間」不需另外記錄：喚醒時 `rtc_counter + base` 直接就是真實現在時刻，跟 `last_tick_time` 的差就是離線時間，走同一條補跑路徑 | 0x10006970、0x10012c90 | [確認] |
| 拔電池（RTC 停）→ 計數器歸零／停止 → 走「時鐘失效」流程，**不補跑**，等玩家重設時鐘（且重設時 last=新時間）| 0x1006ac00、§2.4 | [確認] |
| 進 DPD 的條件：閒置計數 `0x18001cbe` 每 frame +1（`idle_timer_tick` 0x10025a78）；`>= 0x18001cc2`（預設 600，部分場景 0x4b0/0x4ba）時設 `0x18001cba bit6` → 場景切換時 `FUN_10002844` 轉到 0x75 `PSID_SYSTEM_POWERDOWN`；`>= 0x18001cc4`（200）設 `0x18001cbb bit4`（推測：關背光/調暗）。按鍵重置計數（0x10025c6c）。frame 長度不在本子系統，秒數未換算 | 0x10025a78、0x10025c6c、0x1006b200、0x10002844、場景表 0x100954fc | [確認]/秒數[推測] |
| `PSID_SYSTEM_POWERDOWN` update（0x10037848）在 state 1000 印 `[SYSTEM] Deep Power Down` 並呼叫 `care_schedule_alarm` 0x10011b18 | 0x10037848 | [確認] |
| `care_schedule_alarm`：`save+0 = (save+0 & ~4) | 2`（bit1 = 待機中、bit2 清）；從所有待處理事件中找最早時間 `t_wake ∈ (now, now+3600]`，原因寫 `alarm_reason`(0x18000d84)；`next_alarm`(save+0x30) = `rtc_counter + min(t_wake−now, 3600)`；若原因 ≠0 → 清 `save+0 bit1`；`save_commit(1)`；進 DPD（`enter_dpd_alarm` 0x1002a2f0 或 0x1002a268）。事件清單與時間見 01-care.md §2.1 | 0x10011b18 | [確認] |
| 進 DPD：關周邊；`rtc_set_alarm_dpd`(PRAM 0x1fbc)：`alarm = max(next_alarm, rtc+2)`，寫 `0x45000230 = alarm−1`、`dpd_set_boot_image(0x60110000, 0x1000)`（PRAM 0x229c 寫 0x4500023c）；`dpd_enter`(PRAM 0x3268) 關中斷後無限迴圈等待斷電 | 0x1002a2f0、PRAM 0x1fbc/0x229c/0x3268 | [確認] |
| DPD 韌體 flash 0x110000（1548 B 有效，與 `dumps/dpd.bin` 相同）：獨立向量表（SP 0x18001108、reset 0x1dd），只做低階喚醒處理（讀 0x45000200/204/208、`wfi`），不碰遊戲狀態。DPD 會丟失 SRAM；喚醒後重新從 flash 載入 PRAM/XIP 與存檔 | dpd.bin 反組譯（0x1dc–0x4e0）| [確認]／SRAM 丟失[推測：開機無條件 `save_load_boot`] |
| 喚醒原因 `wake_reason_get` PRAM 0x43a8 = `MMIO 0x45000014 | ((0x4500002c & 2) << 2)`。`wake_from_standby(reason, rtcflag, rtc)` 0x10012a0c：bit1 → 清 runtime 時鐘失效 bit、非一般開機；bit3（RTC alarm）→ `silent = (rtcflag==0)`；bit4 → 清 `save+0 bit3`；其他（冷開機）→ runtime 時鐘失效 bit。`boot_count`(save+0xb2)++ | 0x10012a0c | [確認]／各 bit 硬體意義[推測] |
| Silent alarm 喚醒：若 `rtc < next_alarm`（醒太早）→ 不補跑直接再進 DPD；否則若 `save+0 bit1` 仍是 1 → `rtc_rebase`、補跑、若仍 bit1 → 再 `care_schedule_alarm`（又存檔、又睡）。bit1 被清（有事件）→ 繼續正常開機亮螢幕 | 0x10012a0c | [確認] |
| 一般開機收尾：`care_time_update`；`save+0 = (save+0 & ~2) | 4`；`save_commit(1)`；runtime `0x18001be8 |= 2` | 0x10012a0c | [確認] |
| 後果：待機中每 ≤1 小時喚醒一次，每次至少一次兩-slot 寫入（喚醒存 + 再睡存，共 2 次 → 4 次 sector erase）| 上列 | [確認] |

### 2.6 Flash 配置（White Glacier，本 dump 驗證）

| Flash 範圍 | 大小 | 內容 | 讀寫者（證據）| |
|---|---|---|---|---|
| 0x000000–0x000fff | 4K | 韌體 header `SONIXDEV`，指向 0x60001000、0x60011000 | re-notes | [確認] |
| 0x001000–0x010fff | 64K | PRAM 映像（AES 加密）| re-notes | [確認] |
| 0x011000–0x10ffff | ~1M | XIP 碼（0x10000000），本 dump 實際到 ~0xbffff | re-notes | [確認] |
| 0x110000–0x110fff | 4K | DPD 韌體 | PRAM 0x1fe8 `dpd_set_boot_image(0x60110000)` | [確認] |
| 0x111000–0xc33ba3 | 0xb22ba4 | 主資產 ARC2（3 個 entry），byte-sum checksum 0x4f5ded07 驗證通過；0x1b7000–0x1bbfff 為 0 | `asset_init_main` 0x10003160（`arc2_check(0x60111000)`）| [確認] |
| 0xc34000–0xd28fff | | 空（0xff）| | [確認] |
| **0xd29000–0xd29fff** | 4K | **版本資訊**：`+0 u32 field_type`（=6 Glacier，`romtype`）、`+0xff4 u16 major`(3)、`+0xff6 u16 minor`(1) → `RomVersion[6] 3.01`；`+0xff8 u32 = ~sum`、`+0xffc u32 = sum`（0x69347c4d = flash[0:0xd2a000] 全部 byte 相加，含這 4 B 本身）| `save_init_defaults` PRAM 0x8a04、`mon_romver` 0x10074c70、`jig_rom_test` 0x10029f38（掃到 0x60d2a000）| [確認]／sum 定義[推測：數值吻合] |
| 0xd2a000–0xd2dfff | 16K | DLC 下載 staging（ARC2）| `dlc_register_from_staging` 0x10000b88 | [確認] |
| 0xd2e000–0xdcdfff | 40×16K | DLC slot（"additional"），slot n = 0xd2e000+n*0x4000，有效 = ARC2 magic + byte-sum；bit 在 `dlc_mask` | 0x10003238、0x10003384、0x10000b0c、0x1006aab4 | [確認]（本機無 DLC）|
| 0xdce000–0xe45fff | 30×16K | Ghost slot 0..29（表 0x1009bbb8 位址、0x1009bc38 大小）：0 = 自己的 tama（"ghost_tama"）；1..24 = 放牧 tama，slot = 1 + field*8 + idx；25..29 = 死亡 ghost（"ghost_dead"，monitor `destroy dead n` → slot 0x19+n）| `ghost_slot_addr` 0x100186a8、0x100721b4 | [確認] |
| 0xe46000–0xe60fff | 0x1b000 | Ghost slot 30：TCP 接收緩衝（通訊收到的 ghost 套件）| `FUN_10002a90`/`FUN_1008d7b0` TCP RECV | [確認] |
| 0xe61000–0xe65fff | 0x5000 | 自己的星球截圖（註冊好友時複製到好友 slot）| `friend_register` 0x10017588 | [確認] |
| 0xe66000–0xe80fff | 0x1b000 | Ghost slot 31：TCP 送出用的匯出套件；同一區也當 patch 接收：`+0` 解密後 magic `PDLO`(0x4f4c4450) 且簽章 8 B 通過 → `patch_exec` 0x10082268 從 0x60e66010 起執行 `[+4]*4` bytes | 0x1006d990、0x10082268、0x10089038 | [確認] |
| 0xe81000–0xe85fff | 0x5000 | 自己的 tama 截圖（SSHT）| 0x100629dc、0x10063448（`FUN_1002c524(0x60e81000)`）| [確認] |
| 0xe86000–0xefdfff | 24×0x5000 | 好友截圖：好友 n 的星球 = idx 2n、tama = idx 2n+1，`0xe86000 + idx*0x5000`，最多 12 位好友 | `friend_ss_addr` 0x1002c6c0 | [確認] |
| **0xefe000–0xefefff** | 4K | **存檔主 slot** | PRAM 0x8adc | [確認] |
| **0xeff000–0xefffff** | 4K | **存檔備份 slot** | PRAM 0x8adc | [確認] |
| 0xf00000–0xffffff | 1M | 空（0xff），無參照 | MOVW/MOVT 掃描無參照 | [確認] |

跟 GMMan `flash_layout.md`（Land）對照：版本 0xd49000→**0xd29000**、staging 0xd4a000→**0xd2a000**、DLC 0xd4e000→**0xd2e000**（同為 40 slot）、ghost 0xdee000→**0xdce000**（多 8 slot，共 30）。0xe46000 之後完全相同。所有 0x60xxxxxx 常數以 MOVW/MOVT 掃描全部 XIP 與 PRAM 取得（見 §3.4）。

### 2.7 本機 flash 內容（用途對照）

| 範圍 | 實際內容 |
|---|---|
| 0xd29000 | `06 00 00 00` … `ff4: 03 00 01 00` + sum |
| 0xdce000 (slot0) | ghost type 1，eye_chara 3071（= 現在的寵物），total 0x25dc，checksum OK |
| 0xdd2000 (slot1) / 0xdf2000,0xdf6000 (9,10) / 0xe12000,0xe16000 (17,18) | 放牧 tama：field0 1 隻、field1 2 隻、field2 2 隻（與 save `grazing[]`、`grazing_count` 1/2/2 一致）|
| 0xe32000,0xe36000,0xe3a000 (25–27) | 3 個死亡 ghost（與 save 死亡紀錄 3 筆一致）|
| 0xe46000 | 上一次接收的 ghost（eye 2069）|
| 0xe66000–0xe76fff | 匯出區（非明文）|
| 0xe81000 | SSHT，size 0x1000，sum OK |
| 0xe61000、0xe86000+ | 空：沒有好友 |

### 2.8 Ghost / 截圖 / DLC 標頭（header 層級）

- **Ghost 套件**（GMMan `ghost_data.md` 格式成立）：`+0 u32 checksum`、`+4 u32 -checksum`、`+8 u32 flags`（bits0–1 type：0 完整、1 只有眼睛）、`+0xc u16 chara_id`、`+0xe u16 eye_chara_id`、`+0x10 i8 color`、`+0x12 name[9][13] u16`、…、`+0x10c u32 total_length`。本機所有 ghost 都是 type 1、`chara_id`=4017(0xfb1)、`eye_chara_id`=實際角色，≤0x2c00 B，放在 0x4000 slot。checksum = 套件前 total_length bytes 所有 u32 之和（含 checksum/complement，二者和為 0）[確認：0x100183fc + 9 個 slot 實算]。
- **截圖 SSHT**：`+0 u32 sprite_sum`、`+4 u32 ~sum`（和 = 0xffffffff）、`+8 'SSHT'(0x54485353)`、`+0xc u32 size`（≤0x5000，含 0x200 header）、`+0x10 name[9][13] u16`，sprite 從 +0x200 開始 [確認：0x100823bc 建立、0x10082378 計算 sum、0xe81000 實算]。
- **DLC/ARC2**：`'ARC2', u32 sum, u32 length, u32 count`；sum = 從 +8 起 `length+8` 個 byte 的和 [確認：`arc2_check` 0x1000299c]。DLC 的「資產 ID」= `FUN_10023ea4`（archive entry 0 內 `+0x4c` 指到的 u16）[確認]。
- **好友記錄**在 save：`friend_mask` u16（save+0x9ec，12 bit）、`friend_order` u8[12]（save+0x9ee，MRU 順序，新的插在前面）、`friend_info` 12×0xc B（save+0x9fc，註冊時從 caller 結構複製 3 個 u32，`+8 u16` 夾到 1..4、`+0xa u16` 夾到 0..5）[確認：0x10017588]。
- **放牧**在 save：`grazing[f]`（save+0x1f4+4f，f=0..2）u32：byte0 高 4 bit = 最後加入 idx，byte1 = 8 隻佔用 mask，byte2 = 本輪已出現 mask；`grazing_last_field`(save+0x200)&3；`grazing_count[f]` u8（save+0x8d+f，≤99）[確認：0x1001887c/0x100188d8/0x1001892c/0x10018978/0x10072ab0]。

### 2.9 .tamasave（Paramix Studio 備份，v3）

| 偏移 | 內容 | |
|---|---|---|
| 0x00 | `"TAMASAVE"` | [確認] |
| 0x08 | u32 version = 3 | [確認] |
| 0x0c | u32 Unix 時間（0x6ad5fdcd = 2026-10-19 11:23:57 UTC = 檔名的 19:23 +8）| [確認] |
| 0x10 | u32 0 | |
| 0x14, 0x18 | u32 0x86a, 0x869：裝置 RTC 計數（≈ 存檔時的 2118 s + 35 s，吻合）| [推測] |
| 0x1c–0xbf | 一串 u32（含 `0xa55a0000` 標記、`(reg_off, value)` 對：0x100→0x157eb7、0x10c→0x1ffffd、0x200→0xe0、0x230→0x869、0x234→0x100、0x300→0x10 …），看起來是 0x450000xx PMU/RTC 暫存器快照。Paramix 私有，非遊戲資料 | [推測] |
| 0xc0 | chunk 序列：`u32 page (= flash_off>>12), u32 len (=0x1000), u8 data[len]`，每個 0x1008 B；本檔 470 個，連續涵蓋 **flash 0xd2a000–0xefffff**（DLC staging 到存檔備份；不含版本區與資產）| [確認] |
| 0x1d6f70 | trailer：`"PMXE"`, u32 field_type (6), u32 start (0xd2a000), u32 end (0xf00000) | [確認] |

這 470 頁與 flash dump 對應位置**逐 byte 相同**（含兩個存檔 slot），所以 tamasave = 使用者資料區的完整映像，還原時直接寫回這些頁即可 [確認：`decode_save.py` 兩種輸入輸出相同]。

## 3. 資料表

### 3.1 存檔影像欄位表（save+off；RAM = 0x18000be8+off；flash = 0xefe004+off）

本機值來自 `decode_save.py`（slot0/1 皆 CRC 0xb522 有效）。

| save+ | RAM | 型別 | 名稱 | 本機值 | 證據 |
|---|---|---|---|---|---|
| 0x000 | 0x18000be8 | u8 | sys_flags：bit0 遊戲已開始、bit1 待機中（silent wake 可再睡）、bit2 已處理喚醒、bit3 低電量、bit4 debug 預設資料觸發（`FUN_10013158`）、bit5/6 依 day%10 的事件旗標、bit7 時鐘遺失 | 0x05 | §2.4/2.5 |
| 0x001 | 0x18000be9 | u16 | sys_flags2（bit0 = 今天已領 MICROSCOPE +50，跨日清；bit1 死因相關，見 01-care）| 0 | 0x10013234 |
| 0x004 | 0x18000bec | u32 | settings：bits3–4 音量 0..3 → 0/30/60/100%（`FUN_10030bb4`），預設 bits0–4 = 0x11 | 0x3（音量 0）| 0x1006ac00, 0x10030bb4 |
| 0x008 | 0x18000bf0 | i32 | rtc_base_day | 656（2026-10-19）| 0x10006b2c |
| 0x00c | 0x18000bf4 | u32 | rtc_base_sec | 67660（18:47:40）| 0x10006b2c |
| 0x010 | 0x18000bf8 | i32 | 轉盤正轉累計 | 19832 | `mon_rotenc` 0x10074cf4（Turn = (plus−minus)/24）|
| 0x014 | 0x18000bfc | i32 | 轉盤反轉累計（負值）| −12158 | 同上 |
| 0x018 | 0x18000c00 | dt[6] | last_tick_time | 2026-10-19 19:22:58 | 0x10012c90 |
| 0x024 | 0x18000c0c | dt[6] | 初始設定輸入的日期（設定流程寫入，profile 頁 1 顯示其月/日）| 2026-08-18 15:09:00 | 0x100387dc, 0x10047f10 |
| 0x030 | 0x18000c18 | u32 | next_alarm（RTC 計數值）| 2280 | 0x10011b18 |
| 0x034 | 0x18000c1c | u32 | rtcint 模式 0..7 | 0 | 0x10074d84 |
| 0x038–0x06b | | | 未見參照，全 0 | 0 | |
| 0x06c | 0x18000c54 | u32 | device_rand（開始遊戲時 `FUN_00004b70()`，用於通訊/截圖）| 0x7c195f54 | PRAM 0x8ab0, 0x10063448 |
| 0x070 | 0x18000c58 | u16[9] | 星球名（glyph code，0 結尾）| 0x145 0x142 0x149 0x149 | `setplanetname` 0x1007593c |
| 0x082 | 0x18000c6a | u16 | bits0–3 / bits4–8：profile 頁 2 的兩個值；bits9–12：語言+1（0 ja,1 en,2 fr,3 de,4 pt,5 es,6 it,7 zh,8 ko）| 0x0438 → 8 / 3 / **en** | 0x10073010, 0x10047f10 |
| 0x084 | 0x18000c6c | u8 | 星球等級：低 4 bit = rank(1..10)，高 4 bit = rank−1 | 0x99（rank 9）| `planetrankup` 0x10073d8c |
| 0x08c | 0x18000c74 | u8 | 養成成年數（01/02 文件）| 0 | 0x1004dcd0 |
| 0x08d | 0x18000c75 | u8[3] | 各 field 放牧次數（≤99）| 1,2,2 | 0x10072ab0 |
| 0x090 | 0x18000c78 | u8[0x20] | 角色圖鑑 bitset（`FUN_1000580c`）| 10001000 11001000 15101000 … | 0x1000580c |
| 0x0b0 | 0x18000c98 | u16 | play_days（跨日 +1，≤999）| 40 | 0x10013234 |
| 0x0b2 | 0x18000c9a | u16 | 開機/喚醒次數 | 2739 | 0x10012a0c |
| 0x0b4 | 0x18000c9c | u16 | 每日計數（跨日清 0）| 0 | 0x10013234 |
| 0x0b6 | 0x18000c9e | u16 | 每日計數（跨日清 0；>2 時有分支）；0x18000ca0 起為其記錄陣列 | 0 | 0x10013234, 0x1004e1ec |
| 0x0bc | 0x18000ca4 | u32 | Gotchi 點數（≤99999）| 8070 | `points_add` 0x1000f170 |
| 0x0c0 | 0x18000ca8 | | Mission 01–13 計數 u16（cb6 為 1 B bitset）、0xcc4 u32 Mission 14（累計點數，≤99999）、0xcc8/0xcca u16 Mission 15/16、0xccc 3 B、0xccf/0xcd1/0xcd3/0xcd5/0xcd7 各 2 B bitset（Mission 17–22）| 248,42,20,46,58,109,3,…；M14 = 12870 | `missionqualifynum` 0x1007312c |
| 0x0f4 | 0x18000cdc | u32×3 | mission 狀態 bitset（cdc/ce0/ce4，22 bit）| 0,0,0 | 0x10025748–0x100258c8 |
| 0x100 | 0x18000ce8 | u16[3] | 每日計數（跨日清）| 0 | 0x10013234 |
| 0x106 | 0x18000cee | u8[5] | g_req 待處理 bitset（40 id）| 0c 84 03 40 00 | 01-care |
| 0x10b | 0x18000cf3 | u8[5] | g_req_done bitset | 34 84 01 40 00 | 01-care |
| 0x110 | 0x18000cf8 | u32 | g_tama（TamaData+0，亂數，重抽到不同）| 0xce375300 | 0x1000f708 |
| 0x114 | 0x18000cfc | u16 | chara_id | **3071（0xbff，toddle young，stage 4）** | characters.csv |
| 0x116 | 0x18000cfe | u16 | chara_id2 | 3071 | |
| 0x118 | 0x18000d00 | u32 | day_secs（滿 86400 → tama_days+1）| 26960 | 01-care |
| 0x11c | 0x18000d04 | u16 | tama_days | 1 | |
| 0x11e | 0x18000d06 | u16 | 世代 | 1 | |
| 0x120 | 0x18000d08 | u8 | cond | 0（NORMAL）| |
| 0x124 | 0x18000d0c | u32 | status flags | 0 | |
| 0x128 | 0x18000d10 | u8 | bits2–4 stage | 4 | |
| 0x129 | 0x18000d11 | u8 | bits0–4 color | 31（推測：原始色）| |
| 0x12b | 0x18000d13 | u8 | unchibox/外出計數（≤60）| 60 | 01-care |
| 0x12c–0x19b | 0x18000d14 | 0x70 B | **g_care**：onaka 0、gokigen 1、osewamiss 12、evol_support 2、cold 1、field_log 計數 4（值 2,2,2,2）、onaka_max 3、gokigen_max 2、unchi 1、yogore_count 0、onaka_timer 738、gokigen_timer 1218、age_timer 13995 … | 見左 | 01-care §2/§3、02-evolution |
| 0x19c | 0x18000d84 | u8 | alarm_reason | 0 | 0x10011b18 |
| 0x19e | 0x18000d86 | u16 | 清大便計時 | 0 | 01-care |
| 0x1a0 | 0x18000d88 | u16[3] | 裝備中道具 | 0,0,0 | 0x1001e658 |
| 0x1a6 | 0x18000d8e | u16[3][6] + u16[3][6]（0xd9a）| 3 field × 6 擺設位 × 2 層 | 全 0 | 0x10016638/0x100168a4 |
| 0x1ee | 0x18000dd6 | u8 | ? | 0x0b | 0x10078c94 |
| 0x1ef | 0x18000dd7 | u8 | 目前 field（0..2）| 2 | 02-evolution |
| 0x1f0 | 0x18000dd8 | u8 | 天氣（跨日 `FUN_1008fdec()`）| 5 | `weather` 0x100760f0 |
| 0x1f4 | 0x18000ddc | u32[3] | grazing[field] | 0x101, 0x312, 0x312 | §2.8 |
| 0x200 | 0x18000de8 | u8 | 最後放牧 field（&3）| 2 | 0x1001887c |
| 0x204 | 0x18000dec | u8 | 死亡紀錄索引（&7）| 0x13 | 01-care |
| 0x208 | 0x18000df0 | u32[5] | 死亡紀錄環狀 | 0x43, 0x17, 0x47, 0, 0 | 01-care |
| 0x21c | 0x18000e04 | u32[20] | 640-bit 道具 bitset（推測：取得過/新道具）| 只有 bit 0x17*8+6 | `FUN_1007ddd0` |
| 0x26c | 0x18000e54 | u16[640] | 倉庫 item id | 見 §3.2 | `item_find_slot` 0x1007e760 |
| 0x76c | 0x18001354 | u4[640] | 倉庫數量（每 u32 8 格，上限依 item kind 表 0x10093d2c）| | 0x1007e834/0x1007e884 |
| 0x8ac | 0x18001494 | u4[640] | 倉庫第二個 nibble（用途未明）| 全 0 | 0x1007e85c/0x1007e8d0 |
| 0x9ec | 0x180015d4 | u16 | friend_mask | 0 | 0x10017588 |
| 0x9ee | 0x180015d6 | u8[12] | friend_order | 0 | |
| 0x9fc | 0x180015e4 | 12×0xc | friend_info | 0 | |
| 0xa8c | 0x18001674 | u32 | 星球/裝飾輪替狀態 bitfield | 0 | 0x100202cc 等 |
| 0xa90 | 0x18001678 | u32 | 同上（&3 選表 0x10092da0）| 0x380080 | 0x10021f6c 等 |
| 0xa98 | 0x18001680 | u32 | ? | 0 | 0x1006666c |
| 0xa9c | 0x18001684 | u32[2] | MICROSCOPE 收集的 lab code（bit = 碼表 index 0–60，`FUN_1001f848/f8a8`）[確認] | 8, 0 | 0x1001f7f0–0x1001f8a8 |
| 0xaa4 | 0x1800168c | u32 | QRUID1（只產生一次）| 0x17d803cd | 0x10006e14 |
| 0xaa8 | 0x18001690 | u32 | QRUID2 | 0x7359b495 | 0x10006e14 |
| 0xaac | 0x18001694 | u32[2] | dlc_mask（40 bit）| 0 | 0x10003384 |
| 0xab4 | 0x1800169c | u8 | 低 6 bit：循環計數器（`FUN_10002fe4`）| 0 | 0x10002fe4 |
| 0xab5–0xffb | | | 未見參照，全 0（預留）| 0 | |

### 3.2 本機倉庫內容（slot: item × 數量）

0: 7998×1、1: 7025×1、2: 7021×1、3: 5078×1、5: 5066×2、6: 5079×3、7: 5060×5、8: 5059×5、9: 5061×4、10: 5058×5（slot 4 存 5071 但數量 0 = 空）。item 定義見 `data/items.csv`。

### 3.3 Runtime（不存檔）重要欄位

| RAM | 內容 |
|---|---|
| 0x18001be8 u8 | rt_flags：bit0 時鐘失效、bit1 喚醒處理完、bit2 跨日待存、bit3、bit4/5、bit6 暫停補跑、bit7 補跑 skip |
| 0x18001be9 u16 | bit4 Tama ghost 驗證失敗、bit5 略過 SPI 狀態設定 … |
| 0x18001bec dt[6] | g_rtc_now |
| 0x18001cb0 u16 | g_rom_field_type（版本區 +0）|
| 0x18001cb2 u16 | 版本區 +0xff4 |
| 0x18001cb4/cb6/cb8 u16 | 目前場景 / 下一場景 / 上一場景 |
| 0x18001cba/cbb | 場景狀態與待機旗標；0x18001cbe 閒置 frame 計數；0x18001cc2/cc4 門檻（600/200）|

### 3.4 其他表

| 位址 | 格式 | 內容 |
|---|---|---|
| 0x100a00bc | 6×{u32 mode, u32 poly, u32 a, u32 b} | 硬體 CRC 模式表 |
| 0x10091190 | u16[12] | 每月天數 |
| 0x10094830 | 137×0x1c | 場景表 `{u32 id, init, update, draw, exit, u32 flags, char *name}`；flags 0x10 = 進入時強制存檔 |
| 0x1009bbb8 / 0x1009bc38 | u32[32] / u32[32] | ghost slot 位址 / 大小（§2.6）|
| 0x100a4c64 | i32[6] | DEATH 喚醒門檻（01-care）|

## 4. 新命名

```
000088c0 hw_crc16 f
00008908 save_slot_crc f
000089a8 save_init_defaults f
00008ab0 save_mark_started f
00008adc save_slot_addr f
00008b48 rom_field_type_get f
00008b5c save_load_boot f
00008bd8 save_commit f
0000a8a8 save_slot_load f
0000aaa0 save_write f
0000ab40 save_slot_write f
00003394 rtc_counter_get f
00003280 rtc_is_running f
000032a4 rtc_reset f
000033e0 rtc_config f
0000345c rtc_enable f
00001fbc rtc_set_alarm_dpd f
0000229c dpd_set_boot_image f
00003268 dpd_enter f
000043a8 wake_reason_get f
00003754 spi_flash_read f
000034f0 spi_flash_erase4k f
00003968 spi_flash_program f
10006670 hw_crc_start f
10006808 date_to_days f
10006898 datetime_cmp f
10006970 rtc_now_datetime f
10006a1c days_in_month f
10006a7c rtc_rebase f
10006ac4 clock_is_valid f
10006af8 rtc_set_datetime f
10006b2c rtc_set f
10006be0 datetime_to_secs f
10006c24 secs_to_datetime f
1002a20c rtc_init f
1002a25c rtc_restart f
1002a178 enter_dpd f
1002a268 enter_dpd_alarm2 f
1002a2f0 enter_dpd_alarm f
10011b18 care_schedule_alarm f
10012a0c wake_from_standby f
10012c90 care_time_update f
10013234 care_day_change f
10013158 debug_preset_save f
1006ac00 system_boot f
1006b0b4 main_loop f
1006b200 scene_run f
10018260 scene_def_get f
100182b0 scene_has_flag f
10002844 scene_next_override f
10025a78 idle_timer_tick f
10025c6c idle_timer_reset f
10037848 scene_powerdown_update f
1004f7d8 settings_clock_update f
1000299c arc2_check f
10003160 asset_init_main f
10003238 Asset_InitAdditional f
10003048 Asset_CheckValidSaveData f
10003384 dlc_slot_valid f
100009ac dlc_slot_delete f
10000b0c dlc_find_slot f
10000b88 dlc_register_from_staging f
1001e658 item_storage_verify f
1007e71c item_free_slot f
1007e760 item_find_slot f
1007e7b8 item_slot_get f
1007e834 item_count_get f
1007e884 item_count_set f
1007e85c item_nib2_get f
1007e8d0 item_nib2_set f
1001e0d8 item_add f
1001e594 item_set_count f
10018d94 grazing_verify f
1001887c grazing_last_id f
100188d8 grazing_count_in_field f
1001892c grazing_id f
100183fc ghost_slot_verify f
100186a8 ghost_slot_addr f
100186e4 ghost_slot_size f
1002c6c0 friend_ss_addr f
10017588 friend_register f
100823bc ssht_alloc f
10082378 ssht_sum f
1006d990 patch_check_pdlo f
10082268 patch_exec f
10082298 jig_save_fill f
1002baf8 jig_save_test f
10029f38 jig_rom_test f
10030bb4 sound_set_volume_level f
1006aab4 mon_region_lookup f
10074e58 mon_savedata f
10074e34 mon_save f
10072000 mon_clrdata f
10074c70 mon_romver f
10074cf4 mon_rotenc f
10074d84 mon_rtcint f
10071c24 mon_advance f
10072028 mon_date f
10075fec mon_time f
10073010 mon_language f
100721b4 mon_destroy f
10072ab0 mon_grazing f
10073d8c mon_planetrankup f
1007312c mon_missionqualifynum f
1007593c mon_setplanetname f
18000be8 g_save l
18000bec g_settings l
18000bf0 g_rtc_base_day l
18000bf4 g_rtc_base_sec l
18000bf8 g_dial_plus l
18000bfc g_dial_minus l
18000c00 g_last_tick_time l
18000c0c g_setup_date l
18000c18 g_next_alarm l
18000c1c g_rtcint_mode l
18000c54 g_device_rand l
18000c58 g_planet_name l
18000c6a g_profile_lang l
18000c6c g_planet_rank l
18000c75 g_grazing_count l
18000c78 g_chara_book l
18000c98 g_play_days l
18000c9a g_boot_count l
18000ca4 g_gotchi_points l
18000ca8 g_mission_ctr l
18000d84 g_alarm_reason l
18000d88 g_equip_items l
18000dd7 g_field_index l
18000dd8 g_weather l
18000ddc g_grazing l
18000e04 g_item_bits l
18000e54 g_item_ids l
18001354 g_item_counts l
18001494 g_item_nib2 l
180015d4 g_friend_mask l
180015d6 g_friend_order l
180015e4 g_friend_info l
1800168c g_qruid1 l
18001690 g_qruid2 l
18001694 g_dlc_mask l
18001be8 g_rt_flags l
18001bec g_rtc_now l
18001cb0 g_rom_field_type l
18001cb2 g_rom_ver_major l
18001cb4 g_scene_cur l
18001cb6 g_scene_next l
```

## 5. 未解問題

- **frame 長度**：閒置門檻 600/200 是 frame 數；換算秒數要看 `main_loop` 的 frame 同步（`0x1801c3a4`、`FUN_1007b808`）。屬 UI/主迴圈子系統。
- **wake reason 位元**的硬體意義（MMIO 0x45000014、0x4500002c）與 DPD 期間 SRAM 是否保留：程式行為不依賴這點（開機必重載存檔），但硬體文件可確認。看 DPD 韌體 0x2d4–0x4e0。
- `rtc_reset` 是否真的把 0x45000304 清零：由「設定時鐘後 base_sec + 計數 = 真實時間」的數值吻合推得（base 18:47:40、存檔時計數約 2118 → 19:22:58），未讀到硬體文件。
- 版本區 `+0xff8/+0xffc` 的 checksum 範圍（起點 0 還是 0x1000）：數值上 sum(flash[0:0xd2a000]) == `+0xffc` 吻合，但 `jig_rom_test` 的起點初始化（param+0x11c 結構 [2]）未追到。
- save 欄位用途未明：save+0x1ee（0x18000dd6）、0xa8c/0xa90/0xa98 輪替 bitfield、0xa9c 64-bit bitset、`0x18001494` 第二 nibble、0x18000e04 bitset 的語意、c6a 的低 9 bit（profile 頁 2，文字 id 0x1ed/0x1ef 可查 strings.csv）。
- `g_care` 0x70 B 內部欄位以 01-care.md / 02-evolution.md 為準；本文件只做到存檔位置對應。
- .tamasave header 0x1c–0xbf 的確切語意是 Paramix 私有，未讀其原始碼（依規則不下載第三方程式）。還原只需要 chunk 區。
- `PDLO` patch 的解密（`FUN_1007751c`）與簽章檢查（PRAM 0x7b2，金鑰來自 PRAM 0x2f18(5)）未深入，屬通訊/安全子系統。
- 0x1007d3b0 推定為閏年判斷（回傳非 0 → 366 天），未逐行驗證其公式（2025–2079 只需 %4）。
