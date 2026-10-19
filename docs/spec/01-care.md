# 01 — 照顧與時間模擬核心（care / time simulation）

來源：`re/decomp-all/XIP.c`（以下位址皆為 XIP 0x10xxxxxx，SRAM 0x18xxxxxx）。所有時間單位為**秒**，除非另註。

## 1. 摘要

- 主迴圈 `main_loop` (0x1006b0b4) 每個畫面 frame 呼叫 `care_time_update` (0x10012c90)。它讀 RTC（`g_rtc_now` 0x18001bec）換成秒數，跟上次處理到的時間 `g_last_tick_time` (0x18000c00) 比較；**秒數沒變就直接 return**，所以模擬實際上是「每 RTC 秒一次」。
- 經過的秒數交給 `care_tick(t_start, dt)` (0x100134bc)。dt 一次最多 60 秒；超過 60 秒時以 60 秒為一塊迴圈補跑，最多 1440 塊（24 小時）。離線補算與線上是同一條路徑。
- 飢餓 onaka 0–6、心情 gokigen 0–20。各自有一個上數計時器，只在「醒著、沒在午睡、cond≠1」時累加。到達角色定義的 `Hungry×60` / `Happy×60` 秒就 −1 並歸零。降到 0 時發出呼叫（EM_REQ_ONAKA / GOKIGEN）。呼叫 900 秒沒處理，就算一次照顧失誤（osewamiss，只在 stage≥3 計）。
- 每餓 2 格會進入 SOWASOWA（想大便），20 秒後大便 +1（上限 6）。6 坨大便都放滿 3600 秒時，感冒 +1。
- 感冒累計到 5 的倍數就死亡。onaka=0 起算 36 小時沒餵、或感冒 36 小時沒治，進入 DEATH_DELAY。醒來時轉 DEATH（cond 4，3540 秒急救窗口），之後轉 DYING（60 秒），最後 DEAD，並寫入死亡紀錄。
- 睡眠由時刻決定：19:00 發睡覺請求，22:00 強制睡；06:00 發起床請求，10:00 強制醒。睡眠中飢餓、心情、大便、失誤全部凍結，只有年齡與死亡倒數繼續走。
- 待機時 `care_schedule_alarm` (0x10011b18) 算出下一個事件時間，設成 RTC alarm（最遠 3600 秒），喚醒後再補算。

## 2. 詳細規則

### 2.1 Tick 驅動與離線補算

| 規則 | 證據 | |
|---|---|---|
| `main_loop` 0x1006b0b4 無限迴圈，每輪呼叫 `care_time_update` 0x10012c90 | 0x1006b0b4 | [確認] |
| `care_time_update`：`now=datetime_to_secs(g_rtc_now)`、`last=datetime_to_secs(0x18000c00)`。若 `now<last`（時鐘倒退），把 last 設成 now，不補時間；若 `now==last` 就 return | 0x10012c90 | [確認] |
| 前置條件：`0x18000be8 bit0`=遊戲已開始、`bit7`=0、`0x18001be8 bit6`=0（暫停旗標）、`0x18000be8 bit3`=0（`FUN_100047f4()==0` 時會設 bit3，推測是低電量）| 0x10012c90, 0x10012a0c | [確認]/低電量為[推測] |
| 補算：`dt=now-last`。若 dt>60，迴圈 `for i<1440: care_tick(t,60); t+=60; dt-=60; if dt<60 break`，每次呼叫 `FUN_10068370`（推測是 watchdog feed）。最後再呼叫 `care_tick(t, min(dt,60))`。**最多補 86400+60 秒，超過的時間直接丟掉** | 0x10012c90 | [確認] |
| 補算結束後，若「日」欄位與上次不同，呼叫 `care_day_change` 0x10013234 **一次**（跨多天也只呼叫一次）。之後 `last=now` | 0x10012c90 | [確認] |
| `care_tick` 用 `secs_to_datetime(t_start)`（0x10006c24）判斷該時段是否在睡，所以離線補算也會正確套用作息 | 0x100134bc 開頭 | [確認] |
| 待機／喚醒：`wake_from_standby` 0x10012a0c 喚醒後呼叫 `care_time_update`，再呼叫 `care_schedule_alarm` 0x10011b18。後者在 `now..now+3600` 之間找最早的事件時間：飢餓、心情到期（僅在值<2 時）、900 秒失誤、進化、死亡倒數、19:00/10:00、大便 3600 秒、幼兒期感冒 600 秒等。結果寫入 `g_next_alarm` 0x18000c18（`FUN_1002a2f0`/`FUN_1002a268` 設 RTC alarm），原因碼寫入 `g_alarm_reason` 0x18000d84（=request id）| 0x10011b18 | [確認] |
| 喚醒原因 >1 且不是 0xc/0xd 時，播放 `FUN_10024c78(3,100)`（推測是提示音）| 0x100378d0 | [推測] |

### 2.2 `care_tick(t, dt)` 0x100134bc 執行順序

`cond = g_tama_cond` (0x18000d08)，`sleeping = tama_is_sleeping_at(dt_struct)` (0x1007dc1c)，`nap = req_is_done(0x1a)`。

1. **年齡** [確認]（cond<6）：`age_timer(0x18000d40) += dt`，上限 1,209,600（0x127500 = 14 天）。例外：stage 3 且 age≥86400 後，只在「醒著且 cond≠1」時才累加。
2. **清潔計時** `0x18000d86 += dt`，上限 14400，只在醒著時累加 [確認]。用於清大便獎勵（§2.7）。
3. **午睡 NAP**（只在 NAP done 時）：`nap_timer(0x18000d3c) += dt`。>59（滿 60 秒）就歸零、清除 NAP，並依時段補發 SLEEP、EVENT_DODODO、HOMETRG [確認]。
4. **死亡倒數 DeathRemain**（0x18000d78，TamaCare+0x64）：若 `0x18001be9 bit1==0` 就 `-=dt`（下限 0）。到 0 時，如果 DEATH 已發出且 DYING 還沒 → 發 DYING(0xc) [確認]。
5. 以下只在 **cond<4** 時執行：
   - **天數**：`day_secs(0x18000d00) += dt`；>86399 就 `g_tama_days+1`（上限 999），並減 86400 [確認]。注意這是模擬秒數，不看日曆。
   - **DeathByCold**（0x18000d7c）若≠0 就 `-=dt`；到 0 時設 `0x18000be9|=2`（死因=疾病），發 DEATH_DELAY(9) [確認]。
   - **DeathByHungry**（0x18000d80）若≠0 就 `-=dt`；到 0 時清 `0x18000be9 bit1`（死因=飢餓），發 DEATH_DELAY [確認]。
   - 若 **沒 nap 且沒在睡**：
     - **cond==1**（未命名狀態，見 §5）：只累加 onaka_timer。滿 Hungry×60 時歸零，`0x18000d68++`，每 2 次呼叫 `FUN_10062748(1)`（`0x18000d13 += 1`，上限 60）。**不扣 onaka、不扣 gokigen** [確認]。
     - **其他 cond（0/2/3）**：
       a. 若 DEATH_DELAY 待處理 → 清掉它，發 DEATH(8) [確認]。
       b. `onaka_timer += dt`、`gokigen_timer += dt`（u16）[確認]。
       c. 元素／進化計時（屬進化子系統，簡述）：stage 2 時 `elem_timer[cur_field](0x18000d46+2*i)` 每 900 秒推一個元素 icon（≤11 個）；stage 3 時同一 field 累積 3600 秒且≠目前元素 → METAMORPHO。EVOL_SUPPORT 待處理時 `0x18000d44` 滿 900 秒結束。進化年齡表見 §3。
       d. **SOWASOWA** 待處理時 `sowasowa_timer(0x18000d6a) += dt`；>19（20 秒）→ 大便 +1（<6 才加，新那坨年齡設 0）、發 UNCHI(0x10)、清 SOWASOWA [確認]。
       e. **大便老化**：每坨 `unchi_age[i](0x18000d6c+2i) += dt`，上限 3600；統計 ≥3600 的坨數，存到 `0x18000d35`。若本 tick 剛從 <6 變成 ==6，而且 cond≠2、沒有 IMMUNITY → **感冒 +1**（規則見 2.5）[確認]。
       f. **飢餓**：`onaka_timer >= Hungry*60`（Hungry≠0）→ timer=0。若 onaka>0：`care_add_onaka(-1)`，`hunger_drops(0x18000d68)++`；滿 2 次就歸零、`sowasowa_timer=0`、發 SOWASOWA(0xe)。之後若 onaka==0 → 發 ONAKA(0x11)＋HOMETRG(0xf)（每個週期都會重發）[確認]。
       g. **飢餓失誤**：ONAKA 待處理且 `onaka_timer>899` → 清 ONAKA；stage>2 時 `osewamiss++`，log 記 0x11 [確認]。
       h. **心情**：`gokigen_timer >= Happy*60`（Happy≠0）→ timer=0，`care_add_gokigen(-1)`；若 gokigen==0 → 發 GOKIGEN(0x12)＋HOMETRG [確認]。
       i. **心情失誤**：GOKIGEN 待處理且 `gokigen_timer>899` → 清 GOKIGEN；stage>2 時 `osewamiss++`，log 記 0x12 [確認]。
       j. stage==2（嬰兒）：若從沒發過 COLD 且 age>600 → 發 COLD（嬰兒 10 分鐘必定感冒一次）[確認]。
       k. stage≥3 且本 tick osewamiss 有增加：`r=rand(0,99)`。若 `osewamiss%6==0` **或** `r < def.Disease`，而且 cond≠2、沒有 IMMUNITY → **感冒 +1** [確認]。（log 字串 `OSEWAMISS CHECK :%d %d<%d`）
       l. 若 DEATH 待處理 → 再發一次 DEATH（同一 tick 內的冪等呼叫）[確認]。

### 2.3 飢餓 onaka / 心情 gokigen

| 規則 | 數值 | 證據 | |
|---|---|---|---|
| onaka 範圍 | 0–6，超出時夾住 | `care_add_onaka` 0x1000f3a0 | [確認] |
| gokigen 範圍 | 0–20 (0x14) | `care_add_gokigen` 0x1000f23c | [確認] |
| `care_add_onaka(d>0)` | 清 ONAKA 請求、`onaka_timer=0`、`DeathByHungry=0` | 0x1000f3a0 | [確認] |
| `care_add_gokigen(d>0)` | 若 GOKIGEN 待處理：清請求、`gokigen_timer=0`（**只有在有請求時才歸零**）| 0x1000f23c | [確認] |
| HAPPY 狀態 (0x04) | gokigen 不會下降（負增量被忽略）| 0x1000f23c | [確認] |
| onaka_max (0x18000d32) | stage 3–4 時，onaka 從 <6 補到 6，就 +1（上限 99）| 0x1000f3a0 | [確認] |
| gokigen_max (0x18000d33) | stage 3–4 時，gokigen 從 <20 補到 20，就 +1（上限 99）| 0x1000f23c | [確認] |
| 遞減間隔 | `char_def.Hungry`（+0x12，分鐘）×60 秒扣 1 onaka；`char_def.Happy`（+0x14，分鐘）×60 秒扣 1 gokigen；值為 0 表示不遞減 | 0x100134bc | [確認] |
| 各階段的典型值 | egg(1)：0/0（不遞減）；baby(2)：3/5 分鐘；child(3)：45/20；teen(4)：60/30；adult(5)：Hungry 5–90、Happy 10–90，逐角色不同 | `docs/spec/data/characters.csv` | [確認] |
| 計時器只在醒著且非 nap 時跑 | cond 1 只跑 onaka | 0x100134bc | [確認] |
| 呼叫優先序 | `req_top_priority` 0x10010b54 依 0x100a2fdf 表挑第一個待處理請求；ONAKA 與 GOKIGEN 同時待處理且 `gokigen_timer<onaka_timer` 時改挑 GOKIGEN；`0x18001c6b bit4` 設時回傳 0 | 0x10010b54 | [確認] |

### 2.4 大便 unchi / 髒污 yogore

- 大便數 `g_care_unchi`（0x18000d34）0–6；每坨年齡 u16 存在 0x18000d6c[6]，上限 3600 [確認]。
- 產生方式：每扣 2 次 onaka 就進 SOWASOWA，20 秒後 +1（§2.2 d/f）。玩家在 SOWASOWA 期間處理（例如帶去廁所，`FUN_10039eb8` → `req_done(0xe)`）：`0x18000cb4++`（上限 10）、gokigen +1，**不會產生大便** [確認：0x10068aa0 case 0xe，反組譯 0x10068fb4 `movs r0,#1`]。
- 清大便 `care_clean_unchi` 0x100108d4：unchi=0。回傳獎勵值 = `clamp(unchi − old_count(0x18000d35), 0, 6)`，再加上 bonus：`k=min(Σ FUN_100188d8(0..2), 12)`。`clean_timer`(0x18000d86) 在 [3600,7200) 時 +2k，在 [7200,14400) 時 +k。之後 clean_timer=0、`0x18000cb0++`（≤100）、`0x18000cb2 += 獎勵`（≤1000）[確認]。獎勵用途由呼叫者 0x10044f78 決定 [推測：點數]。
- yogore：`yogore_count`（0x18000d36）0–3。日期變更時（`care_day_change`），若當天 OTEIRE 沒 done 且 cond<6，就 +1 [確認]。早上 WAKE done（`req_done(2,0)`）時，若 count>2 → 設 `g_tama_status |= DIRTY(0x10)` [確認]。
- OTEIRE（打理）done（`req_done(5,0)`，場景 0x10040df4 中按 4 次）：清 DIRTY、count=0、gokigen +1、`0x18000cac++`（≤100）[確認]。

### 2.5 感冒 cold / 受傷 injury / 失誤 osewamiss

- `g_care_cold`（0x18000d18）表示感冒次數（getcold 0–4）。每次「感冒 +1」之後，若 `cold%5==0`（第 5 次）→ cold=0、`be9|=2`（死因=疾病），並且：從大便路徑來的 → 發 DEATH_DELAY(9)；從失誤路徑來的 → **直接發 DEATH(8)**。否則發 COLD(10)＋HOMETRG [確認 0x100134bc；%5、%6 用 python 驗算過乘法反元素寫法]。
- `req_raise(10)` COLD：若 COLD 這輪還沒 done：cond=2，`DeathByCold` 若為 0 就設 129600（36 小時），status 只保留 DIRTY [確認 0x10069218]。
- 吃藥（場景 0x1003e4a4）→ `req_done(10,0)`：gokigen +1，cond=0（若同時受傷則 3），DeathByCold=0。**一劑就好** [確認 0x10068aa0]。
- 受傷：`care_injure` 0x10010de4（cond<4 時發 INJURY(0xb)，cond=3），呼叫者有 0x1003f1f0、0x10041df8、0x10058b98、0x1008c430（推測是遊戲／事件）。治療（場景 0x1003e664）→ `req_done(0xb)`：gokigen +1，cond=0（若同時感冒則 2）。受傷**沒有**死亡倒數 [確認]。
- osewamiss：`g_care_osewamiss`（0x18000d16）是累計總數（u8，monitor 上限 99）。`miss_idx`（0x18000d2b，0–6）＋ `miss_log`（0x18000d2c[6]，記失誤種類 0x11/0x12）[確認]。道具 0x17a0 → REDU_CM(0x1b)，done 時 `miss_idx--`（**不會減總數**）[確認 0x1001021c、0x10068aa0]。monitor 印出 `OsewaMiss idx(total)`。
- 失誤只在 stage≥3 計算 [確認]。

### 2.6 狀態旗標 `g_tama_status`（0x18000d0c）

| bit | 名稱 | 設定來源 | 效果 |
|---|---|---|---|
| 0x01 | LOVE | 道具 0x177b（只限 adult）或 0x177f/0x179f 隨機 → req 0x16 done | 其他系統使用（0x10011…）[推測] |
| 0x02 | IMMUNITY | 道具 0x177c 或隨機 → req 0x17 | 擋掉所有「感冒 +1」[確認] |
| 0x04 | HAPPY | 道具 0x177a（另加 gokigen +20）或隨機 → req 0x18 | gokigen 不下降 [確認] |
| 0x10 | DIRTY | yogore count>2 時在 WAKE done 設定 | 由 OTEIRE 清除 |
| 0x20 | DANCE | 道具 0x1780 | 只影響外觀 [推測] |
| 0x40 | KIRAKIRA | 道具 0x1781 | 只影響外觀 [推測] |

LOVE/IMMUNITY/HAPPY/DANCE/KIRAKIRA 彼此互斥（設定時用 `status & 0x10 | bit`）。睡覺 done、COLD、INJURY、DEATH、NAP 開始、睡眠中的 `FUN_1000f508` 都會清掉它們（`&=0x10`）[確認]。道具 0x177f/0x179f：`rand(0, adult?2:1)` 結果 0→HAPPY、1→IMMUNITY、2→LOVE [確認 0x1001021c]。

### 2.7 餵食 `care_eat_item(4, itemId)` 0x1001021c（由 `req_done(4,item)` 呼叫）

- `item = item_def_get(id)`（0x10022ff0，每筆 0x2e bytes）。`onaka += item[+0x26]` [確認]。
- `pref = food_pref_bonus(chara,id)`（0x100171fc）：kind(+0x16)==1 時，`item[+0x22]==def[+0xc]`（ItemGrp）得 +1，`==def[+0x1a]` 得 −1；stage>4 吃 id 0x138a 時為 −1。kind==2、adult、`item[+0x24]==1` 時 +1 [確認]。
- kind==1（正餐）：gokigen += pref，`0x18000ca8++`（≤500）。kind==2（點心）：gokigen += `item[+0x28]` + pref，`0x18000caa++`（≤500）[確認]。
- stage<4 時另有「專長」累積：`0x18000d4b[item+0x1c] += item+0x28`，每滿 5 推入 0x18000d27 清單（最多 4 個）[確認，細節屬進化子系統]。
- 特殊道具 0x1772–0x17a2：顏色、HAPPY/LOVE/IMMUNITY、DOSSARI(0x19)、NAP(0x177e：開始 60 秒午睡)、DANCE/KIRAKIRA、REDU_CM，見 0x1001021c [確認]。
- 道具數值表見 `docs/spec/data/items.csv`（onaka/gokigen 欄）。

### 2.8 睡眠作息

`tama_time_period(dt)` 0x1008e704 依小時分段：0–5→4，6–9→0，10–15→1，16–18→2，19–21→3，22–23→4 [確認]。

`tama_is_sleeping_at` 0x1007dc1c [確認]：
- stage 1（egg）：永遠不睡。
- stage 2（baby）：age ∈ [1800, 2100) 時睡覺（進入時自動 SLEEP request+done，離開時自動 WAKE request+done），其他時間醒著。
- 其他：period 0 → WAKE 待處理時算在睡（還沒被叫醒）；period 3 → SLEEP **不是**待處理時算在睡（已上床）；period 4 → 在睡；period 1/2 → 醒著。
- cond>3 時一律視為醒著。

`care_time_update` 的作息事件（stage>2、沒 nap）[確認]：
- period 4：WAKE、SLEEP 待處理的都自動 done（22:00 強制睡）。
- period ≠4：WAKE 從沒發過就發 WAKE（06:00 起）；period≠0 且 WAKE 待處理 → 自動 done（10:00 強制醒）；period 3 且 SLEEP 這輪沒發過 → 發 SLEEP（19:00 起）。
- period 3 且 cond==1 → cond=0、gokigen −5（反組譯 0x10012eda `mvn r0,#4`）。
- 每月 13 日 17 時、`0x18000c6c&0xf>5`、stage>2 → EVENT_DODODO 特殊事件（屬事件子系統）。
- `FUN_10010c80` = nap done 或正在睡（給 UI 顯示「Sleeping」用）。
- 睡著時：飢餓、心情、大便、元素、失誤全部停止；年齡（stage 3 除外）、DeathRemain、DeathByCold、DeathByHungry、天數照走 [確認]。
- 待機時每天 10:00 是「延後喚醒點」：睡眠中、小時<10 時，死亡事件 alarm 被延到 10:00 [確認 0x10011b18]。
- **沒找到燈光（lights）開關**，也沒有相關字串，推測 Paradise 沒有關燈機制 [推測]。

### 2.9 死亡流程

cond 值（`g_tama_cond` 0x18000d08，名稱來自 0x1007417c）：0 NORMAL、1 （未命名）、2 COLD、3 INJURY、4 DEATH、5 DYING、6 DEAD、7 ASCENSION（XIP 內找不到設定 7 的地方）。

1. 觸發 DEATH_DELAY(9)：DeathByHungry 到 0（onaka 到 0 起算 36 小時沒吃）、DeathByCold 到 0（感冒 36 小時沒治）、或第 5 次大便感冒 [確認]。
2. 醒著的 tick：DEATH_DELAY → DEATH(8)。`req_raise(8)`：清 COLD、INJURY，發 HOMETRG，`DeathRemain=3540`（0xdd4），cond=4，status&=DIRTY [確認 0x10069218]。第 5 次失誤感冒則直接跳 DEATH。
3. DEATH 期間：救援場景 0x1003eae4 → `req_done(8,0)`：cond=0、DeathRemain=DeathByCold=0、cold=0，`DeathByHungry = (onaka==0 ? 129600 : 0)` [確認]。待機喚醒點：剩餘時間降到表 0x100a4c64 的 {1740,1140,840,540,240} 時以 DEATH 為原因喚醒，降到 0 時以 DYING 為原因 [確認]。
4. DeathRemain 到 0 → DYING(0xc)：清 DEATH，`DeathRemain=60`，cond=5 [確認]。
5. 場景 0x1003d474：`req_done(0xc,0)` → 清 DIRTY、發 DEAD(0xd)；`req_done(0xc,1)` → cond=0、DeathRemain=0（被救回）[確認]。
6. `req_raise(0xd)` DEAD：清 DYING；`death_log_add(cause, g_tama_days)` 0x1000affc，cause：`be9 bit1` 有設（疾病）→ 1，否則（飢餓）→ 3；cond=6 [確認]。
7. 死亡紀錄 `g_death_log` 0x18000df0：5 筆 u32 環狀緩衝，索引在 `0x18000dec&7`。bit0=used、bits1–3=cause、bits4–11=days、**bit12 = days>29（大往生 daiojo）**。大往生不是另一條死法，只是死亡時 tama_days ≥30 [確認]。`cmd_dying 1` 把 days 設 30 來模擬大往生。
8. 之後：新蛋由 `tama_birth_reset` 0x1000f708 產生（chara 1000、stage 1、days=0、清空 0x70 bytes care 區塊與所有請求，保留生日、新年等事件）。呼叫者有 cmd_birth 0x10071fec 和場景 0x100148f4、0x1003ccb0、0x1004dcd0、0x1004e1ec、0x1008c430。傳入前代 id 時會繼承顏色（50%）與世代計數 `0x18000d06`（≤99）[確認]。UI 流程（墓碑→蛋）[推測]。
9. 沒有自然壽命：年齡計時器上限 14 天，也沒找到依天數觸發死亡的程式 [確認：g_tama_days 只在 0x100134bc 累加、0x1000f708 歸零、0x10072550 debug 寫入]。

### 2.10 請求系統（EM_REQ）

`g_req_raised`（0x18000cee[5]）與 `g_req_done`（0x18000cf3[5]）是兩組 bitset，共 40 個 id。
- `req_raise(id)` 0x10069218：設 raised，並依 id 做副作用（見上）。
- `req_clear(id)` 0x10068a50：兩組 bit 都清掉。
- `req_done(id,arg)` 0x10068aa0：設 done，並依 id 做副作用。
- `req_is_pending(id)` 0x10010c68 = raised && !done（`0x18001c6b bit4` 設時只對 0xe、0x13 有效）。
- `req_was_raised` 0x10010cb8、`req_is_done` 0x1000fc5c。

| id | 名稱 | id | 名稱 |
|---|---|---|---|
| 0 | NONE | 0x12 | GOKIGEN |
| 1 | EVOLUTION | 0x13 | EVOL_SUPPORT |
| 2 | WAKE | 0x14 | METAMORPHO |
| 3 | SLEEP | 0x15 | COLOR |
| 4 | GOHAN（吃）| 0x16 | LOVE |
| 5 | OTEIRE | 0x17 | IMMUNITY |
| 6 | BIRTHDAY | 0x18 | HAPPY |
| 7 | NEWYEAR | 0x19 | DOSSARI |
| 8 | DEATH | 0x1a | NAP |
| 9 | DEATH_DELAY | 0x1b | REDU_CM |
| 0xa | COLD | 0x1c | EVENT_DODODO |
| 0xb | INJURY | 0x1d | EVENT_DODODO_IGNORE |
| 0xc | DYING | 0x1e | EVENT_METEOR |
| 0xd | DEAD | 0x1f | EVENT_SHOOTINGSTAR |
| 0xe | SOWASOWA | 0x20 | EVENT_UFO |
| 0xf | HOMETRG（叫回主畫面）| 0x21 | EVENT_SANTA |
| 0x10 | UNCHI | 0x22 | EVENT_EVE |
| 0x11 | ONAKA | 0x23 | EVENT_XMAS |

[確認：0x1007417c 逐一印出 id→名稱]

### 2.11 日期變更 `care_day_change` 0x10013234

yogore 規則見 §2.4；`play_days(0x18000c98)++`（≤999）；清 WAKE、SLEEP、OTEIRE、BIRTHDAY、NEWYEAR、所有 EVENT_*；`0x18000dd8 = FUN_1008fdec()`（推測是天氣）；`0x18000c9c/c9e/ce8[3]` 歸零 [確認]。

## 3. 資料表

| 位址 | 格式 | 內容 |
|---|---|---|
| 0x100a2fdf | u8[20] | 呼叫優先序：13,12,8,15,2,20,1,29,10,11,19,17,18,28,14,16,3,6,7,0 |
| 0x100a4c64 | i32[6] | DEATH 倒數的喚醒門檻：1740,1140,840,540,240,0 |
| 0x100913ac | i32[6] | 各 stage 的進化年齡（秒）：0,60,3600,86400,86400,1209600（stage 2 在程式裡另用 14400 作強制上限；stage 3 寬限 +14400）|
| char def | 15×u16，`chara_def_get` 0x10023e10 | +0x12 Hungry（分鐘）、+0x14 Happy（分鐘）、+0x18 Disease（失誤時感冒機率 %）、+0xc 喜歡的食物群、+0x1a 討厭的食物群、+0x1c 預設顏色。已匯出：`docs/spec/data/characters.csv` |
| item def | 0x2e bytes，`item_def_get` 0x10022ff0 | +0x16 kind、+0x1c 專長 idx、+0x22 食物群、+0x24 flag、+0x26 onaka、+0x28 gokigen。已匯出：`docs/spec/data/items.csv` |

**TamaCare 欄位補充**（base 0x18000d14）：+0x03 evol_support 計數（0x18000d17）、+0x17 miss_idx、+0x18 miss_log[6]、+0x21 old_unchi、+0x28 nap_timer、+0x30 evol_support_timer、+0x32 elem_timer[3]、+0x54 hunger_drops（0x18000d68）、+0x56 sowasowa_timer、+0x58 unchi_age[6]、+0x64 DeathRemain、+0x68 DeathByCold、+0x6c DeathByHungry。TamaData+0x08（0x18000d00）= day_secs。

## 4. 新命名

```
1006b0b4 main_loop f
10012a0c wake_from_standby f
10012c90 care_time_update f
100134bc care_tick f
10011b18 care_schedule_alarm f
10013234 care_day_change f
1000f708 tama_birth_reset f
1000f508 care_resync_sleep_reqs f
100108d4 care_clean_unchi f
1001021c care_eat_item f
100171fc food_pref_bonus f
10022ff0 item_def_get f
1000affc death_log_add f
10010de4 care_injure f
10010aa4 tama_cond_get f
10010c68 req_is_pending f
10010cb8 req_was_raised f
1000fc5c req_is_done f
1006916c req_test f
10069218 req_raise f
10068a50 req_clear f
10068aa0 req_done f
10010b54 req_top_priority f
1008e704 tama_time_period f
1007dc1c tama_is_sleeping_at f
10010c80 tama_is_sleeping_now f
10010c18 tama_time_period_now f
1007d668 alarm_time_ok f
10006be0 datetime_to_secs f
10006c24 secs_to_datetime f
1007417c mon_carestatus f
1003e4a4 scene_medicine f
1003e664 scene_injury_treat f
1003eae4 scene_death_rescue f
1003d474 scene_dying f
10040df4 scene_oteire f
18000d08 g_tama_cond l
18000d00 g_tama_day_secs l
18000d17 g_care_evolsupport_cnt l
18000d2b g_care_miss_idx l
18000d2c g_care_miss_log l
18000d35 g_care_unchi_old l
18000d3c g_care_nap_timer l
18000d68 g_care_hunger_drops l
18000d6a g_care_sowasowa_timer l
18000d6c g_care_unchi_age l
18000d78 g_care_death_remain l
18000d7c g_care_death_by_cold l
18000d80 g_care_death_by_hungry l
18000d84 g_alarm_reason l
18000d86 g_care_clean_timer l
18000cee g_req_raised l
18000cf3 g_req_done l
18000c00 g_last_tick_time l
18000c18 g_next_alarm l
18001bec g_rtc_now l
18000dec g_death_log_idx l
18000df0 g_death_log l
100a2fdf req_priority_list l
100a4c64 death_alarm_thresholds l
100913ac evol_age_table l
```

## 5. 未解問題

- **cond 1** 是什麼：由 0x1004d304 的場景設定，條件是 onaka、gokigen 都 >0、`TamaData+0x10==0`、時間早於某個時刻。19:00 結束，gokigen −5。期間只走 onaka 計時，並累加 0x18000d13（≤60）。推測是「外出／放養」類活動，要看該場景的 sprite（0x831–0x834）。
- `0x18001be9 bit1`（跳過 DeathRemain 遞減）與 `0x18000be9 bit1`（死因）是兩個不同變數，前者的語意待確認。
- 誰讓玩家把 SLEEP 標成 done（主動「哄睡」）：XIP 內 `req_done(3,…)` 只出現在自動路徑，可能經由變數 id 呼叫（例如 `FUN_1000fc98`、場景 0x1004f7d8）。
- 救援場景 0x1003eae4 和 dying 場景 0x1003d474 的進入條件（需要哪個道具或操作）。
- `FUN_100188d8`（清大便 bonus 的來源）、`0x18000cb2` 累計值的用途。
- `cond 7 ASCENSION` 在 XIP 中沒有寫入點，可能在 PRAM（ram.c 中沒搜到 0x18000d08）或已廢棄。
- stage 2 的進化路徑（EVOL_SUPPORT、`FUN_10081c68()==3`）屬進化子系統，這裡沒展開。
