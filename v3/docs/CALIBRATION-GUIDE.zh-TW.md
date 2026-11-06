# v3 校正實作指南

這是 **USER_GUIDE §6** 的逐步操作版，假設你用的是 v3.0.10 的 **Calibrate** 分頁，重點放在最容易出錯的
地方：**拍攝、點選拍攝結果，以及在畫面上畫標記**。標記的定義見 `spec/CALIBRATION.md`；17 種標記的清單
見 USER_GUIDE §6.2。

---

## 1. 開始之前

- 遊戲請用**視窗模式**（或無邊框視窗），執行在你平常使用的解析度與 Windows 縮放下。
- **每次顯示環境變更都要重新校正。** 換螢幕、改解析度、改 Windows 縮放或換電腦都要重做；v3 不會把舊
  標記按比例換算。
- 每次拍攝前，**遊戲視窗必須在前景**。殼層會為了拍攝隱藏自己一個影格，拍到的是遊戲，不是 Seal Tools。
- 拍攝前先讓要校正的遊戲畫面出現：商店背包、寵物選單、改造師狀態視窗、要量測的提示面板等。

## 2. Calibrate 畫面速覽

| 區域 | 用途 |
|---|---|
| **Captures**（左上方） | 這次 Calibrate 拍下的影格。可拍攝、選影格、看 `n / 4` 快取數量。 |
| **Targets**（Captures 下方） | 17 種標記，依工具分組；具名實例會展開目標。會談列位在各自工具的群組內，標示 `session`。 |
| **圖片/畫布**（中間） | 拍到的遊戲畫面。**點擊或拖曳標記就是在這裡做。** 上方有 Fit / 1:1 / 縮放 / 平移。 |
| **標記編輯器**（畫布下方） | **Surface**、**Target instance**、`x / y / w / h` 欄位、寶石空結果問題、**Save mark**。 |
| **Sessions**（最下方） | 兩個多標記會談：Tuner OCR 3-box + Spring point、Pet tooltip offset。它們有自己的拍攝按鈕，不使用 Captures 貨架。 |

## 3. 拍攝，然後點選拍攝結果（最常漏掉的一步）

1. **把遊戲切到前景。** 先點一下遊戲視窗；前景不能是 Seal Tools。
2. 在 **Calibrate → Captures** 按 **Capture the window**（第一次之後按鈕會變成 **Capture another**）。
3. 殼層隱藏一個影格、拍下遊戲客戶區、再開回來。Captures 會出現一張 **Frame 1 · hh:mm** 的卡片，中間
   出現畫面。
4. Captures 標題的 **`0 / 4` → `1 / 4`** 是「目前保留幾張影格」相對於**四張快取上限**，不是「需要拍四
   張」。**校正只需要一張有效的拍攝。** 第五張會淘汰最舊的一張。
5. **點擊那張影格卡片本身來選取它。** 被選取的卡片會有高亮背景、勾選圖示與較粗邊框。最新一張會自動被
   選取；只有要刻意用舊影格儲存時，才去點較舊的卡片。
6. 卡片上還會顯示 **`id f1`**（儲存需要的即時 frame id）；若是四張中最舊的，會顯示
   **`next capture evicts this one`**；若核心已拒絕該影格，會顯示 **`refused by the core — capture again`**。
7. **儲存永遠使用被選取那張影格的 frame id。** 如果你正在用最舊的影格、又準備拍第五張，先儲存。

> **點影格卡片只是選擇「要在哪張畫面上畫」**，它本身不會放標記。標記是在中間那張**大圖片**上點擊或
> 拖曳產生的。

如果殼層回來了但畫面全黑或不對，通常是遊戲不在前景或被遮住：點一下遊戲，再按 **Capture another**。
Captures 下方出現的拒絕句子是核心的回答，不是點擊失效。

## 4. 選擇 Surface（與 Target）

1. 在標記編輯器選 **Surface**，或直接在左邊 **Targets** 樹點某個種類；兩者效果相同：選定標記種類並
   載入它已存的值。
2. 若該 surface 是**具名實例**，還要選 **Target instance**（例如寶石等級 `DG`、寵物列 `2`、背包頁
   `ITEM1`）。**未選目標前 Save mark 會停用。**
3. 切換 surface 會刻意清掉目標，避免順手把標記存到錯的實例。

## 5. 在畫面上畫標記：點擊與拖曳的差別

形狀會標在 surface 旁邊；圖片只接受該形狀的操作。

**點類型（Point）**（例如 Focus/scroll point、MAX button、grade point、寵物 menu/feed 點）：

1. 在圖片上**點一下**正確位置。
2. 標記出現，`x / y` 欄位被填入；需要精確值時直接改欄位。
3. 再點別處可移動，或直接改欄位。

**矩形類型（Rectangle）**（例如 Bag grid、Bag slot、Shop list region、result gem area、寵物列矩形）：

1. 在目標區域的**一角**按住左鍵。
2. 拖到**對角**再放開。
3. 矩形畫出來，`x / y / w / h` 被填入。
4. 拖曳距離小於 **4 px** 會被當成太小而忽略；拖遠一點，或直接輸入欄位。
5. 要調整就再拖一次取代，或直接改精確數值。

兩種形狀都適用：

- 請點在**遊戲圖片內**。點到黑邊/letterbox 會被忽略，不會被硬塞成標記。
- 圖片放大時，**Fit** 顯示全部、**1:1** 是一個顯示像素對一個影格像素；滾輪縮放、**Pan** 平移。標記會依
  目前縮放/平移換算，所以照畫面上看到的位置畫即可。
- **`x / y / w / h` 欄位才是權威**：拖曳只是填入欄位，**Save mark** 送出的是欄位值。不確定時直接輸入
  遊戲的像素座標。
- 矩形的 `w`/`h` 必須為正；bag slot 必須與 bag grid 的間距一致，否則核心會用自己的句子拒絕。

## 6. 儲存並確認一般標記

1. 只有 **Result gem area** 要先回答空結果問題：**現在結果框是空的**，或**框裡有寶石**。未回答前 Save
   停用。
2. 按 **Save mark**。
3. 結果行會顯示 surface、新 revision 與標記。`bag_grid` 另外顯示推導出的間距；結果寶石裁切圖出現在
   **Artifact**。
4. **Refresh stored** 會重新讀取已存標記；**Stored mark** 區塊顯示核心對目前 surface/target 的值，或
   「Not calibrated yet」。
5. 拒絕是核心的文字，不是點擊遺失。修正標記或目標後再存一次。

## 7. 全新安裝的建議順序

1. **Shop：** Bag grid → Bag slot → Shop list region → MAX button → Focus/scroll point。
2. **Gem：** Result gem area（回答空結果問題）→ 會用到的 grade point → 會用到的 resource slot point。
3. **Pet：** menu point、feed point、Boarding X point、bag page tab point，接著 boarding bag grid + slot，
   再來是會用到的每一列 row 矩形。全部用上方標記編輯器逐個做 —— 已經沒有獨立的 Pet rows/crops 會談
   （那只會重複這些標記）。
4. **Tuner：** **Tuner OCR 3-box + Spring point** 會談（§8.1）—— 一次拍攝涵蓋四個 tuner 目標。
5. **Pet tooltip：** hover offset + shape 會談（§8.2）。

先校正工具真的會讀的標記，再啟動該工具；缺標記時工具會回報，不會亂猜。

## 8. 兩個 Session-shaped 校正

這些面板在 Calibrate 最下方的 **Sessions**。每個都會**自己拍攝**（Captures 貨架是給上面一般標記用的），
一次儲存多個相關值；畫面會畫出影格，欄位仍是權威。

### 8.1 Tuner OCR 3-box + Spring point

用途：改造師的三處讀取（grade letter、三列 attribute、remaining count）與選用的 Spring point ——
**全部用同一次改造師拍攝**，和 v2 一樣。

1. 遊戲中打開改造師狀態視窗，在 **Tuner OCR 3-box + Spring point** 卡片按 **Capture screen**。這一次
   拍攝同時用於三個 OCR 框與 Spring point，不需要為發條點再拍一次。
2. **依序**拖三個框：**Grade letter**（綠）→ **Attribute rows**（藍）→ **Remaining count**（琥珀）。拖曳
   會填入目前卡片並自動跳下一張；要回頭可點卡片/chip。每張卡片有自己的 `x / y / w / h` 與像素尺寸；
   拖曳小於 4 px 會被忽略。
3. 選用：選 **Spring point** 後在同一張畫面上點它；只有在不想讓 Check 量測列高時才手動設 **row height**。
4. 按 **Check boxes**。結果卡片顯示推導出的 region、列高、grade、remaining、filter 判定與符合的屬性列
   數；與遊戲不符就修框再 Check。
5. 按 **Save tuner**：寫入 `tuner.ocr` + `tuner.spring_point` 與標註過的 `calib_tuner.png`；狀態列顯示核心
   revision。

### 8.2 Pet tooltip offset + shape

用途：提示面板相對於游標的位置與大小。

1. 把遊戲放前景，讓選定點 hover 時會出現要量測的面板。在 **Tooltip offset + shape** 卡片設定
   **hover x / y** 與延遲，按 **Capture hover**。核心把游標放過去並拍攝；畫面回來時游標是粉紅十字。
2. 在畫面上把面板框起來——核心要求至少 **20 × 20 px**——或直接填 **offset x / y** 與 **width /
   height**。公式行顯示 `cursor + offset = panel top-left`；面板框會畫在游標上。
3. 按 **Save tooltip**（寫入 machine 的 `tooltip` 區塊：offset、size、delay）。
4. 按 **Test read** 用已存值再 hover 讀一次。**Read result** 卡片列出 OCR 行；除錯裁切圖在 `logs/reads/`。

九個 Pet 標記（menu/feed/close/page 點、bag grid/slot、列矩形）就是上方標記編輯器的一般標記 —— 沒有
另一個 Pet rows/crops 會談。列與佇列寵物在 **Pet** 分頁編輯。

## 9. 疑難排解

| 症狀 | 可能原因與處理 |
|---|---|
| 拍攝回紅字拒絕／殼層回來但沒有影格 | 遊戲不是前景（或被最小化）。點一下遊戲，再按 **Capture another**。 |
| 拍到全黑或拍到 Seal Tools | 同上，前景問題；點遊戲後重拍。 |
| 在圖片上點擊沒反應 | 目前 surface 是**矩形**（要拖曳），或點到黑邊/letterbox。點類型請點在遊戲圖片內。 |
| 拖曳沒有產生標記 | 拖曳小於 4 px，或該 surface 是**點**（要單擊）。拖遠一點／換成正確形狀。 |
| Save mark 停用 | 具名 surface 未選 target、沒有選取拍攝，或寶石空結果問題未回答。 |
| 拖完後儲存被拒絕 | 核心規則（例如 bag slot 與 grid 間距、tooltip 小於 20×20、設定沒有該 target）。讀句子並修正標記。 |
| 顯示 `next capture evicts this one` | 你正在用四張中最舊的影格。先儲存再拍，否則會被當成 unknown frame id 拒絕。 |
| 改顯示設定後標記偏移 | 重新校正；不要跨解析度/縮放重用舊標記。 |
| Targets 樹上某列顯示 `0/4` | 該種類四個具名實例中還沒存任何一個（不是要拍四張）。 |

## 10. 一頁摘要

1. 遊戲在前景 → **Capture the window**。
2. **點選 Frame 卡片**，決定要編輯哪一張拍攝。
3. 選 **Surface**（需要時再選 **Target instance**）。
4. 在大圖片上：**點類型 = 單擊；矩形 = 拖曳**，或直接輸入 `x / y / w / h`。
5. 有問寶石空結果就回答 → **Save mark**。
6. 多標記工作改用 **Sessions** 卡：Tuner OCR 3-box + Spring point（一次拍攝）或 Pet tooltip offset；
   其他全部都是上方的一般標記。
