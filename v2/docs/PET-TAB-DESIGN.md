# The Pet tab, and how the schedule and the reload flow work

**What this is:** what the current code does, read off the source, for review. It proposes nothing.
Line references are to `SealTools.Pet/PetTool.cs` and `SealTools.Launcher/MainWindow.xaml.cs`.

---

## 1. The tab, section by section

In the order they are built — `BuildPetTab`, MainWindow.xaml.cs:5280.

| # | Section | Line | What is in it |
|---|---|---|---|
| 1 | **The bag, right now** | 5344 | `Page to edit` · `Clicking a bag cell marks…` · the 64-cell bag picker · **Mark FOOD cells** / **Mark the RETURN slot** / **Mark a pet to queue** · a cell info line |
| 2 | **Setup so far — this tab** | 5354 | the checklist of what is marked and what is missing |
| 3 | **Ready to run** | 5363 | the readiness verdict and the **Start** |
| 4 | **Timing** | 5417 | `Wait after empty (min)` · `Action wait (ms)` · `Food load` (right-click / drag — "needs firmware 2") |
| 5 | **Rows and boarding state** | 5463 | the per-row **RUN** tick, the per-row **boarding right now** tick, and **RELOAD EVERY ROW ON START** |
| 6 | **The queue — which pets to breed** | 5544 | the captured icons by label · `Name for the next one` · **Capture the marked pet's icon** / **Remove last** |
| 7 | **Find them** | 5570 | **Scan the bag for these pets** |
| 8 | **Read a pet's panel** | 5585 | **Test read the pet panel** |
| 9 | **Find the pets that can still be fed** | 5607 | **Scan bag for feedable pets** / **Scan + rebuild the queue** |
| 10 | **Save** | 5643 | writes this tab's half — the run's state, not the calibration |

**All the marking is on this tab** — the bag cells, the return slot, the pet icons. I had this wrong in
the first draft of this document.

**The two ticks are a fallback, not the source of truth.** The hint at 5464 says it: the tool opens the
breeder at start and *looks* at each row's pet slot, so a row already feeding is left alone. The ticks
settle the rows whose slot could not be read, and the tool keeps them up to date after every reload.
**RELOAD EVERY ROW ON START** is the override for what looking cannot see: the slot says a pet is in
the loader, it does not say how much *food* is left.

---

## 2. Two paths through the same window

```
        THE LOOK  (InspectRows, PetTool.cs:487)      THE RELOAD  (ReloadRow, PetTool.cs:700)
        ───────────────────────────────────          ─────────────────────────────────────
  once, at run start                             once per row, at that row's own time

  ClaimGame ────────────────┐                    ClaimGame ────────────────┐
  open the boarding window  │                    open the boarding window  │
  for EACH active row:      │                    (end boarding, if running)│
    · pet slot empty?       │                    place a pet               │
    · feeder counts (OCR)   │                    load the food             │
    · → next[row]           │                    start boarding            │
  close the window          │                    close the window          │
  ReleaseGame ──────────────┘                    ReleaseGame ──────────────┘
  returns a SCHEDULE                             writes the state
  (writes nothing)                               (boarded, fed, flag set)
```

They are separate visits, and **the reload never re-reads** — it acts on what the look decided, which
may have been hours earlier.

---

## 3. The schedule

```
Run()                                                PetTool.cs:153
 │  log "run started — board firmware <n>, N row(s) at 3/min, plus 5 min after empty: …"
 │
 ├─ ok = ClaimGame() .............. waits for another tool to finish, if one owns the game
 ├─ next = ok ? InspectRows() : {}         ← the LOOK; one window visit for all rows
 ├─ PublishSchedule()                      ← card: "boarding R1 · next R2 at 19:40 · 2 more queued"
 │
 └─ while (!QuitPressed && !ct.IsCancellationRequested)      PetTool.cs:250
      │
      ├─ row = live.OrderBy(r => next[r]).First()      ← the SOONEST due row
      ├─ if (!SleepUntil(next[row], ct)) break          ← hours, typically
      ├─ ClaimGame(state, …)                            ← yields mid-wait if another tool starts
      ├─ try   ok = ReloadRow(row)
      │  finally ReleaseGame()
      │
      ├─ ok  → failures[row] = 0
      │        next[row] = now + CycleMinutesFor(row)
      │        "Row N reloaded. Next HH:mm."
      │  !ok → failures[row]++;  the row is DROPPED after 3
      └─ loop
```

**One schedule per row.** The free row holds 2 food stacks and a paid row 5, so a full load lasts 200
minutes on one and 500 on the other; one clock for both would reload a paid row while half full, or
leave the free row dry for five hours (PetTool.cs:170).

### Where `next[row]` comes from

```
CycleMinutesFor(row) = LoadMinutesFor(row) + WaitAfterEmptyMinutes
                     = (stacks × MaxStack) / ItemsPerMinute + 5
                     = (2 × 300) / 3 + 5  =  205 min            Row 1
```

`MaxStack = 300` (`FeederCount`). So the number is arithmetic about **food**, with no pet in it.

`ScheduleFor(row, itemsLeft)` (PetTool.cs:573) sets it three ways:

| read | `next[row]` | card/log |
|---|---|---|
| counts read | `items / rate + WaitAfterEmptyMinutes` | `381 item(s) left — reloading in 132 min` |
| counts not read | the full-load cycle above | `couldn't be read — assuming a full load, 205 min` |
| `!row.BoardingRunning` | **now** — the row needs a pet | `slot reads EMPTY — it will be reloaded now` |

A fourth: `ReloadOnStart` returns `now` for every row, so all of them are due at once.

---

## 4. Inside one reload

```
ReloadRow(row)                                       PetTool.cs:700
 │
 ├─ "opening the boarding window…"   目錄 → the feed icon → Enter
 ├─ try
 │   ├─ if (row.BoardingRunning)  end boarding — returns the leftover food
 │   │     · press the toggle, set BoardingRunning = false, persist
 │   │     · SleepCheck(Max(EndWait, ActionWait))
 │   │     · CHECK THE EFFECT, do not assume it (a toggle pressed with the wrong idea
 │   │       does the opposite) — a bad check stops rather than clicking on
 │   ├─ "placing the pet"   → PlacePet
 │   ├─ "loading the food"  → LoadFood   (right-click + MAX, or DRAG on firmware ≥ 2)
 │   ├─ "starting boarding" → StartBoarding
 │   ├─ row.BoardingRunning = true;  persist
 │   ├─ SleepCheck(Max(StartWait, ActionWait))
 │   └─ "reload complete"   → return true
 └─ finally: CloseBoarding(ser)          ← EVERY exit closes the window
```

The close is in a `finally` and the comment at 709 says why: leaving the window up would sit on the
game through a 205-minute wait and the next cycle would click 目錄 behind it.

**The open and the close are inside `ReloadRow`, and the loop calls it once per row — so N rows due at
once is N opens and N-1 closings.** Today's run did four in a row:

```
15:32:09  reload Row 1 …   15:35:56  reload Row 2 …   15:36:01  reload Row 3 …   15:36:03  reload Row 4
```

That is a cold start: every row read `EMPTY` at once, so every `next[row]` was in the past and the loop
found four due rows with no wait between them. In normal running one row comes due at a time.

---

## 5. The pet placement, in detail

```
PlacePet(row, out error)                             PetTool.cs:845
 │
 ├─ candidates = FindQueuedPetCandidates()           PetTool.cs:1066
 │     for EACH page: SelectPage → ParkCursor → capture
 │       for EACH queued icon: ScoreAll(bag, grid, icon)
 │         EVERY cell ≤ MatchLimit  →  a candidate (page, cell, score, label)
 │     sort best-first
 │
 ├─ for each candidate:
 │     SelectPage → move onto the cell
 │     ┌─ THE GUARD ────────────────────────────────────────────────┐
 │     │ move onto the cell (a MOVE — a click switches the pet)     │
 │     │ wait HoverDelayMs → OCR the tooltip region → PetPanel.Parse│
 │     │ IsFinished (+9 and 100%)?  → skip, try the NEXT candidate  │
 │     │ unreadable?                → board it as before            │
 │     └────────────────────────────────────────────────────────────┘
 │     right-click → check the slot → retry up to 3×
 │
 └─ none left → "Every queued pet reads +9/100% — nothing can be boarded"
```

---

## 6. Where each decision is made

| decision | made by | reads | when |
|---|---|---|---|
| does this row need feeding? | `InspectRows` | pet slot + feeder counts | run start, once |
| when does it need it? | `ScheduleFor` | the counts, or the cycle | run start, once |
| which row now? | the loop | `next[row]` | continuously |
| which pet? | `FindQueuedPetCandidates` | the queued icons | per reload |
| can that pet be fed? | the guard | the hover panel | per reload |

Rows 1–4 are the same code with different coordinates — there is no per-row branching.

---

## 7. Points worth a verdict

Not proposals — the decisions embedded in the design, listed so they can be judged.

1. **The look and the reload are separate visits.** The reload never re-reads, so a row whose state
   changed between the look and the reload is not noticed.
2. **N rows due at once is N window opens.** Each open is 目錄 → feed icon → Enter and a set of cursor
   placements; two of today's four failed on exactly that.
3. **The cycle is about food, never about the pet** — `300 × stacks / rate` cannot know that a pet has
   finished, that a feeder ran dry early, or what the pet itself needs.
4. **The guard reads growth and EXP% at board time, and nothing else uses them.** Those two numbers
   are in hand at the one moment a schedule could be computed from them.
5. **A finished pet is not a special case in the schedule.** It is only refused at board time, so a row
   whose pet has finished still gets a reload scheduled, opened, and refused.
