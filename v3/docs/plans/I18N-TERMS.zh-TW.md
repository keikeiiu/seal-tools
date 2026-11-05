# Proposed zh-TW UI glossary (working draft)

**Status:** proposal only — not shipped, not a live plan row. It supports the parked
multi-language plan in [../backlog/OPEN.md §7](../backlog/OPEN.md). The player confirmed **CH-ZH =
Traditional Chinese (`zh-TW` / `zh-Hant`)**; Simplified is out of scope.

**Player-confirmed game terms (2026-11-05):** Magic Tuner = **發條**, boarding = **代養**, grade
ladder = **等級表**, Hold Space = **長按空白鍵**. The remaining policy question is whether every
tool label becomes Chinese (B) or keeps English proper names (A/C); these terms are used wherever a
Chinese display label is needed.

The first decision to approve is the **tool-name policy**:

| policy | result | trade-off |
|---|---|---|
| **A. Keep English tool names** (recommended) | `Skill Spammer`, `Magic Tuner`, `Gem Composer`, `Pet Feeder`, `Hold Space` stay as proper names; every surrounding label, button, help line and error is Chinese | players recognise the names from the guides and the game community; less risk of inventing a wrong game term |
| B. Translate tool names | 技能連發 / 發條鑑定 / 寶石合成 / 寵物餵食器 / 長按空白鍵 | fully Chinese; needs the player to confirm each game term |
| C. Bilingual | 技能連發（Skill Spammer） | unambiguous but long; awkward in the rail and cards |

Tables below assume **A** for tool names and translate everything else. Entries marked **待確認** need
the player or the TW game client to confirm the term.

## Navigation and shell

| English | Suggested zh-TW | Notes |
|---|---|---|
| Cards | 卡片 | |
| Run | 執行 | |
| Tools | 工具 | |
| System | 系統 | |
| Navigation / sidebar | 導覽列 | |
| Top bar | 頂端列 | |
| Status strip | 狀態列 | |
| Logs | 日誌 | |
| Quick dock | 快速面板 | |
| Mini running bar | 迷你執行列 | |
| Full window | 完整視窗 | |
| Always on top / pin | 置頂 / 釘選 | pin = 釘選, unpin = 取消釘選 |
| Appearance | 外觀 | |
| Follow system | 跟隨系統 | theme |
| Light / Dark | 淺色 / 深色 | |
| Setup | 設定 | |
| Calibrate | 校正 | |
| Hotkeys | 熱鍵 | |
| Attributes | 屬性 | |
| Arduino | Arduino | keep the product name |
| Copy diagnostics | 複製診斷資訊 | crash view |
| Stderr tail | 錯誤輸出（stderr） | technical label; keep `stderr` |
| Restart core | 重新啟動核心 | |
| Exit code | 結束代碼 | |
| Core / shell | 核心 / 殼層 | |

## Tool names (policy A: keep English)

| English | Suggested treatment | Notes |
|---|---|---|
| Skill Spammer | keep `Skill Spammer` | description: 技能連發／按鍵輪替 |
| Magic Tuner | **發條** (player-confirmed) | keep `Magic Tuner` in code/config; display 發條 |
| Gem Composer | keep `Gem Composer` | description: 寶石合成 |
| Buy / Sell | keep `Buy` / `Sell` | or 購買 / 販賣 if the player chooses policy B |
| Quest Hand-in | keep `Quest Hand-in` | description: 任務繳交 |
| Pet Feeder | keep `Pet Feeder` | description: 寵物餵食器 |
| Hold Space | **長按空白鍵** (player-confirmed) | keep `Hold Space` in code/config |

## Card states and actions

| English | Suggested zh-TW | Notes |
|---|---|---|
| Start | 啟動 | card action |
| Stop | 停止 | |
| Restart | 重新啟動 | |
| Running | 執行中 | |
| Idle | 閒置 | |
| Starting / Stopping | 啟動中 / 停止中 | |
| Ready / Not ready | 就緒 / 尚未就緒 | |
| Busy | 忙碌中 | |
| Unavailable | 無法使用 | |
| Last run | 上次執行 | |
| Run summary | 執行摘要 | |
| Attempt | 嘗試次數 | |
| Rotation | 輪替 | Spammer preset selector |
| Preset | 預設組合 | shorten to 預設 only where space is tight |
| Cooldown | 冷卻時間 | |
| Priority | 優先順序 | |
| Startup sequence | 開場序列 | |
| Combo | 連段 | |
| Queue | 佇列 | |
| Dry run | 試跑 | |
| Test one loop | 測試一次循環 | Quest |
| Target flow / Sequence | 目標流程 / 序列 | Quest |

## Calibration workspace

| English | Suggested zh-TW | Notes |
|---|---|---|
| Calibration / calibrate | 校正 | |
| Mark | 標記 | |
| Mark kind | 標記種類 | |
| Target | 目標 | |
| Target tree | 目標樹 | |
| Instance | 標記位置 | 17 kinds / 34 instances → 17 種標記、34 個位置 |
| Capture | 擷取 | noun/verb both |
| Captured frame | 擷取畫面 | |
| Capture shelf | 擷取列表 | |
| Display environment | 顯示環境 | |
| Canvas | 畫布 | |
| Drag / click | 拖曳 / 點擊 | |
| Fit / 1:1 | 適應視窗 / 原始大小 | |
| Pan / zoom | 平移 / 縮放 | |
| Overlay | 覆蓋層 | |
| Stored marks | 已儲存標記 | |
| Session | 工作階段 | calibration session |
| Run session | 執行工作階段 | |
| Check / Save / Test read | 檢查 / 儲存 / 測試讀取 | |
| Reference gallery | 參考圖庫 | |
| Grade ladder | 等級表 | player-confirmed |
| Minimum grade | 最低等級 | |
| OCR | OCR（文字辨識） | keep `OCR` in compact labels |
| Retries | 重試次數 | |
| Row height | 列高 | |
| Spring mode | 發條模式 | player-confirmed system term |
| Mouse guard | 滑鼠防護 | |
| Filter | 篩選 | |
| Grade / Remaining | 等級 / 剩餘數量 | |
| Boarding | 代養 | player-confirmed; guide's 繁殖列 wording may need a follow-up pass |
| Reload | 重新載入 | pet feeder cycle |
| Food cells | 食物格 | |
| Bag grid | 背包格線 | |
| Feeder row | 餵食列 | |

## Configuration, drafts and errors

| English | Suggested zh-TW | Notes |
|---|---|---|
| Config / settings | 設定 | |
| Draft | 草稿 | |
| Dirty / unsaved | 有未儲存變更 | |
| Saved | 已儲存 | |
| External change | 外部變更 | |
| Conflict | 衝突 | |
| Revision | 版本編號 | protocol `rev`; conflict = 版本衝突 |
| Validate / Apply | 驗證 / 套用 | |
| Merge | 合併 | |
| Portable / machine section | 可攜區塊 / 本機區塊 | technical docs only |
| Reload configuration | 重新載入設定 | |
| Save / Cancel | 儲存 / 取消 | |
| Refresh | 重新整理 | |
| Add / Rename / Duplicate / Delete | 新增 / 重新命名 / 複製 / 刪除 | |
| Clear / Select all | 清除 / 全選 | |
| Browse / Choose file | 瀏覽 / 選擇檔案 | |
| Confirm / Done | 確認 / 完成 | |
| Back / Next | 返回 / 下一步 | |
| Warning / Error | 警告 / 錯誤 | |
| Unknown | 未知 | |
| Enabled / Disabled | 啟用 / 停用 | |
| Required / Optional | 必填 / 選填 | |
| Serial port / COM port | 序列埠 / COM 埠 | |
| Baud | 鮑率 | |
| Firmware / firmware level | 韌體 / 韌體等級 | existing guides use 韌體等級 |
| Protocol / protocol level | 通訊協定 / 通訊協定等級 | board firmware is 韌體等級 |
| Client / variant | 遊戲客戶端 / 客戶端版本 | TW / US |
| Window title | 視窗標題 | |
| Crash view | 當機畫面 | |
| Connection failed / disconnected | 連線失敗 / 已中斷連線 | |
| Not checked yet / checked | 尚未檢查 / 已檢查 | firmware probe |
| Queried now / current level | 剛剛查詢 / 目前等級 | firmware probe |
| Input lease / tool busy | 輸入使用權 / 工具忙碌 | technical; player sees the full sentence |

## Units and formatting

| English | Suggested zh-TW | Notes |
|---|---|---|
| seconds / minutes / milliseconds | 秒 / 分 / 毫秒 | e.g. `in 5 s` → `5 秒後` |
| pixels | 像素 | compact labels may keep `px` |
| × (multiply) | × | e.g. `Bought 3× 'item'` → `已購買 3×「item」` |
| quote marks | 「」 | Traditional Chinese UI convention |
| date/time | 2026-11-05 14:30 | keep ISO-like order; no AM/PM |

## Decisions still needed

1. Tool-name policy **A**, **B** or **C**. The confirmed game terms above apply either way.
2. Remaining Chinese tool labels if policy B: Skill Spammer, Gem Composer, Buy, Sell, Quest Hand-in,
   Pet Feeder. Proposed: 技能連發 / 寶石合成 / 購買 / 販賣 / 任務繳交 / 寵物餵食器.
3. Whether core/domain refusal sentences are translated by the core or mapped by the shell. The
   protocol currently carries English, and card/error lines are rendered verbatim.
4. Locale persistence: config field or shell preference; and whether the in-app language follows the
   system locale on first run.
