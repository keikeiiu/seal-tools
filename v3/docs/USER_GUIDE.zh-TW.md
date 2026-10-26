# Seal Tools v3.0 — 使用指南

[English](USER_GUIDE.md) · 繁體中文

v3 殼層（shell）每一張卡片、每一個分頁與按鈕的功能，以及何時使用。第一次安裝請看
[INSTALL.md](INSTALL.md)；座標為什麼這樣運作，請看 `v3/docs/spec/CALIBRATION.md`；哪個設定放在哪個檔案，
請看 `v3/docs/spec/CONFIG.md`。

**v3.0 不需要重新燒錄韌體。** Arduino 韌體自 v2.15 起沒有變更（韌體等級 2）：v3.0 在
`SealTools-v3.0-firmware.zip` 裡提供同一份 `seal_mouse.ino`，所以已經跑 v2 韌體的板子可以直接用。
韌體壓縮檔是給新板子用的，不是更新時要重刷的。

> **一次只能跑一個工具 —— 寵物餵食器例外。** 所有工具共用同一塊 Arduino，所以另一個工具在跑時，
> 啟動會被拒絕（不會自動停掉正在跑的那個）。**寵物餵食器（Pet Feeder）** 是常駐工具：啟動其他工具時
> 它仍保持排程，若輪到餵食而遊戲正被別的工具使用，它會等待。

---

## 目錄

1. [v3 是什麼](#1-v3-是什麼)
2. [視窗](#2-視窗)
3. [第一次啟動：先設定，再校正](#3-第一次啟動先設定再校正)
4. [卡片：七個工具與一次執行的意義](#4-卡片七個工具與一次執行的意義)
5. [工具分頁](#5-工具分頁)
6. [校正：18 種標記](#6-校正18-種標記)
7. [日誌與檔案](#7-日誌與檔案)
8. [v3.0 的熱鍵](#8-v30-的熱鍵)
9. [常見工作流程](#9-常見工作流程)
10. [疑難排解](#10-疑難排解)
11. [v3.0 尚未實作的部分](#11-v30-尚未實作的部分)
12. [其餘文件在哪裡](#12-其餘文件在哪裡)

---

## 1. v3 是什麼

Seal Tools 透過 **Arduino Pro Micro** 操作 Seal Online：板子會以真正的 USB 滑鼠與鍵盤出現，所以遊戲
接受它的點擊與按鍵，而軟體模擬的輸入會被阻擋。OCR 讀取遊戲畫面顯示的內容（改造師的等級與屬性、寶石
結果框、寵物面板與時間列），工具再依讀到的內容決定下一步。

v3 是**同一個工具行為的新前端**：

- `SealTools.exe` 是你操作的視窗。它會啟動同一個資料夾裡的 `seal-core.exe`（背景程序），兩者以本機
  協定溝通。你不需要手動啟動核心。
- 工具、拒絕訊息與寫出的檔案都是 v2 的行為；**視窗**是新的（分頁殼層，而不是 v2 的卡片視窗加五個校正
  分頁）。
- 韌體、`config/local.yaml` 的格式與量測到的座標都沒有改變。v2 安裝要變成 v3 安裝，只要複製它的
  `config/local.yaml` 與校正圖片 —— 見 [從 v2 遷移](INSTALL.md#migrating-from-v2)。

**v3.0 沒有全域熱鍵輪詢器。** 熱鍵的*名稱*可以編輯與儲存（第 8 節），但目前在遊戲中沒有任何東西在監聽
F12/F11 這類按鍵。**Stop 與殼層裡的每個按鈕永遠有效。** 這是刻意的延後：背景程序能否在遊戲取得焦點時
讀到按鍵，必須先量測，因為防作弊可能會阻擋。

---

## 2. 視窗

視窗開啟時最上方是 **Cards**，下面是一排分頁，底部是**狀態列**。核心停止或無法啟動時，整個頁面會變成
**當機畫面（crash view）**。

- **標題列 / App bar：** `Seal Tools v3`、右側的 **Hold Space** 切換鈕，以及小小的 `v0.1.0` —— 這個小版本
  是殼層的協定用戶端版本，**不是**產品版本；產品版本是 3.0。
- **分頁列：** `Cards`、`Spammer`、`Tuner`、`Gem`、`Buy/Sell`、`Quest`、`Pet`、`Attributes`、`Arduino`、
  `Hotkeys`、`Setup`、`Calibrate` —— Cards 頁面加上十一個分頁。
- **狀態列：** `core <版本> · rev <n>`、核心正在使用的 **root** 資料夾，以及 **Logs** 按鈕。
- **當機畫面：** 核心沒在跑時取代整個頁面。它會顯示失敗原因、root、核心的 **stderr 尾端**，以及
  **Restart core** 按鈕（協定版本不合時除外：那需要換成一對相符的執行檔，不是重啟）。

### 2.1 Logs 面板

狀態列的 **Logs (N)** 會打開即時日誌面板，開啟時按鈕顯示 **Hide logs**。面板顯示核心發布的 `log` 行
（工具訊息、值得顯示的拒絕、警告）；`N` 是目前保留的行數。這是即時檢視；持久的分次執行檔案在
`<root>/logs/` —— 見第 7 節。

### 2.2 Cards 頁面

Cards 頁面是七張卡片的格狀排列 —— Skill Spammer、Magic Tuner、Buy、Sell、Gem Composer、Quest
Hand-in、Pet Feeder —— 再加上 App bar 裡的 Hold Space 切換鈕（Hold Space 是第八個工具，但刻意沒有卡片；
它的狀態與失敗訊息會顯示在 Skill Spammer 卡片上）。第 4 節說明每張卡片的執行。

### 2.3 Hold Space

App bar 的切換鈕會**按住空白鍵**，讓遊戲撿起物品，放開時鬆開。它沒有卡片，因為它沒有可排程的執行：
按 **Hold Space**，遊玩，按 **Release Space**。它的訊息（尤其是釋放失敗，那才是重要的）會出現在
**Skill Spammer** 卡片與 `holdspace.log`。若釋放失敗，卡片會說明並要你在遊戲中按一下空白鍵。

---

## 3. 第一次啟動：先設定，再校正

如果你原本沒有 `config/local.yaml`，程式已為你複製了一份種子檔。那份種子包含**來自舊設定的預留量測值**，
在你的機器上一定是錯的。順序是：**Setup → 校正 → 執行工具。**

### 3.1 Setup

打開 **Setup**。它會一次儲存四項：

| 欄位 | 意義 |
|---|---|
| **Game window title** | 要尋找的視窗。比對是**子字串**比對，所以只要客戶端標題中好辨認的部分即可。 |
| **Client variant** | `tw`、`us`，或你的設定（`game.variants`）定義的變體名稱。變體同時決定視窗標題**與**要載入哪一本屬性字典。請選對：選錯會變成「找不到遊戲視窗」，或改造師什麼都對不上。 |
| **Arduino VID / PID list / Baud** | 沒有指定埠時，用來尋找板子的方式。出貨值是這裡使用的 Pro Micro；只有換板子才需要改。 |
| **COM port override** | 選填。留空表示依 ID 尋找板子。**指定了不存在的埠會被拒絕**，不會默默改用另一塊板子。 |

**唯讀**區塊顯示目前生效的標題與變體（以及變體指向的字典檔）。三個**動作** —— **Find window**、
**Measure**、**Probe board** —— 請核心檢查標題、回報量到的視窗框/客戶區矩形，或回報板子的韌體回應。
顯示的行是核心的原文。欄位正確後按 **Save**；切換變體會先解析、驗證、載入字典，最後才寫入，所以不會
切到一半。

### 3.2 校正門檻

工具在它需要的標記完成校正前會拒絕啟動，而且拒絕訊息會指出要開哪個分頁（例如「The bag grid isn't
calibrated — Calibrate Buy/Sell.」）。這不是 bug：在猜測的座標點擊可能買錯或賣錯東西。**Calibrate**
分頁（第 6 節）是唯一繪製所有工具標記的地方。

在替需要手調相對位移的工具（Gem Composer 的 `tuned` 移動模式）校正之前，請**關閉** Windows 的
*增強指標精確度*：

`設定 → 藍牙與裝置 → 滑鼠 → 其他滑鼠選項 → 指標選項` → 取消勾選 **增強指標精確度**。它會改變相對移動
實際走的距離，遊戲中的游標就會落到別的位置。

### 3.3 熱鍵（v3.0 只有名稱）

**Hotkeys** 分頁編輯四個名稱 —— Start、Quit、Advance grade、Pause。核心會把你輸入的名稱翻成設定裡存的
虛擬鍵碼，並把碼的名稱顯示回來；打錯字會在輸入框下方出現一行訊息，且儲存會被拒絕。**這些鍵碼會被儲存，
但 v3.0 不會輪詢它們** —— 見第 8 節。

---

## 4. 卡片：七個工具與一次執行的意義

每張卡片顯示工具的**名稱**、**階段**（`idle`、`starting`、`running`、`stopping`）、核心給目前步驟的
**訊息**、工具發布的**列**（名稱 / 狀態 / 細節 / 下一步行動），以及**最後的完成行**或**拒絕行** —— 兩者
都是核心原文。拒絕不是當機：它會說要修什麼（「No buy preset selected.」、「The MAX button isn't
calibrated — Calibrate Buy/Sell.」）。

| 卡片 | 一次執行做什麼 |
|---|---|
| **Skill Spammer** | 按目前預設的冷卻輪替送出按鍵：開場序列，然後是連段（每一步必須原子完成），最後是各自冷卻的單鍵。它沒有自然結束，跑到 Stop 為止。別的工具需要遊戲時它會讓出。按鍵前加 `*` 代表快速輕點。 |
| **Magic Tuner** | 透過板子點擊發條按鈕、OCR 讀結果面板，重複到達到目標等級且符合篩選、重試次數用完、同樣結果連續三次、游標防護跳脫、面板讀不到，或你按 Stop。 |
| **Buy** | 一次計數執行：先讓遊戲取得焦點、依預設捲動商店列表，然後對每一次：點該列、點 MAX、Enter、Enter。完成行 `Bought {n}x '{item}'.`，自己停止。 |
| **Sell** | 對備妥的格子選擇執行一次，編號高的先賣，受單次上限限制。完成行 `Sold {n} slot(s).`，自己停止。賣出無法復原。 |
| **Gem Composer** | 選等級、移到 Register、合成；結果框為空（達到設定的空結果次數後）就推進 N → G → DG，最後一個等級結束後執行自行停止。 |
| **Quest Hand-in** | 在任務 NPC 前重播你的流程（或序列）指定次數，監看游標防護，最後以 `DONE — {n} loop(s)…` 結束。它不讀畫面。 |
| **Pet Feeder** | 常駐工具：在寵物用完食物前重新載入餵食器，讓每個勾選的繁殖列都有食物。它會為每一列發布排程列，並持續執行（與等待）到 Stop。下一次重新載入時，若其他工具正佔用遊戲，會先等待。 |

**啟動與停止。** 核心就緒、卡片閒置且沒有其他工具在執行時，**Start** 才可用；**Stop** 只在這張卡片
對應的工具執行中可用。啟動/停止進行中時兩個按鈕都會停用。啟動第二個工具會被**拒絕**並指出持有者
（「… is using the game right now — stop it, then try again.」），不會替你停掉第一個工具。

**常駐的寵物餵食器。** 啟動其他工具不會解除餵食器；若輪到重新載入而該工具正在跑，餵食器會等待，卡片也會
顯示它在等待。這就是為什麼停止前景工具永遠不會結束四列的餵食排程。

**Buy 卡片的執行選擇（v3.0）。** Buy 卡片有 **Buy item**（`buy_sell.presets` 之一）與 **count** 加減器。
它們只影響**這一次執行** —— 不會存進設定，下一次執行又回到預設自帶的次數。沒有預設時，卡片的 Start 會送出
一般啟動，核心的「No buy preset selected.」會出現在卡片下方；請到 Buy/Sell 分頁新增預設。

**Quest 卡片的選擇。** **Target flow** 與 **Sequence** 不同：它們*就是*設定欄位，所以更改會透過核心儲存
（`quest.active` / `quest.active_sequence`），並套用核心的驗證（序列指向不存在的流程會被拒絕）。
**Single flow only** 是空的序列選擇。

**即時倒數。** 有排定下一步的卡片會顯示核心發布的時間，每秒遞減；那只是顯示核心的值，不是第二套排程。

---

## 5. 工具分頁

分頁編輯設定；由**核心**合併與寫入，所有拒絕行都原文顯示。變更用各分頁的 **Save** 儲存（部分卡片的
選擇會立即儲存 —— Buy 卡片的執行選擇是例外，永不儲存）。

### 5.1 Spammer

按鍵輪替的預設集。**Presets** 列出預設；可以 **Add**、**Rename**、**Duplicate**、**Delete**，而
**Active preset (runs)** 是卡片 Start 使用的那一個。每個預設包含：

- **Opening** —— **Lead-in (s)** 與有序的 **Opening step (s)** 項目。
- **Keys — preset (priority order)** —— 每個 **Key** 名稱與 **Seconds** 冷卻，依優先順序排列；板子不支援的
  按鍵會被指名回報。
- **Combos** —— 必須一起送出的按鍵群組，以及步驟間隔。

按鍵前加 `*` 是快速輕點。沒有作用中的預設或預設清單是空的，Start 會以指名分頁的行拒絕。

### 5.2 Tuner

改造師的可攜設定；本分頁上的校正幾何區塊在**這階段是唯讀**的（在三個框校正會談完成前由設定檔管理 ——
第 6.4 節）。

- **Grades** —— 等級清單、**Target grade**，以及篩選的 **require_grade** 部分。
- **Retries and timing** —— **Max retries**、**OCR retries**、**Click/enter delay (s)**、**OCR delay
  (s)**、**Save captures**（會寫入 `logs/captures/` PNG —— 見第 7 節）。
- **Springs** —— **Spring mode**（`hid` 把游標放到已校正的發條點；`manual` 留在原處）、**Mouse guard**
  （`off`/`stop`/`recenter`）與其 **Guard px**、**Recenter max**。
- **Filter** —— 啟用、比對模式（`any`/`all`/`per_attr`）、require-grade 與屬性規則（名稱、數量、min、
  max），以及覆寫規則。名稱必須與載入的字典相符 —— 請看 **Attributes** 分頁。

### 5.3 Gem

寶石合成器的等級與行為；已校正的 **Grade positions**、**Resource slots**、**Result gem area** 在**這階段
是唯讀**的（在 **Calibrate** 繪製）。

- **Grades** —— 等級清單與開始的 **Start grade**。
- **Empty result** —— **Empty mode**（`stop`、`advance_grade`，或 `advance_grade_clear`：先清空資源格
  再推進）、**Empty streak**（連續幾次空結果才動作，以免一次讀錯就推進等級）、**Colored gap min**、
  **Save empty captures**。
- **Movement** —— **Move mode**：`tuned` 送出手調的相對位移（請關閉*增強指標精確度*），`arduino` 把游標
  放到各路線已校正的目的點。

### 5.4 Buy/Sell

本分頁的校正幾何區塊在**這階段是唯讀**的 —— 那些標記在 **Calibrate** 繪製。

- **Buy presets** —— 物品名稱，接著是 **Scroll notches**、**Row**、**Count per run**。用 **Add item** /
  **Delete item** 新增或刪除預設。Buy 卡片會為單次執行挑一個（不儲存）。
- **Sell** —— **Max slots per run (sell cap)**，單次賣出的硬上限。**注意：** 選擇*哪些*背包格子要賣的編輯器
  是 v3.0 尚未實作的介面之一（第 11 節）；沒有選擇時 Sell 卡片會以「No slots are selected to sell.」拒絕，
  而核心永遠不會超過上限。

### 5.5 Quest

具名的**流程（flows）**在任務 NPC 前重播固定的點擊/按鍵序列；**序列（sequences）**連續執行數個流程。分頁
有三部分：

- **Active** —— **Active flow** 與 **Active sequence (empty = single flow only)**。Quest 卡片的選擇也是
  編輯這兩個欄位。
- **Flows** —— 每個流程：**Initial wait (s)**、**Loops**，以及有序的步驟清單 **Action**（`click` /
  `right_click` / `key` / `enter` / `wait`；`wait` 是不輸入的等待）、**Key value**、**Delay (s)**，可
  上移/下移/移除。以名稱新增或刪除流程。
- **Sequences** —— 每個序列：**Initial wait (s)**、**Master loops**，以及流程項目 **Flow** 名稱與
  **Times**。序列指向不存在的流程時，核心會拒絕儲存。

執行只會在**你**於定位倒數期間放好游標的位置點擊，並依 **Cursor guard px** 監看游標；滑鼠移動超過防護範圍
就會停止。

### 5.6 Pet

餵食器的**可攜**設定 —— 機器列、裁切圖與標記在寵物設定/校正介面完成前都是**唯讀**的（第 6 與 11 節）。

- **Feeding** —— **Items per minute (fallback rate)**（只有在讀不到浮空窗自己的速率行時使用）與
  **Occupied above (0–1 distance)**（格子的讀數要多不像空的參考值才算「有寵物」）。
- **Machine rows — read-only this wave：** 設定中的繁殖列（啟用/停用、堆疊數、餵食條）與 **Queue** 數量。
- **Calibration crops — read-only this wave：** 浮空 X 裁切與食物圖示裁切是否存在、浮空背包格、食物格、
  回歸格與食物載入模式。

全新安裝沒有繁殖列，而 v3.0 還沒有列編輯器：請遷移 v2 的 `config/local.yaml`（裡面有這些資料），或開著
`v3/docs/spec/CONFIG.md` 手動編輯該區塊；否則餵食器會以設定訊息拒絕。**Calibrate** 分頁仍可繪製餵食器的
標記（`pet_*` 種類）；浮空 X 的參考裁切圖由執行時自行學習。

### 5.7 Attributes

讀取核心為目前客戶端載入的字典 —— **唯讀**。顯示 **Client variant**、**Dictionary file**、
**Search names and OCR forms** 搜尋框、顯示數/總數，以及屬性名稱與其 OCR 形式的分組。改造師的篩選規則與
寵物面板的讀取都對照這裡的名稱與形式；若篩選規則對不上，先看這裡。**Refresh** 會請核心重新讀取字典。

### 5.8 Arduino

板子測試介面，不是設定：**Refresh probe** 請核心開啟板子並回報韌體回應；**Click**、**Move**（dx、dy）、
**Send key** 各送出一個測試動作。用它來區分「板子有問題」與「程式設定有問題」，再怪工具之前先試這裡。
**Setup** 分頁的動作區也有同樣的探測（**Probe board**）。

### 5.9 Setup

視窗、客戶端變體與板子設定，加上三個唯讀動作。見第 3.1 節 —— 同一個分頁就是第一次啟動的設定介面。

### 5.10 Hotkeys

四個按鍵名稱 —— **Start**、**Quit**、**Advance grade**、**Pause**。輸入名稱（`F1–F24`、`Esc`、
`CapsLock`、`Space`、`Tab`、`Enter`，或單一按鍵）；核心會翻譯、把儲存的鍵碼名稱顯示回來，無法解析的欄位會
在其下方顯示一行訊息。**v3.0 不會輪詢儲存的鍵碼** —— 見第 8 節。

### 5.11 Calibrate

所有工具共用的校正介面。第 6 節是完整流程：拍下遊戲視窗、選**種類**、必要時選**目標實例**、繪製標記、
**Save mark**，然後在下方讀核心回報的已存標記/衍生值/產物。分頁上的說明也再次說明拍攝行為：殼層會為了
擷取隱藏自己、等一個影格、再開回來。

---

## 6. 校正：18 種標記

校正標記以**遊戲視窗客戶區的實體像素**儲存（原點 = 客戶區左上角，不含標題列），所以移動視窗不會讓校正
失效；它**不會**被 Windows 顯示縮放換算。契約與原因在 `v3/docs/spec/CALIBRATION.md`；本節是操作者視角。

### 6.1 儲存一個標記

1. 把遊戲視窗放到最前面（遊戲必須是前景視窗；殼層會為了擷取隱藏自己，所以拍到的是遊戲，不是 Seal
   Tools）。
2. **Capture** —— 視窗會閃一下：Seal Tools 隱藏一個影格、拍下完整客戶區、再開回來。畫面與量到的顯示摘要
   會出現。
3. 從分組清單選擇 **Kind**。若是*具名實例*種類，還要選 **Target instance**（選了才會列出目標；未選目標
   前 Save 停用）。
4. 在圖片上繪製（點擊設點；拖曳設矩形）或直接輸入 **x / y / (w / h)** 欄位 —— 欄位才是權威，圖片只是
   鏡射。
5. **Result gem area** 先回答**空結果問題**：現在結果框是空的嗎？**回答前 Save 停用。** `empty: true`
   會同時儲存裁切圖*並*取樣顏色特徵；`empty: false` 只取樣特徵（拍到有寶石的框會污染強的空結果測試）。
6. **Save mark**。結果行顯示種類、修訂版與標記；`bag_grid` 另外顯示衍生間距，結果寶石裁切圖會出現在
   **Artifact** 下。**Refresh stored** 重新讀取已存標記。
7. **Stored mark** 區塊顯示所選種類/目標的標記（以及核心回傳的目標），或「Not calibrated yet」。

拒絕是核心的句子，不是默默失敗：背包格必須與背包格網的間距一致（「兩個不可能都對的數字代表拖錯了」）、
設定沒有承載的目標會被拒絕，未儲存的標記不會寫入任何東西。

### 6.2 種類（分組）

選擇器以分組呈現，讓 18 個項目保持可讀。**各工具使用：**

| 群組 | 種類 | 形狀 | 使用者 |
|---|---|---|---|
| Shop | **Bag grid** | 矩形（整個 8×8 背包） | Buy/Sell —— 推導全部 64 個格子中心；其間距會與背包格核對 |
| Shop | **Bag slot** | 矩形（一格） | Buy/Sell —— 獨立的間距核對 |
| Shop | **Shop list region** | 矩形（可見的列） | Buy/Sell —— 列的上/下緣由此推導 |
| Shop | **Focus / scroll point** | 點 | Buy/Sell —— 讓遊戲取得焦點並對準滾輪的無害點擊；不可落在列或格子上 |
| Shop | **MAX button** | 點 | Buy/Sell 與寵物數量對話框 |
| Gem | **Result gem area** | 矩形 + 空結果判定 | Gem Composer —— 空結果測試比對的裁切圖/特徵 |
| Gem | **Grade point** | 點 | Gem Composer —— 具名實例 N、G、DG、Register、Combine |
| Gem | **Resource slot point** | 點 | Gem Composer —— 具名實例 "1".."3" |
| Pet — points | **目錄 (menu) point** | 點 | Pet Feeder |
| Pet — points | **Pet feed icon point** | 點 | Pet Feeder |
| Pet — points | **Boarding X point** | 點 | Pet Feeder |
| Pet — points | **Bag page tab point** | 點 | Pet Feeder —— 目標 ITEM1..ITEM3 |
| Pet — bag | **Boarding bag grid** | 矩形 | Pet Feeder —— 它自己的格網：浮空背包在畫面的另一個位置 |
| Pet — bag | **Boarding bag slot** | 矩形 | Pet Feeder —— 該格網的一格 |
| Pet — rows | **Row start/end button** | 矩形 | Pet Feeder —— 目標 "1".."4" |
| Pet — rows | **Row boarding pet slot** | 矩形 | Pet Feeder —— 目標 "1".."4" |
| Pet — rows | **Row food strip** | 矩形 | Pet Feeder —— 目標 "1".."4"（依該列的堆疊數分割） |
| Tuner | **Spring point** | 點 | Tuner —— 只在 `spring_mode: hid` 時讀取 |

**具名實例。** 標記重複項目的種類需要一個**目標** —— 玩家看得懂的實例名稱，不是陣列索引：
`pet_page_tab` 用 `ITEM1`…`ITEM3`；三種寵物列種類用列 `"1"`…`"4"`；`gem_grade` 用 `N`、`G`、`DG`、
`Register`、`Combine`；`gem_resource` 用 `"1"`…`"3"`。設定沒有承載的目標會被拒絕（先新增該列/等級）；
儲存某一列的標記不會動到該列的堆疊、裁切圖與其他標記。切換種類會重設目標，所以標記不可能不小心存到錯的
實例。

### 6.3 寶石空結果問題

**Result gem area** 是唯一會提問的種類。儲存前依實際遊戲畫面回答：

- **結果框目前是空的** → 儲存裁切圖並取樣 `empty_signature`。這是合成器判斷等級完成所用的強測試。
- **框裡有寶石** → 只取樣顏色特徵，不存裁切圖。合成器仍有顏色備援，只是少了強的裁切圖測試。

答案不是「哪個比較好」，而是「你擷取那一刻框裡有什麼」。之後可以在框是空的時候重新儲存同一種類，補上裁切圖。

### 6.4 目前尚不能校正的部分

以下不是「可以隨手補的標記」：每一項需要不同形式的會談，v3.0 刻意不提供假的替代品。

- **改造師的三框幾何** —— v2 拖出等級區、屬性帶與剩餘帶，核心再*推導* OCR 區域、各帶與列高。v3.0 只有
  **Spring point** 種類，沒有 `tuner_region`，改造師的 OCR 幾何在該會談完成前仍由 `local.yaml`
  （`tuner.ocr`）設定。Calibrate 分頁的改造師說明也會這樣寫。
- **提示面板（tooltip）** —— 停住、懸停、擷取量到的偏移無法用單張畫面的標記表達；由 v2 設定手動編輯。
- **寵物的參考裁切圖與食物格** —— 即時畫面的會談功能（連浮空 X 裁切都由執行時學習），不是畫面標記。
- **賣出格子選擇** —— 是背包格編輯器，不是校正標記。
- **環境檢查（Detect）** —— 比對規則已在核心，但 v3.0 的 Setup 還沒顯示「你的顯示環境變了」。若換螢幕、
  縮放、視窗大小或換電腦，請**重新校正**；標記是實體位移，v3 刻意不做比例換算（遊戲 UI 不是線性縮放）。

---

## 7. 日誌與檔案

### 7.1 即時日誌面板

狀態列的 **Logs (N)** 顯示核心執行時發布的日誌行。它是用來*觀看*執行，不是持久紀錄。

### 7.2 分次執行檔案：`<root>/logs/`

`<root>` 是殼層告訴核心使用的安裝資料夾（狀態列會印出）。核心只附加、不截斷舊檔：

| 檔案 | 內容 |
|---|---|
| `<tool>.log` | 每次執行一行開始、每一則**變更的**狀態訊息，以及最終原因。工具 id：`spammer`、`tuner`、`buy`、`sell`、`gem`、`quest`、`pet`。「它做了奇怪的事」時先讀這個。 |
| `holdspace.log` | Hold Space 自己的診斷軌跡（形狀是 v2 的 `HoldDiag`；釋放失敗會落在這裡）。 |
| `error.log` | 內部錯誤與工具開啟失敗。可歸因於玩家的拒絕**不會**在這裡 —— 它們在卡片上，這是刻意的，以免蓋掉真正的錯誤。 |
| `ocr_log.jsonl` | 每次完成的 OCR 掃描一筆 JSON。 |
| `ocr_errors.jsonl` | 完成但沒有產出該模式要的結果的掃描。 |
| `captures/capture_<stamp>.png` | 只在 **Save captures** 開啟時（Tuner/Gem）。每筆紀錄會以 `capture_path` 指出檔案。 |

日誌寫入失敗會被吞掉：它永不使指令或執行失敗。`shutdown` 會把檔案沖出。

### 7.3 `config/` 與 `docs/`

- `config/defaults.yaml.example` —— 出貨範本（可攜設定）；載入時作為基底。
- `config/local.yaml` —— **你機器的狀態**：校正、預設集、可攜設定已儲存的值。核心是唯一寫入者；不要分享；
  更新時保留。
- `config/attributes.yaml` 與 `config/attributes.<variant>.yaml` —— OCR 字典。TW 由發行維護、可手動編輯；
  US 由 `.example` 種下、就地成長。
- `config/calib_gem_result.png` —— 結果寶石裁切圖產物（以空框儲存時）。
- `docs/pet-data.csv` —— 餵食器從 `<root>/docs/pet-data.csv` 讀取的餵食表；少了它，餵食預估不可用，排程
  回到設定的週期。

檔案切分、載入順序與折疊的完整說明在 `v3/docs/spec/CONFIG.md`。

---

## 8. v3.0 的熱鍵

**v3.0 沒有全域熱鍵輪詢器。** 四個熱鍵可以在 Hotkeys 分頁命名、翻譯與儲存，鍵碼也會寫進 `local.yaml`，
但**你在遊戲中按那些鍵時沒有任何東西在監看**，所以現在按 F12/F11/「開始鍵」都不會有反應。

實際上的意思：

- **用卡片上的 Stop 按鈕停止執行**（或殼層自己的控制項）—— 它們永遠有效。
- 寵物餵食器是常駐的；可以讓它跑著，從它的卡片停止，或關掉 Seal Tools。
- v3.0 不要依賴「退出/暫停鍵」；那些行為要等輪詢器回來。

這是刻意的延後，不是藏在文件裡的退步：計畫要求先量測背景程序能否在遊戲取得焦點時讀到按鍵（防作弊可能
阻擋），協定會依量測結果長出指令。它列在 `v3/docs/backlog/OPEN.md`。

---

## 9. 常見工作流程

**每台機器一次（約 15 分鐘，加上校正）：**

1. 安裝並啟動 —— 見 [INSTALL.md](INSTALL.md)。
2. **Setup**：遊戲視窗標題、客戶端變體、板子 ID；**Save**；然後 **Probe board**。
3. **Calibrate**：擷取，然後為你要用的工具繪製標記。要買賣先做商店群組，再做寶石，若要餵食再做寵物種類。
   每種標記一次儲存。
4. 跑一個短工具，確認板子與遊戲有回應。`logs/<tool>.log` 會證明它做了什麼。

**日常使用：**

1. 啟動遊戲（視窗模式），登入。
2. 開啟 Seal Tools；核心會連上（狀態列顯示 `core … · rev …`）。
3. 在工具分頁設定這次執行需要的東西（Spammer 預設、Tuner 目標/篩選、Gem 開始等級、Quest 流程、Pet 列），
   然後在卡片按 **Start**。結束時按 **Stop**。
4. Buy/Sell：Buy 用卡片的執行選擇；Sell 用備妥的選擇，並記住：賣出無法復原且有上限。
5. 結束時關閉 Seal Tools —— 關掉 UI 會停止工具與核心。一次只開一份（v3.0 沒有單一實例防護）。

**顯示環境變更後**（新螢幕、Windows 縮放、遊戲視窗大小、新電腦）：重新校正。已存標記是實體客戶區相對位移，
不會跟著搬移。

---

## 10. 疑難排解

| 症狀 | 意義 / 處理 |
|---|---|
| 整個視窗顯示 **「The core is not running」**，有 **Stderr tail** 與 **Restart core** | 核心當掉或開機被拒絕。尾端就是原因（開機拒絕會印到 stderr，不是協定訊息）。**Restart core** 會開新程序與新手勢。**協定版本不合**（`bad_version`）刻意隱藏 Restart：請把 `SealTools.exe` 與 `seal-core.exe` 換成一對相符的版本；不要混用。 |
| `bad_version` | 你混用了執行檔。請用同一份 zip 的一對。 |
| **找不到遊戲視窗（game window not found）** | 視窗標題或變體錯（`Settings → Game window title` / `Client variant`），或遊戲沒開/被最小化。Setup 的 **Find window** 會回報核心看到的東西。 |
| **`game_not_foreground`** | 遊戲前面有別的公視窗。把遊戲帶到最前面再試；Calibrate 分頁的擷取會自己隱藏殼層一個影格。 |
| **`ocr_unavailable`** | 找不到 OCR 模型或模型載入失敗。`detail.reason` 會是 `models_env_unset`、`models_dir_missing` 或 `model_load_failed:<file>`，而 `detail.message` 會列出**每一個嘗試過的資料夾**。解析順序：`SEAL_OCR_MODELS`（有設定就優先，失敗就明確失敗）、`<root>/models`、`seal-core.exe` 所在資料夾 + `models`；資料夾必須有三個 `.onnx` 才算數。還原出貨的 `models/`（或為自訂位置設 `SEAL_OCR_MODELS`）。 |
| **`board_unavailable`** | 設定的埠上沒有板子，或埠打不開。檢查線材（要能傳資料，不是純充電線）、板子的 USB 驅動、Setup 的 **VID/PID/port**，並按 **Probe board**。同一個 COM 埠只能有一個程式使用。 |
| 卡片出現「The bag grid isn't calibrated — Calibrate Buy/Sell.」這類行 | 不是 bug；該工具需要的標記不存在。到 **Calibrate** 繪製（行內會指出工具群組）。 |
| 「No buy preset selected.」/「No slots are selected to sell.」 | 在 Buy/Sell 新增/選擇買入預設（卡片的選擇器）。Sell 需要它的格子選擇，而 v3.0 沒有編輯器（第 11 節）：若遷移來的設定有選擇才可能執行，否則工具跑不了。 |
| 「The active preset … was not found — pick one in the Spammer tab.」 | 在 Spammer 分頁設定 **Active preset (runs)**。 |
| 寵物餵食器啟動被拒絕（`… isn't calibrated` 或 `No breeding row is set up` 之類） | 餵食器的標記與列必須存在。校正它的 `pet_*` 種類；新安裝還沒有列（第 11 節）—— 遷移 v2 的 `local.yaml`，或依 CONFIG.md 編輯該區塊。 |
| 熱鍵在遊戲中沒反應 | v3.0 的正常情況 —— 沒有輪詢器（第 8 節）。用殼層的 Stop。 |
| 「按 Capture 時視窗會閃」 | 正常：殼層隱藏一個影格，讓擷取拍到遊戲，然後開回來。v2 閃約 0.3 秒；v3 是一個影格加上來回通訊。 |
| 需要上次執行的細節 | 讀 `<root>/logs/<tool>.log`（狀態列會印 root）；`error.log` 與 `ocr_errors.jsonl` 就在旁邊。 |

---

## 11. v3.0 尚未實作的部分

列在這裡，讓缺少的能力是已知邊界，而不是謎團。最新清單在 `v3/docs/backlog/OPEN.md`；以下每一項都是
「3.0 沒有」，不是「明天就會有」，除非倉庫另外說明。

- **全域熱鍵。** 沒有輪詢器；見第 8 節。Stop 與殼層控制項都能用。
- **迷你模式 / 置頂 / 隱藏卡片。** v2 的卡片視窗外觀不再；v3 視窗是分頁殼層，精簡模式是之後的版面決定。
- **改造師的三框校正會談。** 只有發條點可校正；`tuner.ocr` 幾何在會談完成前由設定手動編輯。
- **提示面板（tooltip）兩次擷取會談** —— 由 v2 設定手動編輯。
- **寵物參考裁切/食物格編輯器與寵物列/佇列設定介面。** Pet 分頁在 v3.0 只顯示機器列為唯讀；遷移的 v2 設定
  會帶著它們，全新安裝需要編輯器（或手動編輯）才能讓餵食器上架。
- **賣出格子選擇編輯器。** 上限可編輯；選擇哪些背包格要賣還不行。
- **環境檢查（Detect）介面** —— 顯示環境變更後請手動重新校正。
- **單一實例防護。** 協定的實例鎖有規格但未實作：不要同時開兩份 Seal Tools。
- **安裝程式與自動更新。** v3.0 是壓縮檔；更新時替換程式檔案並保留你的 `config/`。
- **v2 專屬的外觀與文件。** 這裡不使用 v2 的 WPF 截圖；v2 文件仍是 v2 版本的紀錄，不是這個視窗的說明。

---

## 12. 其餘文件在哪裡

- `v3/docs/INSTALL.md` —— 安裝、遷移、更新、移除、疑難排解（與本檔同一資料夾）。
- `v3/docs/spec/CONFIG.md` —— 每個設定檔、哪個是你的，以及載入/折疊規則。
- `v3/docs/spec/CALIBRATION.md` —— 座標契約、DPI/縮放為何如此，以及產物規則。v2 的
  `docs/spec/COORDINATES.md` 與 `CALIBRATION.md` 是量測紀錄。
- `v3/docs/spec/PROTOCOL.md` —— 殼層↔核心契約（給工程師；讀拒絕碼時有用）。
- `v3/docs/spec/TOOL-FLOWS.md` —— 每個工具如何決定，以及它可能產生的玩家訊息原文。
- `v3/docs/backlog/OPEN.md` —— 最新的未實作清單，包含全域熱鍵量測。
