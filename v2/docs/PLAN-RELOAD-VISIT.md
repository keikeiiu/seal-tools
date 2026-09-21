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

## 6. The schedule must be RE-MEASURED, not fixed at run start

**What happens now.** `next[row]` is computed once, at run start, by `InspectRows` — from a reading of
every row. After that, only the row that was just reloaded gets a new value, and it is
`now + CycleMinutesFor(row)` (PetTool.cs:285). **Every other row keeps a time derived from a reading
that may be hours old.**

**Why that is wrong, in the player's words:** *"sometimes some rows may be due soon — let's say the row
is on +9 and it is loaded, we cannot revisit this after 10 hours."* Concretely, three things change a
row's real state and none of them is noticed until its own timer fires:

- the pet reaches `+10` and is mailed, so the row is empty and needs a new pet;
- the feeder runs dry early (a short load, a missed drag, a pet eating faster);
- a row that failed and was retried is now fed, while a row that succeeded may not need touching.

**The change.** A **visit re-reads every row and re-derives every row's `next`** — including the rows it
did not act on. The schedule becomes a per-row *measurement* rather than a per-row *assumption*, which
is the same move this tool already made once ("look first, don't reload first", PetTool.cs:175), applied
to the timer instead of to the reload.

**What is already per row, and stays:** `next[row]`, the cycles (`205` vs `505` min), `RetryMinutes`,
`MaxFailures`, and the failure counting. Nothing about this section makes the clocks more separate —
they are already separate. What changes is that each one is **re-derived from a fresh reading**.

## 7. The `+9` case — needs a rule only the player can give

**The tool already reads the boarded pet's growth and EXP%** — the guard hovers the pet and parses the
panel before it right-clicks (bc6f438). **Nothing uses those two numbers beyond skip-or-board.**

If a pet at `+9` with a low EXP% will finish before the food runs out, then: the pet is mailed, the row
is empty, and today nothing notices for up to **505 minutes**. That is the case the player is pointing
at, and it is not solvable by reasoning about the tool — it needs the game's arithmetic:

| question | why it matters |
|---|---|
| **How do we know when a `+9` pet finishes?** | the whole point — an EXP-per-minute or EXP-per-item figure, or an ETA the game shows |
| **Does the panel or the boarding window state it?** | if yes, it is a capture region, not a table |
| **If it is a rate, what is it?** | `EXP%` per food item, or per minute — with the rate, `(100 − EXP) / rate` is the time to finish |
| **When the pet finishes, what should the row do?** | come back with a new pet immediately, or at the next cycle? |

**Until that is answered, the honest fallback is a floor:** never schedule a row further out than the
soonest thing that could change it — the food running out, or (if the EXP% is high) a much nearer
re-check. That is deliberately not a rule, and it should not be built as one.

## 8. Order of work

1. **Page tracking (§3a)** — small, safe, pays immediately, removes the kind of click that failed today.
   Independently valuable, so it lands on its own.
2. **The remembered pet cell (§3b)** — also small-ish, also pays every reload, and no reason to wait
   for the restructure.
3. **The visit (§2)** — the restructure, with the single-row path provably unchanged.
4. **Re-measured schedule (§6)** — needs the visit, because re-deriving every row's `next` means
   reading every row, which is what a visit does. Doing it before the visit would mean two readers
   again.
5. **The `+9` rule (§7)** — blocked on the player's answer, and worth having the answers before any of
   it is designed.

Each its own commit, each verified by the Release build and the test suite. Each is independently
useful: stopping after any of them leaves the tool working and faster.
