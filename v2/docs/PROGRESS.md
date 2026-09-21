# Progress log

A dated, append-only record of what was actually done and why — the reasoning that is not visible in
the code or the commit titles. One entry per session or per landed victory, newest first.

**How to use it:** append an entry when a change lands. Keep it short: what was the goal, what was
decided and why, what was measured, what is still open. Link to the commit and to the doc that owns
the detail (`CURSOR-INVESTIGATION.md`, `MOVE-SETS.md`, …). Do not restate what the code already says.

---

## 2026-09-22 (26) — the return slot is tried FIRST, and hovered to decide

**The player's ordering, and it is the cheap one:** *"when we need to reboard, look for the return slot
first; if the return slot is not a pet we can board, then do the page scanning."*

The return slot is **one cell and one hover** against a **three-page scan and three captures** — and it
is where the pet this row was just feeding comes back to, so it is both the cheapest thing to check and
the likeliest thing to board.

**This reverses what the code did, deliberately.** It was the queue first with *"deliberately NO fallback
to the return slot"* — the reasoning being that right-clicking a marked cell on the assumption that it
holds the pet is the guess the icon matching exists to replace. That reasoning held while the slot was
**assumed empty**, and it is not: the pet the reload just ending put back is sitting in it.

**The hover IS the decision**, which is why the guard now runs on both kinds of candidate:

```
return slot, nothing readable there  →  nothing to right-click; scan instead
return slot, a pet at +5 40%         →  board it
return slot, a pet at +9 100%        →  finished; scan for one that can be fed
no return slot marked                →  scan, as before
```

**One asymmetry kept on purpose.** A *matched* candidate still boards when its panel cannot be read —
unknown must never remove a boarding — but the return slot does **not**, because it is a fixed cell that
is normally empty, so no panel there means nothing to click rather than that we do not know. It only
applies when the tooltip is calibrated; without one the slot is used as it was before.

**The failure message now says which shape it is** — pets found and all finished, or nothing found at
all — because they need different fixes.

**Verified:** Release build clean, 0 warnings; 172/172 tests. **NOT verified live.**

---

## 2026-09-21 (25) — "no pet available to feed" is a WAIT, not a failure

**Live, and the player called it:** row 3 was empty, the bag held only `+9/100%` pets, and the guard
refused all four candidates — which was **correct**, there was nothing boardable. But the row was counted
as a **failure**, and three failures drop it for the day. A tool that gives up on a row because the
player has not caught another pet yet is punishing them for their bag.

> *"If find the bag is no available pet to feed it is not error."*

**The two nothing-to-board paths now say so** — no candidates matched at all, or every candidate read
`+9/100%` — and the outcome carries a `NothingToBoard` flag rather than being lumped in with a click that
missed. The row keeps its place: **no failure counted, no drop.**

**And it waits half an hour, not five minutes.** A real failure is retried in five because it might be a
transient miss; this cannot be, because nothing changes until a feedable pet exists. Thirty minutes is a
compromise between noticing soon and opening the boarding window on a bag that has not changed.

**Everything else that run did was right**, and it is worth recording because three separate pieces were
being watched for the first time:

- **the time line read on this machine** — `[到2為止预計所需時間：約6分]`, first try, from ratios rather
  than pixels. The OCR even wrote `预計` in Simplified where the game has `預計` and the parser did not
  care, which is why it anchors on `約 N分`.
- **row 3's pet finished and was mailed** exactly as the completion line had said it would — `slot reads
  EMPTY` — instead of the row sitting untouched until midnight.
- **the empty-slot threshold, measured live**: `0.0 %` for the empty slot against `46.2 %` and `31.6 %`
  for slots holding food whose number the reader missed. Both directions correct on real pixels, with the
  line at 2 %.

**Verified:** Release build clean, 0 warnings; 172/172 tests. **NOT verified live.**

---

## 2026-09-21 (24) — the game states the next check itself, and row 3 was the proof

**The boarding window writes a time line under every row's food slots**, and it changes FORM when the pet
is nearly done. Read off a live window:

```
代養完成預計所需時間：約14分        boarding COMPLETES in about 14 min   ← row 3
到2為止預計所需時間：約 20分        reaches level 2 in about 20 min      ← rows 1, 2, 4
該欄位約27日9時51分16 秒後到期。     the food's shelf life — not a countdown
每1分 攝取3個。                     consumes 3 per minute
```

**Row 3 is why this matters.** Its line said the boarding completes in **14 minutes**, and the tool had the
row scheduled for **00:31 — six hours out**. The pet would be mailed, the row would go empty, and nothing
would look at it until midnight. That is the failure the player has been describing all day, stated by the
game in plain text the whole time.

**The rule:**

```
the line says 完成 in N min  →  next = now + N + margin        (the pet is finishing)
otherwise                    →  next = the food figure          (which is what it did before)
and either way               →  whichever runs out first
```

**The level form is deliberately NOT a trigger.** It resets every level and reads 20–39 minutes on a
healthy pet, so visiting on it would visit constantly. It is parsed and quoted in the log; the food keeps
the row fed.

**The region is derived as RATIOS of the slot height**, not as pixels — `gap 0.53`, `line 0.40`,
`width 4.7`, `x −0.10` — measured on two rows that agree (+10/+32 and +11/+34). That is the answer to
*"how do you derive it for another PC"*: the strip is already calibrated per machine and scales with the
UI, so a PC whose slots are taller gets a proportionally lower and taller region **with nothing new to
calibrate or drag**. A fixed `+32` would be right here and wrong on any screen that scales.

**The trap, and it was measured rather than reasoned about:** the food-expiry line carries a `約` and a
number near a `分` too. It is excluded two ways — by 到期, and by requiring the number's unit to be 分
(its number is followed by 日) — so a shelf life cannot be read as twenty-seven days of countdown.

**Verified:** Release build clean, 0 warnings; **172/172** tests (ten added). **NOT verified live.**

---

## 2026-09-21 (23) — an empty feeder reads as "no digits", and the tool called that a full load

**The bug that left a pet unfed.** Row 1's feeder ran dry. Both its slots read *nothing*, the tool said
*"the feeder counts couldn't be read — assuming a full load"*, and scheduled the row **205 minutes** into
the future. The row went unfed and the tool planned to wait until 20:42.

**The read was correct. The interpretation was wrong**, and the crops prove it — both are blank cells,
no food and no digits:

```
empty cell    0.0 %          warm pixels (R − B > 40)   ← both slots of the dry row
has food     30.4 – 37.0 %                              ← every slot of a full one
```

**The code even states the distinction, and the distinction was unreachable.** Its comment reads
*"NOTHING read is null; ZERO is a real reading meaning an empty feeder… null says 'assume a full load',
zero says 'reload NOW'."* But **ZERO can never be produced**: an empty slot has no digits to read, so
every empty feeder arrives as "nothing" and lands in the *assume a full load* branch. The one case that
most needs reloading now was the one case that always waited.

**The fix uses pixels the tool was already holding.** When the OCR returns no number, the slot's crop is
measured for food colour: bare beige scores 0.0 %, an ochre icon 30 %+, and the threshold sits at 2 % —
an order of magnitude inside the gap either way. Empty is then a **real reading of zero**, which the
existing null-versus-zero logic already treats as *reload now*.

**It costs a screen grab only on the slots that failed to read**, and the number is logged either way,
so whether the threshold is sitting in a gap or on a cliff is visible rather than asserted.

**Three theories died on the way here**, all recorded above: the two-digit crop theory (row 2 read `81`,
two digits, perfectly), the fraction too-far-left theory (real, and fixed to 0.45 — but it never
explained row 1, whose slots were empty), and the "read is flaky" theory (the read was never flaky;
there was simply nothing to read).

**Verified:** Release build clean, 0 warnings; **162/162** tests (four added). **Not verified live.**

---

## 2026-09-21 (22) — scope 4b: the next check is computed from the pet that was boarded

**The chain is complete and it runs on data the tool already had.** The guard reads the boarded pet's
growth and EXP% (`bc6f438`); the queue entry names the pet's LINE; the table turns those into minutes.

```
next[row] = min(the pet's remaining minutes, what was just loaded) + WaitAfterEmptyMinutes
```

**Whichever runs out first**, which is the player's rule — and a fresh reload is always a full load, so
the food half is `LoadMinutesFor`. **A null pet figure leaves the arithmetic exactly as it was** —
`CycleMinutesFor`, i.e. loaded + margin — so nothing regresses when no line is named, no panel was read,
or the table has no row for that line at that stage. Null is the normal case, not an error.

**The identification decision, made concrete.** The queue entry carries `Species` — one of **eleven**
lines, not 327 pets, because a line holds many pets and they share one figure. It is picked once, when
the icon is captured, because the icon IS the identity: the pet is found by matching that crop, so the
species rides with the thing that is already unique. Matching the panel's NAME to the table was the
alternative and is blocked — Traditional game text against Simplified site data.

**The projection got all three places**, which is the point: `PetQueueEntry.Species`,
`LocalPetQueueEntry.Species` + `ToConfig()`, and the save direction in `AppliedSession`. This projection
has silently dropped a field **three times across two sessions**, so both directions are now pinned by
tests using a **real non-ASCII species** — a null in the fixture would have passed whether or not the
field was carried.

**Verified:** Release build clean, 0 warnings; 158/158 tests. **NOT verified live.**

---

## 2026-09-21 (21) — scope 4a: the feeding arithmetic, and the table that answers it

**The formula was already in the repo** — `PET-DATA.md`, scraped 2026-09-15 and measured per level on
2026-09-16 — and its own opening line says it exists so *"how long until this pet is done"* is a per-pet
number rather than a rule of thumb. Nothing read it. Now `PetFeeding` does.

```
cost(+n)      = wyz × (1 + n/10)      n = 0 … 9
to +9 / 100%  = wyz × 14.5            the +9 bar fills too — 14.5, not 9
```

**It collapses to one line, because the table already carries the total.** Whatever fraction of the
total *value* is left is the same fraction of the total *minutes* — both describe the same run — so no
per-item value and no per-minute rate has to be derived or configured:

```
minutes = remainingValue / line.TotalValue × line.Minutes
```

**The measured example the whole thing was for:** a stage-6 pet at `+9` 10 % has 8,892 of its 75,400
feeding value left, so it is done in **~99 minutes** — where the configured cycle would have waited
**505**. That is the player's *"if the pet is already +9 80 % you don't need another 2 hours"*, as
arithmetic.

**Two things the data taught, both found by tests failing rather than by reasoning:**

- **The stage-7 `.G` pets sit at 33000 and cannot be boarded.** Left in, seven of the sixty-nine
  `(species, stage)` pairs are ambiguous. They are dropped on load.
- **`(species, stage)` does NOT identify a pet — it identifies a `wyz`.** A line holds several pets at
  the same stage, all sharing one figure. The first version of the test asserted uniqueness and failed;
  what holds, and what is now pinned, is that every row in a group agrees on `wyz` **and** on the total
  value and minutes — or the answer would depend on which row was found first.

The shipped csv is parsed by a test rather than trusted: it asserts the full table loads and every group
agrees, from the real file, so it cannot rot while a fixture keeps passing.

**Verified:** Release build clean, 0 warnings; **158/158** tests (nine added).

---

## 2026-09-21 (20) — scopes 2 and 3: the visit, and a schedule re-derived on every look

**One open does every row that is due.** Four rows due at once used to be four opens and three closes —
nine window operations where one would do — and two of a live run's four failed on the reopen. Now the
loop collects every row whose `next` is in the past, orders them **row 1, 2, 3**, and hands them to one
visit: open once, do them all, close once.

**Scopes 2 and 3 collapsed into one change.** The visit has to read every row to decide what to do, and
that same read is what re-derives every row's time — so batching the acting and re-measuring the
schedule are the same code. Splitting them would have meant a visit that reads and then ignores what it
read.

**What moved:**

| before | after |
|---|---|
| `InspectRows` — open, read, close | `InspectRows` = open + `ReadRows` + close; **`ReadRows`** is the look with the window already open |
| `ReloadRow` — open, one row, close | **`ReloadRowInPlace`** — one row, window already open, no open or close |
| — | **`Visit`** — owns the open and the close for the whole batch |

**The subtlety worth recording.** A row the visit *acted on* takes its next from the reload; a row it did
**not** touch takes its next from the read. That split is not fussiness: the reading happens **before**
the reload, so for a row that was just reloaded it describes the state the reload replaced. Applying it
there would schedule the next reload from a stale tray.

**A bug caught in review, before it ran.** The open-failure path returned the *fallback* schedule for
every active row — which the loop would then have applied to rows it had not read, overwriting each one's
measured time with "assume a full load". A single failed open would have quietly discarded everything the
run knew. It now returns **nothing**, so a look that did not happen re-derives nothing.

**A side benefit, not designed for:** the read now refreshes `BoardingRunning` immediately before the
reload, so the "end boarding first" decision uses the state as it *is*. At 15:32 that decision ran off a
tick that could have been hours old.

**Unchanged, deliberately:** the per-row failure counting and `MaxFailures`, `RetryMinutes`, the `+9/100%`
guard, the drag check, and the row order. The single-row case — the normal one — goes through the same
path it always did; the only addition is the read, a few seconds before a reload that takes minutes.

**Verified:** Release build clean, 0 warnings; 149/149 tests. **Not verified live.**

---

## 2026-09-21 (19) — scope 1: the bag page is not re-clicked when it is already showing

**Measured, not assumed.** All **22 marked food cells are on one page**, and `SelectPage` ran before
every stack — so a 5-stack row clicked that one tab five times, four of them provably redundant, each
costing a cursor placement and about 1.2 s of waiting. Every one was also another chance for the cursor
to miss, which is the failure that cost three rows today.

**The change:** `_bagPage` remembers where the bag was put; `SelectPage` logs and returns when it is
asked for the page already showing, and records the page when it clicks. The value is **dropped whenever
the boarding window opens or closes**, because reopening brings the bag up on whatever page it likes.

**The risk, stated rather than buried:** the page is never READ back, so this introduces a new way to be
confidently wrong — skip a needed click and the next action aims at the wrong page. Two things bound it.
Every page change in the tool goes through `SelectPage`, and the value is dropped at every window
open and close — so the assumption only ever covers **the seconds between one stack and the next**.

**Verified:** Release build clean, 0 warnings; 149/149 tests. **Not verified live.**

---

## 2026-09-21 (18) — verified live: a finished pet is skipped, and a row feeds end to end

**The first clean run.** Row 2, five stacks, all five dragged first time and boarding started:

```
16:06:02   the bag cell is 0.099 different at the drop, before the count dialog
16:06:06   the bag cell is 0.822 different from before the drag    ← the stack left
…
16:06:30   starting boarding
16:06:31   click the start button at (746,851)
16:06:33   reload complete
```

**Three things were proved on that run, and all three were guesses this morning:**

- **The guard skips a finished pet and boards the next one** — `A at page 2 cell 2 reads +9, 100% ←
  finished (+10) — NOT boarding it, trying the next queued pet`, then cell 4, then the right-click.
- **The drag check works, and its numbers are not marginal.** At the drop the cell reads **0.078–0.123**
  (the stack is lifted and the game gives it back); after the count it reads **0.81–0.82** when the
  stack left, and exactly **0.000** when it did not. The threshold at 0.10 sits inside a gap an order of
  magnitude wide, and the failing case is zero rather than near the line.
- **The drag itself is sound** — five in a row at the first attempt, no re-drag.

**And the check earned its place on its first run, by catching a real failure.** A drag was attempted on
`page 2, cell 15` — **a marked food cell with no food in it.** Three identical attempts, each reading
`0.000` afterwards. Before the check existed, that reload would have clicked MAX and pressed Enter at a
count dialog that was never raised, carried on, and reported success.

**The cause is the one already written up as Issue C in ANALYSIS-FOOD-LOAD.md:** `NextFoodCell` walks
the marked list as a **cursor** — "cell 2/23, now 3/23" — and a cursor cannot know that a cell it is
pointing at is empty. The fix is the move this tool has already made once for rows: **look before
acting** — check the cell holds something, and step to the next marked one if it does not.

**Still open:** that cell cursor is unchanged, so the next reload can walk onto an empty cell again.
Everything else on the food path is now checked.

---

## 2026-09-21 (17) — the food drag gets a look, a re-drag, and honest logging

**Every other risky action in this tool checks its own effect.** The pet placement looks at the slot and
retries three times; the boarding toggle checks it did not do the opposite; the feeder counts refuse to
judge a capture that is not the game. **The food drag asserted success** — it sent the frames, logged
`"picked up"` and `"stack 1 onto"`, and the run went on to click MAX and press Enter at a count dialog
that a missed drag never raised. So a missed drag lost food *and* fired a stray click and keypress into
the game, and neither was visible in the log.

**What the log now records, per action:**

- **where the cursor LANDED**, not just where it was sent — `landed (1313,267)` on every move. The step
  logs used to print the intent, so a move that landed elsewhere read identically to one that hit.
- the drag as **what it did** — `leftdown at (…)`, `leftup at (…)` — rather than "picked up", which was
  a claim about the game made at send time.
- the bag cell's difference **at the drop** and again **after the count**, both as numbers.

**What it does about it:** the dragged bag cell is compared with how it looked before the drag. If it
did not change, the drag is re-attempted, up to three times.

**Unknown is not failure**, and that direction is deliberate: re-dragging a cell that is already empty
picks nothing up and then fires MAX and Enter at a dialog that is not there — worse than the missed
drag it would be fixing.

**A hold between the press and the pull** (`DragGrabWait`, 0.35 s). The player watched the first drag
miss and the second work, and the log agrees they differ: the first presses and drops inside the same
second. The drag used to press and move in the same frame. **The size is a guess** — but the landed
positions and the two cell-difference numbers are now in the log, so it can be corrected from evidence.

**One measurement with no decision taken on it** (yet): the cell's difference *at the drop, before the
count*. If the cell already reads empty then, a missed drag can be caught **before** the MAX click and
the Enter — which are the stray input. One run's numbers answer which of the two it is.

**Verified:** Release build clean, 0 warnings; 149/149 tests. **NOT verified live.**

---

## 2026-09-21 (16) — the guard slept for eleven and a half minutes on its first hover

**`SleepCheck` takes SECONDS and I passed `HoverDelayMs` — milliseconds — as 700.** It does not sleep a
little long. It computes `steps = 700 / 0.05 = 14000` and sleeps `14000 × 50 ms`:

```
2026-09-21 15:32:27   found a queued pet: page 2, cell 2, match 0
                      ... 700 seconds of nothing ...
```

The live run sat wedged exactly there — **responding, 0% CPU, no error** — which is why it read as
"stopped" rather than as a crash. Nothing in the log said a word, because nothing was wrong except the
unit.

**The trap is that the same number is right one file over.** The launcher's own hover uses
`await Task.Delay(... HoverDelayMs)`, and `Task.Delay` DOES take milliseconds. Only the tool-side
`SleepCheck` takes seconds, and nothing in the name or the signature stops you passing the other one.

**Also verified live in the same run**, and this one is good news: the start line now reads

```
run started — board firmware protocol level 2 (current)
```

**The reflash took** — the board answers `V` — which closes the "flash both boards" item's first half.
And the icon scan found **8 candidates** on the player's bag, which is the every-cell fix working where
one winner used to be.

**Verified:** Release build clean, 0 warnings; 149/149 tests. **Not verified live** — the corrected
hover has not been tried.

---

## 2026-09-21 (15) — the run needed the same fix the sweep got: every cell, not the best one

**Starting the run would have boarded nothing**, and the sweep's own report said so before it happened.

The run kept one cell per icon — `scores[0]`, the best match. With 8 pets across 2 icons, the
best-matching instance of *each* icon was a **finished** one:

```
A cell 3, match 0.000, +9 100%   -> skipped
B cell 2, match 0.000, +9 100%   -> skipped
```

Both skipped, the candidate list exhausted, row unboarded — with a `+0` `A` one cell away and three
feedable `B`s on the next page. **"Try the next one" cannot work if the run does not know a next one
exists**, and (13) fixed exactly this in the sweep without fixing it in the run.

The run now takes **every cell under the limit per icon**, best first, so the guard walks a real list:

```
A cell 3  +9 100%  -> skip
A cell 4  +0 0.06% -> board
```

Cost: ~1.5 s per candidate skipped, for a few candidates. The best/runner-up logging is unchanged —
it still reports how close the call was for the icon's best cell.

**Verified:** Release build clean, 0 warnings; 149/149 tests. **NOT verified live.**

---

## 2026-09-21 (14) — "+99": the growth pattern read two digits where the game shows one

**The sweep's report, read for the first time as a file** (`logs\reads\scan_20260921_152331.txt`), found
**8 cells — exactly what the owner sees with their eye**: 3 × `B` on page 1, 3 × `A` and 2 × `B` on
page 2, nothing on page 3. The finding and the reading both work.

**And in that report was the failure the owner had been describing all along:**

```
B at cell 64 (match 0.13)  stage 6, +99, 100%   ← FEEDABLE
```

**`+99` is not a growth.** The game shows `+0 … +9` and nothing else, and the owner confirmed that pet
is **+9/100%** — finished. `IsFinished` wants *exactly* 9, so 99 fell straight through as feedable and
the run would have right-clicked a pet that cannot be boarded, into the error dialog.

**The cause was the pattern, not the value:** `GrowthPattern` was `\+(\d{1,2})` — **two** digits. One
digit is what the game ever shows. And two digits was wrong in *both* directions, the second of which
was already pinned by a test nobody had connected to a live read: `+15 Days`, the paid extension's
**duration** label, read as a growth of 15.

**One digit fixes both at once.** `+99` reads 9 → finished, which is right; `+15 Days` reads 1 → not
finished, which is the expectation `GrowthAboveTheMaximumIsNeverFinished` already held.

**Clamping was my first attempt and it was worse.** Clamping anything above the maximum down to the
maximum fixed `+99` and broke `+15 Days` — a +3 pet with a full bar would have been called finished and
stopped being fed, which is the one direction this tool refuses. The test caught it. **A wrong value and
a wrong pattern look identical in the symptom and need opposite fixes; the pattern is where the
authority is.**

**Verified:** Release build clean, 0 warnings; **149/149** tests (one added).

---

## 2026-09-21 (13) — one icon is a KIND of pet, and the sweep was taking only one cell

**The player's bag: 8 pets, 2 icons.** The report from `Scan the bag for these pets` — 3×`B` on page 1,
2×`B` and 3×`A` on page 2 — is **all eight**, so the matcher was never the problem.

**The sweep was.** It kept only `scores[0]`, the single best cell per icon, so two icons could never
report more than two pets. One icon stands for a **kind**, and a bag holds several of each: one icon
legitimately matches many cells. It now takes **every** cell under the limit, one hover per cell —
which is what `Scan the bag for these pets` has always done, and the two must not disagree.

**And a correction this file owes.** I diagnosed the same symptom as `MatchLimit` being `0.12` — just
inside the match band rather than in the gap between 0.20 and 0.83 — and proposed raising it to 0.25.
`MatchLimit` **is 0.5**. The `0.12` came from *this document*, from the entry recording the value
**before** it was changed; I read a historical note as current state and built a confident diagnosis on
it. It was caught only by opening the file before editing, and nothing was changed. **A number quoted
from a log is a quotation, and a quotation of a past state is not a measurement of the present one.**

**Verified:** Release build clean, 0 warnings; 148/148 tests. **NOT verified live.**

---

## 2026-09-21 (12) — the sweep is rebuilt on the matcher that was already there

**I invented a mechanism that already existed, and the player had to say so three times** — *"we had
this logic in the flow"*, *"we always knew where the pet icons are"* — before I went and looked.

**`IconMatch.ScoreAll(bag, grid, icon)` already answers "which bag cell holds this pet".** One capture
per page, matched against the stored queue icon, no hovering at all. The run uses it before every
boarding, and there was already a **"Scan the bag for these pets"** button sitting on the same tab
doing it on demand. `OccupiedCells` — the closest-pair empty-reference detector from (11) — was a
second answer to a question the codebase had already answered, and a worse one: it hovered cells to
learn what the matcher gets from one screenshot.

**Measured, the detector did work** — replayed over the live page captures it split 39 cells at exactly
`0.000` from 25 at `0.47`+, and every threshold from 0.05 to 0.30 agreed. **That is not the point.**
Being right does not justify a parallel mechanism: the next change to either one would have left two
things that had to agree and no reason to.

**What the sweep is now: find, then hover.** Photograph each page, `ScoreAll` each queued icon, and
hover **only the cells that matched** to read the panel. The finding is the existing matcher; the new
part is only the hover. About **130 lines came back out.**

**What is genuinely given up, stated plainly:** the sweep can no longer *discover* a pet that has never
been captured. It reports on the pets in the queue and nothing else. That is the honest scope, and it
matches what the tool can actually do — discovering an unknown pet needs its icon, which is the manual
"Mark a pet to queue" step.

**Verified:** Release build clean, 0 warnings; 148/148 tests. **NOT verified live.**

---

## 2026-09-21 (11) — the sweep stops hovering all 64 cells

**Four and a half minutes to discover that most cells are empty.** The sweep hovered every cell on
every page and let the OCR tell it what was there. The player's answer was the right one: *don't skip
the empty ones, FIND the cells that hold something and ask only those.*

**The mechanism needs no calibration and assumes nothing.** A bag is mostly empty, and an empty bag
slot renders identically in every cell — so **the closest-matching pair of cells on a page are both
empty, and that pair IS the page's empty reference**. Every other cell is then measured against it with
`IconMatch.DifferingFraction` under `PetSlotOccupiedAbove`, which is the *same primitive and the same
number* the boarding slot already uses for "is there something in this slot". Nothing new was invented;
an existing measured rule was pointed at a second question.

**It returns every occupied cell, pet or not.** Food and loot get hovered and rejected by the parser,
which costs one hover each. Telling a pet's portrait from a stack of food is a harder problem than the
one being solved, and `PetPanel.Parse` already solves it exactly.

**The margin is a number in the log, not a claim.** Each page reports *busiest empty*, *quietest
occupied*, the threshold and the empty floor — so the gap the threshold sits in is visible. This is the
`MatchLimit` lesson applied before the fact rather than after: `0.12` was inherited, cost half of every
scan, and the truth (0.0–0.15 against 0.83+) only came out because someone measured. If the gap here is
thin, the log says so on the first sweep.

**It falls back to the slow path, never to a wrong answer:** no page image means every cell is asked.

**Also fixed: a name collision I introduced.** There was already a `PetScanBag(TextBlock hint)` — the
older report of where the *queued* pets are — and the sweep was added as an overload of it. Two methods,
one name, different jobs: it compiled, which is exactly why it would have bitten later. The sweep is now
`PetScanFeedable`, and the doc comment says which question each one answers.

**Verified:** Release build clean, 0 warnings; 148/148 tests. **NOT verified live** — the detector has
never seen a bag.

---

## 2026-09-21 (10) — the sweep can now rebuild the queue

**The scan found the feedable pets, and the run still could not use the answer.** The queue held icons
of pets that had finished and been mailed, so the icon scan matched nothing and nothing boarded. The
missing link was turning "this cell holds a pet that can still be fed" into "the queue knows this pet".

**A second button, `Scan + rebuild the queue`** — the same sweep, then it replaces the queue with an
icon for every pet that read as feedable. Finished pets get no icon, so the run cannot board one by
accident; that is the guard from (9) with nothing left to guard against.

**Icons come from a CLEAN page image, taken before any hover.** This is the part that is easy to get
wrong: the tooltip follows the cursor, so an icon cropped from the screen while a panel is up has that
panel lying over its neighbours. It is the same trap the icon scan already documents — *"a pet the
cursor covers is a pet the matcher cannot see"*, which once cost a whole pet's worth of matching. So:
park the cursor, grab the page once, then hover. The crop itself is the **same recipe the single-icon
capture uses — `Pitch`, not the slot size** — so a swept icon and a hand-captured one are the same
pixels and the matcher cannot tell them apart.

**Labels say where, not what.** The panel states growth and EXP and no name, so a harvested entry is
labelled `page 2, cell 3` — which is at least what the report above it is talking about.

**The guard that matters: it never writes on nothing harvested.** A failed capture, or a page whose
crops would not come out, must not be allowed to clear a queue the player built by hand. Replacing a
queue with an empty one is the worst outcome available here and it is one edit away.

**A separate button, not a checkbox on the existing one.** A scan you press to look at must not quietly
rewrite the queue.

**Verified:** Release build clean, 0 warnings; 148/148 tests. **NOT verified live.**

---

## 2026-09-21 (9) — the run refuses to board a +9/100% pet, and tries the next one

**A finished pet cannot be placed.** The right-click simply does not put it in the slot, and a live run
at 13:58 spent three attempts on one and reported *"the pet did not go in after 3 right-clicks"* — which
reads as an intermittent click failure and is not one. Boarding one also raises the error dialog that
wedges everything after it.

**`FindQueuedPet` returned ONE winner**, so there was no "next one" to fall back to. It now returns a
**best-first candidate list** — every queued icon whose best cell scores at or under `MatchLimit` — and
`PlacePet` walks it: switch to the candidate's page, hover its cell, read the panel, and **skip it if it
reads finished**, boarding the first pet that can still be fed. Same page loop and same page captures as
before; the only new cost is about 1.5 s per candidate tried.

**Unknown boards as before, and that was the player's call, not mine.** A null panel — no tooltip
calibrated, the cursor won't move, the read throws, the panel doesn't parse — falls through to the
click. The guard may only ever REMOVE a boarding. My instinct was the opposite (refuse on doubt,
because a finished pet wedges the run), and it was overruled for a reason worth keeping: inventing a
skip is the direction that leaves a pet unfed.

**The hover is a MOVE, never a click** — a click on a pet in the bag SWITCHES THE EQUIPPED PET, so
clicking to measure would change the thing being measured. Same rule as `FocusThenHover`.

**The OcrEngine is built lazily and disposed with the placement**, following `InspectRows`: a placement
with nothing to check should not pay for an ONNX session.

**Verified:** Release build clean, 0 warnings; 148/148 tests. **NOT verified live** — and it cannot be,
without a run.

**Not covered, deliberately:** the return-slot path is not guarded. It is documented as kept empty, so
there is normally no pet there to check, and guarding it would be a behaviour change beyond the ask.

**And this alone does not get the +0 pet fed.** Today's queue holds icons of pets that finished and were
mailed, so the guard finds only finished pets and boards nothing — better than three wasted right-clicks,
still an unboarded row. For it to help, the +0 pet has to be IN the queue.

---

## 2026-09-21 (8) — the hover read finally gets a consumer: which pets can still be fed

**The bag holds a mix of +9/100% pets and +0 pets, and the player needs to find the +0 ones.** The
read that exists for it has been a capability with no consumer since 2026-09-18, waiting on exactly
this.

**The reason it matters is not wasted food.** Boarding an already-finished pet raises an **error
dialog**, and a blocking modal wedges everything after it — so a finished pet is a stuck run, not a
bad reload. That is what turned "nice to have" into the next thing built.

**Nothing new was needed.** `PetPanel.IsFinished` (`Growth == 9 && Exp >= 100`) already existed and was
already tested; so did the hover (a MOVE, never a click — a click on a bag pet switches the equipped
pet), the calibrated tooltip region, `BagGrid.Centres`, the page tabs and the OCR read. `IsFinished`
was called from nowhere in the run, and `PetPanel.Parse` from exactly one place.

**The parser is the filter, and that is forced rather than chosen.** The icon matcher only knows pets
that are already queued — and the pets this exists to find are precisely the ones nobody has queued
yet. So the scan hovers every cell and lets `Parse` decide: a +0 pet renders no `+N` at all and reads
as growth 0 (measured, 2026-09-19), while an empty cell or a stack of food simply does not parse as a
pet panel.

**A failed read is reported as neither, never as finished.** Skipping a pet that needed feeding is the
one outcome here that cannot be undone — the same reasoning that makes `IsFinished` `==` and not `>=`.
Unknown stays unknown.

**Two choices worth knowing:** the launcher hides **once per page**, not once per cell (the per-crop
hide pays a 300 ms compositor wait, which over 192 cells is a minute of flicker for nothing), and the
focus click happens **once** rather than through `FocusThenHover`, which clicks the focus point on
every call because it was written for a single read. Pages are reported as they finish so a four-minute
scan is not four minutes of a window that looks hung.

**Verified:** Release build clean, 0 warnings; 148/148 tests. **NOT verified live** — no hover has
been performed against the game. Plan: [PLAN-PET-BOARD-CHECK.md](PLAN-PET-BOARD-CHECK.md), which also
records what is deliberately *not* in v1: queueing the feedable pets, and the run-side guard (whose
blind-`Enter` dismissal is unverified for *this* dialog — Enter may confirm it rather than dismiss it).

---

## 2026-09-21 (7) — released as v2.11

**Merged to main and packaged.** `v2-pet-drag` fast-forwarded onto `main`, which is now 119 commits past
the v2.10 tag — the release notes are [RELEASE-v2.11.md](RELEASE-v2.11.md) and they are the record of
what changed, so this is only about the packaging.

**The verification the v2.9 incident demands was run first**, because an incremental `dotnet publish`
silently drops the 11 native OCR DLLs and the exe is 182 MB either way: the public zip's file list
diffed against v2.10 gives **17 files against 17, no additions and no losses**, with `onnxruntime.dll`,
`OpenCvSharpExtern.dll` and `libSkiaSharp.dll` all present and no `.pdb` or `.lib` strays. The public
zip carries the three config templates and no `local.yaml`; the local zip carries the full `config\`
with the four calibration screenshots.

**Two features ship inert**, and both say so in the release notes rather than in a commit nobody reads:
food-by-drag and the firmware `V` command need firmware 2, so until the board is flashed they fall back
to exactly today's behaviour — right-click loading, and a board that cannot be asked its version. Which
is the shape Part 2 was designed for: a board cannot report anything, including its own age, so the
release cannot be wrong about it — only silent.

---

## 2026-09-21 (6) — the count read works, and it took five designs to get one

**It reads. All slots, at 1.00.** That is the outcome; the route there is the useful part, because
four of the five designs were mine and each failed on the same three-pixel band.

**What holds: 40% across each slot to its own right edge, at the slot's full height, computed from the
slot.** Nothing drawn. Two things about it were measured and are not negotiable:

- **Full height.** A crop cut to the digits' height finds **NOTHING** while holding a perfectly legible
  number — `174`, `36`, `18`, three separate crops, all legible to the eye, all zero detected boxes.
  **The mechanism is not known.** The engine's own "text below ~20px" note does not explain it, because
  the digits are ~66px tall upscaled. It is a measured rule with no explanation, and it is written down
  as one.
- **The left edge has a band about three pixels wide** — 0.369-0.431 of the slot. Outside it, in one
  direction the detector finds nothing, and in the other it returns a **confidently wrong number**: a
  clipped `138` came back as `3` at 0.99, the same confidence as a correct read.

**The four failures, all the same mistake in different clothes.** A count box; a count slot read as
drawn; a count slot applied by offset; and both of those again after redrawing. **The player missed the
band four times, because a hand cannot put a box into three pixels** — and no instruction fixes that.
Two of the attempts were designs I argued for, and the second one I argued for *after* the first had
already failed.

**What replaced them is computed, and the player's own conclusion.** "can you just ocr the count slot
region now?" led to drawing it by hand, which failed; what actually works is not drawing it at all.
`FeederCountLeftFraction` (0.40) is a SETTING on the Calibrate Pet tab rather than a constant, because a
narrow band means another machine has to be able to move it — which was the player's objection to a
constant two designs earlier, and it survived every rewrite since.

**Two tools came out of the failures and both earn their place.** The **magenta line** on the capture
draws where the read starts, so "0.40" stops being an abstract number; and the **row geometry editor**
shows each row's strip as four typed numbers with its slots and crop positions beside them, because a
capture cannot answer *"is this row a pixel out from its neighbours?"* — and that was a real failure:
row 2's crops started 1-2px right of row 3's and its widest number ran past the crop edge, which is
invisible on a picture and obvious in that column.

**The read is also where it belongs now: Calibrate Pet, reading every ticked row in one press.** It used
to sit on the Pet tab while reading the row selected on Calibrate Pet — state you cannot see from where
the button is, which had already produced one confused report.

**Verified live, including the part that matters — a run scheduling from the counts.** The Test read
returns `300` at 1.00 on row 1's slots and `150`/`153` on rows 3 and 4; and then, started for real:

```
Row 1: 300 item(s) left in the feeder — reloading in 105 min
```

which is `300 / 3 per minute + 5 minutes after empty`, to the minute. **And the other path in the same
log**, two attempts earlier, when the read failed: *"the feeder counts couldn't be read — assuming a
full load, so reloading in 205 min"* — the configured cycle. Both are now proven on live data, and the
tool says WHICH it used, so a fallback cannot pass for a reading.

Also in that log, unplanned: *"board firmware did not answer — its firmware predates version
reporting"* — Part 2's `V` command doing its job, and answering the sticky-spacebar question with a
definite no from the other side of the port.

**And one crash, mine, from the tool built to fix the last problem:** the geometry editor indexed a
NULLABLE read region with `!`, and a half-typed width makes it null — `344` passes through `3`. It died
while the player was typing, twice. The nullable was there for a reason and I overrode it in the one
place a transient value is guaranteed to arrive.

---

## 2026-09-21 (5) — and then the player asked why it needed any of that

**"we tested if we have the correct crop that we can read it anyway? why need this much logic?"** — and
they were right, so most of (4) came back out the same night.

**We had established exactly that.** Given the correct crop the count reads at 0.99-1.00 at every
upscale, stably. So the run needs **one read of one crop**, and the four offsets and the majority vote
were compensating for a crop nobody had found yet — machinery in every read for a problem that belongs
in the calibration that finds the crop.

**Removed:** `FeederLayout.ReadOffsets`, the multi-offset read, `FeederCount.Vote` and its five tests.
The read is `ReadRegion(slot, fraction)` → `Parse`, once. 154 tests became 148 and the code lost the
whole voting apparatus.

**What is left is one number**, which is what the entire feature comes down to: how far across the slot
the count is read from. `FeederCountLeftFraction`, a **setting on the Pet tab** rather than a constant,
because the band that reads is a few pixels wide — the scan that measured it found 0.369-0.431 ("138")
and 0.323-0.477 ("300"), so 0.40 sits in the overlap, and a machine outside it has to be able to move
it without a rebuild. That was the player's objection to a constant two designs ago, and it survives the
simplification.

**So the calibration is now four strips and a number.** The count slot and the count box are both gone
— the former because row 1's first slot is derived exactly (130 ÷ 2), the latter because a box cannot be
drawn into a three-pixel band and no longer needs to be.

**Verified:** Release build clean, 148/148. **Not verified live.**

---

## 2026-09-21 (4) — the count IS readable: derived region, four offsets, a vote

**The read works now, and getting there killed two designs.** The count had to be read from the right
part of a slot, and both attempts to let the player say where failed — the second one only after it was
built.

**The measurement that settled it, on the player's own slots:**

```
                        "138"                      "300"
as drawn (21px tall)    nothing / "3" @0.99 (wrong) nothing
full slot height        nothing, then reads        reads @0.99
left edge swept:  0-12px off the crop   wrong or nothing
                 13-22px                reads at EVERY upscale 2..6
                 23px and beyond        junk
```

Two things fall out. **Height is not negotiable**: a crop 21px tall inside a 58px slot found ZERO
detected boxes while holding a perfectly legible number, and read it the moment it was taken at the
slot's full height. And **the left edge has a band about THREE PIXELS wide** — outside it the failure is
not blank, it is a confident wrong number: a clipped `138` came back as **`3` at 0.99**, the same
confidence as a correct read.

**So both attempts to hand it to the player were wrong, and the second one was mine.**

1. **A fraction of the slot** (`0.42`). The player pushed back — *"u think it would be consistent on
   every pc?"* — and was right in principle: a fraction is resolution-independent only if the game draws
   the number proportionally to the slot, and nobody had measured a second machine.
2. **A dragged count box**, on the reasoning that a measured position beats an assumed proportion. The
   reasoning was sound; the gesture was not. **Asked twice to put a box into a three-pixel band, the
   player missed it twice** — and no instruction makes that a fair thing to ask. The player's own
   conclusion, after two failures: *"do we need that count box?"*

**What replaced it: the fraction, read at several offsets, with a vote.** `ReadLeftFraction` (0.40, the
middle of the measured 38.9-43.5% band) from each slot's 40% mark to its own right edge at full height —
and `ReadOffsets` reads it at 0.24 / 0.32 / 0.40 / 0.48, `FeederCount.Vote` taking the value at least two
readings agree on. **The constant only has to be roughly right**, which is what makes a fitted number
acceptable here and was not acceptable on its own. Offsets are spread WIDE (8% of a slot, enough to move
a clip into the clear) and biased LEFT, because the two directions fail differently — too far left
swallows the food icon and the detector finds nothing, which the score gate rejects; too far right CLIPS,
which is the one failure that returns a plausible wrong number.

**A tie votes nothing**, not the lower number: two readings saying 138 and two saying 300 means the
offsets straddle a clip, and picking either would be a guess wearing a vote's clothes.

**Calibration is now FOUR drags — the strips — and nothing else.** The reference slot and count box are
gone from the config, the calibrator, both save projections and the tests. The derived regions are drawn
on the capture (magenta; the faint one is the widest offset) so a wrong strip is visible before a run.

**Also fixed: the Test read hid and showed the launcher once per CROP** — six flashes and six 300 ms
compositor waits per press. It is hidden once for the whole read now.

**Verified:** Release build clean, **154/154** tests; the vote is mutation-checked (letting a single
reading carry, or breaking a tie by value, fails two). **Not verified:** none of this has read a live
slot yet — the four-offset vote and the derived region have only been exercised against the two saved
crops from the last run.

---

## 2026-09-21 (3) — six drags instead of twenty, and the fraction is the player's idea to kill

**The player's correction, and it is a better design than mine.** I had the count read as a *fraction*
of the slot box — `0.42` across to the right edge — and put it in config so another PC could move it.
Their objection: *"u think it would be consistent on every pc? why dont let me drag on one reference
slot and u calculate how much u need to cut."*

**They are right, and the honest answer to the first half is that I do not know.** A fraction is
resolution-independent only if the game draws the number at a size *proportional* to the slot. If it
draws the digits at a fixed size while the slot scales, `0.42` is right on this machine and wrong on a
smaller or larger one. I have measured exactly one machine, and carrying an unmeasured assumption to
another PC is the specific error this file records over and over. A dragged reference is a
**measurement**; a fraction is a guess wearing a decimal point.

**Six drags, replacing fourteen hand-drawn boxes and both fractions:**

| | |
|---|---|
| 4 | a strip across each row's food slots — divided by that row's stack count (2/5/5/5) |
| 1 | ONE reference slot — the anchor |
| 1 | the count region on that reference slot — its offset and size |

Every other count region is `slotBox + (refText − refSlot)`, so the position is measured rather than
proportioned, and a resolution where the digits render at a different relative size is expressed
exactly rather than approximated.

**Built, wired, and green: `Core.FeederLayout` plus every consumer.** Two decisions in the derivation
worth naming: the slot segments are computed on the SLOT EDGES with integer arithmetic rather than by
dividing the width and re-multiplying, because rounding the width first leaves a sliver unclaimed at the
right end — which is precisely where the count is read from; and a missing reference returns null rather
than a region at (0,0), so an un-migrated config reads as "nothing to read" instead of reading the
top-left corner of the screen.

**What the wiring replaced.** `PetSlotConfig.FeederSlots` (the fourteen boxes) became `FeederStrip`
(one per row), and `FeederCountCropLeft`/`CropLeftShifted` became `FeederCountSlot`/`FeederCountText`
plus a pixel shift. The food drag, the count read and the Test read all derive from those now, so none
of them can drift from the calibration. `Ready()` gained its own check: a DRAG has to name the box, so
in drag mode a row with no strip is refused before the run starts rather than failing three times and
being dropped — right-click is untouched, because letting the game choose the box is the whole
difference between the modes.

**A legacy config keeps working, approximately.** A file written before the rows existed migrates its
two old boxes into one strip by taking their SPAN. That is deliberately an approximation — those boxes
framed the numbers, not the slots, so the span runs between the digits and comes out a little narrow —
and it is carried only because without it such a file could not load food at all. The migration test
asserts the span and says why.

**The one number that eats into this, recorded before it bites:** dividing a strip into five is out by a
pixel or two, because the player's own row 2 slots measured 62, 64, 66 and 62 apart rather than evenly.
Harmless for the food DRAG — a 64px slot does not care about 2px — and survivable for the count read,
whose individual crops tolerate 7–10px. But the crops' COMMON window measured only ~3px, so this is the
thinnest part of the chain and the first thing to suspect if reads start disagreeing.

---

## 2026-09-21 (2) — the feeder count IS readable, but only from the right crop

**It was never the model or the font.** The whole-slot image the tool has been feeding the OCR returns
**zero detected boxes** — not a bad read, no read — and the same engine reads the same digits
**perfectly** from the slot's right half at full height: `84` at 0.99, `300` at 1.00. Every earlier
conclusion I drew about "PP-OCRv4 cannot read this font" was wrong, and wrong in a specific way worth
naming: **I tested the image the tool was asking for, not the image the reader needed.**

**Two things fall out of that immediately, and both are counter-intuitive:**

- **Tighter is worse, not better.** Cutting the height off the crop finds nothing at all, exactly like
  including the icon does. So the rule is not "less clutter"; it is a particular shape.
- **The failure outside the window is a confident WRONG number.** At a slightly tighter crop `84` comes
  back as **`4` at 0.89**, and `300` as **`0` at 0.56** — a full stack read as empty. That is far more
  dangerous than the blank read we had, and it is why the count is read TWICE from two crops a few
  pixels apart and only accepted when both agree.

**The window is narrow and that is the honest headline.** Digits are right-aligned and grow leftward,
so the crop a two-digit count needs is not the one a three-digit count needs:

```
"84"  correct for a left edge of 0.404 - 0.505 of the slot's width
"300" correct for                      0.29  - 0.45
```

The overlap is about **three native pixels**. `FeederCount.CropLeft = 0.42` sits in it, with the second
read at 0.45 — a three-percent shift, small because a wider pair does not fit inside the overlap. That
makes the agreement check **weaker than it should be**, and it is exactly why this is wired into the
Pet tab's Test read and not into scheduling: how often it holds on real slots is a measurement, not an
assumption.

**The confidence gate is measured, not chosen.** A genuine count scores 0.90–1.00; everything the food
icon produces scores at most 0.63 in the same crop. 0.75 sits in that gap — and it also rejects a
genuine read taken at the very edge of the window, which is the right call, since a marginal crop is the
one that clips.

**Also fixed while here:** `OcrEngine.ReadLines` returned only text, so a caller that has to CHOOSE
between lines could not. `ReadLinesScored` now keeps each line's confidence, because the junk beside a
real count can outscore one taken at a bad crop — so "drop the weak lines" was not enough.

**Verified:** Release build clean, 144/144 tests (17 new). The agreement rule is mutation-checked:
replacing it with "take whichever read succeeded" fails two tests, which is the clipped-digit case.
**Not verified against a live slot, and deliberately not yet wired into scheduling** — the run path
falls back to a full-load assumption whenever the two crops disagree.

---

## 2026-09-21 — "run only row 1" had no expression, so it has one now

**The tool drove every row it was configured with, and there was no way to say otherwise.** `Ready()`
demanded every row be fully marked, and the UI could ADD rows but never remove one — so the only way to
run a single row was to delete the others from `local.yaml`, which takes their calibration with it. The
player asked for exactly this, to keep feeding while the four-row placement bug is unexplained.

**`PetSlotConfig.Enabled`, defaulting ON**, so every existing file describes exactly what it used to do.
An unticked row is **invisible rather than skipped**: not validated, not read, not scheduled, not
clicked. `Ready()` only demands the geometry of the rows it will actually drive, which also means a
half-calibrated row no longer blocks a run that never touches it. Every consumer goes through
`PetConfig.ActiveRows` — iterating the raw `Slots` list is exactly how a disabled row gets clicked
anyway, and the doc comment on the helper says so.

**The tick is on the Pet tab, beside the one-per-row boarding ticks — and the player is why.** It was
built on Calibrate Pet first, next to the row strip, on the reasoning that a row is selected and edited
there. That was the wrong call: *which rows to drive* is a RUN decision (a row being fed by hand this
week, a row still being set up), not a fact about the machine, and this project's own split is
"Calibrate writes the machine's half, the Pet tab writes the run's". The Pet tab already had the
precedent — a per-row boarding tick that saves on toggle.

**Where the UI lives and which projection writes it are independent, and that is not obvious.** `Enabled`
has to appear in **both** projections, for two different reasons:

- **`ApplySession`** — because the tick is on the Pet tab, so the session save is what a toggle calls.
  This is the only thing that persists it.
- **`ApplyCalibration`** — because that one **rebuilds** the row list from the fields it names, so a
  field it does not carry is dropped by the next Calibrate save. A setting going missing without
  anything failing, which is this project's most expensive recurring bug.

**Worth knowing for the player's actual problem:** running row 1 alone is not merely "for now" — it is
the configuration the current code is *correct* for. The right-click food bug only bites a row with an
empty box somewhere ABOVE it, and row 1 has nothing above it. That is why row 1 has never failed once in
the whole log. One row makes the ordering bug unreachable, with no reflash and no drag.

**Verified:** Release build clean, 127/127 tests. The nested projection guard caught the new field
immediately — `EveryNestedPetFieldIsCopied` failed until `Enabled` was added to `LocalPetSlot.ToConfig`
AND to `ApplyCalibration`, which is precisely the class of omission that has silently dropped a field
twice in this project. The fixture uses `false` rather than the default, so the guard cannot pass
vacuously. **Not verified:** the tick has not been clicked — `MainWindow` is unreachable from the test
project, so it is a compile check.

### And moving it to the Pet tab exposed a hole in the guards worth more than the feature

Deleting `t.Slots[i].Enabled = p.Slots[i].Enabled` from **`ApplySession`** left the whole suite green.
The two existing guards both stop short of it: `EveryNestedPetFieldIsCopied` walks the LOAD
(`LocalPetSlot.ToConfig`), and the split test only ever asserts the **calibration** half's rows. So the
per-row fields the **session** half owns were covered by nothing at all.

That is the `ActionWaitMs` shape exactly — a field some save never wrote, so it moves in the UI, appears
to work, and is gone by the next launcher start. It was worth a test rather than a note, because the
tick lives on the Pet tab and `ApplySession` is therefore the *only* thing that persists it.
`TheSessionHalfCarriesThePerRowRunFlags` now does, and is mutation-checked: removing the line fails it.
Both flags start `null` (they are `bool?`), so writing `false`/`true` is distinguishable from the field
never being touched — a fixture using the defaults would have passed either way.

### And a question answered while in there: the return slot is dead once a queue exists

Asked by the player. `PlacePet` branches on `Queue.Count > 0` first, and the comment is explicit:
*"THE QUEUE IS THE ANSWER when it exists, and there is deliberately NO fallback to the return slot if it
finds nothing."* So the return slot is read **only when there is no queue at all** — with pet icons
captured it is never consulted, and `Ready()` accepts either one. It is a fallback for the no-queue case,
not a second opinion.

**One reason to clear it anyway:** `Ready()` still validates the return slot's PAGE when it is marked,
even for a run that will never read it. So a stale mark pointing at a bag page whose tab is no longer
calibrated will block the start of a run that does not use it. A mark you do not need can only cause
trouble.

---

## 2026-09-20 (6) — the food boxes are one queue, and a right-click cannot name one

**The player found it in the game, and it is a design error rather than a quirk.** A right-click drops a
food stack into the **earliest empty box in the boarder**, and every row's boxes are one queue ordered
top-down — so a stack meant for a lower row lands in an upper row's box whenever that row has run dry.
The tool has never looked at where the food went; it counts the clicks it sent and calls the reload
complete, so this has been silently possible for as long as four rows have existed.

**How it survived four-row testing, from the log rather than from reasoning.** When rows come due
together the loop reloads them 1→2→3→4 — the 21:56 and 22:00 reloads on 2026-09-19, and 06:23–06:26 on
2026-09-20 — so each row's food lands correctly because everything above it was *just* refilled. The
failure needs a LOWER row reloaded while an UPPER row has an empty box: a window only
`wait_after_empty_minutes` wide (5), because the reload is scheduled that far past the feeder emptying.
**Row 1 is never exposed, and the player's own words were "the first row was fine because it is the
first row".** That is the whole explanation, and it came out of reading `pet.log` for the redundant
case rather than from any theory of mine.

**And the code's own comment was wrong about the thing the fix needed.** `FeederSlots` is documented as
framing the food COUNT, "normally two, and NOT Stacks wide, which is two whether the row holds two
stacks or five". The live config holds 2/5/5/5 of them at ~63×56 px, where a count box would be a
fraction of that. I asked rather than assumed — the answer is that the player drags each box around the
**food item's icon** in the slot — and a drag was briefly designed around a target that does not exist.
Comment corrected, and the measurement recorded in it.

**The fix is a setting, and it has to be.** `pet.food_load_mode`: **right_click** (the game picks the box
— works on every board ever flashed) or **drag** (the row's own box is named). Drag needs firmware 2 and
the `L`/`l` commands, which is why it cannot be the default. And because an old board **ignores** those
letters — the button is never pressed, the stack is never picked up, and it looks exactly like a
mis-aimed drag — the tool **refuses to start** a drag-mode feeder on a board that cannot drag instead of
running one that silently loads nothing. That refusal is the first place `FirmwareVersion` changes
behaviour rather than being printed, which is what Part 2's number was for.

**Two hazards that come with it, both handled:**

- **A held mouse button is the stuck-spacebar bug wearing a different hat**, and worse — a left button
  left down follows the player's *real* cursor and drops whatever it is over on the next press. Three
  layers: the drag releases in a `finally`, so a failed move still lets go; `StopTool` now releases the
  mouse as well as the spacebar, so every stop path does; and the firmware's host-gone failsafe releases
  it too. `HeldKeys` grew from "needs a `U`" to "can leave something held", and `ReleaseSpace` became
  `ReleaseHeld`, sending **both** commands always — choosing between them would mean the release path
  had to be right about which tool was stopping, and being wrong about that is how a release path fails.
- **A bad release can lose an item**, which nothing else in this tool can do. Not measured yet, and the
  plan says so: a deliberate mis-drag before this runs unattended.

**Verified:** sketch compiles for the Pro Micro via arduino-cli (11948 bytes, 41 %), Release build
clean, **126/126 tests** (10 new on the mode and the board check). The projection guard is
mutation-checked — deleting `t.FoodLoadMode = p.FoodLoadMode` fails two tests, which matters because
that projection has silently dropped a field twice. **Not verified: nothing has been flashed and the
launcher has not been run.** The drag path has never moved a real cursor.

**On branch `v2-pet-drag`, not merged** — it cannot do anything until a board is reflashed.

---

## 2026-09-20 (5) — the pet feeder becomes resident, on a branch

**The feature the player asked for**: *"while pet tool running we could very well need to buy sell and
spam, and some gem compose."* Built on `v2-resident-pet`, **not merged** — it rewrites the exact code
path the live pet feeder runs from, and it cannot be live-verified until that run ends. Same shape as
the tuner-spring work.

**It opens with a refactor that changes no behaviour** (`2b426c6`): `_cts` / `_toolTask` / `_state` /
`_currentId` were four parallel fields that encoded "there is only ever one tool" in the shape of the
class rather than in a rule anyone could read. They travel together as a `RunningTool` keyed by id now,
so a stop is "cancel this one" and "is THAT tool still running?" is answerable. Also decided there:
`_startInProgress` **stays** global, not per-slot as the plan wanted — one port, one boot delay, and
two concurrent starts must still not both open it.

**The whole rule is two lines.** Starting anything else displaces every other tool *except* the pet;
starting the pet displaces only a *previous pet*. Both are `StopAll(keep:)` predicates rather than a
"keeper id", and that is not style — a single keeper name **cannot express both**, and writing it as one
reintroduced a real bug: pressing Start on the Pet card twice left the first pet loop alive and
orphaned, still clicking, with no way to stop it. The predicate was the fix, not a tidy-up.

**The gate is what makes the pet's decision safe to make off the dispatcher.** `_running` is a plain
`Dictionary` — safe only because only the UI thread touches it. The pet runs on its own thread and has
to ask "is the game free?", so it asks `Core.PortGate` instead, where the check and the claim are one
operation under a lock. That is also why `IsRunning(id)` was written and then **deleted**: it would have
been a second, unsafely-threaded way to ask the same question.

**Two decisions with a cost, both recorded where they bite rather than only here:**

- **The gate is handed back in the stop *continuation*, not at `Cancel()`.** Releasing the moment a stop
  is requested would let the pet start clicking while the dying tool is still sending its last command.
  The cost is that a loop which never exits keeps the game — and the pet says `waiting for <tool>` on its
  card for as long as that lasts, which is the honest reading rather than a silent overlap.
- **`_currentId` is deliberately never the resident.** Mini mode keys off it, and collapsing the window
  onto a schedule that runs for days would hide everything else for the length of it. **A pet feeder
  running alone therefore no longer shrinks the window** — the one behaviour change the plan's table did
  not predict, and the right one, because the old collapse was justified by "only one tool can run, so
  nothing is hidden that could be used anyway". That justification is exactly what residency deletes.

**And the plan was wrong about `_startInProgress`.** It said the flag should become per-slot. It should
not: one port, one boot delay, and two concurrent starts must still not both open it, so a single flag is
the correct shape rather than a limitation. The per-slot version would have allowed the double-open the
flag exists to prevent. Its wart is unchanged — a Start during another start is still a silent no-op.

**Also worth naming:** the deferral sits exactly where the plan said, between `SleepUntil(next[row])` and
the reload, and it needs no bookkeeping at all — `next[row]` is left in the past so the next pass picks
the same row again. What it does need is a *voice*: the card says which tool it is waiting for and how
long the row's food lasts, and past that point says `OUT of food`, because a starving pet and a feeding
one look identical from outside. The moment is derived from `next[row] − WaitAfterEmptyMinutes` with no
new state.

**Two bugs found by reading my own diff rather than by a test**, both in code no test can reach: the
early return when a run is stopped while waiting to look never cleared `state.Running`, so the card would
have claimed `● RUNNING` for a tool that was already gone; and the initial look's release was not in a
`finally`, so a throw out of it leaked the gate and nothing could ever take the game again.

**Verified:** Release build clean, 109/109 tests. **Not verified: the launcher has not been run at all.**
Every behavioural claim here is reasoning from the code. The live steps — including three cases no plan
row covers (starting the pet while a tool runs, starting the pet twice, stopping it mid-reload) — are at
the end of [PLAN-RESIDENT-PET.md](PLAN-RESIDENT-PET.md).

---

## 2026-09-20 (4) — Part 3's gate, and stopping short of the wiring on purpose

**Only the first slice of [Part 3](PLAN-RESIDENT-PET.md) is built: `Core.PortGate`.** Nothing uses it,
so nothing about the tool's behaviour has changed — deliberately, and this is the entry that explains
why the rest was not written.

**The gate is one Arduino and one cursor, so who holds it has to be answerable.** Two decisions are the
whole class: a claim is **refused rather than queued or reference-counted** (including from the owner
asking twice — a tool that claimed twice and released once would leave the gate free while it still
ran), and **a release from anyone but the owner is ignored** (a stale tool finishing late must not free
the gate its competitor is waiting on, which is precisely how two tools end up writing at once). It is
not a lock around each serial write, and the waiting is the caller's job — two tools *interleaving* on
the cursor is still wrong, so the gate makes them take turns.

**9 tests, mutation-checked in one direction:** dropping the ownership check from `Release` fails two of
them. The honest limit is recorded in the plan too — the racing test (30 callers, exactly one winner) is
*evidence* for why the check and the claim share one lock, not proof; a check-then-act mutation is not
reliably caught by 30 threads, so it is not the mutation that was used to check it.

**Why it stopped there.** The rest of Part 3 rewrites `LauncherService`'s single-tool core — the four
slots become per-tool, `StopTool` becomes slot-aware, `_startInProgress` becomes per-slot, `RefreshStatus`
and mini mode change, `PetTool` defers, `ToolBase` learns to ignore the Quit hotkey, and nine comment
sites asserting the one-tool invariant get corrected. That is a large change to the exact code path a
**live pet feeder is running from right now**, and none of it can be live-verified without the game and
a launcher restart. The docs already carry the reason that matters: a reload is the one permanent
failure, and an unverified refactor under a run that is feeding four pets is the wrong trade to make
unasked.

**Verified:** 109/109 tests, Release build clean. **Not verified:** anything about the wiring, because
none of it is written.

---

## 2026-09-20 (3) — Part 2: the board can finally be asked a question

**Goal.** The second part of [PLAN-RESIDENT-PET.md](PLAN-RESIDENT-PET.md) — a `V` command, so that a
board can report what it is running. Every command until now went one way, which is why "did the board
ignore that, or did it act and nothing happened?" had no answer: both look like nothing.

**The valuable half is the silence.** The sketch that predates `V` writes nothing at all, so a board
that stays quiet is a definite **no**, not a failed read. That is only true because the sketch ignores
unrecognised letters — which is also what makes the launcher's half **safe to ship before the flash**:
sending `V` to an old board costs nothing and produces exactly the silence the code is written to read.

**Two corrections to the plan, both in the file now.**

- **The level starts at 1, not 3.** The plan's `3` was counting back over behaviour changes that predate
  reporting — but no board can report anything today, so levels 2 and 3 cannot exist and cannot be told
  apart. A future reader would hunt for two sketches that were never flashed. The plan's *reasoning*
  about the number was right and is what stands: a protocol level, bumped when behaviour changes, so the
  number answers "does this board have the feature I need?" rather than "how old is it?".
- **"The board resets when the port opens" is not true here.** `Arduino.Open`'s own comment already
  records that the 32U4 does not reset on DTR, and `setup()` calls `Serial.begin()` **before** its
  `delay(3000)` — so a `V` written during that delay sits in the USB CDC buffer and is read as soon as
  `loop()` starts. It is not lost. The retry stayed (cheap, and it is the DTR-resetting case that would
  lose it), but the thing that actually decides this is the **read window**, sized to reach past the 3 s
  mark. The plan named the right failure and reached for the wrong fix.

**The decision went to `Core` again**, for the same reason as Part 1: "is this line a version reply, or
something else that happened to arrive?" is the part worth pinning, and `LauncherService` is not
reachable from the test project. 10 tests, mutation-checked — loosening the two-token rule lets
`"V 1 extra"` parse and the suite fails.

**And the sketch was compiled, not eyeballed.** The Arduino IDE ships `arduino-cli` at
`resources/app/lib/backend/resources/arduino-cli.exe` (the user libraries are under
`Arduino15/libraries`, so it needs `--libraries` to find `Mouse.h`). `arduino-cli compile --fqbn
arduino:avr:micro` → 11878 bytes, 41 % of flash. Note that this is a **compile**, not an upload: the
running launcher holds the port, so nothing has touched a board.

**Verified:** Release build clean, 100/100 tests, sketch compiles. **Not verified:** that a board
answers at all. It cannot be — a board without `V` has nothing to say, so the change is only
observable after a flash, and a flash needs the port. The steps are in the plan, including how to force
the silence case without an old board.

---

## 2026-09-20 (2) — Part 1 of the resident-pet plan: the Hold Space toggle stops inverting

**Goal.** The first of the three parts of [PLAN-RESIDENT-PET.md](PLAN-RESIDENT-PET.md) — the live bug
on the other PC, where the Hold Space toggle will not stop and the spacebar stays held.

**The bug is an inversion, and the guard was asking one question too many.** `CurrentId == "holdspace"`
*and* `Running == true` — and when a release write has failed, `Running` is already false while the key
is still down (`HoldSpace` sets `Running` **before** it writes the release), so the press fell to the
`else`, which starts hold space and **re-holds the key the press was meant to let go of**.

**The fix that matters is the service, not the button.** `StopTool` now releases the spacebar whenever
the tool it is stopping holds a key, so the card's Stop, the toggle, another tool's Start, the Quit path
and `Dispose` all release — instead of the toggle being the only path that could, and the tool's own
`finally` being the only release when the loop is not what is being interrupted. The firmware's `U` is
idempotent, so the two releases arriving together are harmless.

**The decision went to `Core`, deliberately.** `LauncherService` is not reachable from the test project,
so *"which ids need a `U` when stopped"* is `Core.HeldKeys.NeedsSpaceRelease`. One of its three tests
lists every id `StartToolAsync` accepts and asserts none holds a key — a new tool cannot start doing so
without that test failing.

**And the plan was wrong about where a failed release gets reported.** It said the tool's `Release()`
"reports on the card" and the service's copy was the silent one. The reverse is true: **Hold Space has
no card** — it is the top-right button, and `Tools` does not contain it — so the `state.Message` that
`HoldSpace.cs:32` sets is written and never rendered by anything. The service's swallow was therefore
the *only* release path with any chance of being seen, and it was the one discarding the error. The
message now lands on `LauncherService.LastSpaceReleaseError`, drawn on the status line beside the toggle
as `● space may be stuck`, and cleared when a fresh hold is started (the state that warning describes is
over). This is the fourth time this session-family that a message was written to a surface that does not
exist.

**One addition the bug implied rather than the plan naming it.** The toggle's *label* keyed on the same
two-part `holding` test as its guard, so in exactly the broken state it read **"Hold Space"** while
pressing it now stops. Label and guard key on `CurrentId` together; the dot stays on `Running`, because
"holding" is a claim about the key being down and the tool only knows that while its loop says so.

**Verified:** Release build clean, 84/84 tests pass (3 new). **Not verified:** this part has never run.
`MainWindow` is unreachable from the test project, so the guard and the status line are a compile check
only — the same standing limitation as the Buy tab's picker. The live steps are in the plan.

**The run was not disturbed.** The pet feeder was live throughout; the compile check was `-c Release`
and the Debug output it is running from was never touched.

---

## 2026-09-20 — the feeder ran all four rows, and three of my own conclusions were wrong

**A long evening of live testing, and the theme is measurements overturning things I had already written
down.** The run itself ended the session working: **all four rows reloaded at 21:56** and **row 1 again on
schedule at 01:22:44**, unattended, with the icon scan finding `match 0` and a runner-up at `0.019`.

### `MatchLimit` was a guess that had been costing half of every scan

The player's scan found **6 of their 10 pets**. The crops were right; measured against their own pages,
the pets scored **0.000–0.150** and everything else in the bag **0.831 and up**. The limit was **0.12** —
the composer's empty-box number, carried over with the reasoning that strict is safe because a near-miss
right-clicks the wrong item. Sound reasoning, an unmeasured number, and it was throwing away four pets a
scan. It is **0.5** now: not tuned until the answer came out right, but placed in the middle of a gap
from 0.32 to 0.83.

**And the fix that looked obvious was wrong.** "The neighbours leak into the cell" suggested comparing
only the middle — at coverage 0.7 *every* pet scored 0.84+, strictly worse, because an offset is a
translation of the whole image and cropping harder does not chase one. Sweeping the search radius 3
through 10 then showed it was **not an offset problem at all** — identical scores — so the threshold was
the whole of it.

### A capture is a picture of whatever is in front

Rows failed with *"No queued pet is in the bag — the closest match was 0.915"*. The player found the
cause: **their game window was not focused**, so every page was captured as whatever covered it. The
verdict was right and the image was wrong, and those are indistinguishable in a log. Three guards now:
a foreground check on the feeder read and the bag scan, and **parking the cursor off the bag** before
each capture — a pet under the pointer reads as a different pet, which is the "park the cursor" item
TODO has carried for weeks, arriving with a measurement attached.

### Three of my own conclusions, corrected

- **"Row 1's reference is genuinely a different pet"** — no. It was **the mouse arrow** over part of the
  portrait; I had read a capture artefact as a fact about the game, twice.
- **"All four rows have distinct empty-slot references"** — no. My extraction used `\s*` before the
  value, which matches a newline, so for three empty ones it read the *next line's* first token and
  hashed `boarding_running:` four times. I compared four copies of the same string and called them
  distinct.
- **The shared empty-slot reference** — the real bug underneath. One crop for all four rows read **three
  empty rows as occupied**, so the tool left four starving pets in the bag and did nothing, then started
  an **empty boarding** on one of them — which the game answered with an error box blocking every click.
  Each row has its own reference now, and the reload **checks that the end press worked** rather than
  assuming it.

### And the projection bug, for the second time

`LocalPetSlot.ToConfig()` — the **load** path — never copied `PetSlotEmptyPng`. So every launcher start
discarded each row's reference and the next save wrote the blank back. Row 1 survived only because the
migration re-supplies it, which made the loss look like "rows 2–4 have no reference" rather than like a
projection bug. The reflection guard covered `LocalPet` but **not the nested types**; my first attempt at
fixing the guard compared through `From`, which never calls `ToConfig`, so deleting the field left it
green. It compares the stored object against the config now, and is mutation-checked.

### And the co-run question, answered by the game rather than by us

The player asked whether the pet feeder could stay resident while other tools come and go. The
investigation is in [PLAN-RESIDENT-PET.md](PLAN-RESIDENT-PET.md), and the answer is **not about our
code**: the boarding window and the **tuner's** window cannot both be open, and the tuner's must be
closed first — so there is never a moment where two tools drive the game, and the cursor-lease design
that asymmetry seemed to want has no subject. The tuner stays a manual schedule; everything else is
plannable, because the pet feeder holds nothing between reloads. **Nothing is built.**

**Also found while exploring it: the Hold Space toggle inverts.** Its guard asks `CurrentId` *and*
`Running == true`; when `Running` has gone false with space still held, the press falls to the `else` and
**re-holds** the key. v2.9.1's tag carries identical code, so the older launcher on the other PC is a red
herring.

**Left open:** the three parts of `PLAN-RESIDENT-PET.md`; the feeder-count read built but never verified
against a live feeder; one pet still unexplained at 0.320 against the crops; and ~57 food cells a day.

---

## 2026-09-19 (5) — the four-row build, and a matcher that was only finding half the pets

**The longest session yet, and the one that built the multi-pet feature end to end.** Four rows
configured and driven one at a time, a queue of pet icons, a scan that finds the next pet by icon, and
a start that no longer disturbs a feed already running. Then the player ran the scan and it found **3
of their 7 pets** — which turned into the most valuable thing here.

### The matcher was comparing whole cells, pixel-for-pixel

Measured on the player's own bag capture, every crop against all 64 cells:

```
crop1 (blue):  6:0.000  7:0.000  4:0.434   then 53:0.832
crop2 (gold):  3:0.000  2:0.556  1:0.562  0:0.784   then 60:0.838
```

Two pets scored **exactly zero** — pixel-identical to the crop — while their own kind scored 0.43 to
0.78. The crops were right. **The pets' sprites are drawn at different sub-cell offsets**, so a
pixel-exact comparison only matched the cells where the sprite happened to land in the same place.

**The obvious fix was wrong, and the measurement is what said so.** Comparing only the middle of the
cell — the answer to "the neighbours leak in", which is what it looked like — made it strictly worse:
at coverage 0.7 *every* pet scored 0.84 or more. An offset is a translation of the whole image, and
cropping harder does not chase a translation.

Scoring each cell at a spread of offsets and keeping the best took it from **3 pets to 6**, with the
separation now 0.000 against 0.830 — so `MatchLimit` stays at 0.12 and nothing has to be loosened. A
sweep of radii 0/2/3/4/5/6 on the real bag: 0 found three, everything from 2 up found six with
identical scores. Three, at a squared cost per cell.

**And the seventh was the CURSOR** (player, 2026-09-19). I had written it up as a genuinely different
pet — its sprite "drawn smaller, with the cell's frame visible around it" — and that was a bad read of
my own evidence: what I took for a smaller sprite was part of the portrait covered by the mouse arrow.
The capture reads the screen, so the pointer lands in the image, and a pet under it differs at every
offset. It was never a different pet.

Both bag captures now move the cursor to the buy/sell inert point first — the scan and the reload's own
pet search. This is the "park the cursor" idea that had been sitting in TODO.md, arriving with a
measurement attached rather than as a precaution.

**The lesson, and it is the second time today:** I read a difference in a picture as a fact about the
game when it was a fact about my own capture. `TODO.md` has carried "park the cursor off the OCR bands
— needs a measurement first" for weeks; the measurement turned out to be a pet I had already declared
different.

### The rest, in order

- **The rows became a list** (`pet_slots`), alongside `return_slot`, `food_slots` and a `queue`,
  **with a migration** — `IgnoreUnmatchedProperties` means an old local.yaml does not fail, it
  silently loses every key nothing maps to any more, which is a player's whole calibration.
- **One schedule per row**, because the free row holds two food stacks and a paid row five: 200
  minutes against 500. A failing row is dropped and the rest carry on, rather than one bad
  calibration starving three.
- **Saves are scoped to their tab.** Calibrate Pet writes the machine's half, the Pet tab writes the
  run's — through writers that MUTATE the loaded block, because a scoped save that assigned a fresh
  object would blank the half it is not about, which is how the food cells were lost twice.
- **A start looks before it reloads.** Every row used to be due immediately, on the reasoning that the
  tool cannot read the state so it must establish it. That expired: the boarding slot is readable, so
  a run opens the breeder once, leaves the feeding rows alone, and schedules from what it saw.
- **Marked-and-saved marks now show.** The overlay drew only the row being edited, so switching rows
  looked like the previous one had been erased.

### The pattern worth naming, because it recurred seven times

**A number that belongs to a row, written down once as if it belonged to the tool.** The load size,
the food-count drag chain, the overlay's box drawing, the feeder test read, both readiness checks and
the checklist. Every one came from extending a single-pet tool rather than rewriting it, and the
`Row` property — a `Slots[0]` accessor — was deleted because it made writing the next one a keystroke.

### Three alignment bugs, all the same trap

A control inheriting a default meant for a different context: two number fields of different widths
centring against each other because a fixed `Width` plus the default `Stretch` centres in its column;
and `MakeButton`'s 10px top margin sitting two buttons in a row at different heights, where
`MakeInlineButton` already existed for exactly that and the lesson was already in this file.

### Left open

- **The seventh pet's capture**, and with it the question of whether a sprite that renders smaller is
  a different pet or the same one at a different level. If icons change with level, a queue breaks as
  pets grow — worth one deliberate check.
- **The feeder counts are still unread**, so a row left alone is scheduled a full cycle out and that
  assumes a full feeder. The counts would say, and reading them is the next thing worth building.
- **The food budget**: ~57 marked cells a day across four rows, against a 64-cell bag holding the
  pets. A restock and re-mark is a daily chore, not a one-off.

---

## 2026-09-19 (4) — the paid rows are open, and §11's two unknowns are answered

**The player bought the expansion.** The capture shows `該欄位約29日23時58分43秒後到期`, so a 30-day
purchase, and the `PET BREED` window now carries four rows: one free, three paid.

**§11 listed exactly two unknowns as blocking the multi-pet work, and both are now answered — per row:**

| §11's question | Answer |
|---|---|
| Does each row have its OWN start button? | **Yes.** Row 1 reads `結束代養` (running) while rows 2–4 each read `開始代養`. |
| Are the food slots per row or shared? | **Per row.** Rows 2–4 each render their own boxes under their own expiry line. |

**And the game states two things the tool currently configures or computes**, both on the active row:

```
每1分 攝取3個。                    the burn rate — 3/min, per row
到9為止預計所需時間: 約 89分        the ETA to +9, per row
```

The first retires §11's worry about one configured rate being wrong for a stage-7 row. The second is the
answer to the question the player asked earlier — *"having to view the pet % and stage and +?, can we
calculate its actual remaining time?"* — and it needs **none** of what was designed for it: not `wyz`,
not the 327-pet table, not the name lookup, not two reads a cycle apart. The game already displays it.
That whole line of work was solved by looking at the window.

**And one thing I got wrong, which is now the third of its kind.** The capture showed the boarded free
row rendering two food boxes (`198`, `300`) and the three idle PAID rows each rendering five. I read
five as what a row holds and made it the default. The player: *"1 free row is 2 slots for food / 3 paid
row are 5 slots for food"*. The screenshot was right; generalising from one instance was not. The value
is now a per-row field — `StacksPerReload`, default 2 because 2 is the row the calibration points at.

**What that difference is worth, once the paid rows are driven:** ~1.7 reloads per stage-6 pet instead
of ~4.2. Every reload is a chance to leave the pet unboarded — §7's one permanent failure — so a paid
row is worth boarding on with one pet, never mind four.

**And a second capture, with all four rows boarding, settled what the EXP% means — and this document's
own §2 was wrong about it.** §2 said the percentage was of the WHOLE STAGE; it is of the CURRENT LEVEL.
The window proves it by printing its own ETA and naming the next level:

```
row 1 (free, +8)  25.99%   到9為止預計所需時間: 約 77分
row 2 (paid, +0)  17.37%   到1為止預計所需時間: 約 48分
row 3 (paid, +0)  15.63%   到1為止預計所需時間: 約 49分
row 4 (paid, +0)  10.44%   到1為止預計所需時間: 約 52分
```

`到9` on the pet at `+8` and `到1` on the three at `+0` is the giveaway — a stage-wide reading would say
`到9` on all four. And `(1 − e) × wyz × (1 + g/10) ÷ 90` gives **77.0, 47.7, 48.7, 51.7** minutes against
the displayed **77, 48, 49, 52**. Four confirmations to the minute, none rounded into agreement.

**§2's argument for the stage reading dissolves rather than being overruled**: it said a per-level
reading could not explain why the breeder stops at `+9`, but at `+9` there is no next level — that is
what "top" means. The reading that made the stopping behaviour mysterious was the one that was wrong.

**Useful side effect:** each row prints its own ETA, so the tool can quote the game instead of
reproducing its arithmetic. And the four rows report ~3.6 h (the `+8` pet) and ~13.96 h each (the three
at `+0`) to `+9` 100 %.

**Left open:** the four-row work itself. The plan is `return_slot` + a list of `pet_slots` + a list of
`food_slots`, per the player's own shape, with the swap (find the next pet by icon when one finishes)
after it because the paid days are what is running. The tool still drives one row.

---

## 2026-09-19 (3) — the reload ran live, and the empty-slot reference finally decided something

**The tool's core action ran end to end on the live game, first time.** One reload, 30 seconds, Start to
`reload complete`:

```
目錄 → the feed icon → Enter (clears the out-of-food message)
→ END boarding
→ ITEM2 tab → right-click the pet's cell (page 2, cell 2)
→ ITEM2 tab → right-click FOOD cell 52 → MAX → Enter
→ ITEM2 tab → right-click FOOD cell 51 → MAX → Enter
→ start boarding → close the window
```

**The placement was VERIFIED rather than assumed, and that is a line that is not in the log.** `PlacePet`
logs nothing on a first-attempt success, `"the pet went in on attempt N"` only when a retry was needed,
and `"pet slot not checked"` when the reference is missing or unreadable. None of the three appears, so
the crop comparison ran and answered *occupied* on the first right-click. Captured 2026-09-18, this
reference had until now only ever been written about.

**Measured in the log rather than on the card:** two food cells consumed (11 → 13 of 16), boarding
restarted, the window closed, and `boarding_running: true` afterwards — which is only trustworthy on a
failure path because of `ecbbc5d`.

**And a question the plan had already answered, re-opened and re-closed.** Topping up a starved feeder
does **not** resume the feed: the game pops the error, the pet hangs unfed, and the only way forward is
Enter → END → re-place the pet and the food → start (player, 2026-09-19). [§9](PLAN-PET-AUTOFEED.md)
recorded "Answered: no" on 2026-09-17 without the mechanism, which is why the simplification was
proposed, investigated, and killed a second time. **The mechanism is what §9 was missing.**

**Not verified — and it is the one that matters next:** the slot read with the pet *starved and
boarding running*, which is what the state-read design rests on. The read that ran here was
post-placement. Nothing has read the slot before a click, and there is no button that would.

---

## 2026-09-19 (2) — the panel parser, and the player correction that deleted a section of the plan

**Goal.** Thread 1 of the handover. The hover read has worked since 2026-09-18 and had no consumers at
all: `ReadLines` returned strings, two UI buttons printed them, and nothing turned a panel into
something a tool could act on. This session added the parser and the first consumer.

**`PetPanel` reads the numbers and nothing else** — and that is the design, not a shortcut. The
measured read is `（6）真蔚蓝凤凰+7[52.18%]` against a screen showing `(6階) 真蔚藍鳳凰 +7 [52.18%]`:
every digit exact, the Chinese not, and the recogniser's errors are not uniform — some characters
arrive Simplified and `text_fixes` converts them, others arrive Traditional already, others are
misread. A parser keyed on any character inherits all of it. Nothing is anchored to a bracket or a
parenthesis either: the EXP is found by its `%`, the growth by its `+`. Two decisions worth naming:
the finished test needs **both** numbers (`+9` with a filling bar is not done; a full bar on `+7` is
not either), and it is **equals, not at-least**, because "at least 9" answers "finished" to a garbage
growth — the one direction that stops feeding a pet that still needs it. 15 tests, mutation-checked.

**The player correction is the more valuable half, because it removed work.** §13 had the reload read
*the boarded pet's* panel to decide whether it was finished. The player, 2026-09-19: **a finished pet
is mailed by the game** and appears in neither the boarding window nor the bag. So the boarded pet
never needs reading at all, and the finish check belongs where we go looking for a pet to **board** —
reading one **in the bag**, which is the case the read was verified on. The unverified hover inside
the boarding window — the thing that made me want to prove the read before wiring it — is simply not
needed. Also settled: the stop-breeding button returns the pet to the **first available bag slot**,
not to where it was taken from, which is part of why the icon scan exists.

**What the read still cannot give is `wyz`**, and that is the number the remaining time needs:
`remaining = 14.5 − done(growth, exp)` in units of `wyz`, then ÷ the stage's rate. The read supplies
the stage (→ rate, from PET-DATA's confirmed table) and the position; `wyz` is per **species + stage**
and varies 27× within a single stage, so two stage-6 pets cannot be told apart by the panel. Three
ways to get it, in the order they would be tried: the panel itself, if it states the current level's
所需喂养值 (then `wyz = that ÷ (1 + growth/10)`, one read, no table); the pet's name against the
327-entry table; or two reads a cycle apart, which measure the rate and so `wyz`, with no table at all.
The open question is now item 4 of [PLAN-HOVER-INFO.md](PLAN-HOVER-INFO.md).

**The Pet tab's Test read is how that question gets answered.** It hovers the marked PET cell on its
marked page, reads the panel at the calibrated offset, and reports the raw lines, the parse, and
**every number it saw with its surrounding characters** — the last being the point, since a report
that echoed only the parse could not say whether anything else is in the panel. It hovers with a MOVE,
never a click: a click on a pet in the bag switches the equipped pet, so measuring would change what
is measured (already why `FocusThenHover` is split).

**And the handover's own pending task turned out to be impossible, which is the find of the session.**
The instruction was to set the Pet tab's Timing fields and press Save. Checking whether that would
survive a restart: the boxes write `ActionWaitMs` and `WaitAfterEmptyMinutes` to the in-memory config,
`ConfigLoader` reads both back from `LocalPet` — and **nothing ever wrote them**, on either pet tab.
The values would have reverted on the next launch, silently, which is `gem.move_mode` again
(PROGRESS.md, 2026-09-13). The same audit found the worse half: Calibrate Pet's Save built its own
`LocalPet` field list and **replaced** the object, and that list omitted `PetIconRect`, `PetIconPng`
and `MaxButton` — so saving a calibration would have wiped the queue crops and the pet's own MAX.

Both are now one projection, `LocalPet.From(PetConfig)`, called by both save buttons. The fix that
matters is not the two fields restored but the shape: `EveryLocalPetFieldIsCopiedFromTheConfig` walks
`LocalPet`'s properties by reflection against a fully-populated `PetConfig`, so adding a field and
forgetting it there fails the test suite rather than the player's next session. Mutation-checked —
deleting `ActionWaitMs` from the projection fails both new tests.

**Status when written:** the parser was pure code with tests and nothing had run the Test read. That
changed the same session — see below. No tool acts on a read yet. The save fix is a compile check
plus tests only: the launcher is not reachable from the test project, so the buttons themselves have
not been pressed.

### The Test read ran, and it answered three questions at once

**First hover of the session, three reads** (`logs\reads\petpanel_*.png`). Two were the wrong thing —
a bag item and a quest letter — and the third was the pet:

```
(6階) 真蔚藍米魯 [0.06%]
所有職業皆可使用。
等級限制 150
名望限制 51595
販賣價格 - 500000s
```

**`所需喂养值` is NOT in the panel**, so `wyz` cannot fall out of one read and the expected shortcut
is dead. But the same read found the route that does work, and it is the one this project had ruled
out: **the name reads cleanly and matches the scraped table exactly.** The panel's `真蔚蓝米鲁` is
character-for-character [PET-DATA.md](PET-DATA.md)'s `真蔚蓝米鲁`, id 24759, stage 6, `wyz` 5200.
PLAN-HOVER-INFO's script-mismatch warning was about the game *displaying* Traditional — true, but the
*recogniser* outputs Simplified and the table is Simplified, so the two agree as long as the name has
no character that `text_fixes` converts. That caveat is the new open question.

So the remaining-time chain closes end to end for this pet: 14.5 − done(0, 0.06%) = 14.4994 units ×
5200 = 75,397 喂养值 ÷ 30 per item ÷ 3 per minute = **838 minutes**, against the table's own 838.0.

**And the parser was wrong in two ways the live panel exposed, both fixed with the read as the
fixture.** A **+0 pet renders no `+N` at all** — the row is a space where a +7 sits on a levelled pet,
with the image crisp enough that nothing was lost — so requiring one rejected every pet at the start
of its run, which is the pet this tool feeds most. A missing growth now reads as zero, and the test
that said otherwise was simply wrong. Separately the 階 is misread as 踏 (`（6踏）`), so the stage
pattern tolerates a couple of mangled characters. All three anchors — zero-growth, the stage-unit
tolerance, and the bracket that makes a percentage an EXP bar — are mutation-checked.

---

## 2026-09-19 — the pet checks, the timing model, and what the game actually does

**Long session, mostly on the Pet Feeder's edges.** v2.10 shipped at the start of it; everything after
is the work that makes the tool trustworthy rather than merely working.

**A 12-hour unattended run, and the failure it exposed.** Five reloads, all reported as successes — and
the pet was at +5 26%, roughly half the progress twelve hours of feeding should produce. The cause: a
right-click can fail to register, and **nothing looked at the result**. The placement is verified to
within 2px, so the cursor was on target when the click went out; the game simply did not act on it.
An intermittent action cannot be aimed out of existence, so the tool now **checks the pet slot after
each placement and clicks again** — up to three times — against a captured reference of the empty slot.
Unknown (no reference) is deliberately NOT failure.

**Three mechanics the player corrected, each of which changes code:**

- **`+10`** — our name for `+9` at 100%, the state that lets a pet evolve. **The string appears nowhere
  in the game**, which shows `+9` with a percentage. `+9` alone is the wrong guard in both directions.
- **Boarding auto-stops on `+9` 100% and on logout, and on nothing else.** The food running out does
  *not* stop it: the game raises `寵物食物不足` in a bubble, writes the same line to the chat log, and
  leaves the button reading `結束代養` while the pet simply goes unfed. So the toggle covers finished
  and unboarded, and the chat line is the only "food is out" signal — and the better one, being text in
  a fixed region.
- **A stage is `base × 14.5`, not `× 12.6`** — the old figure summed only the nine transitions and so
  measured the cost to *reach* `+9`, not to finish it. Every published total was 15% low. Corrected in
  `PET-DATA.md`, the matrix, the `.G` total, the CSV and the plan.

**Timing is now modelled the way it is meant.** The cycle is `load + wait_after_empty`, positive by
design: reloading *after* the feeder empties guarantees it IS empty when two stacks go in, and what the
game does with a top-up onto a partial stack is unknown. The earlier "safety margin" model had the sign
backwards and discarded 45 items a cycle.

**The hover read works and was verified live** — `OcrEngine` moved to Core, a plain `ReadLines` added,
and a Test read that saves both the image and the text it read. **Every digit came back exact; the
Chinese did not**, non-uniformly. `attributes.yaml` already carries the fix table for exactly this,
with the comment saying so — my own two attempts at explaining it are recorded in the doc as wrong in
opposite directions.

**Left open** — the handover (`HANDOVER.md`) carries the ordered list: the parser and the `+10` guard
first, then the icon scan and the queue, the feeder-count read, the two-writers race, and filling
`text_fixes` from the OCR logs.

---

## 2026-09-17 — the Pet Feeder, and the cursor bug it uncovered

**Shipped as `v2.10`.** A new tool that keeps a boarded pet fed while nobody is watching, plus two
fixes to the shared cursor placement that reach every other tool.

**The design work was in the documentation, not the session.** `PLAN-PET-AUTOFEED.md` was written first
and corrected repeatedly against what the game actually does — the mechanism is 代養 (boarding), not
餵養; the load is two stacks of 300; ending the feed returns the pet *and* the food, so a reload is
end → replace → load → start rather than a top-up. `PET-DATA.md` carries all 327 normal pets with their
boarding food and the per-level 喂养值 cost, measured off the live site (`base × 12.6` for a full
stage — the cost rises by a tenth of the base each level).

**The tool's trigger is a schedule, and that is a conclusion rather than a shortcut.** The hunger
readout at the bottom right belongs to the *carried* pet, not the boarded one, so nothing on the main
screen says anything about the pet being fed. What is left is arithmetic, and it is enough: reloading
early is nearly free because the leftover food comes back.

**The cursor placement bug it uncovered is the more valuable find.** `HidPointer.WaitForCursorToSettle`
treated two polls reading the same position as a finished move — which is equally true of a move that
has not *started*. On a full-height move the loop fed corrections forward against a stale position,
three asks landed together, and the cursor clamped at the top of the screen. Two fixes: a move is not
settled until movement has been *seen*, and the step budget went 6 → 16 because a damped correction
needs ~10. Every tool that places a cursor inherited this; the shop never exposed it because its
corrections are small.

**Still unexplained:** something moves the cursor that is not the tool — one trace shows 550 px of
travel in answer to a 61 px request. A re-aim before each click covers it. If it is the game's own
auto-farming moving the pointer, it affects every placement.

**Deliberately not built:** reading the boarding state (a tick box stands in), and the pet-slot and
feeder-slot checks — both slots are calibrated and neither is read, so a reload that half-fails
currently reports success.

**Left open**

- The boarding state is asserted by the player, not read. A crop comparison of the 開始代養 / 結束代養
  label replaces the tick box and needs no new machinery.
- The pet slot and feeder slots are calibrated but unread — the guards that would let the tool stand
  behind `reload complete`.
- Most inter-step delays are still inherited from the shop tool's values; only the 2.5s after ending
  and the 0.9s after a page tab are measured.
- `OcrEngine` lives in `SealTools.Tuner`, so the pet tool cannot use it for the EXP% read that would
  stop it feeding a finished pet. Moving it to `Core` is the prerequisite.

---

## 2026-09-15 — the cards can be hidden, so a small screen can still calibrate

**Reported from the second PC.** Its screen is small enough that maximizing the launcher to drag a
capture canvas left the five tool cards eating height the canvas needed. The cards are ~500 logical px
— on a maximized window that is a third of the space, and the canvas is the one thing on that tab
that cannot be scrolled or shrunk.

**`▸ Tools`** now hides them, sitting to the left of `Configuration` because it controls the row above
it. Verified by measurement: hidden, the tab strip moves from y=986 to y=234, the window keeps its
height, and the tabs take all of it.

**The two toggles move together, each in one direction**, and that is the whole design:

- hiding the cards **opens** Configuration — hiding them is only meaningful when you are using the
  tabs, and this is also what keeps the window from ever being empty;
- collapsing Configuration **restores** the cards.

Either rule alone leaves a reachable state with nothing in the window. Confirmed both ways: hidden →
1845 px tall with 0 `Start` buttons; collapse Configuration → 995 px with all 5 back.

**Cards-hidden counts as "not mini".** Mini mode shows one card and sizes the window to it, so the two
cannot both apply — without the guard, collapsing the cards while a tool ran would have shrunk the
window to the height of a card that is no longer on screen.

**Not persisted**, unlike the placement and pinning. It exists for one task, and a launcher that
opened with its status cards missing would read as broken rather than as a setting.

**Shipped as `v2.9.1`**, with the `v2.9` release removed rather than left beside it — the v2.9 zip had
been up for a day and this supersedes it, so two live downloads would only split the audience. The tag
stays, so `git show v2.9` still works. Replacing the *asset* inside v2.9 was the alternative and was
rejected: the tag would then name a commit that is not the build in the zip.

**Left open:** the layout sweep. Three defects in this area were found by the user *looking* at a
narrow window (Save Preset above what it saves, a clipped Delete, the Fast tick box's missing right
border), and none is reachable from the test project. This change is more of the same surface.

## 2026-09-14 (4) — v2.9 packaged, and the firmware had never shipped at all

**The release.** `v2.9` is tagged on `main` (the branch fast-forwarded, 32 commits, 0 behind), with
three assets in `dist\`:

| Asset | Size | Contents |
|---|---|---|
| `SealTools-v2.9.zip` | 137.8 MiB | exe, 10 native DLLs, 3 models, template config only |
| `SealTools-v2.9-local.zip` | 149.9 MiB | the same plus the full `config\` (live `local.yaml` + 3 screenshots) |
| `SealTools-v2.9-firmware.zip` | 3.6 KiB | `seal_mouse\seal_mouse.ino` |

Verified by **diffing the public zip's file list against v2.6**, which is the check that caught the
incremental-publish trap last time: the lists are **identical**, and the DLL count is 10 in both. All
10 errors in the first build attempt were `MSB3027`/`MSB3021` copy failures against the running
launcher, not compile errors.

**The firmware had never been in a release.** `publish.bat` copied `models\` and `config\` and
mentioned the sketch nowhere, so every zip back to v2.1 shipped an app that drives a board and no way
to flash one — the `.ino` was reachable only from the repo. It is now a **separate** zip rather than a
folder inside the app zip, on the reasoning that it is not part of the runtime install (nothing
extracts it) and a player with an already-flashed board never needs it. `Compress-Archive -Path
'..\arduino\seal_mouse'` keeps the sketch in a folder of its own name, which is what the Arduino IDE
requires to open it.

**Two things found while packaging, both by looking rather than reasoning:**

- The local zip was shipping `local.yaml.corrupt-backup` (present since v2.4, nobody had looked) and
  would have shipped the fresh dated backup taken this session. `xcopy` has no name-pattern exclude,
  so both are deleted from the publish folder after the copy. A release should not carry backups.
- The `public` copy has no such problem and is confirmed clean: it carries `attributes.yaml`,
  `defaults.yaml` and `local.yaml.example`, and no real `local.yaml`. The zipped local zip's
  `local.yaml` was diffed against the live one and is byte-identical.

**Verified live:** the Buy tab's row picker — a dropdown reading "Row 1" … "Row 10", with `Springs`
landing on Row 9 and `HighPet` on Row 8. Both halves of the buy/sell tool are now live-verified.

**Left open:** the spammer preset. `v2/config/local.yaml`'s `spammer:` key is empty and no copy of a
personal preset exists — every `spammer:` block in every zip in `dist\` is the shipped `*0`–`*9`
template, and `config/local.yaml` is gitignored so git never had it. Almost certainly the
`519fad6`-to-`a575928` window: that build deleted the block from the template without yet adopting it
into `local.yaml`, so the first save from any other tab destroyed it. Unrecoverable here; the fix is
to re-enter the rotations. **The durability hole itself is still untested** — no test asserts that a
`SaveDefaults` from one tab cannot drop a section belonging to another.

## 2026-09-14 (3) — the Buy row field became a picker, so the off-by-one can't be typed

**Goal.** Close the last open item on the buy/sell tool. A preset's row was a free-text **0-based**
index with "0 = top row" in the label — a number that is wrong one way round and labelled the other,
which is the worst possible pairing. Typing `1` bought the *second* item down and nothing on screen
said so; the label was the only clue and it had to be read every time.

**What changed.** The field is now a dropdown listing the rows as **"Row 1" … "Row N"**, filled from
the configured `ShopRows`. The stored value is still the 0-based index the geometry uses — only the
labeling changed, so nothing downstream moves. `SelectedIndex` *is* the row, so there is no number to
parse and no fencepost to get wrong.

Three details that are the whole change:

- **The save-time check is numbered the same way.** `ShopRowOk` printed `Row {row}` with the raw index,
  so a rejected "Row 4" pick would have been answered with "Row 3 is past the bottom" — the off-by-one,
  reintroduced by the error message that exists to explain it. It now reports `row + 1`, and the
  saved-item confirmation likewise.
- **The picker is refilled, not built once.** `ShopRows` is a config value; a picker built at startup
  could offer a row this shop's list does not have. It is rebuilt by `SyncBuyRowPicker` alongside
  every preset refresh.
- **A preset saved under a larger row count stays representable.** Its row gets an entry of its own
  ("Row 11 — past the bottom of the list") rather than being dropped. Dropping it would blank the
  field, and the next Save would then write the wrong row — trading a clear message for a silent one.
  Selecting it leaves `SelectedIndex` past the bottom, which is exactly what `ShopRowOk` reports.

**Also fixed, because it was the same bug wearing a different hat:** the picker is filled from
`RefreshBuyPresets`, not only from the preset picker's `SelectionChanged`. That event does not fire
when nothing is selected — which is precisely the fresh install that has to pick a row to create its
first item. Without this the dropdown came up empty and no item could be created at all.

**Verified:** builds clean, 45/45 tests pass. **Not verified:** the tests do not reach `MainWindow`, so
that is a compile check — nothing has exercised the control. The layout is the one thing that wants a
live look: the picker is `Width = 160`, matching the Name field beside it, but a WPF-UI `ComboBox`'s
height and padding are not a `TextBox`'s and the Position rows are independent grids, so nothing forces
them to agree. A DPI-aware capture of the Buy tab is the check.

**Left open, unchanged:** the count seeding from the preset's usual count, and the sell selection
never being persisted. Both were reviewed and both stay as they are.

## 2026-09-14 (2) — the buy/sell tool, and four lessons that cost real time

**Selling verified too.** Both halves work on the live game. Selling takes a set of bag slots, picks
them on an 8×8 grid standing in for the bag, and sells **highest slot first** — chosen so it is correct
whether or not the bag closes the gap after a sale, without needing to know which. Cap enforced.

Added since: a per-row select button in the sell grid (one click for a row of eight), the Buy card
carrying its own preset picker and count so a run needs no Configuration at all, and the count moved
out of the item — a preset is now the item's *identity* (row, scroll) plus a usual amount, and the run
decides the rest.

### What the layout work taught

Every misalignment traced to **a control inheriting a default meant for a different context**, and each
had to be found by measuring because the eye cannot tell 6px from 0px in a screenshot:

- A stepper built with `MakeButton` — sized for "Start", `MinWidth 84` — so "−" and "+" were each 84
  pixels wide.
- A vertical StackPanel stretches children to the column width, and that column is as wide as its
  widest row. So the Buy card's Start/Stop were stretched to the width of the preset row above and laid
  out from *its* left edge, sitting 50px left of every other card.
- `MakeButton`'s 6px right margin put `Stop` 6px inside the column edge while the marginless steppers
  ran to it — so the "+" overhung by exactly 6. The *buttons* were already pixel-perfect across all
  five cards; the row above them was not.
- `MakeButton`'s 10px top margin drops a button below any text field it sits beside. **The gem
  calibrator had already patched this by hand with a comment explaining why** — the reason existed and
  was rediscovered anyway. It is `MakeInlineButton` now.

**Every one of these was fixed by measuring, and every guess I made instead was wrong.** The
`FontSize` case is the sharpest: I assumed the WPF-UI template overrode it and tried growing the row
height, which does nothing — the text stays centred and clipped whatever the box height. A deliberately
absurd size of 9 settled it in one cycle. A change small enough to be indistinguishable from no change
is not a test.

### And one about my own tooling

Three of my screen captures were taken by a **DPI-unaware process**, which scaled the desktop and
cropped the launcher's left column out of the image. I reported the card names as missing and the
Start/Stop buttons as absent. Neither was true. Making the capture DPI-aware — the user's suggestion —
produced the first trustworthy picture of the session. Before that I had twice reported bugs that were
artifacts of my own measurement, which is worse than not looking at all.

## 2026-09-14 — the buy path works on the live game; and what first-contact cost

**Verified:** a real buy run lands on the shop row and completes — right-click the row, MAX,
Enter, Enter, for the count set on the preset. Selling is not yet tested.

Getting there took a run of failures that were all the same *kind* of failure, which is the part
worth keeping. The whole transaction is fire-and-forget: the board sends a click or a wheel notch,
nothing reports back, and the run has no way to tell a command that worked from one the game ignored.
So every distinct cause below presented identically — the cursor moved to the right place and then
nothing happened:

- **The wheel needs the game FOCUSED.** Not hovered, focused. Starting the tool means clicking the
  launcher, which takes focus away, so the run's first scroll was ignored. Fixed by clicking the
  scroll point first — which flipped that mark's requirement from "somewhere in the game" to
  "somewhere INERT", because the run now left-clicks it.
- **A left-click on a shop row does nothing.** Buying opens the count dialog with a RIGHT-click, the
  same gesture as selling. It was left-clicking, so the click landed perfectly and was ignored — and
  it was chased as a positioning bug (icon versus name) before being found as a button bug.
- **The firmware's scroll ceiling was load-bearing.** "Scroll to the top" was sent as one command at
  the maximum, so the cap *was* the reach: a list longer than it left every preset measured from the
  wrong origin. 30 reached halfway down the real list; 200 looked equally broken. Removed entirely —
  the list is expected to be at the top already, which is setup rather than a scroll the tool sends.
- **The run's post-scroll wait was 0.45s** against a board that was still scrolling. The cursor
  placement that follows is a closed loop reading `GetCursorPos`, so it ran against a stale position
  and kept correcting. Would have clicked in the wrong place.

Also found by measurement rather than reasoning: `Mouse.move`'s wheel argument is a `signed char`,
which limits one *call* to ±127 — irrelevant, since each notch is its own call. There is no
hardware or OS ceiling; the number in the firmware is purely a guard against a malformed value
wedging a board that cannot read serial while it loops.

**The recurring lesson, stated three times in this file already:** every number picked by reasoning
was wrong (30, then 200), and every one found by measurement was right. The cap is 400 notches now
because "ten seconds is an acceptable worst case" is a *decision*; 30 was a guess dressed as one.

## 2026-09-13 (3) — review sweep: 19 fixes, 4 recorded as deliberate

A full review of v2 in three passes (Core, launcher, tools), then the fixes. Every candidate was
verified by reading the code before anything was written, and that rule earned its place immediately:
one item handed over as a bug — the composer's empty-check gate — turned out to be a load-bearing
arming flag, and "fixing" it would have re-enabled auto-advance for a user who had explicitly
declined it. It was retracted rather than changed.

What mattered most:

- **`gem.move_mode` was deleted by every launcher save.** `SaveDefaults` serialises an explicit field
  list and the gem projection omitted it, so choosing `tuned` could not be persisted and the next
  launch fell back to `arduino`. The round-trip test that exists to catch exactly this compared the
  property default to itself, so it passed either way. It now sets a non-default value and is
  mutation-tested. Same shape as the `ocr_retries` regression that test was written for.
- **Two Start clicks could orphan a tool loop** on the shared serial port: unreachable by Stop, still
  writing, still driving the game. Guarded — plus the matching hole where a Stop during a cold start
  silently did nothing and the tool started anyway.
- **The tuner could stop below the target grade** (a stop branch missing `filter.Enabled`) and
  **treat a failed OCR read as a repeat**, ending a run after two bad reads behind a message claiming
  "x3". A read that recognises nothing now stops immediately and says so.
- **`CaptureScreen` threw instead of returning the null its callers were already written against**, so
  a locked session or a zero-sized region took down the tool rather than being handled.

Four items were deliberately **not** changed, and are recorded in [TODO.md](TODO.md) with their
reasoning because three would be reverted by a well-meaning cleanup: the null attribute value that
intentionally satisfies a bounded filter rule (`減少傷害` is rare enough that the name match is the
signal), `Arduino.Find`'s duplicated WMI query — which carries a name-based fallback `Diagnose` does
not have, the tuner's fail-open mouse guard, and the dispatcher sleeps in the calibrator's test
buttons, where the real fix is a five-handler refactor rather than `await Task.Delay`.

Also landed: spammer presets moved to `local.yaml` so they stop shipping, with adoption for any
already stranded in `defaults.yaml`; the test project can finally reach `AttrMatcher` and the other
tool logic (23 → 30 tests); and a CI workflow.

**Needs a live run** — in-loop behaviour the test project cannot reach: the tuner's failed-read stop,
the `filter.Enabled` guard, and the spammer's disconnect message. **Needs a flashed board**: the
firmware's host-gone key release (hold space, kill the launcher, confirm release). **Not yet run**:
the CI workflow's build step, which needs a push — the running launcher held a file lock locally.

## 2026-09-13 (2) — presets left in defaults.yaml are adopted into local.yaml, not deleted

Follow-up to the entry below, which fixed the leak but opened a data-loss path. On a machine that
only ever ran the older build the presets lived in `defaults.yaml` alone. The first Save on *any*
tab rewrites that file without a spammer block (tuner, gem and hotkeys all call `SaveDefaults`), so
that Save deleted the player's only copy, and the next launch found no presets and quietly created a
blank `default`. The entry below called these presets "stranded" — they were deleted on first save.

`Load()` now adopts them before anything can rewrite the file. `AdoptSpammerPresetsIntoLocal` fires
when `local.Spammer` is null and `defaults.yaml` still carries presets, writes them through
`SaveLocal`, and never fires again because `local.Spammer` is set afterwards. A first run seeded from
`local.yaml.example` carries no spammer block, so it does not fire there either.

**`MigrateSpammerPresets` had to move above `ApplyOverrides`, and that is a fix in its own right.**
It converts a pre-presets flat `spammer.keys` list into a preset named `default`. Running after the
merge, it saw `Presets.Count != 0` whenever `local.yaml` supplied any preset at all, skipped the
conversion, and the legacy keys were dropped without a word. Before the merge they become a preset,
are adopted, and survive in `local.yaml`. Both `Presets` reads now use `is { Count: > 0 }`: an
explicitly empty `presets:` key deserialises to null and `.Count` threw.

Tests: `LoadAdoptsSpammerPresetsLeftInDefaultsIntoLocalYaml` checks the presets reach `local.yaml`
and survive a `SaveDefaults` + reload — the assertion that actually catches the loss —
and `LegacyFlatKeysReachLocalYamlThroughAdoption` covers the ordering. Both were verified to fail
against a deliberately inverted adoption guard, so neither passes vacuously.

**Left open.** `defaults.yaml` keeps its stale block until the next Save, so on an affected machine
a preset deleted in the UI would resurrect once from it before the block goes. Self-healing after
one Save; stripping it during `Load()` would mean rewriting `defaults.yaml` (reformatting, losing
its comments) on every affected machine. Also unguarded: `publish.bat` public mode does not check
that `defaults.yaml` is free of a `spammer:` block before shipping it.

## 2026-09-13 — spammer presets move to local.yaml; Hold Space releases the key on stop

**Spammer presets were being published.** The "Save Spammer Config" button wrote every preset and
`active` into `defaults.yaml` via `SaveDefaults` — the one file `publish.bat` copies into the public
zip. So a player's personal rotations shipped with every release; the `Knight0-9` rename sitting in
the working tree was the symptom, not the cause. Spammer was the last personal setting with no
`local.yaml` home: calibration, tuner geometry, spring point, gem moves and UI placement all had one,
but `LocalOverrides` had no `Spammer` field at all.

Fix: `LocalOverrides` gains `LocalSpammer { Active, Presets }`, merged per preset *name* in
`ApplyOverrides`. The spammer tab now saves through `SaveLocal` (same shape as the tuner tab) and
says so in its InfoBar; the button is "Save Preset", not "Save Spammer Config".

**The seed had to go, and that decided the design.** `SaveDefaults` rewrites `defaults.yaml` from an
explicit field list, so removing spammer from that list would silently delete the block the first
time any *other* tab saved (tuner, gem and hotkeys all call it) — a seed that vanishes on first use
is worse than none. So `defaults.yaml` now carries no spammer block at all and `local.yaml` is the
single source of truth; `BuildSpammerTab` already creates an empty `default` preset when none exist,
so a fresh install still opens on a usable editor.

Guarded by three tests: presets load from `local.yaml`, `SaveDefaults` leaves no preset names in the
written `defaults.yaml`, and `SaveLocal` round-trips them. The first version of the no-leak test
asserted on a *reloaded* config and failed — reloading merges `local.yaml` back on top, so `Active`
is correctly the personal one again. It now reads the file on disk, which is where the leak would be.

**Hold Space could leave the spacebar down** (`d28a63a`). Stop clicked mid-loop skipped the release,
so the key stayed held. `LauncherService.ReleaseSpace()` writes `U` directly, independent of the
tool's loop, and the stop path calls it first. The top-right card also gained an idle/holding dot.

**Left open:** the log has a two-day gap before this entry (`1b85c59`, the Hold Space commits, and
the composer move set all landed unlogged). The preset migration this entry flagged turned out to be
a data-loss bug rather than a tidiness one — see the entry above.

## 2026-09-11 — two live-testing bugs fixed: grade parsed as G, and the matcher dropping lines

Both found while the user ran the tuner against a real DG item.

**Grade parsed as "G" when the item was "DG".** The OCR log proved the text was `GRADE：DG` and
the colour score was `DG=287` (yellow) — so OCR was right and the *parser* was wrong. `DetectGradeFromLine`
stripped the label, then a "rightmost match wins" scan let the bare `"G"` at index 1 override the longer
`"DG"` at index 0. It stayed latent for N/G items (no nested grade letter). Now longest-first.

**"One line less" — a matcher drop, not a read failure.** The user saw ~2 of 3 attribute lines.
The captures (`logs/captures/*.png`) are decisive: every DG frame shows all 3 lines, so the OCR reads
3 and `AttrMatcher` throws the third away when a misread character breaks the dictionary match. Four
recurring confusions, all in `text_fixes.substring`:
`每`→`国/盘/地`, `等級`→`等` (drops `級`), `幸運`→`幸莲`, `必殺技`→`必毅技` (`毅`≠`殺`).

This **closes the open "OCR row-bucket pooling" question** ([TODO.md](TODO.md), [IDEAS.md](IDEAS.md)):
the evidence shows no `row_height` pooling — the lines are all read; the drop was in the matcher. The
fix is `b42247f` (grade) and `3b8023b` (attributes.yaml cleanup rules). Note the launcher caches
`attributes.yaml` at startup, so the cleanup rules need a restart to take effect.

A **third drop shape** followed: a dropped *trailing* stat character (`幸運` → `幸`). Because the
truncated form is a prefix of the correct one, a plain substring replace would corrupt correct reads,
so `71f9672` adds a `regex` fix type (negative lookahead) for all six per-level stats.

A **fourth** (`必殺技` → `必技`, middle char dropped) was the only remaining shape after a 3,189-attempt
capture run (2 drops, ~0.06%) — fixed in `54ad743`. Merged to `main` and tagged `v2.4` (2026-09-12).

---

## 2026-09-11 — the tuner places the cursor on 發條 and guards it (branch `v2-tuner-spring`)

**Goal.** Give the Magic Tuner the composer's closed-loop cursor: put the mouse on the 發條 button
automatically, opt-in, and stop (or re-centre) when the mouse moves away mid-run — the
human-takes-over safety. Full design: [PLAN-TUNER-SPRING.md](PLAN-TUNER-SPRING.md).

**What was decided, and why**

- **Opt-in, mirroring `gem.move_mode`.** `tuner.spring_mode: manual | hid` (default `manual` — today's
  behaviour unchanged); `tuner.spring_point: [x,y]` in `local.yaml`, calibrated by a 4th Calibrate Tuner
  step (click the 發條 button) with a Test Click to verify. In `hid` mode the cursor is placed at the
  end of the 5-second countdown; a missing point / window / failed placement stops the run and reports
  on the card — never click blind.
- **The guard is a three-way toggle** `tuner.mouse_guard: off | stop | recenter` (default `off`), because
  the user wants all three: "refocus" (recenter), "take over" (stop), "leave home and let it run" (off).
  `stop` halts on the first drift past `guard_px`; `recenter` re-places the cursor and keeps rolling
  toward the target grade (which outranks a stray mouse), stopping only after `recenter_max` runaway
  drifts. Checked once per attempt, before the click.
- **`GemPointer` → `HidPointer`** (mechanical), so the tuner reusing it reads honestly.

**What's left open (the data questions, still unanswered)**

- **Does the game warp the cursor during a run?** Until measured, `stop` can false-trigger if the game
  re-centres the pointer on its own — that is why the guard defaults to `off`, and why the plan's extra
  mitigations (two-consecutive-polls, foreground-only judging) are the first thing to add if a run shows
  one. **Verified live (2026-09-11):** `hid` + `off` (auto-place, no guard) and `hid` + `stop` (halt on a
  drifted mouse) both behaved as intended, with no false-trigger seen in `stop` — weak evidence the game
  does not warp the cursor, not yet a rigorous per-attempt measurement.
- The guard's "once per attempt" cadence is a first cut; a live run should show whether the poll needs
  to be more frequent (e.g. on the `SleepCheck` loop).

**Commits** (branch `v2-tuner-spring`, not merged): `63ff486` rename, `d5ff0f3` config, `61bf8b9`
4th calibrate step + Test Click, `f0de93d` placement, `8b9e2e2` guard + UI.

---

## 2026-09-11 — the v2.3 zip is built; publish.bat gains a public/personal split

**Goal.** Build the v2.3 distributable and close the packaging gap: `publish.bat` produced a
`publish\` folder but never the zip the README points at, and it copied `config\` wholesale — shipping
the machine's real `local.yaml` and 14 MB of `calib_*.png` screenshots into a public zip.

**What was decided, and why**

- **Two publish modes.** `publish.bat` (public, the default) ships only the config templates
  (`attributes.yaml`, `defaults.yaml`, `local.yaml.example`); `publish.bat local` ships the full
  `config\` for a same-machine reinstall that is already calibrated. The safe one is the default, so
  an unthinking `publish.bat` cannot leak a calibration.
- **The publish folder is deleted before `dotnet publish`** — not just the mode's config. A stale
  folder carries whatever a previous mode left, so the clean delete is what guarantees a public build
  can never ship a `local.yaml` a local build left behind.

**Three traps found while writing the script**

- **An incremental `dotnet publish` silently drops the native OCR DLLs.** A second publish without a
  source change produced only `SealTools.Launcher.exe` + models + config — no `onnxruntime.dll`,
  `OpenCvSharpExtern.dll`, `libSkiaSharp.dll`, … (11 DLLs, ~170 MB). The exe is 182 MB either way, so
  nothing looked wrong until the zip was listed against the known-good v2.1 release, which ships all 11
  alongside the exe. The fix is the clean delete above, which forces a full publish every time. This is
  the measured, don't-assume lesson: the "obvious" build worked, the second one didn't, and only
  diffing the zip's file list against v2.1 caught it.
- **NuGet content ships an 89 MB `libSkiaSharp.pdb` and `*.lib` import libraries** that
  `-p:DebugType=None` does not touch (it only stops *our* symbols). The script strips `*.pdb` / `*.lib`
  after publish; v2.1's zip never had them.
- **`set VERSION=v2.3` leaked into `dotnet publish`.** MSBuild reads env vars as properties
  (case-insensitive), so `VERSION` overrode the `Version` property and `'v2.3'` failed semver. The
  variable is now `RELTAG`.
- **LF-only line endings broke cmd's batch parser** (`goto`/`if` blocks → spurious
  "not recognized" errors). The file is CRLF, and the CRLF fix must be the *last* edit — GNU
  `sed -i` re-strips `\r`.

**Measured.** `dist\SealTools-v2.3.zip` (public) = 144 MB — 11 native DLLs + exe + 3 models + 3
templates, no `.pdb`/`.lib`, no `local.yaml`. `SealTools-v2.3-local.zip` = 159 MB — the same plus the
full `config\` (`local.yaml` + three `calib_*.png`). The public zip built *after* the local one still
holds only the templates.

**Left open.** The zip is built but **not uploaded**: `gh` is unauthenticated and the GitHub release
`v2.3` does not exist yet. Nor does the `v2.2` release the README advertised — its features shipped
inside v2.3, and that row is now folded into v2.3's. The old v2.0/v2.1 folders and zips in `dist\`
are still there, to delete once the release lands.

---

## 2026-09-11 — the launcher adopts WPF-UI; the window learns where it belongs

**Goal.** Make the launcher readable and keep the tool status visible while playing. The project had
WPF-UI loaded but only ever used `FluentWindow`, `ui:TitleBar` and `ui:Button` — every tab was plain
WPF with a second, hard-coded palette beside the theme.

**What was decided, and why**

- **Adopt WPF-UI properly, one tab per commit.** Card-based sections, `ui:` controls, and the
  theme's semantic brushes instead of our hex ones. The information architecture carried over
  untouched: grouping, renames, the `Hotkeys` rename. Full checklist in
  [PLAN-UI-CLEANUP.md](PLAN-UI-CLEANUP.md).
- **The shell change was tried and reverted.** A `ui:NavigationView` rail rendered correctly but
  clicking an item never switched the page, and the tab strip read better anyway — so the tabs
  stayed and the experiment is recorded so nobody retries it blind.
- **The window now belongs to the user.** It opens as just the tool cards, the configuration tabs
  hide behind a chevron, it can be pinned above the game, and while a tool runs it shrinks to that
  tool's card. Placement, size and expanded height are remembered in `local.yaml` and written
  ~0.7 s after a move or resize settles — not only on a clean close, which is what used to lose a
  resize when the process was killed.

**Two real bugs found in review (both fixed, both were live-facing)**

- **The move-set selector disabled itself.** It sat inside the arduino card, which is disabled
  whenever the other set is active — so choosing `tuned` disabled the only control that could switch
  back. It has its own card now.
- **The empty check was comparing the box's border.** The frame shifts by a pixel when the game
  window moves, which made an *empty* box score 7.7 % against a 0.01 gate and stalled the composer.
  Comparing the interior only: empty 0.0 %, gem 0.68–0.82 on a live run.

**Verified live.** Pin and placement survive a relaunch (moved to logical `(1927,3)`, reopened there
at `619×430`, pinned). Mini mode: `920×430` idle → `920×320` running → back on stop. A full composer
run advanced correctly with the inset fix.

**Left open.** The v2.3 zip is not built or published. The robustness list, the tuner spring plan and
the spammer key pad are all designed and waiting in [IDEAS.md](IDEAS.md).

---

## 2026-09-10 (6) — the empty check was comparing the box's border, and a moved window broke it

**Symptom (reported live).** The composer kept combining and never advanced, with the result box
visibly empty. The card said "stopped" only because the run had been stopped by hand.

**First check: not the guard.** The foreground guard from entry (5) was the obvious suspect, but the
log showed it working — `diff=0.077 empty=False` for six cycles, and exactly one
`refused: not foreground` line, at the moment focus moved to VS Code. So the check was running and
judging; it was judging wrongly.

**Diagnosis.** `save_empty_captures` was turned back on, the run repeated, and the saved crop showed
an *empty* box. Comparing that crop against the reference per-row showed the differing pixels were
not in the middle but in horizontal bands at the very top and bottom — y=0,1,4 and y=55–58: the box's
drawn frame. A one-pixel shift in where the crop lands moves those rows while the flat interior stays
identical. That alone was 7.7 % of the box — six times the 0.01 gate.

**Fix.** The comparison now skips a 6 px border on each edge (`EmptyCompareInset`, passed through to
`GemColorAnalyzer.DiffFraction`). Measured on the real crops:

| inset | empty | gem |
|---|---|---|
| 0 (before) | 7.7 % | 35.6 % |
| 6 (now) | **0.0 %** | **54.6 %** |

The gem is drawn in the interior, so the separation gets *better*, not worse. A new test pins the
inset, including the fallback when the inset would swallow the whole image.

**Verified live afterwards**, same run, from `empty_check.txt`:

```
23:38:47  diff=0.818  empty=False     ← gem in the box (0.68–0.82 across the run)
23:39:13  diff=0.000  empty=True      ← emptied, so the composer cleared and advanced
```

The gem signal on a real run is even wider than the offline measurement (0.82 vs 0.55), and the empty
state is exactly 0.000 — the two states are now further apart than at any point before, with the gate
untouched at 0.01.

**Lesson worth keeping:** a 0.01 gate is only safe when the empty state really is pixel-identical. It
was — until the window moved. The inset is what makes the tight gate honest.

---

## 2026-09-10 (5) — the empty check refuses to judge a screen grab that isn't the game

**Why.** The check crops the result box from a screen grab (`CopyFromScreen`), so it measures
whatever is *in front*. During the empty-detection investigation a check ran with the launcher in
front and returned `RGB(26,26,46)` — the launcher's own dark UI — which produced a verdict about a
window that had nothing to do with the game. Nothing warned about it; the log line just looked odd.

**Fix.** `IsResultBoxEmpty` now requires the game window to be the foreground window before it
judges. When it isn't, it answers **"not empty"** — the safe direction (the composer keeps combining
instead of advancing a grade on a bad read) — and writes `refused: not foreground (fg="…")` to
`empty_check.txt` so the reason is visible rather than silent. The refusal shares the same log path
as a normal check, via a small `LogEmptyCheck` helper.

This is item 1 of the "Small robustness wins" list in [IDEAS.md](IDEAS.md), which is now ticked off.
Not yet verified live: the refusal only fires when something steals focus from the game mid-run, so
the next composer run with a stray click on the launcher is the observation to look for.

---

## 2026-09-10 (4) — a run ends after the last grade

**Symptom.** The first live `arduino`-mode run worked — combines until empty, N → G → DG, empty check
correct throughout — but after DG's material ran out it went back to N and started over.

**Why.** `AdvanceGrade` advanced with `gidx = (gidx + 1) % grades.Count`, an intentional endless
loop from v1. A run is meant to be N → G → DG once.

**Fix.** `AdvanceGrade` returns false when there is no next grade; the composer reports "All grades
done (last was DG) — composer stopped." on the card and breaks out of the loop. The manual F9
advance still wraps. Commit `6a28ecf`.

**Evidence from the run** (`<bin>\logs\empty_check.txt`) — the new empty check behaved exactly as
designed: gem frames `diff=0.44–0.54`, empty frames `diff=0.000`, gate `0.01`.

---

## 2026-09-10 (3) — empty-result detection rebuilt on a pixel difference

**Goal.** The composer advanced the grade while the result box plainly held a gem, so `empty_mode:
advance_grade_clear` ran away. Chased it to the empty check, not the moves.

**What was measured** (62×59 crop, the real empty reference vs a real gem frame)

- The old metric — mean of six absolute colour differences — scored the pair **0.100**, under the
  0.18 threshold: a box full of gem read as "empty". The one strong signal, the coloured fraction
  (0.556 → 0.930), was being divided by six.
- Euclidean norm over the same six: **0.294** (empty vs gem) and **0.001** (empty vs itself).
- Per-pixel difference vs the saved empty crop: **0.000** for an empty box at every channel
  threshold 10–60, **0.357** with a gem. The empty slot is static UI and renders pixel-identical.
- A metric sweep showed *every* pure-colour feature is the wrong family: `dominant hue` is identical
  (60°) for both, and mean R/G/B invert depending on gem colour. Structure metrics (edges 6×,
  distinct colours 8×, laplacian variance 10×) all separate and are colour/shape-blind.

**What was decided, and why**

- **Primary test = fraction of pixels differing from the saved empty crop** (>30 on any channel),
  threshold `gem.empty_distance` lowered 0.18 → **0.01**: ~35× below the gem signal, ~10× above the
  floor. Colour- and shape-blind, so any gem colour or grade shape reads the same.
- Euclidean colour signature kept only as a fallback for when the crop is missing.
- **The empty reference is now taken from the launcher-hidden screenshot**, not a live screen grab:
  the launcher covering the box at save time is what had poisoned the stored signature (0.17 away
  from its own reference crop).
- Diagnosis trap worth remembering: the sampler reads *screen* pixels, so a covering window makes
  every reading garbage — a check run with the launcher in front returned `RGB(26,26,46)`.

**Commit** — `f4b5f02` (branch `v2-arduino-moves`). 5 new tests, 17/17 pass.

**Left open** — the composer has not yet run a full session in `arduino` mode with the new empty
check; the next live run should show `diff=0.000` on empty boxes and `diff≈0.36` on gems in
`<bin>\logs\empty_check.txt`.

---

## 2026-09-10 (2) — Test Full Cycle (Arduino)

**Goal.** Let the new move set be judged on a real run before the composer is switched to it: one
button in Calibrate Gem that plays a whole cycle with arduino moves only.

**What was decided, and why**

- The sequence mirrors the **composer's own loop body**, not a guess: select → Register → Combine,
  then the composer's normal-path *deregister + register* (two Register clicks), then Combine again.
  Without that pair a second combine does nothing. The user asked for the combine loop twice at N and
  G; DG combines once and the cycle stops (no trailing resource clear — the user's choice).
- It always uses the **arduino** set regardless of `gem.move_mode`; that is the point of the button.
- All points are resolved *before* the first click, so a missing calibration can't half-run a cycle.

**Measured (live)** — `test-cycle-arduino done steps=21`, game focused the whole way, gold down ~2.16M
(the combines really ran), slots and result box empty afterwards.

**Commit** — `ac0f8ec` (branch `v2-arduino-moves`).

**Follow-up, same session:** the composer was switched over — `gem.move_mode: arduino` in
`defaults.yaml` (this commit). The composer now runs the arduino set on every route; the tuned counts
stay in `local.yaml` untouched, so switching back is one line.

**Left open** — same as the entry below: no full composer run with `gem.move_mode: arduino` yet, and
the reason `SetCursorPos` is refused in our process is still unknown.

---

## 2026-09-10 — cursor placement rebuilt on the Arduino; a second move set

**Goal.** Finish the `SetCursorPos` bug: test the last untested hypothesis (the game's anti-cheat
reacting to the process holding the Arduino port), then stop depending on the refused API at all.

**What was decided, and why**

- **The port hypothesis is refuted.** A throwaway process opened COM5, drove it, and called
  `SetCursorPos` 30 times while holding it: 30/30 accepted. A second process managed 29/30 while the
  port stayed held. So the refusal is not about the Arduino at all — it stays specific to the
  launcher's process, and stays unexplained. Details and every earlier probe:
  [CURSOR-INVESTIGATION.md](CURSOR-INVESTIGATION.md).
- **The cursor is now positioned with the Arduino** — a closed loop against `GetCursorPos`, which
  always worked in our process. The HID path cannot be refused, and a placement that fails now stops
  the tool instead of clicking somewhere arbitrary.
- **The move set was added, not replaced.** The user asked for the inter-point movement to use the
  same mechanism as Test Click, explicitly *without* discarding the hand-tuned `gem.movements`
  counts. So both sets are live, selected by `gem.move_mode` (default `tuned` — nothing changes for
  an untouched install). [MOVE-SETS.md](MOVE-SETS.md) explains both and how to test each.

**Measured (live, on the reference PC)**

- `D n 0` moves the cursor exactly `n` px in the process's cursor space at 100 / 250 / 500 / 600 px —
  gain 1.0, no acceleration. A placement therefore converges in one move; from a far start, three.
- The firmware walks a `D` move out in 10-px chunks with a 1 ms gap, so a fixed 20 ms settle read a
  stale position and stacked corrections. Polling until the cursor stops moving fixed it.
- Test Click lands exactly on the N radio and the radio selects; the `Register → Combine` arduino
  route places on (829,725) → (785,871) against calibrated targets (830,726) / (786,872).

**Commits** (branch `v2-arduino-moves`, not merged)

| Commit | What |
|---|---|
| `624fddd` | `feat(v2)`: the arduino move set — `GemRoutes`, `gem.move_mode`, `MOVE-SETS.md` |
| `2e0b043` | `fix(v2)`: closed-loop Arduino placement, failure stops the tool, mode wired to the UI |

**Left open**

- Why `SetCursorPos` is refused in the launcher's process — intermittently, while another process
  under the same user/session/integrity succeeds. Not load-bearing any more; the shortest next step
  if it ever matters is a minimal WPF app that only calls `SetCursorPos`.
- The `arduino` move set has been tested per route through the calibrator buttons, not yet through a
  full composer run with `gem.move_mode: arduino`.
- The composer's runtime path (both modes) is the same call the tests make, but a live composer run
  was not started during this session — it clicks in the game.
