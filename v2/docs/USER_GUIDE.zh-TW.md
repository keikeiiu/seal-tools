# Seal Tools v2 — 使用指南（繁體中文）

**English version: [USER_GUIDE.md](USER_GUIDE.md)**

啟動器裡每一個分頁與按鈕的說明，以及什麼時候該用它們。關於座標為什麼這樣運作，請看
[COORDINATES.md](COORDINATES.md)；逐步校正流程請看 [CALIBRATION.md](CALIBRATION.md)。

> 三個工具共用同一個 Arduino COM 埠，所以**同一時間只能跑一個工具**。

> 介面上的按鈕名稱仍是英文，本文以「英文原名」標示，後面附上中文說明。

---

## 視窗結構

```
┌─ 工具卡片 ──────────────────────────────────────────┐
│  Magic Tuner      [Start] [Stop]                    │
│  ● RUNNING / stopped  ＋即時狀態                     │
│  Gem Composer     [Start] [Stop]                    │
│  Skill Spammer    [Start] [Stop]                    │
└─────────────────────────────────────────────────────┘
 Tuner | Gem | Spammer | Attributes | Calibrate Tuner | Calibrate Gem | Arduino | Setup | Hotkeys
```

### 工具卡片

| 控制項 | 功能 |
|---|---|
| **Start** | 需要時開啟 Arduino 埠，然後啟動該工具。找不到 Arduino 時會跳出訊息說明原因（請看 **Arduino** 分頁）。 |
| **Stop** | 取消工具、等待它的迴圈結束，並釋放序列埠。 |
| 即時狀態 | 約每 750 毫秒更新一次：`● RUNNING` / `● paused`，接著是 `Grade`、`Remaining`、`Attempt`、`Cycle`、`Current`、符合的屬性列、`Filter:`，以及任何 `⚠` 警告（例如不支援的技能鍵）。 |

按下 Start 後工具就會直接開始運作，沒有額外的「開始」步驟；不過在工具執行中，可以用工具本身的
F12 熱鍵暫停／繼續（Tuner 與 Gem Composer 適用）。

---

## Tuner 分頁

Magic Tuner 的過濾條件與停止條件。儲存到 `config/defaults.yaml`（可攜設定）。

| 控制項 | 說明 |
|---|---|
| **Target grade** | 要洗到的目標等級（`N → G → DG → XG → SG`）。達到此等級（或更好）時停止。 |
| **Max retries** | 嘗試次數的安全上限。 |
| **Click delay (s)** | Arduino 點擊之後、按下 Enter 之前的等待時間。 |
| **OCR delay (s)** | 按 Enter 之後、讀取畫面之前的等待時間 — 太短會讀到上一次的結果。 |
| **Filter enabled** | 開啟／關閉屬性過濾。關閉時只看等級。 |
| **Match mode** | `any`（符合任一規則即可）、`all`（所有規則都要符合）、`per_attr`（每個規則各自需要對應數量的屬性）。 |
| **Require grade** | 過濾通過所需的等級下限；`None` 表示不限等級。 |
| **Save OCR captures** | 每次掃描都把 OCR 區域存到 `logs/captures/`（僅供除錯，會佔用磁碟）。 |
| **Rules** | 目標清單：屬性 + 數量 + 數值上下限，按 `✕` 刪除，按 **+ Add Rule** 新增。 |
| **Override rules** | 只要符合其中任一項，不論等級都會立刻停止。 |
| **Save Tuner Config** | 把以上設定寫入 `defaults.yaml`。 |
| **Clean up captures** | 刪除 `logs/captures/*.png`，並回報刪了幾張。 |

## Gem 分頁

Gem Composer 的行為設定。儲存到 `config/defaults.yaml`。

| 控制項 | 說明 |
|---|---|
| **Start grade** | 合成器開始的等級（`N` / `G` / `DG`）。 |
| **On empty result** | 合成結果欄位為空時要怎麼做：**Stop**（停止）、**Advance to next grade**（前往下一個等級）、或 **Clear resources, then advance**（先對三個資源欄位按右鍵清除卡住的寶石，再前往下一個等級）。 |
| **Save empty-check captures** | 每個循環都儲存取樣的結果欄位截圖與 `diff=…` 記錄（僅供除錯）。空欄偵測的運作方式見 [CALIBRATION.md](CALIBRATION.md)。 |
| **Save Gem Config** | 把以上設定寫入 `defaults.yaml`。 |

## Spammer 分頁

Skill Spammer 的按鍵輪替設定。儲存到 `config/defaults.yaml`。

Arduino 支援數字 **0–9** 與 **F1–F10**；在鍵前面加 `*` 表示快速按住。其他按鍵會被忽略，並在工具卡片上
顯示 `⚠`。

| 控制項 | 說明 |
|---|---|
| **Preset** 下拉選單 | 目前要使用的具名按鍵組合。切換時，尚未儲存的編輯會暫存在記憶體中。 |
| **name** 欄位 + **+ New** | 用你輸入的名稱建立一個新的空組合。 |
| **Rename** | 把目前組合的按鍵搬到你輸入的新名稱。 |
| **Delete** | 刪除目前組合（最後一個組合無法刪除）。 |
| 按鍵列 | 每個按鍵一列：按鍵、冷卻秒數、`✕` 刪除。 |
| **+ Add Key** | 新增一個空白列。 |
| **Advanced** | 顯示目前組合的原始 `key:seconds` 清單。勾選時會把列內容填入文字框；取消勾選時會用文字框內容重建列。 |
| **Save Spammer Config** | 儲存目前組合，並將它設為使用中的組合。 |

## Attributes 分頁

OCR 屬性字典的唯讀檢視（`config/attributes.yaml`）：**Name**（過濾規則比對用的名稱）、**Category**、
以及會自動修正成該名稱的 **OCR variants**。沒有任何控制項 — 要新增屬性請直接編輯 YAML。

## Calibrate Tuner 分頁

把 Tuner 的 OCR 對準遊戲的發條（Magic Tuning）視窗。

| 控制項 | 功能 |
|---|---|
| **Capture 發條 window** | 以實體像素擷取遊戲視窗。擷取時啟動器會隱藏約 0.3 秒，避免遮住遊戲 — 這個閃一下是正常的。畫面必須顯示完整的遊戲。 |
| 畫布 | 依序拖曳三個框：**等級字母**、**三行屬性**、**彈簧次數**。每個框有顏色區分（綠／藍／橘）。太小的拖曳會被忽略。 |
| **Check OCR** | 對目前畫面執行完整的「讀取 → 比對 → 過濾」流程。使用你剛拖曳的三個框；如果三個框還沒全部拖好，會改用**已儲存**的校正值，並標示 `(checking the saved calibration from local.yaml)`。輸出包含：等級、彈簧次數、三行屬性、`Matched:`（字典辨識出的屬性）以及 `Filter:` 判定結果。 |
| **Save Tuner** | 把區域、子區段與量測到的行高寫入 `config/local.yaml`，記錄顯示環境，並儲存 `config/calib_tuner.png`（畫有你的框的截圖）。 |

## Calibrate Gem 分頁

把合成器對準寶石合成介面，並儲存相對移動量。

| 控制項 | 功能 |
|---|---|
| **Capture gem window** | 同樣以實體像素擷取，擷取時啟動器會隱藏。 |
| 畫布 | 依序點擊：**N、G、DG、Register、Combine**，接著點 **三個資源欄位**，最後在**合成結果寶石**周圍拖曳一個框。 |
| **Diagnose capture** | 回報視窗的框架／客戶區尺寸與非客戶區偏移，並儲存 `logs/captures/diag_capture.png` — 用來確認啟動器沒有遮住遊戲。 |
| **Save Gem Composer** | 把座標點、資源點與結果區域寫入 `local.yaml`。接著會詢問結果欄位現在是否為**空的** — 選 **Yes** 會取樣「空欄位顏色」作為空欄偵測的參考，選 **No** 則不啟用空欄偵測。同時儲存 `calib_gem.png` 與結果欄位截圖。 |
| **Coordinates** 表格 + **Save Coordinates** | 直接輸入 X/Y（結果區域還可輸入 W/H），不必重新擷取。 |
| **from / to** + **Test Click** | 把游標移到選定的點**並點擊**（Arduino `C`）。 |
| **Test Move (rel)** | 點 `from`、送出該路線的原始 `D dx dy`、再點 `to`。用來確認合成器的移動會落在正確位置。 |
| **Check Result Colour** | 立即取樣結果欄位，回報它的顏色、與空欄參考的距離，以及「空／有寶石」的判定。 |
| **Test Result Gem** | 同樣取樣，但附上更多細節（通道差值、主要色調），方便你人工判斷空欄偵測的門檻。 |
| **Debug Cursor (logical)** | 用 `SetCursorPos` 加上換算後的座標把游標移到選定的點，並顯示計算出的目標、API 是否接受、以及游標最後的位置。**不會點擊。** 僅供診斷 — 工具實際上是靠 Arduino 移動游標，不是這個呼叫。 |
| **Debug Physical** | 同上，但改用 `SetPhysicalCursorPos`。保留它是為了比較兩種 API。 |
| **Composer moves** 表格 | 每條路線一列（N→Register、G→Register、DG→Register、Register→Combine、Combine→Register、Register→Resource1、Resource1→Resource2、Resource2→Resource3、Resource3→N/G/DG），可編輯原始 `dx`/`dy`，每列有 **Test** 按鈕。 |
| **Save Composer Moves** | 把 `gem.movements` 寫入 `local.yaml`。這些是手工調校的 HID 次數 — 不是由像素換算而來，並且只對你的 Arduino + 滑鼠速度 + 遊戲內顯示設定有效。 |
| **Composer move mode** | `tuned`（預設）= 合成器送出上面那些手工調校的次數。`arduino` = 改用 Arduino 把游標「定位」到每條路線的目標點（閉環），不需調校，而且每次移動都會重新校正。按 **Save Gem Composer** 後寫入 `defaults.yaml`。詳見 [MOVE-SETS.md](MOVE-SETS.md)。 |
| **New Gem Composer Moves** 表格 | 同樣的路線，但每一列顯示**目標點**，並有 **Test** 按鈕：先點來源點，再把游標定位到目標點並點擊。這就是合成器在 `arduino` 模式下做的事，所以在這裡測得準，合成器就會準。 |
| **Test Full Cycle (Arduino)** | 用 arduino 移動跑一整個合成循環：**N** 選取 → 登錄 → 合成、登出再登錄、再合成一次、清掉三個資源欄；**G** 同樣；最後 **DG** 合成一次就結束。完全不用調校過的次數。想在把 `Composer move mode` 切成 `arduino` 之前確認新移動方式撐得住一整輪，就按這個。需要先開著 GEM COMPOSE 視窗並放好資源。 |

## Arduino 分頁

連線診斷，分成你真正會問的兩件事：*Arduino 在不在*，以及*它的點擊到不到得了遊戲*。

| 區塊 | 控制項 | 功能 |
|---|---|---|
| Connection | 狀態燈號 | 當作業系統上出現符合設定 VID/PID 的序列裝置時顯示綠色。 |
| Connection | 埠清單 | 列出作業系統看到的所有序列埠，符合的以 `>>` 標示，並顯示預期的 VID/PID。 |
| Connection | **Refresh** | 重新掃描（它不會自動更新）。 |
| Input test | **Send a test click** | 開啟埠並在游標目前位置送出一次 Arduino 左鍵 — 確認裝置確實活著的最終檢查。它**不會**移動游標；要連游標一起定位請用 Calibrate Gem → Test。結果顯示在按鈕旁邊，不會蓋掉上面的連線狀態。 |

## Setup 分頁

記錄校正時的顯示環境。

| 控制項 | 功能 |
|---|---|
| **Detect** | 量測遊戲視窗：螢幕實體尺寸 + DPI + 縮放比例，以及視窗的框架／客戶區尺寸。並填入下方欄位。 |
| **Scale** | 每個邏輯像素對應的實體像素數（例如 1.5）。會自動偵測；偵測錯誤時可手動修改。 |
| **Reference client size** | 此校正所對應的遊戲客戶區尺寸（實體像素）。 |
| **Save Setup** | 把 `calibration:` 區塊寫入 `local.yaml`。 |
| 已儲存校正 | 顯示已儲存的縮放比例／客戶區尺寸／時間。 |
| ⚠ 警告 | 當目前客戶區尺寸與已儲存的不同時出現 — **請重新校正**，不要沿用舊座標。 |

## Hotkeys 分頁

全域熱鍵。輸入名稱：`F1–F24`、`Esc`、`CapsLock`、`Space`、`Tab`、`Enter`，或單一字母／數字。

| 控制項 | 說明 |
|---|---|
| **Start / stop rolling** | 預設 F12 — 切換目前工具是否執行。 |
| **Quit (immediate)** | 預設 F11 — 立即停止工具。 |
| **Advance grade (gem)** | 預設 `G` — 讓合成器前往下一個等級。 |
| **Pause (graceful stop)** | 預設 CapsLock — 完成目前循環後停止。 |
| **Save Hotkeys** | 寫入 `defaults.yaml`。 |

> **只有在遊戲視窗「不是」前景時熱鍵才會作用。** 遊戲的反外掛會阻擋背景的按鍵讀取，所以當你在遊戲中時
> F11／F12／CapsLock 都不會有反應。請先點一下啟動器（或直接按卡片上的 **Stop**），再按熱鍵。

---

## 常見流程

**第一次在新機器上使用**

1. **Setup** → **Detect** → 確認縮放比例／客戶區尺寸 → **Save Setup**。
2. **Calibrate Tuner** → **Capture 發條 window** → 拖曳三個區段 → **Check OCR**（必須讀到正確的等級、
   彈簧次數與三行屬性）→ **Save Tuner**。
3. **Calibrate Gem** → **Capture gem window** → 點擊各點並拖曳結果框 → **Save Gem Composer** →
   **Save Composer Moves** → 用 **Test Move** 測試一條路線。
4. **Arduino** → **Refresh** → **Test Click (C)** 確認裝置。

**驗證既有的校正**（不需重新擷取）

- **Calibrate Tuner** → 直接按 **Check OCR**：它會用已儲存的幾何設定，並顯示屬性比對與過濾結果。
- **Calibrate Gem** → 用 **Test Click** 測一個點；用 **Check Result Colour** 測空欄偵測。

**實際執行**

- 在卡片上按 **Start**。要停止時按 **Stop**，或先點一下啟動器再按 F11／CapsLock。

---

## 疑難排解

| 症狀 | 可能原因／處理方式 |
|---|---|
| 按 Start 沒反應，跳出「Arduino not found」 | 埠號／VID／PID 設定錯誤，或裝置沒插好 — 請看 **Arduino** 分頁。 |
| 擷取到啟動器，或畫面有一塊黑 | 遊戲必須是可見的；把啟動器移開，並用 **Diagnose capture** 檢查。 |
| Check OCR 讀到錯誤的文字 | 發條視窗移動了，或被其他遊戲內視窗遮住。請重新擷取並重新拖曳。 |
| 合成器點擊會偏移 | 開啟了「增強指標精確度」，或 Arduino／滑鼠速度改變 — 用 **Test Move** 重新調校 `gem.movements`。 |
| 換螢幕或改解析度後全部偏移 | 打開 **Setup**：若縮放比例或客戶區尺寸不同，請重新校正。 |
| Spammer 完全沒按鍵 | 該按鍵不是數字或 F1–F10 — 卡片會顯示 `⚠`。 |
| 遊戲中熱鍵沒反應 | 這是預期行為：請先讓啟動器取得焦點（見 Hotkeys）。 |
