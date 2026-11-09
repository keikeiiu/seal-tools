# Seal Tools v3.0 — 使用指南

[English](USER_GUIDE.md) · 繁體中文

v3 殼層（shell）每一張卡片、每一個分頁與按鈕的功能，以及何時使用。第一次安裝請看
[INSTALL.md](INSTALL.md)。座標為什麼這樣運作、哪個設定放在哪個檔案，規格放在**專案倉庫的文件**裡 ——
`v3/docs/spec/CALIBRATION.md` 與 `v3/docs/spec/CONFIG.md`。（這個 zip 只帶本指南、它的英文版、
`INSTALL.md`、`THIRD-PARTY-NOTICES.md` 與 `docs/pet-data.csv`，不帶規格樹。）

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
6. [校正：17 種標記](#6-校正17-種標記)
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

視窗開啟時停在 **Cards** 畫面。左側是**導覽列**，分成三組 —— **RUN**（Cards）、**TOOLS**（Skill
Spammer、Magic Tuner、Gem Composer、Buy、Sell、Quest Hand-in、Pet Feeder）與 **SYSTEM**（Attributes、
Arduino、Hotkeys、Setup、Calibrate）—— 底部是**狀態列**。核心停止或無法啟動時，整個頁面會變成
**當機畫面（crash view）**。

- **頂端列：** `Seal Tools`、**核心狀態列**（`core ready · rev <n> · <視窗標題>`）、**Logs** 按鈕、
  **always-on-top 釘選鈕**（`Keep Seal Tools on top`）與小小的產品版本。（殼層↔核心的協定用戶端身分
  是另一個工程值，記在 `PROTOCOL.md`；它不跟著產品版本走。）現在**沒有 Hold Space 切換鈕**：Hold Space
  是 Cards 畫面上的一張卡片（見 2.3）。最左邊的按鈕是**導覽列收合鈕**（Ctrl+B）。
- **導覽列：** 每個畫面一列，依用途分組；**Cards** 上有執行中工具數量的徽章。導覽列取代了原本的十二個
  分頁；執行中的工具不會移動導覽列。按頂端列的導覽列按鈕（或 Ctrl+B）可收起導覽列，讓內容使用整個
  寬度；下次啟動會恢復顯示（僅限本次工作階段）。
- **狀態列：** 左邊是 `core <版本> · rev <n>` 與核心正在使用的 **root** 資料夾，右邊是
  `Ctrl+B sidebar · Ctrl+L logs` 提示。


**Always-on-top 釘選鈕。** 釘選亮起時，殼層視窗會保持在遊戲與其他視窗上方；按一下會立即套用，並儲存
可攜設定 `settings.ui.always_on_top`，所以下次啟動會維持相同狀態。它是設定偏好，不屬於 Calibrate 的
隱藏/顯示流程。若儲存被拒絕，釘選會回到已儲存的值，並顯示核心的訊息；視窗不留在最上層時先檢查這個
釘選鈕。

**視窗會記住位置與大小。** 與核心完成 handshake 後，殼層會把視窗放回上次離開的位置，來源是 `local.yaml`
的機器 `ui` 區塊（`left`、`top`、`width`、`expanded_height` —— 這是本機狀態，不是可攜的 `settings:` 半；
`expanded_height` 是要還原的高度，不是目前高度）。移動或縮放後約 0.6 秒會透過一般的 `ui.window` 指令
自動儲存，所以核心仍是唯一的檔案寫入者。若儲存的矩形已不符任何已接上的螢幕，會被移到主要螢幕的工作區內
（日誌會說明已 clamp），所以視窗不會開到畫面外。`ui.pinned` 會原樣帶過，且**不是** App bar 的釘選鈕；
後者是可攜的 `settings.ui.always_on_top`。

三個誠實的限制：沒有乾淨關閉的儲存 hook，所以關閉前最後一瞬間的移動可能遺失（移動/縮放後稍等一下再
關閉）；最大化的視窗會以它的幾何位置重開 —— 最大化狀態本身不會被記住；混合 DPI 的多螢幕設定會讓 clamp
只是近似。

### 2.1 Logs 抽屜

頂端列的 **Logs (N)**（或 **Ctrl+L**）會在內容底部打開日誌抽屜；再按一次（或再按 Ctrl+L）關閉，也可以按 **Esc**。它顯示
核心發布的 `log` 行（工具訊息、值得顯示的拒絕、警告），每行有等級徽章，並有 **All / Errors** 篩選。
`N` 是目前保留的行數。這是即時檢視；持久的分次執行檔案在 `<root>/logs/` —— 見第 7 節。

### 2.2 Cards 畫面

Cards 畫面先是 **Run** 標頭（右側顯示執行中的數量），並優先回答「現在在跑什麼、健不健康？」。執行中
的工具以**完整卡片**顯示在上方 —— 成功色強調、階段 chip、核心訊息、資料列與 Stop 按鈕。其餘工具是
**單列 idle 磚**：圖示、一句「Ready · …」摘要（說明 Start 會做什麼）與 outlined **Start**（共八個工具，
包含 Hold Space；第 4 節說明每張卡片的執行）。有單次執行或已存選擇的磚（Buy、Quest、Spammer）還有一個
小小的 **tune** 按鈕：它會在卡片內打開那些選項，所以收起的磚仍保持單列。沒有工具在跑時會顯示橫幅提示
下一步 —— 全新啟動是「No runs yet」，其他情況是「Everything is idle」。卡片的階段只在核心發布 `state`
時改變：Start 會顯示進行中標籤，不會樂觀地改階段。

**迷你模式。** 前景工具執行時，視窗會收成遊戲旁的 360×96 長條：工具名稱與階段、核心最新訊息、
下一個時間點與 **Stop**；執行結束後長條改成 **Full window**，不會自己彈回遊戲上方。長條保持最上層、
記住你拖到的螢幕邊緣供下一次使用；常駐的 Pet Feeder 不會收起視窗（它在長條上以腳印徽章顯示）。
按 **Full window** 可在本次執行剩餘時間回到完整視窗；下一次執行會再次收起。工具執行時，也可按
頂端列的子母畫面按鈕手動收起。

**快速面板（Quick dock）。** 沒有工具在執行時，按頂端列的儀表板按鈕（或 **Setup → Quick dock →
Open quick dock**）會把視窗收成小型的常駐最上層啟動器：七個快速啟動 —— Spammer、Buy、Sell、
Quest Hand-in、Gem Composer、Magic Tuner、Hold Space —— 每個磚塊都有自己的 **Start** 按鈕，以及獨立的 **⋮ 選項** 按鈕
（整列不是啟動區），Pet Feeder 則是狀態列，按一下回到完整視窗的 Pet 分頁。
Buy 選預設與數量，並記住本次面板工作階段的最後選擇；Sell 勾選背包格子（可點單格，或按某一列的
**row N** 一鍵填滿/清除整列）。在 Sell 選單輸入名稱後按 **Pin**，會把目前的選取變成一個
**Sell · 名稱** 的一鍵磚塊；釘選會隨面板儲存，按它的 chip 也能把選取載回來。Spammer/Quest 的選擇
會儲存為使用中的預設（與 Cards 的選法相同）；Gem Composer 的選單可選起始等級與空格處理方式，
Magic Tuner 可選目標等級與重試次數 —— 兩者都會立即寫入設定，Start 使用儲存值。Hold Space 使用同一個
Start/Stop 按鈕，沒有選項。執行中的磚塊會顯示階段，按鈕變成 **Stop**。

面板標頭的 **Edit**（鉛筆）可用上下箭頭排序、移除任何磚塊，或把移除的工具加回。**Collapse to
edge** 可把面板縮成小型分頁（rail／pill／island），再點一下或**用滑鼠移上去**即展開。面板的閒置行為可在
**Setup → Quick dock** 或面板的放置選單選擇：**Keep open** 是預設，保持展開直到你手動收合；
**Auto-collapse** 則會在閒置八秒後收合；標頭也有一個 **Keep open** 切換鈕（計時／鎖頭圖示），
不必開選單就能切換模式。
滑鼠停在面板上會暫停自動收合，任何點擊都會重新計時；改變尺寸或位置也會重新倒數。滑鼠移到收合分頁上
會展開面板並保持展開；只有 Auto-collapse 或手動收合會再把它縮小。啟動工具後，同一個
視窗會變成迷你執行列；執行結束後，列上的 dock 按鈕會回到啟動器。可在 Setup 或面板的放置選單選擇
**右側、左側、右下、左下、上方中央**，同一個選單也提供 **Compact、Standard、Large** 三種尺寸：視窗與
控制項會一起縮放（Large 的字與 Start 按鈕更大，Compact 仍保持可讀）。放置位置、尺寸、收合狀態、閒置行為、
磚塊順序與釘選的 Sell 選取都會寫入設定並在重新啟動後保留；同一位置內的拖動偏移只在本次工作階段有效。

快速面板收合後的小型分頁**不是**迷你執行列：它顯示 Skill Spammer、Buy、Sell 三個快捷圖示與展開鈕。
點工具圖示會展開快速面板並打開該工具的選項；點分頁其他位置只會展開，滑鼠移上去也會展開。**迷你執行列**是前景工具執行時才出現的
360 × 96 視窗：紅色 **Stop** 只停止該工具（常駐的 Pet Feeder 會繼續），右上角的完整視窗圖示可展開外殼；
執行結束後還會出現 **Quick dock** 按鈕與填色的 **Full window** 按鈕。

### 2.3 Hold Space

Hold Space 現在是自己的一張卡片（以前是頂端列的切換鈕），因為它就跟其他工具一樣是一次執行：在卡片上
按 **Start** 會**按住空白鍵**讓遊戲撿起物品，按 **Stop** 放開。它沒有設定畫面。即時狀態、階段與拒絕訊息
都在它的卡片上；釋放失敗**仍只記在 `holdspace.log`**，因為放開發生在執行結束時，而且沒有協定事件。

---

## 3. 第一次啟動：先設定，再校正

沒有安裝程式。第一次啟動時，若你原本沒有 `config/local.yaml`，**核心**會為你從
`config/local.yaml.example` 複製一份種子檔。那份種子包含**來自舊設定的預留量測值**，在你的機器上一定是
錯的。順序是：**Setup → 校正 → 執行工具。**

### 3.1 Setup

打開 **Setup**。分頁的 **Save** 會寫入客戶端變體、Arduino ID 與埠；Game window title 是唯讀的，由下面的
動作來設定。

| 欄位 | 意義 |
|---|---|
| **Game window title** | **唯讀。** 要尋找的視窗；比對是**子字串**比對，所以只要客戶端標題中好辨認的部分即可。它由 **Find window** 找到、再用 **Use detected title** 儲存 —— 標題是辨認遊戲視窗的依據，所以不用手動輸入。 |
| **Client variant** | `tw`、`us`，或你的設定（`game.variants`）定義的變體名稱。變體同時決定視窗標題**與**要載入哪一本屬性字典。請選對：選錯會變成「找不到遊戲視窗」，或改造師什麼都對不上。 |
| **Arduino VID / PID list / Baud** | 沒有指定埠時，用來尋找板子的方式。出貨值是這裡使用的 Pro Micro；只有換板子才需要改。 |
| **COM port override** | 選填。留空表示依 ID 尋找板子。**指定了不存在的埠會被拒絕**，不會默默改用另一塊板子。 |

**唯讀**區塊顯示目前生效的變體（以及變體指向的字典檔）；上面的標題也是唯讀的。**動作**會請核心：

- **Find window** —— 尋找遊戲視窗並回報偵測到的標題；
- **Measure** —— 回報量到的視窗框/客戶區矩形；
- **Probe board** —— 回報板子的韌體回應；
- **Use detected title** —— 儲存上一次 Find 偵測到的標題（只有在 Find 成功且標題非空時可用；這是唯一會
  儲存的動作，走一般的 `config.validate`/`config.apply` 流程）。

顯示的行是核心的原文。欄位正確後按 **Save**；切換變體會先解析、驗證、載入字典，最後才寫入，所以不會
切到一半。

Setup 也有一個 **Appearance** 區塊：**Follow system**、**Light**、**Dark**。選擇會立即儲存（整個外殼與
標題列一起切換），並保存在可攜設定裡。預設是 **Dark**，所以既有安裝會維持目前的外觀，直到你主動更改。

### 3.2 校正門檻

工具在它需要的標記完成校正前會拒絕啟動。拒絕訊息會指出要儲存的**標記**與唯一的 **Calibrate** 分頁：
到那裡選那個種類（若是具名種類，再選目標實例）並儲存。這不是 bug：在猜測的座標點擊可能買錯或賣錯東西。
第 6 節是每一種標記的完整流程。

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
都是核心原文。拒絕不是當機：它會說要修什麼（「No buy preset selected.」，或缺少哪個標記、要在
**Calibrate** 儲存）。

| 卡片 | 一次執行做什麼 |
|---|---|
| **Skill Spammer** | 按目前預設的冷卻輪替送出按鍵：開場序列，然後是連段（每一步必須原子完成），最後是各自冷卻的單鍵。它沒有自然結束，跑到 Stop 為止。別的工具需要遊戲時它會讓出。按鍵前加 `*` 代表快速輕點。卡片上的 **Rotation** 選擇器會選擇這套輪替，並將它存成目前預設。 |
| **Magic Tuner** | 透過板子點擊發條按鈕、OCR 讀結果面板，重複到達到目標等級且符合篩選、重試次數用完、同樣結果連續三次、游標防護跳脫、面板讀不到，或你按 Stop。 |
| **Buy** | 一次計數執行：先讓遊戲取得焦點、依預設捲動商店列表，然後對每一次：點該列、點 MAX、Enter、Enter。完成行 `Bought {n}x '{item}'.`，自己停止。 |
| **Sell** | 對這次執行在 Sell 格網中標記的插槽執行一次，編號高的先賣，受單次上限限制。完成行 `Sold {n} slot(s).`，自己停止。賣出無法復原。 |
| **Gem Composer** | 選等級、移到 Register、合成；結果框為空（達到設定的空結果次數後）就推進 N → G → DG，最後一個等級結束後執行自行停止。 |
| **Quest Hand-in** | 在任務 NPC 前重播你的流程（或序列）指定次數，監看游標防護，最後以 `DONE — {n} loop(s)…` 結束。它不讀畫面。 |
| **Pet Feeder** | 常駐工具：在寵物用完食物前重新載入餵食器，讓每個勾選的繁殖列都有食物。它會為每一列發布排程列，並持續執行（與等待）到 Stop。下一次重新載入時，若其他工具正佔用遊戲，會先等待。 |

**啟動與停止。** 核心就緒且**這張**卡片閒置時，**Start** 才可用；**Stop** 只在**這個**工具執行中可用。
啟動/停止進行中時兩個按鈕都會停用。殼層**不會**因為別的工具在跑就停用 Start：按下去之後，核心會用
`tool_busy` 的訊息指名持有者（「… is using the game right now — stop it, then try again.」），而不是替你
停掉第一個工具。

**常駐的寵物餵食器。** 啟動其他工具不會解除餵食器；若輪到重新載入而該工具正在跑，餵食器會等待，卡片也會
顯示它在等待。這就是為什麼停止前景工具永遠不會結束四列的餵食排程。餵食器也會在工作時保存自己的進度
（`food_slots_used` 與每一列的 `boarding_running`），所以重新啟動後會從已到達的格子與 boarding 狀態繼續，
而不是回到執行前的值。

**Buy 卡片的執行選擇（v3.0）。** Buy 卡片有 **Buy item**（`buy_sell.presets` 之一）與 **count** 加減器。
它們只影響**這一次執行** —— 不會存進設定，下一次執行又回到預設自帶的次數。沒有預設時，卡片的 Start 會送出
一般啟動，核心的「No buy preset selected.」會出現在卡片下方；請到 Buy/Sell 分頁新增預設。

**Quest 卡片的選擇。** **Target flow** 與 **Sequence** 不同：它們*就是*設定欄位，所以更改會透過核心儲存
（`quest.active` / `quest.active_sequence`），並套用核心的驗證（序列指向不存在的流程會被拒絕）。
**Single flow only** 是空的序列選擇。

**即時倒數。** 有排定下一步的卡片會顯示核心發布的時間，每秒遞減；那只是顯示核心的值，不是第二套排程。

---

## 5. 工具分頁

分頁編輯設定；由**核心**合併與寫入，所有拒絕行都原文顯示。變更用各分頁的 **Save** 儲存（Quest 卡片的
選擇會立即儲存；Buy 與 Sell 卡片上的執行選擇永不儲存 —— 只影響該次執行）。

### 5.1 Spammer

按鍵輪替的預設集。**Presets** 列出預設；可以 **Add**、**Rename**、**Duplicate**、**Delete**，而
**Active preset (runs)** 是卡片 Start 使用的那一個。每個預設包含：

- **Opening** —— **Lead-in (s)** 與有序的 **Opening step (s)** 項目。
- **Keys — preset (priority order)** —— 每個 **Key** 名稱與 **Seconds** 冷卻，依優先順序排列；板子不支援的
  按鍵會被指名回報。
- **Combos** —— 必須一起送出的按鍵群組，以及步驟間隔。

按鍵前加 `*` 是快速輕點。沒有作用中的預設或預設清單是空的，Start 會以指名分頁的行拒絕。

**Skill Spammer 卡片**有一個精簡的 **Rotation** 選擇器，也就是 v2 卡片上的控制項：選擇一個預設會把它變成
作用中的輪替，並透過一般的設定儲存寫入 `spammer.active`，所以選擇會跨執行與重新啟動保留（v3 在此刻意不同於
v2 的記憶體卡片：核心是唯一的寫入者）。Start 仍然只送出一般的 `{"tool":"spammer"}` —— 沒有單次執行的
spammer 參數 —— 儲存被拒絕時卡片下方會顯示核心的訊息。分頁的 **Active preset (runs)** 是同一個欄位，所以
分頁與卡片永遠一致。

**最後一個預設不能刪除。** **Delete** 按鈕保持可用，但當刪除它會讓 spammer 設定沒有任何可送的輪替時，會以
分頁提示行的「The last preset cannot be deleted.」拒絕。請先建立另一個預設；這是 v2 的防護，被拒絕的動作
不會改動草稿。

### 5.2 Tuner

改造師的可攜設定；本分頁上的校正幾何區塊在**這階段是唯讀**的（在三個框校正會談完成前由設定檔管理 ——
第 6.4 節）。

- **Grades** —— **固定等級順序**以唯讀路徑 `N → G → DG → XG → SG` 顯示；顏色代表角色（主要色 = target，
   警告色 = minimum）。**Target grade** 與 **Minimum grade** 選單就放在路徑下方。
- **Retries and timing** —— **Max retries**、**OCR retries**、**Click/enter delay (s)**、**OCR delay
  (s)**、**Save captures**（會寫入 `logs/captures/` PNG —— 見第 7 節）。**`OCR retries` 只被儲存、目前
  沒有作用：** v3.0 的讀取器只掃描一次；最後一次重試的強制擷取是倉庫待辦清單中的未實作項目。
- **Springs** —— **Spring mode**（`hid` 把游標放到已校正的發條點；`manual` 留在原處）、**Mouse guard**
  （`off`/`stop`/`recenter`）與其 **Guard px**、**Recenter max**。
- **Filter** —— 啟用、比對模式（`any`/`all`/`per_attr`）、require-grade 與屬性規則（**名稱**、**數量**、
  **min**、**max**），以及覆寫規則。**名稱**是從已載入字典的屬性名稱中選取（**Attributes** 分頁列出的
  那些），不是手動輸入：字典以外的既有名稱會保持選取，而字典正在載入、讀取失敗或沒有任何名稱時，
  **Add** 會停用並顯示原因。

**Clean up capture images** 位於 **Save captures** 開關下方。它會先要求確認（「Delete the capture
images?」），然後只刪除 root 的 `logs/captures` 底下已儲存的 OCR 擷取 PNG，並回報「Deleted N capture
image(s) from logs/captures.」；其餘日誌會保留。它只是家務清理，不碰遊戲或板子，所以工具執行期間也能使用。

### 5.3 Gem

寶石合成器的等級與行為；已校正的 **Grade positions**、**Resource slots**、**Result gem area** 在**這階段
是唯讀**的（在 **Calibrate** 繪製）。

- **Grades** —— **固定的等級順序**以唯讀路徑顯示；**Start grade** 以播放角色標記，圖例說明執行從那裡
   往右推進。
- **Empty result** —— **Empty mode**（`stop`、`advance_grade`，或 `advance_grade_clear`：先清空資源格
  再推進）、**Empty streak**（連續幾次空結果才動作，以免一次讀錯就推進等級）、**Colored gap min**、
  **Save empty captures**。
- **Movement** —— **Move mode**：`tuned` 送出手調的相對位移（請關閉*增強指標精確度*），`arduino` 把游標
  放到各路線已校正的目的點。

### 5.4 Buy/Sell

本分頁的校正幾何區塊在**這階段是唯讀**的 —— 那些標記在 **Calibrate** 繪製。賣出格子的*選擇*可以編輯，
但需要已校正的 **Bag grid** 才能對到格子，而且只供下一次執行使用。

- **Buy presets** —— 物品名稱，接著是 **Scroll notches**、**Row**、**Count per run**。**Row** 是依已校正
  商店列表推導的 **Row 1**…**Row N** 選擇器（標籤從 1 起算；設定存的是幾何使用的 0 起算列索引），所以不會
  因手動輸入而差一格。若遷移來的預設列超出已校正列表，會顯示該列被標為「past the bottom of the list」，並附上
  指出可見列數的警告，而且不能再被選取；請改選較低的列。用 **Add item** / **Delete item** 新增或刪除預設。
  Buy 卡片會為單次執行挑一個（不儲存）。
- **Sell** —— **Max slots per run (sell cap)**，單次賣出的硬上限；以及**背包格子選擇**：整個背包的 8×8
  格網，每一格切換一個插槽。上方標頭會印出由已校正 **Bag grid** 推導出的間距；在該標記存好之前，格網會
  停用並顯示一行指向 **Calibrate** 的說明，因為沒有它 Sell 執行無法定位格子。**Select all** 會選滿 64
  格，**Clear** 會清空；每一列背包也有一個 **row 1**…**row 8** 捷徑 —— 整列都已選取時按一下會清空，否則
  會填滿該列。格網下方的計數行顯示 `n of 64 slots selected`，再下方的狀態行有三種：尚未選取、在上限內，
  或**超過上限**（以錯誤色顯示，並說明執行會拒絕）。**這個選擇只供下一次 Sell 執行使用，不會被儲存** ——
  它只存在殼層的記憶體裡（每次啟動都是空的），也沒有儲存按鈕：若把選擇存起來，可能賣掉上次選的格子，所以
  v3 維持 v2 的單次執行行為。按 **Sell 卡片**上的 **Start** 會送出目前選擇；沒有選任何格子時，卡片會顯示
  核心自己的「No slots are selected to sell.」行。分頁的 **Save** 仍然只儲存預設與上限。核心會計算實際
  要賣的不重複有效插槽，不會默默截斷；選擇超過上限時，會在**任何點擊之前**以核心自己的行拒絕：
  「{n} slots selected but the per-run cap is {cap}. Raise the cap on purpose, or narrow the selection.」
  （完整顯示）。執行時核心會依插槽編號由高到低賣出，且不超過上限。

### 5.5 Quest

具名的**流程（flows）**在任務 NPC 前重播固定的點擊/按鍵序列；**序列（sequences）**連續執行數個流程。分頁
有三部分：

- **Active** —— **Active flow** 與 **Active sequence (empty = single flow only)**。Quest 卡片的選擇也是
  編輯這兩個欄位。
- **Flows** —— 每個流程：**Initial wait (s)**、**Loops**，以及有序的步驟清單 **Action**（`click` /
  `right_click` / `key` / `enter` / `wait`；`wait` 是不輸入的等待）、**Key value**、**Delay (s)**，可
  上移/下移/移除。以名稱新增或刪除流程。刪除按鈕保持可用，但當它是最後一個流程時會以分頁提示行拒絕
  「This is the only flow — create another before deleting it.」（沒有流程的 quest 設定沒有任何可重播的
  內容）。
- **Sequences** —— 每個序列：**Initial wait (s)**、**Master loops**，以及流程項目 **Flow** 名稱與
  **Times**。選擇器只提供既有名稱；刪除仍被序列指名的流程會留下懸空參照，核心會在儲存時拒絕（與 v2 相同）
  —— 請先更新該序列再刪除流程。

執行只會在**你**於定位倒數期間放好游標的位置點擊，並依 **Cursor guard px** 監看游標；滑鼠移動超過防護範圍
就會停止。

### 5.6 Pet

餵食器的設定 —— 機器列、佇列、背包食物格與執行設定，加上可攜的備援值 —— 在同一處編輯、一起儲存。
**一次 Save** 送出一次 `config.apply`，帶著機器 `pet` 區塊與/或可攜 `settings.pet` 兩半（有資料的那些）；
被拒絕時編輯會留在原處。

餵食器也會保存自己的執行進度：工作時它會寫入 `food_slots_used` 與每一列的 `boarding_running`，所以當機或
重新啟動後不會再對已用過的格子或錯誤的 boarding 狀態繼續。Pet 分頁的已存值因此可能在執行期間更新 —— 那是
餵食器自己的進度寫入，不是你做的編輯。

頁面會先顯示就緒判定與精簡摘要（列 · 佇列 · 食物格 · 下一隻），寬螢幕時再把編輯器分成兩欄：左邊是
列／佇列／用圖示新增；右邊是背包掃描、食物格、返回格與食物裝載。**Timing & read thresholds** 與可攜的
**Feeding** 欄位收進預設收合的 **Timing & thresholds** 區塊，常用設定因此保持精簡；窄視窗會改成上下堆疊。

**Setup so far & Ready to run。** 分頁在開啟時、按 **Refresh checklists**、成功 **Save** 之後，以及成功勾選
boarding 之後讀取核心的 preflight：先 **Calibration** 清單、再 **Setup so far** 清單，最後是核心的就緒行
原文。**Ready to run.** 表示核心預期 Start 會**接受**這份已儲存的設定 —— 它反映 Start 的拒絕條件，不是
端到端執行一定成功：手改的畸形值仍可能被 Start 拒絕，執行中或非閘門的事實仍可能影響執行；**Not ready yet
— see the lines below.** 表示有就緒行觸發。✓ 是通過、✗ 是失敗，但 Calibration 清單混合了 Start 閘門與有用的
事實 —— 某列的 boarding slot 或 empty-slot reference 可以是 ✗，food strip 只在 `drag` 食物載入模式需要，
page tabs 只在已存標記使用到的頁面需要 —— 所以 ✗ 不代表一定「尚未就緒」：只有就緒行與判定決定。

ℹ 是殼層對核心沒有檢查的 `ok` 項目的標記（其 detail 以「not checked by preflight」開頭；目前只有板子韌體
——「not checked by preflight — normal」），不是第三種核心狀態。清單描述的是**已儲存**的設定：只要這裡有
任何編輯，就會顯示「Based on the saved config — Save to refresh」且不是即時更新；開啟、**Refresh
checklists**、成功 Save 或成功勾選會重新讀取。餵食器自己在執行中的 `boarding_running` 寫入會透過設定鏡像
更新**勾選值**，但不會重新讀取清單。preflight 是唯讀的 —— 不會開啟板子或讀取遊戲。

- **Breeding rows** —— 設定中已存在的每一列：**Row N enabled**（勾選的列會餵食；未勾選的忽略）與
  **Stacks per reload**（餵食條會除以它）。該列的標記 —— start/end 按鈕、boarding pet slot、food strip、
  空格裁切圖與 boarding 狀態 —— 在這裡是唯讀，會原樣帶過儲存。該列的 **Boarding now** 勾選會立即透過核心
  把該列的 `boarding_running` 寫入 `local.yaml`（單一欄位，不屬於這個分頁的 Save），且工具執行中也能使用；
  只要這裡有任何未儲存的編輯就會停用（「Save the row edits first」，若只有可攜編輯則是「Save the edits
  first」）。被拒絕的勾選會在列下方顯示核心的行；勾選顯示的是核心目前的值，所以餵食器自己在執行中的更新
  也會反映在這裡。
- **Queue** —— 每個排隊寵物顯示它的**擷取圖示**（執行比對的身分）、可編輯的 **Label**（只供日誌與此清單）
  與 **Species** 下拉選單，以及 **Remove**。編輯與擷取到的項目都由分頁的 **Save** 儲存 —— 不像 v2 的
  每次編輯自動儲存 —— 擷取到的項目會立刻加入這份清單成為草稿（只有持久化要等 Save），所以可在儲存前
  移除。下拉選單列出遊戲已知的物種（`docs/pet-data.csv` 的 `species` 欄，內嵌於殼層），所以新增寵物是
  挑選而不是打字；舊設定帶入的值會保留為選項而不是被丟棄。選定的物種才會把面板成長換算成分鐘估計；
  空值就沒有估計。佇列為空時，**Return slot** 就是執行時右鍵點擊的備援位置。
- **Bag food cells** —— 每個頁面（**ITEM1**..**ITEM3**）一個代養背包的 8×8 格網；切換哪些格子放寵物
  食物。格網只有在 **Boarding bag grid** 標記校正後才可用；否則會顯示一行指向 **Calibrate**（代養背包
  自己的格網，不是商店的），並要求執行所需的 **Boarding bag slot**。標記了格子的頁面也需要在該處存好它的
  ITEM 分頁點。
- **Return slot** —— **Bag page**（1 起算，ITEM1 = 1）與 **Cell**（0–63，row-major）。兩個值都已知時
  才會寫入；設定以 0 起算儲存頁碼。
- **Food loading** —— **Food load mode**（`right_click` 讓遊戲自己選框；`drag` 指定它，需要該列的 food
  strip 與支援拖曳的板子）與 **Reload on start**（執行開始時開關一次代養視窗）。
- **Timing & read thresholds** —— **Action wait (ms)**、**Wait after empty (minutes)** 與
  **Count read — left fraction (0–1)** / **Count read — min score (0–1)** 門檻。
- **Feeding（可攜 —— 存在 `settings.pet`）** —— **Items per minute (fallback rate)**（只有在讀不到代養
  視窗自己的速率行時使用）與 **Occupied above (0–1 distance)**（格子的讀數要多不像空的參考值才算
  「有寵物」）。

**Add a pet by icon。** 把裝著寵物的背包頁面放到畫面上 —— 擷取需要遊戲在前景，殼層會短暫隱藏自己來拍攝。
用 **ITEM1**..**ITEM3** 頁籤選頁、點擊裝有寵物的格子，再按 **Capture pet icon**（一次只能擷取一個）。頁面
不會替你點擊，而且擷取只用畫面 —— 不會開啟 Arduino。預覽與回報的頁/格/裁切大小**不會驗證正確性** ——
選錯頁會得到看起來合理的錯誤裁切 —— 請自行檢查；圖示會以草稿加入佇列清單，分頁的 **Save** 才會寫入，
儲存前可 **Remove**。擷取被拒絕（別的工具在跑、遊戲不在前景，或頁籤標記未校正）會顯示核心的行且不改變
任何東西。只有背包格網未校正時才不會顯示擷取控制項；格網已校正但頁籤未標記，或工具正在執行時，按鈕仍會
顯示，由核心拒絕。

**Capture food icon。** 先標記一個食物格 —— 最新的標記就是來源 —— 把該頁面放到畫面上並按 **Capture food
icon**。擷取到的圖示就是 **Scan this page for food** 要比對的參考；**Save** 會寫入 `pet.food_icon_png`。
兩個圖示都不是 boarding A1 寫入器：它們都是分頁 Save 儲存的一般設定欄位。

**Scan the bag（唯讀）。** 背包掃描**一次只讀一頁** —— 沒有全頁掃描。請自己把該頁面放到畫面上：殼層會為
拍攝隱藏自己、遊戲必須在前景，而且核心**不會**點擊或驗證 ITEM 頁籤，所以**選錯頁會得到看起來合理的錯誤
結果**。選 **ITEM1**..**ITEM3**，再按 **Scan this page for food** 或 **Scan this page for pets**。掃描
用的是**已儲存**的設定，不是未儲存的草稿（掃描不會被編輯擋住）—— 如果你剛改了食物格或重新擷取食物圖示，
先按 **Save**，掃描才會符合儲存的內容。

- **Scan this page for food** 會比對已儲存的食物圖示，列出找到的格子（從 1 起算）與核心的回報行。證據檔是
  best-effort，位於 `logs/reads/` 底下，可能不存在或不完整。
- **Scan this page for pets** 會顯示每個佇列寵物的命中格子、分數與 runner-up、claimed-cell 衝突，以及推導
  出的「label ×n」行。沒有命中的寵物可能不在頁面上，也可能是它儲存的圖示無法解碼 —— 掃描報告會說明是哪
  一種。

掃描只會看：不會改變任何設定，也不會自動套用。

**Apply a food scan。** 食物掃描後，**Apply N cells to the food list** 是另一個需要確認的動作。它會**取代**
食物格清單並**重設已使用格數**（執行的進度），而且即使執行中也會立即生效。Apply 用的是確認視窗中標明的
那一頁（也就是掃描的那一頁）—— 之後再切換 **ITEM** 頁籤不會改變 Apply 送出的內容。有未儲存編輯時會停用
（「Save the edits first」）；請先檢查回報與證據，因為選錯頁會存下錯的格子。空掃描結果代表「什麼都沒找到」
（成功），所以 Apply 會保持停用。**Capture food icon** 仍是重新製作比對圖示的方法；**Save** 會寫入它。

唯讀區塊裡的校正標記/裁切圖來自 **Calibrate** 分頁或由執行時學習；**Food cells used** 是執行自己的讀數，
只顯示。全新安裝沒有繁殖列，而 v3.0 還不能新增列或重新擷取該列的參考裁切圖 —— 請遷移 v2 的
`config/local.yaml`（裡面有這些資料），或開著倉庫的 `v3/docs/spec/CONFIG.md` 手動編輯該區塊；否則餵食器
會以設定訊息拒絕。**Calibrate** 分頁仍可繪製餵食器的標記（`pet_*` 種類）；代養視窗的關閉鈕的參考裁切圖
由執行時自行學習。

### 5.7 Attributes

讀取核心為目前客戶端載入的字典 —— **唯讀**。顯示 **Client variant**、**Dictionary file**、
**Search names and OCR forms** 搜尋框、顯示數/總數，以及屬性名稱與其 OCR 形式的分組。改造師的篩選規則與
寵物面板的讀取都對照這裡的名稱與形式；若篩選規則對不上，先看這裡。**Refresh** 會請核心重新讀取字典。

### 5.8 Arduino

Arduino 分頁是唯讀的連線卡加上測試介面。它開啟時會呼叫 `board.details`，只列出這台機器的序列埠 —— 不會
開啟板子 —— 所以即使工具正在執行，讀取也是安全的。

- **Connection** —— 三態指示燈與判定，說明 Start 會怎麼做：**Ready — Start will open COM…**、**Start will
  refuse — the named port is missing**（附核心自己的拒絕行），或 **Start will refuse — no Arduino
  found**（附只充電線的建議：插上板子後按 **Refresh**；板子可能亮著卻始終不以連接埠出現）。
- **Ports this machine has** —— 依核心的 chosen-first 順序：**●** 是 Start 會開啟的埠，**○** 是其他埠；
  符合設定 ID 但未被選中的埠會標示「matches the expected ID」。
- **What the launcher looks for** —— 它比對的 VID/PID/baud，以及 `arduino.port` 是否指定了埠（指定埠優先於
  ID）。
- **Board firmware** —— 快取的**協定等級**，不是 v2 舊的拖曳測試 OK/Not-OK 判定：尚未有工具開啟板子前顯示
  「Not checked yet — this is normal.」，之後是「Protocol level N (checked).」（明確查詢後則是
  「Protocol level N (queried now).」）。**Query firmware** 是這個分頁唯一會開啟板子的動作；工具執行中
  無法使用，並會顯示原因（真正的 probe 拒絕會原文顯示核心的行）。
- **Refresh** 會重新讀取唯讀細節。**Click**、**Move**（dx、dy）、**Send key** 不變：各送出一個測試動作。
  Setup 的 **Probe board** 仍是同一個明確的開啟並詢問。

### 5.9 Setup

視窗、客戶端變體與板子設定，加上 **Find window**／**Measure**／**Probe board**、會儲存的
**Use detected title** 動作，以及 **Appearance** 外觀選項（Follow system／Light／Dark）。見第 3.1 節
—— 同一個分頁就是第一次啟動的設定介面。

### 5.10 Hotkeys

四個按鍵名稱 —— **Start**、**Quit**、**Advance grade**、**Pause**。輸入名稱（`F1–F24`、`Esc`、
`CapsLock`、`Space`、`Tab`、`Enter`，或單一按鍵）；核心會翻譯、把儲存的鍵碼名稱顯示回來，無法解析的欄位會
在其下方顯示一行訊息。**v3.0 不會輪詢儲存的鍵碼** —— 見第 8 節。

### 5.11 Calibrate

所有工具共用的校正介面。第 6 節是完整流程：拍下遊戲視窗、選 **Surface**、必要時選**目標實例**、繪製標記、
**Save mark**，然後讀核心回報的已存標記/衍生值/產物。**工具分頁**選擇要校正的對象：**Buy / Sell**、
**Gem Composer**、**Magic Tuner**、**Pet Feeder**、**Tooltip**。前三者會把 Targets 與編輯器限定在該工具，
而且各自記住你在該分頁拍的畫面——切換分頁會回到該工具自己的擷取，不會顯示別的工具的畫面。工作區上方的
精簡**擷取列**顯示目前畫面與顯示環境，並開啟 **Captures**、**Details**（顯示環境 + Detect）與
**Stored marks**；畫布保持固定，目標清單與 inspector 各自獨立捲動，Revert/Save 固定在 inspector 下方。
**Magic Tuner** 與 **Tooltip** 直接把一次拍攝的會談開在工作區，而不是放在已存表格下方。分頁上的說明也再次
說明拍攝行為：殼層會為了擷取隱藏自己、等一個影格、再開回來。標題列也有一個快速外觀選單
（Follow system／Light／Dark）。

---

## 6. 校正：17 種標記

校正標記以**遊戲視窗客戶區的實體像素**儲存（原點 = 客戶區左上角，不含標題列），所以移動視窗不會讓校正
失效；它**不會**被 Windows 顯示縮放換算。契約與原因在倉庫的 `v3/docs/spec/CALIBRATION.md`；本節是操作者
視角。

> **第一次使用這個畫面？** [CALIBRATION-GUIDE.zh-TW.md](CALIBRATION-GUIDE.zh-TW.md) 是逐步操作版：怎麼
> 拍攝、**點選要編輯的影格卡片**、點與矩形的差別，以及兩個 session 校正。

### 6.1 儲存一個標記

1. 把遊戲視窗放到最前面（遊戲必須是前景視窗；殼層會為了擷取隱藏自己，所以拍到的是遊戲，不是 Seal
   Tools）。
2. 在 **Calibrate → Captures** 按 **Capture the window** —— 視窗會閃一下：Seal Tools 隱藏一個影格、拍下
   完整客戶區、再開回來。Captures 下會出現一張 **Frame** 卡片，中間出現畫面。標題的 `1 / 4` 是四張影格
   快取數量，不是「需要拍四張」；一張有效的拍攝就夠校正。
3. **點選要編輯的影格卡片**。最新一張會自動選取；點較舊的卡片即改選那張影格，儲存會用被選取影格的 id。
   標示 **next capture evicts this one** 的是四張中最舊的 —— 先儲存再拍。點卡片只是選影格，不會放標記。
4. 從分組清單選擇 **Surface**（或直接點左邊 Targets 樹的種類）。若是*具名實例* surface，還要選
   **Target instance**（選了才會列出目標；未選目標前 Save 停用）。
5. 在中間的**大圖片**上繪製：單擊設點、拖曳設矩形；或直接輸入 **x / y / (w / h)** 欄位 —— 欄位才是
   權威，圖片只是鏡射。拖曳小於 4 px 會被忽略；點到黑邊會被忽略。縮放/平移細節見 CALIBRATION-GUIDE §5。
6. **Result gem area** 先回答**空結果問題**：現在結果框是空的嗎？**回答前 Save 停用。** `empty: true`
   會同時儲存裁切圖*並*取樣顏色特徵；`empty: false` 只取樣特徵（拍到有寶石的框會污染強的空結果測試）。
7. **Save mark**。結果行顯示 surface、修訂版與標記；`bag_grid` 另外顯示衍生間距，結果寶石裁切圖會出現在
   **Artifact** 下。**Refresh stored** 重新讀取已存標記。
8. **Stored mark** 區塊顯示所選 surface/目標的標記（以及核心回傳的目標），或「Not calibrated yet」。

拒絕是核心的句子，不是默默失敗：背包格必須與背包格網的間距一致（「兩個不可能都對的數字代表拖錯了」）、
設定沒有承載的目標會被拒絕，未儲存的標記不會寫入任何東西。

### 6.2 種類（分組）

選擇器以分組呈現，讓 17 個項目保持可讀。**各工具使用：**

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
| Pet — bag | **Boarding bag grid** | 矩形 | Pet Feeder —— 它自己的格網：代養背包在畫面的另一個位置 |
| Pet — bag | **Boarding bag slot** | 矩形 | Pet Feeder —— 該格網的一格 |
| Pet — rows | **Row start/end button** | 矩形 | Pet Feeder —— 目標 "1".."4" |
| Pet — rows | **Row boarding pet slot** | 矩形 | Pet Feeder —— 目標 "1".."4" |
| Pet — rows | **Row food strip** | 矩形 | Pet Feeder —— 目標 "1".."4"（依該列的堆疊數分割） |

**Spring point 不在這個表裡。** 它和三個 OCR 框一起在 **Tuner OCR 3-box + Spring point** 會談（§6.5）設定，
所以一次改造師拍攝就涵蓋整個讀取。核心仍為相容性提供舊的 `spring_point` 種類；v3.0.10 的 UI 不會要玩家校正
兩次。

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

### 6.4 會談與剩下的非標記工作

只有兩個校正需要會談，因為單一標記表達不了；兩者都列在 **Targets** 中各自工具的群組內，並開啟
**Sessions** 面板：

- **Tuner OCR 3-box + Spring point** —— 一次拍攝，三個 OCR 框與選用的發條點（§6.5）。
- **Pet tooltip offset + shape** —— Pet Feeder 讀取的提示面板（§6.6）。

九個 Pet 標記（menu/feed/close/page 點、bag grid/slot、列矩形）就是上方標記編輯器裡的一般標記 ——
刻意**沒有**另一個 Pet crops/rows 會談，因為那會重複這些標記。列、佇列寵物與食物格仍在 **Pet** 分頁編輯。

剩下的非標記工作：

- **賣出格子選擇** —— 不是校正標記：它是 **Buy/Sell** 分頁上為下一次執行做的、只存在記憶體的背包格選擇
  （第 5.4 節）。它需要已校正的 **Bag grid** 才能對到格子，但沒有自己的校正種類。
- **環境檢查（Detect）** —— 比對規則已在核心，但 v3.0 的 Setup 還沒顯示「你的顯示環境變了」。若換螢幕、
  縮放、視窗大小或換電腦，請**重新校正**；標記是實體位移，v3 刻意不做比例換算（遊戲 UI 不是線性縮放）。

### 6.5 Tuner OCR 3-box + Spring point 會談

1. 在 **Calibrate** 選 **Magic Tuner** 工具分頁。讓遊戲裡的改造師狀態視窗可見，按卡片最上方的
   **Capture screen**（或 **Recapture screen**）。**這一次拍攝就涵蓋三個 OCR 框與 Spring point** ——
   就是 v2 的一次拍攝流程。拍完後畫面固定在左側，框的卡片在右側獨立捲動，所以拖曳與欄位編輯不需要
   上下捲動切換。
2. 選一個 **Target to drag** chip —— **Grade letter**（綠）、**Attribute rows**（藍）、**Remaining count**
   （琥珀）或 **Spring point** —— 然後在**左側的畫面**上直接拖曳。拖曳會填入目前的框並自動跳到下一個；
   要回頭可點卡片標題或 chip。`x / y / w / h` 欄位是替代方式，仍是權威。Spring point 是單擊且選用
   （只在 `spring_mode: hid` 時讀取）。
3. 若不想讓 Check 量測列高，可在 **row height** 指定。
4. 按 **Check boxes**。結果卡片會列出推導出的 region、列高、grade、remaining、filter 判定與符合的屬性列數；
   與遊戲畫面不符就修正框再 Check。
5. 按 **Save tuner**：寫入 `tuner.ocr` + `tuner.spring_point`，並產生標註過的 `calib_tuner.png`；狀態列會顯示
   核心的 revision。

### 6.6 Pet tooltip 位移與面板形狀會談

1. 在 **Calibrate** 選 **Tooltip** 工具分頁。把遊戲放到最前面，讓要量測的提示面板（物品或寵物）可見。
   在 **Tooltip offset + shape** 卡片設定 **hover x / y** 與延遲，按 **Capture hover**。殼層會隱藏、核心把
   游標放到該點並擷取面板；畫面回來後固定顯示在左側，hover/offset 控制項留在右側，游標位置以粉紅色十字
   顯示。
2. 在畫面上框住面板（核心要求至少 20×20 px），或直接輸入 **offset x / y** 與 **width / height**。公式行會顯示
   `cursor + offset = panel top-left`，面板框會疊在游標上。
3. 按 **Save tooltip**：寫入這台機器的 `tooltip` 區塊（位移、大小、延遲）。
4. 按 **Test read** 會再次在同一個點懸停，並用已儲存的位移讀取面板。**Read result** 卡片會列出 OCR 文字；
   結果與 debug 裁切圖也會寫到 `logs/reads/`。

---

## 7. 日誌與檔案

### 您的設定，以及如何搬到另一台電腦

設定依「屬於您」或「屬於這台電腦」分開放：

- `config/setup/` 是**您的設定**（視窗/版本/開發板 ID、快捷鍵、Tuner 與 Gem 行為、Spammer
  輪替、Quest 流程、購買預設、寵物偏好）。要把設定帶到另一台電腦，請**複製這個資料夾**
  （若有筆記，也複製 `config/user.yaml`）。
- `config/machine/` 是**這台電腦量測出來的資料**（校準、OCR 幾何、各標記、tooltip 位移、
  視窗位置、COM 埠覆寫）。請勿複製到另一台電腦，在該機重新校準即可。
- `config/artifacts/` 存放小型參考截圖，重新校準即可重建。
- `config/meta.yaml` 是核心的內部記帳，請不要手動修改。

舊版安裝只有一個 `config/local.yaml`。此版本第一次啟動會自動備份並拆成上述資料夾（請見
INSTALL 的 *Migrating from v2*）。要保留設定，複製 `config/setup/` 與
`config/user.yaml` 即可；`config/machine/` 不必備份，重新校準就會重建。細節請看
`config/README.md` 與 `docs/spec/CONFIG.md`。


### 7.1 即時日誌面板

頂端列的 **Logs (N)**（或 **Ctrl+L**）顯示核心執行時發布的日誌行。它是用來*觀看*執行，不是持久紀錄。

### 7.2 分次執行檔案：`<root>/logs/`

`<root>` 是殼層告訴核心使用的安裝資料夾（狀態列會印出）。核心只附加、不截斷舊檔：

| 檔案 | 內容 |
|---|---|
| `<tool>.log` | 每次執行一行開始、每一則**變更的**狀態訊息，以及最終原因。工具 id：`spammer`、`tuner`、`buy`、`sell`、`gem`、`quest`、`pet`。「它做了奇怪的事」時先讀這個。 |
| `holdspace.log` | Hold Space 的完整軌跡：v2 的 `HoldDiag` 行（釋放失敗、面板行）**以及**一般的分次執行行（`run started`，接著 `finished` / `stopped` / `failed`）—— 它的診斷與執行日誌共用這個檔案。 |
| `error.log` | 內部錯誤與工具開啟失敗。可歸因於玩家的拒絕**不會**在這裡 —— 它們在卡片上，這是刻意的，以免蓋掉真正的錯誤。 |
| `ocr_log.jsonl` | 每次完成的 OCR 掃描一筆 JSON。 |
| `ocr_errors.jsonl` | 完成但沒有產出該模式要的結果的掃描。 |
| `captures/capture_<stamp>.png` | 改造師的 save-captures 診斷檔；紀錄會以 `capture_path` 指出它。 |
| `captures/empty_check_crop_<HHmmss_fff>.png` | 寶石的 **Save empty captures** 診斷檔；每一張也會寫一行 `gem.log` 詳細紀錄。 |

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

檔案切分、載入順序與折疊的完整說明在倉庫的 `v3/docs/spec/CONFIG.md`。

---

## 8. v3.0 的熱鍵

**v3.0 沒有全域熱鍵輪詢器。** 四個熱鍵可以在 Hotkeys 分頁命名、翻譯與儲存，鍵碼也會寫進 `local.yaml`，
但**你在遊戲中按那些鍵時沒有任何東西在監看**，所以現在按 F12/F11/「開始鍵」都不會有反應。

實際上的意思：

- **用卡片上的 Stop 按鈕停止執行**（或殼層自己的控制項）—— 它們永遠有效。
- 寵物餵食器是常駐的；可以讓它跑著，從它的卡片停止，或關掉 Seal Tools。
- v3.0 不要依賴「退出/暫停鍵」；那些行為要等輪詢器回來。

這是刻意的延後，不是藏在文件裡的退步：計畫要求先量測背景程序能否在遊戲取得焦點時讀到按鍵（防作弊可能
阻擋），協定會依量測結果長出指令。它列在倉庫的待辦清單（`v3/docs/backlog/OPEN.md`）。

---

## 9. 常見工作流程

**每台機器一次（約 15 分鐘，加上校正）：**

1. 安裝並啟動 —— 見 [INSTALL.md](INSTALL.md)。
2. **Setup**：客戶端變體與板子 ID；**Save**；**Find window**，再用 **Use detected title**；然後
   **Probe board**。
3. **Calibrate**：擷取，然後為你要用的工具繪製標記。要買賣先做商店群組，再做寶石，若要餵食再做寵物種類。
   每種標記一次儲存。
4. 跑一個短工具，確認板子與遊戲有回應。`logs/<tool>.log` 會證明它做了什麼。

**日常使用：**

1. 啟動遊戲（視窗模式），登入。
2. 開啟 Seal Tools；核心會連上（狀態列顯示 `core … · rev …`）。
3. 在工具分頁設定這次執行需要的東西（Spammer 預設、Tuner 目標/篩選、Gem 開始等級、Quest 流程）。
   Pet Feeder 的分頁會編輯既有的列（啟用/堆疊）、新增或移除佇列寵物（圖示擷取）、標記背包食物格，並設定
   載入模式與計時；新增列或重新擷取列裁切圖仍要從遷移的 v2 `local.yaml` 或手動編輯的區塊而來。然後在卡片按
   **Start**。結束時按 **Stop**。
4. Buy/Sell：Buy 用卡片的執行選擇；Sell 先在 **Sell** 格網標記插槽，再按 **Sell 卡片**上的 **Start** ——
   選擇只供該次執行、不會儲存。並記住：賣出無法復原且有上限。
5. 結束時關閉 Seal Tools —— 關掉 UI 會停止工具與核心。一次只開一份（v3.0 沒有單一實例防護）。

**顯示環境變更後**（新螢幕、Windows 縮放、遊戲視窗大小、新電腦）：重新校正。已存標記是實體客戶區相對位移，
不會跟著搬移。

---

## 10. 疑難排解

| 症狀 | 意義 / 處理 |
|---|---|
| 整個視窗顯示 **「The core is not running」**，有 **Stderr tail** 與 **Restart core** | 核心當掉或開機被拒絕。尾端就是原因（開機拒絕會印到 stderr，不是協定訊息）。**Restart core** 會開新程序與新手勢。**協定版本不合**（`bad_version`）刻意隱藏 Restart：請把 `SealTools.exe` 與 `seal-core.exe` 換成一對相符的版本；不要混用。 |
| `bad_version` | 你混用了執行檔。請用同一份 zip 的一對。 |
| **找不到遊戲視窗（game window not found）** | **Setup** 分頁的變體錯，或標題已不再符合遊戲（標題是唯讀的 —— 用 **Find window**，再用 **Use detected title**），或遊戲沒開/被最小化。Setup 的 **Find window** 會回報核心看到的東西。 |
| **`game_not_foreground`** | 遊戲前面有別的視窗。把遊戲帶到最前面再試；Calibrate 分頁的擷取會自己隱藏殼層一個影格。 |
| 視窗沒有留在遊戲上方 | 檢查 App bar 的 **always-on-top 釘選鈕**：亮起代表已開啟，它會儲存 `settings.ui.always_on_top`（可攜、預設關閉）。儲存被拒絕時，釘選會留在已儲存的值並顯示核心的訊息。 |
| 視窗回到奇怪的位置 | 那是從機器 `ui` 區塊還原的已存位置。移動/縮放後稍等一下再關閉，讓 debounced 儲存寫入；若矩形來自已不存在的螢幕，會被 clamp 進主要螢幕的工作區，日誌會說明。 |
| **`ocr_unavailable`** | OCR 管線在這台機器上無法完成這次讀取。`detail.reason` 是程式在「管線建不起來」時給的原因 —— `models_env_unset`、`models_dir_missing`、`text_fixes_invalid`、`model_load_failed`、`runtime_failed` 之一 —— 而 `detail.message` 會指出失敗的檔案、模型或資料夾（若是模型缺失，會列出**每一個嘗試過的資料夾**）。若失敗發生在掃描已建好的管線時（`bad_image`、`det_output_shape`），會以 `internal` 回報並附上 OCR 訊息：那是壞畫面或程式錯誤，不是機器無法提供 OCR。模型解析順序：`SEAL_OCR_MODELS`（有設定就優先，失敗就明確失敗）、`<root>/models`、`seal-core.exe` 所在資料夾 + `models`；資料夾必須有三個 `.onnx` 才算數。還原出貨的 `models/`（或為自訂位置設 `SEAL_OCR_MODELS`）。 |
| **`board_unavailable`** | 設定的埠上沒有板子，或埠打不開。檢查線材（要能傳資料，不是純充電線）、板子的 USB 驅動、Setup 的 **VID/PID/port**，並按 **Probe board**。同一個 COM 埠只能有一個程式使用。 |
| 卡片出現「某個標記尚未校正」這類行 | 不是 bug；行內會指出要儲存的**標記**與唯一的 **Calibrate** 分頁。到那裡選該種類（若是具名種類，再選目標實例）並儲存。 |
| 「No buy preset selected.」/「No slots are selected to sell.」 | 在 Buy/Sell 新增/選擇買入預設（卡片的選擇器）。Sell 請在 Buy/Sell 的 **Sell** 格網標記插槽（格網需要已校正的 **Bag grid**，第 5.4 節），再按 Sell 卡片上的 **Start**；沒有標記任何插槽時卡片會顯示這一行。選擇不會被儲存。 |
| 「{n} slots selected but the per-run cap is {cap}…」 | 選取的插槽數超過 **Max slots per run (sell cap)**。刻意調高上限，或縮小選擇；核心會在**任何點擊之前**拒絕啟動，並完整印出這一行（含數量與上限）。 |
| 「The active preset … was not found — pick one in the Spammer tab.」 | 在 Spammer 分頁設定 **Active preset (runs)**。 |
| 寵物餵食器啟動被拒絕（標記或繁殖列相關的行） | 餵食器的標記與列必須存在。在 Calibrate 的標記編輯器校正它的 `pet_*` 種類；Pet 分頁可編輯設定中既有的列（啟用/堆疊）與食物格，但新安裝還沒有列 —— 遷移 v2 的 `local.yaml`，或開著倉庫的 `v3/docs/spec/CONFIG.md` 編輯該區塊。 |
| 熱鍵在遊戲中沒反應 | v3.0 的正常情況 —— 沒有輪詢器（第 8 節）。用殼層的 Stop。 |
| 「按 Capture 時視窗會閃」 | 正常：殼層隱藏一個影格，讓擷取拍到遊戲，然後開回來。v2 閃約 0.3 秒；v3 是一個影格加上來回通訊。 |
| 需要上次執行的細節 | 讀 `<root>/logs/<tool>.log`（狀態列會印 root）；`error.log` 與 `ocr_errors.jsonl` 就在旁邊。 |

---

## 11. v3.0 尚未實作的部分

列在這裡，讓缺少的能力是已知邊界，而不是謎團。最新清單在倉庫的 `v3/docs/backlog/OPEN.md`；以下每一項
都是「3.0 沒有」，不是「明天就會有」，除非倉庫另外說明。

- **全域熱鍵。** 沒有輪詢器；見第 8 節。Stop 與殼層控制項都能用。
- **獨立的寵物 crops/rows 校正會談。** 九個寵物標記在標記編輯器個別校正；Pet 分頁編輯列/佇列/食物格。
  沒有第二個寵物會談。
- **環境檢查（Detect）介面** —— 顯示環境變更後請手動重新校正。
- **單一實例防護。** 協定的實例鎖有規格但未實作：不要同時開兩份 Seal Tools。
- **安裝程式與自動更新。** v3.0 是壓縮檔；更新時替換程式檔案並保留你的 `config/`。
- **v2 專屬的外觀與文件。** 這裡不使用 v2 的 WPF 截圖；v2 文件仍是 v2 版本的紀錄，不是這個視窗的說明。

---

## 12. 其餘文件在哪裡

**zip 帶什麼：** 本指南、它的英文版、`INSTALL.md`、`THIRD-PARTY-NOTICES.md` 與 `docs/pet-data.csv`。
以下以 `v3/` 開頭的路徑都是**專案倉庫**中的路徑，不是 zip 內的檔案。

- `v3/docs/INSTALL.md` —— 安裝、遷移、更新、移除、疑難排解（也在 zip 裡，與本指南同一個資料夾）。
- `v3/docs/spec/CONFIG.md` —— 每個設定檔、哪個是你的，以及載入/折疊規則。
- `v3/docs/spec/CALIBRATION.md` —— 座標契約、DPI/縮放為何如此，以及產物規則。v2 的
  `docs/spec/COORDINATES.md` 與 `CALIBRATION.md` 是量測紀錄。
- `v3/docs/spec/PROTOCOL.md` —— 殼層↔核心契約（給工程師；讀拒絕碼時有用）。
- `v3/docs/spec/TOOL-FLOWS.md` —— 每個工具如何決定，以及它可能產生的玩家訊息原文。
- `v3/docs/backlog/OPEN.md` —— 最新的未實作清單，包含全域熱鍵量測。
