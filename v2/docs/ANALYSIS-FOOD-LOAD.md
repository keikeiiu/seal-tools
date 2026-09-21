# Analysis — the food load path: what actually needs fixing

Written before any change. For each issue: what was observed, whether it is real, the **cause**, and
what could be done. Line references are `SealTools.Pet/PetTool.cs`.

**Scope: the five issues raised today that touch the food load and the placement.** Nothing here has
been changed; this is for a verdict.

---

## What the code does, in order

`LoadFood` (PetTool.cs:1223), once per stack:

```
for stack in 0..stacks-1:
    cell = NextFoodCell()          ← ADVANCES and PERSISTS the cursor, then logs "used"
    SelectPage(page)
    drag ? DragFood(bagCell → the row's box)      DragFood, PetTool.cs:1311:
         : Click(bagCell, right)                      PlaceOn(bagCell); wait; re-aim; LeftDown
                                                      PlaceOn(slot);    wait; Log "stack N onto"
                                                      finally LeftUp
    wait
    Click(MAX)                     ← assumes a count dialog is up
    wait
    Enter()                        ← assumes the same
    Log "enter (confirms the count dialog …)"
```

---

## Issue A — the first drag misses

**Observed:** you watched the first drag fail and the second succeed. The log agrees they are different:
drag 1 presses and drops inside the same second, drag 2 takes a second.

**Real?** Yes — the food is not in the feeder afterwards, and nothing else notices.

**Cause — one of two, and they are distinguishable but not yet distinguished:**

1. **No beat between the press and the move.** `DragFood` sends `LeftDown` and then immediately calls
   `PlaceOn(slot)`. There is no wait for the game to register the *grab* before the cursor leaves the
   cell. A human drag holds still for a moment; this one starts moving in the same frame.
2. **The press goes out before the cursor has settled.** `PlaceOn` is called twice (move, then re-aim)
   precisely for this, but if `PlaceOn` returns before the cursor is where it claims, the press lands
   beside the cell.

Both would hit the **first** drag hardest, because the first drag follows a page switch and a longer
cursor journey than the second.

**What could be done:** a hold after `LeftDown` before the move (minutes of work, one line plus a
constant); or verifying the cursor is settled before the press. The first is cheap and addresses both
causes weakly; measuring which one it is would need the cursor trace across a failing drag.

---

## Issue B — the drag is never verified, and MAX + Enter fire regardless

**Real?** **Yes, and this is the one I would rank highest — not for the lost food, for the stray input.**

`DragFood` logs `"picked up"` immediately after sending `LeftDown` and `"stack N onto"` immediately
after the move, before the release. Both are claims about intent. The caller then clicks **MAX at
(1531,968)** and presses **Enter** whether or not anything was picked up.

So a missed drag does not merely fail quietly, it **fires a left-click and a keypress into the game**
at coordinates that were chosen for a dialog that is not there. On a screen where the boarding window
is open, that is input the tool never intended to send.

**Cause:** every action in `LoadFood` after the drag assumes the drag worked; there is no
"click, then LOOK" step. The pet placement has one (`PetSlotIsEmpty`, three attempts), the feeder
counts have one, the toggle has one — the food load has none.

**What could be done:**

1. **Verify the count dialog is up before MAX.** Something has to be looked at; the dialog's position
   is not currently marked, so this wants a capture region (a small calibration job). This is the
   principled fix and it also makes a missed drag *detectable* rather than silent.
2. **Verify the bag cell emptied.** The dragged stack should be gone from the bag after a successful
   drag — so a before/after look at that cell answers "did it move". No new calibration: the cell
   centre is already computed and the empty-cell comparison already exists in the codebase
   (`DifferingFraction`).
3. **Cheapest partial:** make the log honest — say "sent", not "picked up" — so the log cannot be read
   as evidence of success. Not a fix, but it stops the log from lying, which is how today's confusion
   started.

---

## Issue C — the food-cell counter advances on intent, and persists

**Real?** Yes, proven from the code and the log ordering:

```
15:35:37   food cell 1/23 used (page 2, cell 7)      ← the cursor moved HERE
15:35:40   drag: picked up FOOD cell 7 …             ← the load happened 3 s later
```

`NextFoodCell` increments `FoodSlotsUsed`, persists it, and logs **"used"** before anything has been
used. A missed load therefore still burns a cell, permanently.

**Does it matter?** Two consequences, one mild and one not:

- The cursor drifts ahead of the bag, so the tool skips a cell that still holds food. With 23 cells
  and 2 per reload, a few misses are absorbed.
- When the cursor reaches the end, `LoadFood` fails: *"No food cells left — N of M used. Mark the
  cells holding food again"* — **and that is the failure that leaves a pet unboarded.** The drift
  brings it forward with no visible cause.

**Cause, and it is the more interesting half:** the counter is a **cursor**, not a statement about the
world. It cannot be checked against anything, so it cannot correct itself. That is the same class of
problem the row schedule had before this tool moved to "look first, don't reload first" (PetTool.cs:175)
— the fix there was to *read* the state instead of assuming it.

**What could be done:**

1. **Advance after a successful load**, not before — but "successful" needs Issue B first, so this
   alone just moves the guess.
2. **Look first, as the rows now do:** find the next food cell by checking which marked cells are
   still non-empty, and drop the cursor entirely. No new calibration — the cells are marked and the
   comparison exists. This removes the drift by construction.
3. **Status quo + re-mark.** It is recoverable and the error message already says what to do.

---

## Issue D — cursor placement failures, on a reopened window

**Observed:** rows 2 and 3, `FAILED re-aiming at the pet feed icon` / `FAILED moving to the 目錄
button`, the cursor stopping 100–600 px from the target. Also seen at 10:29 and 13:58.

**Real?** Yes, and it is the biggest single cost today — it is why rows 2–4 were not fed.

**Cause: not established.** What is known: placement lands within a pixel in a manual test minutes
later (14:07 trace) and in the icon sweep; it fails during a run, and worst on a window that has just
been reopened. That points at the game being busy or animating while the tool moves the cursor, but
that is a hypothesis — nothing measured yet separates "the game swallowed input" from "the move
overshot".

**What could be done:** nothing sensible before measuring. The cursor trace is the instrument, and it
was noted today that it stops recording earlier than the tool stops acting — so it needs checking
first, or the evidence is missing exactly where the failures are.

---

## Issue E — a failed row retries every 5 minutes, three times, then is dropped

```
next[row] = DateTime.Now.AddMinutes(RetryMinutes);   // RetryMinutes = 5
if (failures[row] >= MaxFailures) next.Remove(row);  // MaxFailures = 3
```

**Real?** Working as designed, but the design assumes a transient fault.

**The question worth a verdict:** a row that fails because the *game state* is wrong will fail the same
way three times in fifteen minutes and then be **dropped for the day** — while the other rows carry on
(PetTool.cs:296). Is that better than leaving it due at its normal cycle, so it is tried again in a few
hours when whatever was wrong may have passed? Today it means rows 2–4 burn 15 minutes of retries and
finish unfed either way.

---

## Summary — a verdict on each

| | Needs fixing? | Confidence in the cause |
|---|---|---|
| **A** first drag misses | yes, but it is a symptom | medium — two candidate causes, neither measured |
| **B** drag unverified + stray MAX/Enter | **yes — highest** | high — proven from the code |
| **C** counter advances on intent | yes, but recoverable | high — proven from code and log |
| **D** cursor placement failures | **yes — biggest cost** | low — no cause established |
| **E** 3 × 5-minute retries | your call, not a bug | high — it is doing exactly what it says |

**If one thing is fixed:** B, because it is the only one that sends input the tool never intended, and
it is the reason a missed drag is invisible.

**If one thing is investigated:** D, because it is what actually stopped three rows being fed today,
and it is the only one with no cause attached.
