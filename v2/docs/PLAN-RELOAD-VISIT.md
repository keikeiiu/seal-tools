# Plan — one window open should finish everything that open can do

**Goal, in the player's words:** *"each window open should finish all the action that window open
should do"*, and *"we reduce those redundant actions"*.

Today the tool opens the boarding window, does **one** row, closes it, and opens it again for the next.
Four rows due at once is four opens and three closes — nine window operations where one would do — and
two of today's four failed on exactly that.

Line references are to the current `SealTools.Pet/PetTool.cs` and
`SealTools.Launcher/MainWindow.xaml.cs`.

---

## 1. What happens now

```
Run()                                                          PetTool.cs:153
  next = InspectRows()          ← one open, reads EVERY row, writes nothing, closes
  loop:
     row = the soonest-due row
     SleepUntil(next[row])
     ClaimGame
     ReloadRow(row)             ← open → ONE row → close      PetTool.cs:700
     ReleaseGame
```

So the reading is already batched — one open for all rows — and the **acting** is not.

Four rows due at once (a cold start, or `RELOAD EVERY ROW ON START`):

```
15:32:09  reload Row 1 … open → act → close
15:35:56  reload Row 2 … open → act → close
15:36:01  reload Row 3 … open → act → close     ← FAILED on the reopen
15:36:03  reload Row 4 … open → act → close     ← (row 3's neighbour survived)
```

## 2. The change

**A VISIT replaces the per-row reload.** When any row is due, everything that is due is done in one
open.

```
Run()
  loop:
     due = every row whose next[row] is due NOW, soonest first
     if due is empty:
         SleepUntil(the soonest next[row])   ← unchanged
         continue
     ClaimGame
     Visit(due)                              ← open once … close once
     ReleaseGame
```

and inside it:

```
Visit(due)
  open the boarding window
  try
     ── READ ────────────────────────────────────────────────────────────
     for each active row:  pet slot occupied?   feeder counts?
     ── DECIDE ──────────────────────────────────────────────────────────
     needs[row] = no pet in the loader  OR  the counts could not be read  OR
                  the row is flagged reload-every-row-on-start
     ── ACT, one row at a time, in order ────────────────────────────────
     for row in due where needs[row]:
         end boarding if running
         place a pet            ← the guard
         load the food          ← the drag check
         start boarding
     ── nothing else needs doing ────────────────────────────────────────
  finally
     close the window
```

**The read half already exists** — `InspectRows` (PetTool.cs:487) does exactly this and returns the
schedule. It is reused rather than rewritten: two readers for one question is how they come to disagree.

## 3. The redundant actions to remove

Measured from the last good run, not estimated.

**a. The same page is selected once per stack.** All **22 marked food cells are on one page**
(`page index 1`), and `LoadFood` calls `SelectPage` before every stack:

```
16:06:00  click the ITEM2 tab at (1064,217)   ← already there
16:06:07  click the ITEM2 tab at (1064,217)   ← already there
16:06:15  click the ITEM2 tab at (1064,217)   ← already there
16:06:22  click the ITEM2 tab at (1064,217)   ← already there
```

Four of five provably redundant, each costing a cursor placement and `SleepCheck(Max(PageWait,
ActionWait))` — about **1.2 s** — and each one a fresh chance for the cursor to miss. The same happens
in the guard when two candidates share a page.

**Fix:** remember the current page; `SelectPage` becomes a no-op when it is already there. Reset to
"unknown" whenever the boarding window opens or closes, because reopening brings the bag up on whatever
page it likes.

**b. The bag is swept for the pet on every reload.** `FindQueuedPetCandidates` clicks all three tabs and
captures all three pages to find a pet that is almost always where it was last time, and then clicks
back to that page.

**Fix, and only after (a):** remember the pet's last (page, cell), try that cell first — verify with the
same panel read the guard already does — and fall back to the full sweep when it fails. This is the
biggest single saving left, and it is the same "look first" move the schedule already made.

**c. The window opens and closes per row.** Removed by §2.

## 4. What must not change

- **The single-row case must behave exactly as today.** That is the normal case for the rest of the
  day: one row comes due, one open, one close. If that path changes at all, every future reload is put
  at risk for the sake of a case that happens once per cold start.
- **A row that fails must not take the visit with it.** Today each row counts its own failures and is
  dropped after 3; in a shared visit, row 2 failing must not stop row 3 being fed.
- **The window closes on every exit**, including a throw — today that is a `finally`, and it stays one.
- **The schedule is unchanged**: the per-row cycles, `RetryMinutes`, `MaxFailures`, and the look-first
  behaviour of `ScheduleFor`.

## 5. How it can be tested without a cold start

`RELOAD EVERY ROW ON START` makes every row due at once, on demand — so the batch path can be exercised
deliberately rather than only when a bag has been left un-fed. Without it, the new path would be
untestable outside a cold start, which is how it would rot.

## 6. Order of work

1. **Page tracking (a)** — small, safe, pays immediately, removes the kind of click that failed today.
   Independently valuable, so it lands on its own.
2. **The visit (c)** — the restructure, with the single-row path provably unchanged.
3. **The remembered pet cell (b)** — the biggest saving, and the one that most wants (1) in place first.

Each its own commit, each verified by the Release build and the test suite.
