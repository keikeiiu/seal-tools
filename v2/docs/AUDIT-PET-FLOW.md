# Audit — the pet feeder's action and decision flow

Written 2026-10-06, from [PetTool.cs](../SealTools.Pet/PetTool.cs) as it stands. **An audit, not a
change.** Every action the tool sends to the game, and every branch it takes, in one place — so the
flow can be judged rather than reconstructed from a 2 246-line file.

Line numbers are given so any claim here can be checked against the code.

**How to read it.** §1 is the vocabulary. **§2 is the inventory — every input sent to the game, which is
where the "extra actions happening in the background" live.** §3–§8 are the flows as diagrams. §9 is the
one scheduling rule. **§10 is every decision point in a single table.** §11 lists what I think is worth
your verdict, separated from what is merely description.

---

## 1. The cast

| Term | What it is |
|---|---|
| **Row** | One breeding slot you feed. Up to four, each independently ticked, each with its own clock. |
| **Visit** | One opening of the boarding window, doing every row that is due inside it. |
| **Reload** | One row's offload → re-board → load food → start. |
| **The bag** | The boarding bag: an 8×8 grid per page, with page tabs. Pets AND food live in it. |
| **The look** | The startup read — open, read every row, close, without reloading anything. |
| **The gate** | `PortGate`. One holder at a time; the feeder *waits* for it and says who it waits for. |

---

## 2. Every action sent to the game

This is the complete input inventory. Anything here that you did not expect is a candidate for your
verdict — several are defensive and fire even when nothing needs them.

| # | Action | Where | When | Defensive? |
|---|---|---|---|---|
| 1 | Click **目錄** | `OpenBoarding` 1345 | every visit + the look | no |
| 2 | Click **feed icon** | `OpenBoarding` 1347 | " | no |
| 3 | **Enter** | `OpenBoarding` 1350 | **unconditionally**, every open | **yes** — dismisses the "food used up" dialog, which otherwise blocks 結束代養. A no-op when no dialog |
| 4 | Click **ITEM *n*** tab | `SelectPage` 2157 | only when the wanted page differs from `_bagPage` | no |
| 5 | Click the **boarding X** | `CloseBoarding` 1359 | every visit, in a `finally` | no |
| 6 | Click **toggle** (end) | `PressToggle` 1212 | only when `row.BoardingRunning` | no |
| 7 | **Enter** | 1238 | straight after the end press | **yes** — clears an error dialog a bad toggle raises |
| 8 | **Move** onto a bag cell (hover, NOT a click) | `ReadHoverPanel` 1572 | per candidate, when the tooltip is calibrated | read only |
| 9 | **Right-click** a pet cell | `Click` 1511 | up to **3** attempts, until the slot reads occupied | no |
| 10 | **Park cursor** | `ParkCursor` 1620 | before every capture (page scans, bag cells) | read only |
| 11 | **Right-click** a food cell **or** drag it | `LoadFood` 1897/1899 | once per stack | no |
| 12 | Click **MAX** | 1918 | after every food load, drag or right-click | no |
| 13 | **Enter** | 1923 | confirms the count dialog — **one**, not two | no |
| 14 | Click **toggle** (start) | `StartBoarding` 1271 | once per successful reload | no |

**Side effects that are not game input, but are actions:**

- **`_persistState()` — writes `local.yaml`** at three points: after the end press (1226), after the
  start press (1277), and at the end of every `ReadRows` (783). It rewrites the session half each time.
- **An `OcrEngine` (an ONNX session) is constructed and disposed per use** — per visit (`ReadRows` 766),
  per placement (`ReadHoverPanel` 1477), and once more per reload for the landed-line log (1295).
- **Captures**: the bag is captured once per page during a pet scan (`FindQueuedPetCandidates` 1694) and
  **saved to `logs/reads/`**; a bag cell is captured twice per drag attempt (1893, 1911).
- **Instrument-only OCR that decides nothing**: after every successful reload, the tool reads the
  feeder's time line and logs **every** line under the slots (1293–1323). The comment is explicit that
  it schedules nothing yet — it exists to gather evidence on whether the completion form appears the
  moment a pet lands.

---

## 3. Run start

```
Run()                                                                    PetTool.cs:283
 │
 ├─ Ready()  ── any precondition missing? ──────────────────────────►  STOP, message on the card
 │              (目錄, feed icon, X, bag grid+slot, page tabs,             :285-291, :580-654
 │               ≥1 ticked row, each ticked row's toggle label,
 │               drag mode → every row's food strip,
 │               every marked cell's page tab calibrated,
 │               return slot OR ≥1 queued icon, ≥1 food cell, MAX)
 │
 ├─ PetFeeding.Load(pet-data.csv)   ← a failure here costs the COMPUTED schedule only
 │
 ├─ CLAIM THE GAME ── if another tool holds it, WAIT (says who) ──►  stopped? → end the run
 │                                                                    :321-344
 ├─ InspectRows() = the LOOK
 │    ├─ OpenBoarding()                              (actions 1,2,3)
 │    ├─ ReadRows()            ← slot state + feeder counts + time line, per row
 │    └─ CloseBoarding()  (finally)                  (action 5)
 │
 ├─ failures[row] = 0 for every active row
 ├─ PublishSchedule()   → the card's standing line + NextActionAt
 └─ ENTER THE MAIN LOOP                                                :386
```

`Ready` is the tool's only "refuse to start" gate, and it checks **every ticked row**, not just the
first — a row that is switched off is not driven and is not required to be calibrated.

---

## 4. The main loop — one pass

```
while not cancelled
 │
 ├─ live = rows still being carried;  empty → "every row has failed" → STOP        :388-393
 │
 ├─ soonest = the row whose next is earliest
 ├─ SleepUntil(next[soonest])            ← this is where the tool spends the day       :396
 │
 ├─ batch = every row due NOW, in ROW ORDER (normally one row)                        :401-406
 │                                                          batch empty → batch = [soonest]
 │
 ├─ CLAIM THE GAME ── held by another tool?  WAIT, and say who
 │      the row's next is left IN THE PAST, so the same row is picked again after        :412
 │      the wait — no bookkeeping, and no silent stall
 │
 ├─ Visit(batch)                                                          :417, §5
 │
 ├─ Fold the result back:                                                            :427-489
 │    ├─ rows READ but not acted on  → take the freshly measured next
 │    └─ rows ACTED on:
 │         ├─ Ok          → next = now + min(pet needs, load) + WaitAfterEmpty
 │         ├─ NothingToBoard → next = now + 30 min, failures NOT incremented   (:465)
 │         └─ Failed      → failures++ ; ≥3 → DROP THE ROW, others carry on     (:480)
 │                          otherwise next = now + 5 min
 │
 ├─ PublishSchedule()
 └─ Beep: 523 if every row in the visit was Ok, else 200                            :492
```

**Two reschedules that are easy to miss:** a row that merely *reads* gets its next from the reading, and
the **30-minute** wait for "nothing to board" is deliberately **not** a failure.

---

## 5. One visit

```
Visit(batch)                                                             PetTool.cs:1093
 │
 ├─ OpenBoarding()  ── failed? ──► every row fails, and NO schedule is returned
 │                                  (returning the fallback would overwrite every    :1104-1112
 │                                   other row's measured next with "assume full")
 │
 └─ try
     ├─ ReadRows()             ← READ FIRST, every row, not just the due ones         :1121
     ├─ for each row in batch, IN ROW ORDER:
     │      ReloadRowInPlace()   — one at a time; a failure does not take the visit    §6
     │
     └─ the SECOND time-line read, window still open                                 :1148
          ├─ row acted on     → keep its finish figure, folded in with the estimate
          └─ row NOT acted on → only ever pull its next EARLIER
                                 (a bad read can never delay a row)
    finally → CloseBoarding()                                                        :1172
```

**The read happens twice on purpose**, and it costs one OCR pass — no extra window, click or food. The
first read is seconds after the window opens; a row that read nothing gets a second chance a minute
later instead of waiting hours for the next visit.

---

## 6. One row's reload

```
ReloadRowInPlace(row)   — the boarding window is ALREADY OPEN                    PetTool.cs:1189
 │
 ├─ if row.BoardingRunning:                            ← the pet is in the loader,
 │    ├─ PressToggle()  (action 6)                        not in the bag              :1208
 │    ├─ row.BoardingRunning = false + persist           ← the flag follows the PRESS
 │    ├─ Enter()  (action 7)  ← clears an error dialog a bad toggle may have raised
 │    └─ CHECK IT WORKED: PetSlotIsEmpty == false? ──► FAIL
 │         (the toggle can do the opposite; a live run got a boarding STARTED
 │          with no pet, and the game's error box wedged every click after it)
 │
 ├─ PlacePet()          → see §7                                                    :1254
 │
 ├─ petMinutes = MinutesForBoardedPet(row)   ← asked HERE, while the panel still
 │                                             describes the pet just boarded        :1263
 ├─ LoadFood()          → see §8                                                    :1267
 │
 ├─ StartBoarding()  (action 14)                                                    :1271
 ├─ row.BoardingRunning = true + persist      ← the press is what starts the feed
 ├─ Sleep(StartWait 2.0)
 │
 └─ INSTRUMENT: read the time line again and LOG EVERY LINE under the slots         :1293
      (schedules nothing; exists to find out whether the completion form appears here)
```

**The reload sends up to three Enters that are not obviously part of it** — one clearing the breeder's
dialog, one after the end press, one confirming the count. Each is deliberately unconditional, because
"no dialog → Enter does nothing" is better than "dialog → missing Enter wedges the flow".

---

## 7. Placing the pet

```
PlacePet(row)                                                                    PetTool.cs:1386
 │
 ├─ candidates = [ THE RETURN SLOT ]  ← tried first: one cell, one hover, vs a
 │                                       three-page sweep                       :1414-1418
 │  + [ the icon scan ]  ← every queued icon against every calibrated page      :1424, §2 of the scans
 │
 ├─ for each candidate, best match first:
 │    ├─ SelectPage()                                    (action 4)
 │    ├─ hover the cell  (action 8)  → the +9/100% GUARD:
 │    │     ├─ panel reads FINISHED → REFUSE this one, try the next              :1480
 │    │     ├─ panel is NULL and this is the RETURN SLOT → skip, go to the scan  :1493
 │    │     │     (an empty fixed cell means nothing to click, not "unknown")
 │    │     └─ panel is NULL otherwise → BOARD IT ANYWAY                         :1500
 │    │           (unknown must never remove a boarding — that leaves a pet unfed,
 │    │            the unrecoverable direction)
 │    │
 │    └─ up to 3 right-clicks (action 9), each followed by a SLOT CHECK:          :1509
 │         ├─ slot OCCUPIED → done                                                :1519
 │         ├─ slot UNREADABLE → done, "unknown is not failure"                    :1524
 │         └─ slot still EMPTY → retry
 │
 └─ every candidate refused → nothingToBoard = true (a WAIT, not a fault)         :1555
```

**Two different rules for "the panel could not be read"**, and the asymmetry is deliberate: a
*matched* pet boards on unknown, the *return slot* does not.

---

## 8. Loading the food

```
LoadFood(row)   — once per stack (row.Stacks)                                     PetTool.cs:1841
 │
 ├─ cell = NextFoodCell()   ← the next marked cell NOT yet used; counter is persisted
 │      null → FAIL: "no food cells left" (the list is exhausted, and the tool
 │              refuses to guess a cell rather than click an empty one)            :1851
 │
 ├─ SelectPage(page)                                                               :1859
 │
 ├─ DRAG mode?  → the row's OWN box for this stack, from the food strip           :1875
 │      no strip to divide → FAIL (a drag that releases over whatever is there
 │                           is the one failure here that can lose an item)
 │
 ├─ up to 3 attempts (drag only; a right-click gets 1 — the game chooses the box,
 │   so there is no cell to look at afterwards):
 │    ├─ capture the bag cell (before)
 │    ├─ DRAG it   (action 11)     or   RIGHT-CLICK it (action 11)
 │    ├─ [drag] capture again and log the difference at the drop (measurement only,
 │    │          no decision taken on it)                                          :1909
 │    ├─ Click MAX    (action 12)
 │    ├─ Enter         (action 13 — ONE, not two)
 │    └─ [drag] was the cell emptied?
 │         ├─ yes            → done
 │         ├─ COULD NOT TELL → done, and do NOT re-drag ("unknown is not failure":
 │         │                    re-dragging an empty cell fires MAX and Enter at a
 │         │                    dialog that is not there — worse than the miss)     :1933
 │         └─ no             → re-drag
 │
 └─ still not moved after 3 → FAIL, nothing loaded for that stack                  :1948
```

---

## 9. The one scheduling rule

`ScheduleFor(row, itemsLeft, eta)` — [PetTool.cs:809](../SealTools.Pet/PetTool.cs#L809). Everything that
decides *when* is here, and it is checked in this order:

```
1. reload_every_row_on_start?         → NOW
2. the row is not boarding            → NOW          (it needs a pet and a fill)
3. the feeder counts could not be read:
      a. the game's finish line reads → now + finish + WaitAfterEmpty
      b. otherwise                    → now + the CONFIGURED cycle   (assumes a full load)
4. itemsLeft <= 0                     → NOW          (already past empty; a margin
                                                       here is sleep for its own sake)
5. otherwise                          → now + min(food, finish) + WaitAfterEmpty
                                        where food = itemsLeft / items_per_minute
```

**"Whichever runs out first"** — the food in the tray, or the pet itself — is the whole point of step 5,
and steps 3a and 5 are the only places the game's own time line drives anything.

---

## 10. Every decision point

| # | Decision | Where | One way | The other |
|---|---|---|---|---|
| 1 | Any precondition missing? | 285 | stop with a message naming it | run |
| 2 | Another tool holds the game? | 412 | wait, saying who | act |
| 3 | Slot reads empty / occupied / unreadable | 736 | `BoardingRunning` follows it | keeps the ticked state |
| 4 | Feeder counts readable? | 767 | schedule from items ÷ rate | cycle, or the finish line |
| 5 | Items left ≤ 0 | 828 | reload now | wait it out |
| 6 | Game's completion line readable? | 818 | min(needs, load) | load alone |
| 7 | Row boarding? | 1208 | end it first, then verify | go straight to placement |
| 8 | Pet came back after the end press? | 1241 | carry on | **stop**, do not click on |
| 9 | Return-slot hover empty? | 1493 | fall through to the scan | board there |
| 10 | Candidate reads finished (+9/100%)? | 1480 | refuse, try the next | board it |
| 11 | Hover panel unreadable? | 1500 | board anyway | — |
| 12 | Pet slot occupied after the click? | 1517 | done | retry (×3) / unreadable → done |
| 13 | No candidate boardable at all? | 1555 | `NothingToBoard` — a 30-min WAIT | — |
| 14 | Bag cell emptied after a drag? | 1932 | done | unreadable → done / not moved → retry |
| 15 | A row has failed 3×? | 480 | **drop that row**, the others carry on | retry in 5 min |
| 16 | Second pass reads a finish line? | 1162 | pull the next EARLIER only | leave it |

**The recurring shape is "unknown is not failure"** (10, 11, 12, 14), and it is always resolved in the
direction that can still feed the pet. The one place it resolves the other way is #8, and that is
because a toggle that did the opposite leaves a boarding running with nothing in it.

---

## 11. Worth your verdict

Not description — the things I would want a decision on, in the order I would ask them.

1. **Three unconditional Enters** (actions 3, 7, 13). Each is defensible alone, and together they mean a
   reload can send up to three keypresses that do nothing in the common case. The alternative is to send
   them only when a dialog is known to be up, which needs a read the tool does not currently do.

2. **The instrument block after every reload** (§2, 1293–1323). It reads the feeder's time line and logs
   every line, scheduling nothing. It is deliberate evidence-gathering, and it means a run does an extra
   OCR pass per reload until someone reads the log and decides.

3. **`PetSlotIsEmpty` is the only verification that a press landed** (decision 8). Every other press in
   the tool — the toggles, MAX, the food clicks — assumes it landed. That is stated in the code, and it
   is why a mis-press surfaces one row later as a mystery rather than at the moment it happened.

4. **`ReadRows` writes `BoardingRunning` from the slot reading** (decision 3), and the reload then
   writes it again from the *press* (1225, 1276). Two writers for one flag, each correct about a
   different thing, is worth confirming you want.

5. **The food list is consumed, not re-derived mid-run** (`NextFoodCell`, 2092). Once exhausted the
   reload fails until the cells are re-marked — and the scan added on 2026-10-02 re-marks them during a
   pet scan, which is when a pet is looked for. A row that reloads without any pet scan would not get
   that refresh.

6. **`_persistState` writes `local.yaml` three times per cycle** (1226, 1277, 783). Small, but it is the
   same file the UI saves from, and the lost-update half of that race is an open item in TODO.md.

---

## 12. What this audit did not look at

The startup preconditions are listed by their checks (§3) but each is not traced to its failure
symptom; the OCR reads themselves (`FeederEta`, `PetPanel`, the count gate) are audited elsewhere; and
the placement primitive (`PlaceOn` / `Click` / `WaitForCursorToSettle`) is shared with every other tool
and is not pet-specific.
