# v2.12 — the pet feeder, finished

Two days of work on the Pet Feeder: the reload becomes one window visit for every row that is due, the
next check comes from the pet rather than from a fixed clock, the return slot is tried first, and the
food cells are **found instead of clicked**.

**Nothing here needs a reflash.** Everything is live the moment you launch it; the firmware-2 items
(food-load by drag, the board reporting its own version) are unchanged and still fall back.

**What was verified live, and what was not.** The schedule work was verified on the live game on
2026-09-22/23 — the examples below are real log lines. **The food scan (§5) has never run against a live
bag**: it compiles, it is unit-tested, and its first run is yours to judge. That is said plainly here
rather than left for you to discover.

---

## 1. One window open does every row that is due

The reload used to open and close the boarding window **per row**, so four rows due together was four
opens and three closes — nine window operations where one would do, and two of a live run's four failed
on exactly that reopen.

Now **the look and the reload are separate visits with different scopes**: one open reads *every* row,
acts on the ones that are due, and closes once. A row that fails does not take the visit with it.

The look and the reload are deliberately different, and that has a consequence worth knowing: **the
reload never re-reads.** It acts on what the look decided, which may have been hours earlier.

## 2. The next check comes from the pet, not from the clock

Three figures now compete, and **whichever runs out first** wins:

| figure | where it comes from |
|---|---|
| the food | `stacks × 300 ÷ 3 per min`, the full load just put in |
| the **game's own line** | `代養完成預計所需時間：約26分` — read under each row |
| the **pet's own need** | its hover panel (`+N` and EXP%) against the feeding table |

**The game's line is accurate to the minute, and that is measured, not hoped:**

```
20:54:40  Row 2: 300 item(s) left — reloading in 31 min  [代餐完成预計所需時間：約26分]
21:25:41  visit: opening the breeder once for 1 row(s) — Row 2      ← 31 min later, to the second
21:25:47  Row 2: slot reads EMPTY, so this row is not boarding — it will be reloaded now
```

The pet had finished and been mailed exactly when the game said it would.

**Only the completion form drives a schedule.** A row mid-run shows the *level* form (`到7為止…約21分`),
which resets every level and reads 20–39 minutes on a healthy pet — acting on that would visit
constantly. It is read and quoted in the log, and ignored.

## 3. The return slot is tried first, and hovered to decide

*"when we need to reboard, look for the return slot first; if the return slot is not a pet we can board,
then do the page scanning."*

It is **one cell and one hover** against a three-page scan and three captures — and it is where the pet
this row was just feeding comes back to. This reverses what the code did (the queue first, with
deliberately *no* fallback), and the old reasoning held only while the slot was *assumed empty*, which
it is not.

**The hover is the decision:**

```
return slot, nothing readable there  →  nothing to right-click; scan instead
return slot, a pet at +5 40%         →  board it
return slot, a pet at +9 100%        →  finished; scan for one that can be fed
```

A pet that reads **+9/100% is refused and the next candidate tried** — boarding one raises an error
dialog and wastes the cycle.

## 4. Reading the feeder honestly

- **An empty feeder is a reading of zero, not an unreadable one** — and a tray already at zero
  **reloads now**, rather than waiting out the five-minute margin. *"if we know it is empty, why wait
  another 5 minutes?"*
- **A partial count read understates, and that is the safe direction**: a slot with no number in it is
  simply not counted, so a row looks emptier than it is — and reloading early is nearly free, because
  ending boarding returns the leftover food with the pet. Overstating would leave a pet unfed, which is
  the one outcome here that cannot be undone.
- **"Nothing in the bag can be fed" is a wait, not a failure** — the row keeps its place and is looked
  at again in 30 minutes instead of counting towards being dropped for the day.
- **The bag sweep is cheaper**: it no longer re-clicks the tab for the page already showing, and it
  finds the occupied cells rather than hovering all 64.
- **`SleepCheck` takes SECONDS** and the launcher's `Task.Delay` takes milliseconds. Passing one to the
  other slept for eleven minutes and looked exactly like a hang. It happened once, live.

## 5. The food is found, not marked

Marking food cells by hand is nineteen precise clicks to add and nineteen more to take the stale ones
off — and **a stale mark is worse than none**, because the tool right-clicks whatever is in that cell
now. The food moves every time you play.

**One crop of one food item finds them all**, through the bag grid that is already calibrated and the
same `IconMatch` sweep the pet queue uses:

1. click **one** cell holding food,
2. **Capture food icon** — it brings that cell's bag page up itself and crops from the grid,
3. **Scan the bag for food** to look, or **Scan + update the food cells** to replace them.

Two buttons rather than one with a flag, for the reason the queue has two: a scan you press to *look* at
must not quietly rewrite what a run acts on. The scan resets the used-cell count, because a changed set
makes the old count meaningless. **One crop, because you feed one stage at a time** — several types at
once would need a crop each.

## 6. The feeding line is set per queue entry, and shown

The estimate is only as good as the pet's **line** — one of eleven — and that is namable only by you,
because the hover panel carries stage/growth/EXP and no name at all (a name lookup was measured out: the
game is Traditional and the table is Simplified, so it could not have matched).

The line used to be settable **only at the moment of capture**, by a picker that reset on every launcher
start — so the first capture after a restart wrote an entry with **no line**, and **nothing displayed the
value**, making the loss invisible. Three of five live entries were blank for exactly that.

The queue is now **one row per pet** — its crop, its label, and its own line dropdown, with a ✕ that
drops that row. The capture picker now seeds from the last line already on the queue, so a restart no
longer starts a capture on the blank.

**Name the line the pet actually belongs to.** The colour (异色) lines are listed only up to the stage
where they merge into their normal line, so a pet past that stage takes the **normal** line — naming it
the colour one finds no row for its stage and the estimate silently does nothing.

## 7. Two things that were silently broken, and the evidence for it

**A pet boarded from the return slot could never be estimated.** The candidate is built with no queue
entry, so there was no line to look up: of 23 feeding-estimate failures in the live log, **10 were
return-slot boardings**. It cannot be *read* either — the hover panel is taken from a **bag** cell and a
boarded pet is in the loader — so the row now **remembers what it boarded** and the pet that comes back
is estimated from that.

**A row's time line could read nothing while its neighbours read fine.** The region aimed a 24-pixel box
at a measured offset; a row carrying an extra line above its timer puts it about **7 pixels lower**, and
a crop that misses returns **nothing** rather than something wrong. The band is now deliberately
generous — and drawn **on the capture**, with its pixels in the row-geometry readout, so a region that is
off can be *seen* instead of inferred from a log line.

That last one is the general lesson: **a derived region must be tolerant, not exact.** The count crops
survive a 7-pixel error because they read from inside a 60-pixel slot box; a 24-pixel box aimed at a
10-pixel line has no slack at all.

---

## Files

`config/local.yaml` gains `pet.food_icon_png` (one food crop) and, per queue entry, its line. An existing
`local.yaml` keeps working: unknown keys are ignored on load, so nothing has to be migrated by hand —
but **the three blank queue entries and the food cells want re-doing**, and §5 and §6 are the fast way.

## Upgrading

Unzip over the old folder, keeping your `config\`. Nothing to reflash. If you use the pet feeder, set the
line on each queue row (§6) — the estimate is inert without it.
