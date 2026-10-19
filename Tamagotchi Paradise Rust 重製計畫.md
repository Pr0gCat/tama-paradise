# Tamagotchi Paradise Rust 重製計畫

Oct 19, 2026 · @LTY

## 目標與範圍

用 Rust 重寫 Tamagotchi Paradise 的遊戲邏輯，編成 WASM 在瀏覽器上玩，執行時不依賴原廠 ROM。僅供自用，不公開散布。

- ROM 只 dump 一次，用途是研究遊戲規則和抽取素材，不拿來執行
- 遊戲邏輯全部是自己的 Rust code，之後可以自由改規則、加角色
- 素材與程式碼分離，未來可替換成自繪素材
- 「可更新」靠資料驅動：角色、進化、食物、事件寫在資料檔，改資料不用重新編譯

## 硬體背景與 ROM dump

Paradise 是 2025 年 7 月上市，主控為 Sonix SNC73410，雙核心 ARM Cortex-M3。韌體可透過 UART 讀出，社群已有不需焊接的做法。

| 項目 | 內容 |
| --- | --- |
| SoC | Sonix SNC73410（雙核 Cortex-M3） |
| Dump 介面 | UART：RX / TX / GND |
| 工具 | CH341A 或 CH347 USB 轉接器，約 30 美元 |
| 電壓 | 3.3V，接線前先用電錶確認 |
| Bootrom | 有隱藏區段，Yukai Li 用 FPB 單元讀出 |

- [x] 先完整備份 flash 與存檔，再做任何其他操作
- [x] 參考 Yukai Li 的專案與 Tamagotchi Center 社群的既有成果

參考：[Hackaday：用 MCU 自身 debug 周邊破解 bootrom 保護](https://hackaday.com/blog/page/413/?cat=en-CA)、[Paradise dump/restore 教學](https://ko-fi.com/s/d5742961cf)

## 專案架構

一個 Cargo workspace，分三個 crate。core 只依賴輸入和時間，平台相關的東西全部放在外層。

| Crate | 職責 | 主要依賴 |
| --- | --- | --- |
| `tama-core` | 純遊戲邏輯 state machine：吃按鍵輸入與經過時間，輸出 framebuffer 與音效事件；可做成 `no_std` | serde（存檔、資料檔） |
| `tama-web` | 瀏覽器平台層：canvas 畫面、WebAudio 音效、IndexedDB 存檔、鍵盤與觸控輸入 | wasm-bindgen、web-sys |
| `tama-extract` | 一次性 CLI：從 dump 抽出 sprite、調色盤、音效，轉成 PNG/WAV 或自訂格式 | image、hound |

畫面解析度很低，直接寫 framebuffer 即可，不需要 Bevy 這類遊戲引擎。素材在執行時載入，不編進 binary。

## 核心設計原則

- **確定性**：同樣的輸入序列與時間，必定產生同樣的狀態。測試、重播、離線補算都靠這點。
- **固定 tick**：遊戲時間以固定步長推進，和畫面 FPS 脫鉤。
- **離線時間補算**：關頁時存下時間戳；開頁時算出經過時間，用 tick 快轉補上飢餓、成長、生病等變化。補算期間不畫畫面，速度很快。
- **資料驅動**：角色數值、進化條件、食物、事件寫成 RON 或 JSON，core 只負責解讀規則。
- **存檔版本化**：存檔帶版本號，資料結構改動時寫 migration，舊存檔不會壞。
- **素材抽象**：core 只引用素材 ID，實際圖檔由平台層載入，原廠素材和自繪素材可以互換。

## 逆向工程流程

規則的來源是韌體加實機觀察，兩邊互相對照。

1. 在 Ghidra 載入 dump，設定 Cortex-M3 Thumb-2，標出向量表和 reset handler
2. 找出 MMIO 區段，辨識 LCD、按鍵 GPIO、timer、RTC、SPI flash 等周邊的存取點
3. 定位素材資料：sprite 表、調色盤、音效，交給 `tama-extract` 解析
4. 追遊戲主迴圈與 state 結構：飢餓、心情、年齡、進化計時等變數放在哪裡、多久更新一次
5. 還原數值與規則，寫進資料檔
6. 用實機驗證：同樣操作下比對數值與事件時間點

如果靜態分析卡住，可以另外做一個簡易的 Cortex-M3 直譯器加 MMIO trace，只拿來觀察韌體行為，不是最終產品。

## 里程碑與下一步

- [x] **M0 開案**：建 Cargo workspace 三個 crate，`tama-web` 能在瀏覽器顯示測試 framebuffer
- [x] **M1 Dump**：備份實機 flash 與存檔
- [ ] **M2 素材**：`tama-extract` 抽出第一批 sprite，網頁上能顯示
- [x] **M3 最小遊戲**：一隻寵物，餵食、清理、時間流逝、存檔（畫面仍是除錯色塊）
- [x] **M4 離線補算**：關頁再開，狀態正確推進
- [ ] **M5 進化與事件**：資料驅動的進化樹與隨機事件（進化已完成，事件未做）
- [ ] **M6 擴充**：小遊戲、更多角色、自繪素材

開放問題：畫面解析度與色深、音效格式已確認（128×128 RGB565、單音蜂鳴器），見 `docs/game-spec.md`。存檔是否和實機互通尚未決定。
