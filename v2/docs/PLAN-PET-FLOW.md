# Plan — the pet feeder's visit flow

Written 2026-10-10, revised the same day after the player read the first draft.
**DESIGN ONLY — no code changed, nothing built.**

The ask, in the player's words: *"every reload of pet the tool goes through all the pages to find the
pet again … we should know all the pet positions with **one** scan and check the return position for
each reload, and if the reload position is empty/not suitable then go for the scanned results — but we
don't need to scan for the 2nd reload as we should already know where the 2nd pet option is. Every time
it should be: return slot, then the stored positions."* Then, over the same session: *"sometimes the
tool has its action predetermined before the visit"*, and *"when the tool revisits again, the pet state
should be checked again."*

**The one principle under all of it:** the reading is allowed to change **when** something happens, and
never **what** happens. Every defect below is a place where a decision was made from the timer, and the
fresh read was only permitted to adjust a number.

**Why this tool deserves its own plan.** It is the only tool that runs for days, and its failure
direction is the unrecoverable one: an unfed pet. Faster is worth having; nothing here may make a
boarding *less* likely. §7 is the list that must survive.

---

## 0. What the flow is today

```
RUN START                                                            PetTool.cs:283
 ├─ Ready()          every precondition, every ticked row — else STOP naming it
 ├─ PetFeeding.Load  a failure costs only the COMPUTED schedule
 ├─ claim game       held by another tool? WAIT, saying who                       :322-334
 ├─ InspectRows()    THE LOOK — open → ReadRows(every row) → close (finally, muted) :701-725
 │                   the reading IS the schedule: each row's next comes from it
 └─ publish + enter the loop

MAIN LOOP                                                            :386-492
 ├─ soonest = earliest next ;  SleepUntil(soonest)      ← where the day is spent
 ├─ batch = rows due NOW, in row order                                    :404-407
 ├─ claim game  (per visit — resident tools yield)
 ├─ Visit(batch)                                                          :1115-1199
 │    OpenBoarding → ReadRows(every row) → per row: ReloadRowInPlace
 │    → the SECOND time-line read, window still open → CloseBoarding (finally, muted)
 └─ fold back   Ok → measured next · NothingToBoard → 30 min, NOT a failure
                Failed → 5 min, three strikes → drop the row, others carry on        :427-489
```

Evidence base for every number in this document: `pet.log`, **25,992 lines, 2026-09-18 → 2026-10-09**,
127 `run started`, 122 looks, 285 visits.

---

## 1. The scan is eager on purpose — it carries the food re-mark

`PlacePet` builds its candidates in the right order, return slot first:

```
:1438-1441   candidates.Add(the RETURN SLOT)                        ← first, deliberately
:1448        candidates.AddRange(FindQueuedPetCandidates(ser, out why))   ← then the scan
:1469        foreach (cand in candidates) { hover → guard → click → slot check }
```

`AUDIT-PET-FLOW.md:198-200` states the intent: *"THE RETURN SLOT — tried first: one cell, one hover,
**vs a three-page sweep**."*

**The scan at `:1448` is eager, and it has to be.** `FindQueuedPetCandidates` does two jobs on one set of
page captures: it matches the queued pet icons **and it re-marks the food cells**
(`IconMatch.CellsHolding`). In the log both analyses land on the same sweep — `food scan: 30 cell(s) hold
food` and `icon scan: best 0 at page 2 cell 11` at the same second, `22:36:34`.

That second job cannot be deferred. The food list is **consumed, not re-derived mid-run**
(`NextFoodCell`, `:2092`), and `AUDIT-PET-FLOW.md:329-332` says it plainly:

> Once exhausted the reload fails until the cells are re-marked — and the scan added on 2026-10-02
> re-marks them during a pet scan. **A row that reloads without any pet scan would not get that refresh.**

### 1.1 The tempting change that must not be made

Making the scan **conditional on the return slot failing** looks free and is not: on exactly the reloads
where the return slot works — the common case — the food cells would go un-refreshed, the list would run
dry, and the reload that then fails is the one that cannot be undone.

**Proposed, declined, recorded here so it is not re-proposed.** The comparison in the comment is why the
return slot is *used* first; it is not a licence to skip the sweep.

### 1.2 What is still wasteful: the scan runs per row, not per visit

It lives inside `ReloadRowInPlace` (`:1213` → `:1278`), so a batch of three rows pays three full sweeps —
three passes over every calibrated page, each three tab clicks plus three page waits. Log: **290**
`icon scan:` against **249** `trying the return slot first`; a 500-line sample (~19 reloads) shows **43**
`click the ITEMn tab` against **20** `bag page N is already showing — no tab click`. The page optimisation
merged in `b61c7e3` is working (**760** avoided clicks over the log) and is the template: *do not repeat
work whose result you already have.*

**One scan per visit is the correct shape regardless of how often a batch is plural.** The bag is
*visit-level* state — it belongs to the window that is open, not to the row being reloaded — so scanning
inside the row loop re-derives the same visit-level fact once per row. That is a flaw in shape, not a bad
average.

And the plural batch is not the rare case it looks like. The log shows batches of one because the tool is
usually already running; **every restart after downtime puts several rows past due at once**, and that is
the visit with the most rows to reload and the longest work. Measured example, `2026-10-01 10:08:54`:

```
feeder counts: [0 / 0 / 0 / 0 / 0] -> 0 (5 of 5 slots read)
Row 2: 0 item(s) left in the feeder — reloading in 0 min
```

All four rows read zero at once → a 4-row batch. Rare, guaranteed, and the worst one to be slow.

---

## 2. The read cannot cancel the act

The batch is chosen from the schedule before the window is even opened:

```
:396   var soonest = live.OrderBy(r => next[r]).First();
:397   if (!SleepUntil(next[soonest], ct)) break;
:404   var batch = live.Where(r => next[r] <= now).OrderBy(…).ToList();
:407   if (batch.Count == 0) batch.Add(soonest);          ← "not due" → reload it anyway
…
:1143  schedule = ReadRows(ser);                          ← reads EVERY row
:1145  foreach (var row in batch)
:1147      var ok = ReloadRowInPlace(ser, state, row, …)  ← unconditional
```

`Visit` reads every row and then reloads every row in the batch **without consulting what it just read**.
The reading can re-derive a row's `next` and decide how much food goes in; it cannot cancel the reload.
`:407` is the blunt form: sleep until the soonest row is due, and if nothing is due, reload it anyway.

**That contradicts the tool's own rule**, written at `:310-314`:

> **LOOK FIRST, DON'T RELOAD FIRST** (player, 2026-09-19). … the boarding slot says whether a pet is in
> there, and the feeder counts say how much food is left — **so a look answers what a reload used to be
> performed to find out.**

The principle was implemented for *building the schedule* at run start. Inside a visit the tool still
reloads first and reads alongside.

**When it bites:** the player reloads a row by hand and the schedule cannot know; a pessimistic count read
sets an early `next`; the estimate drifts over the hours since it was measured; and every restart after
downtime puts all rows past due at once.

**The fix is smaller than it looks, because the machinery exists.** The fold-back already handles
*"rows READ but not acted on → take the freshly measured next"* (`AUDIT-PET-FLOW.md:117-118`). The gap is
only that a **batch** row can never become one. After `ReadRows`, drop any batch row whose *fresh* next is
comfortably in the future, and log the skip — using the schedule the visit already computed.

### 2.1 The danger, and the rule that contains it

A misread count that says "full" would skip a reload that was needed, and an unfed pet cannot be undone.
So the veto must be **positive-only**: skip only on a confidently-read count above the threshold — never
on null, unreadable or borderline. The tool already distinguishes null from zero and has a score gate;
**unknown keeps meaning act.**

---

## 3. A revisit re-reads, but cannot reclassify

The revisit *does* re-read the row: `Visit` calls `ReadRows` first (`:1143`). What it cannot do is change
the verdict that scheduled the retry:

```
Failed → failures++ ; under 3 → next = now + 5 min ; at 3 → drop the row
```

**20 of the 30 give-ups in the log are `No queued pet is in the bag`**, and the tool's own message names
both the cause and the fix:

```
2026-09-19 16:30:16  icon scan found nothing within 0.5: best was 0.98 across 2 icon(s) and 3 page(s)
2026-09-19 16:30:16  FAILED placing the pet: No queued pet is in the bag … If the pet finished it was
                     mailed, so this is the queue being empty of live pets — capture the next one's
                     icon on the Pet tab.
2026-09-19 16:30:17  Row 1 given up on after 3 failures — the other rows carry on
```

**That cause cannot clear itself in five minutes** — the player has to capture an icon. So the revisit
re-reads the bag, finds the same empty queue, fails again, three times, and then the row stops being fed.

And the bucket for exactly this already exists: **`NothingToBoard` → 30 minutes, explicitly "a WAIT, not
a fault"**, and it does not count toward the three. On a revisit, an empty queue **confirmed by a fresh
read** should reclassify the row from `Failed` to `NothingToBoard` — keep trying, do not burn the row
down, and say it once instead of three times.

**A second, worse gap in the same area:** for some failures the log records **no reason at all**.

```
2026-10-01 10:08:56   click the start/end boarding button at (750,283)
2026-10-01 10:09:01   Row 1 failed inside the visit — the other rows carry on
```

Nothing between the press and the failure. That row was then given up on. A failure that costs a row its
feeding should always say what failed.

---

## 4. The window is closed and reopened — and one close in 26 fails

### 4.1 Per operation, by design — *not* waste

The window is opened for the look and for each visit, closed in a `finally` every time. Between
operations the tool sleeps for **hours**, and there is a harder, game-side reason: **the boarding window
and the tuner's window cannot both be open** — the player's own report, `PLAN-WATCHER.md:42-49`
(*"you need to close the tuner before even you can open the pet window"*). An open breeder blocks the
tuner from starting.

### 4.2 The look → first visit pair — **this is waste**

`InspectRows` opens, reads and closes, releasing the game. If the look finds a row due **now** — the usual
reason the tool was started — the main loop immediately claims the game again and `Visit` opens the same
window and calls `ReadRows` a second time. Two complete open/read/close cycles seconds apart, and one of
the two `ReadRows` is redundant.

**Withdrawn measurement.** The first draft claimed "~34 opens with no matching close". That arithmetic
does not survive: 122 looks + 285 visits − 22 failed opens = **385 opens**, against **452 closes**. Closes
exceed opens, which is impossible — so the log's lines are **not** a 1:1 event stream across three weeks of
restarted debug builds. The claim is withdrawn; the waste above is argued from the code, not from those
counts.

### 4.3 A failed close — measured, and it bites twice

```
2026-10-09 22:36:37   close: couldn't reach the X — the cursor wouldn't move to (844,327)
                                                  — it stopped at (861,327)
```

**17 occurrences.** A failed close leaves the window **open**, and the next operation opens again with no
"is it already open?" check — so it clicks 目錄 while the breeder is on screen. `:712` already names that
hazard: *"leaving the boarding window on top of the game for the next cycle to click 目錄 behind."*

---

## 5. The target flow

```
RUN START
  claim game
  LOOK    open breeder → ReadRows (every row) → derive each row's next
          └─ a row due NOW?  → stay in this window and reload   ← no close/reopen
             nothing due?    → close, release, sleep

MAIN LOOP
  sleep until the soonest row's next
  batch = rows due NOW                    ← never "the soonest anyway"
  claim game → VISIT
     open breeder            (or notice it is already open — guard, not a blind click)
     ReadRows                → the schedule, AND the veto:
                                 fresh next comfortably ahead → SKIP the reload
                                 otherwise                    → act
     ONE bag sweep           → pet candidates + food cells, shared by every row
        per row:  return slot first (verify that cell)
                  else the scanned positions, best match first (verify that cell)
        per drag: verify the food cell before loading
     the second time-line read, window still open
     close (retry once; if it still will not close, say so and probe next time)
  FOLD BACK
     Ok                        → measured next
     NothingToBoard            → 30 min, NOT a failure
     queue empty, confirmed    → NothingToBoard, not Failed          ← new
     Failed                    → 5 min, three strikes
```

---

## 6. What must not change

1. **Return slot first, then the stored positions** — every time. Already the intent at `:1438-1441`.
2. **The scan runs once per visit and is never skipped** — it is the food re-mark (§1.1).
3. **A stored cell is verified with a single-cell read before it is clicked.** The sharing saves the
   sweep, never the check. `PLAN-RELOAD-VISIT.md:109`: *"the queue is icon-based **because** position cannot
   be trusted — a returned pet lands in …"*. Placing a pet consumes the item; a stack empties; the bag can
   compact.
4. **The veto is positive-only** (§2.1). Unknown always acts.
5. **Unknown does not skip.** A matched pet boards on an unreadable panel; the **return slot does not** —
   an empty fixed cell means "nothing to click" (`AUDIT-PET-FLOW.md:220-221`).
6. **Click, then LOOK.** Up to 3 right-clicks each followed by a slot check; an unreadable slot is *done*.
7. **The deliberate double read.** The second time-line read inside a visit (`:1148`) is not waste — one
   OCR pass, and it only ever pulls a next **earlier**.
8. **Cleanup is never abandoned.** Both closes run under `WithoutCancellation` (`:723`, `:1196`).
9. **`_bagPage` stays the single source of truth for which page is showing** (`:173`, `:2180-2191`).

---

## 7. Open questions ⚑

1. ~~Does the scan become conditional?~~ **DECIDED: no** (§1.1).
2. **How much margin** before a fresh reading may cancel a reload the timer asked for? Too tight and the
   veto never fires; too loose and it skips a genuine reload.
3. **What happens to `:407`** ("reload the soonest anyway") once a veto exists? As written, a fallback that
   forces an act and a read that may cancel it pull against each other.
4. **How is a confirmed-empty queue distinguished from an unreadable one?** The reclassification in §3
   must fire only on a positive "the bag was read and holds no live pet" — never on a failed read.
5. **Cache lifetime:** one visit (cleared on close), or until the bag changes? Visit-scoped is trivially
   correct: the window closes, the world may have moved.
6. **The failed close:** retry the X, or probe whether the window is already open before the next open?
   The probe is strictly more useful — it also covers a window left open by a stopped run.
7. **Should every failure log its reason?** One row's give-up currently can be reasonless (§3).

---

## 8. Order of work

Ordered by certainty of the waste and by the failure direction.

1. **Close retry + already-open guard** (§4.3) — smallest change, removes a measured failure mode
   (17 in the log), trades nothing away.
2. **Let the fresh read veto and reclassify** (§2, §3) — one change, because it is one gap. Where 131
   in-visit failures and 20 unfixable-cause give-ups live. Carries the unfed-pet risk, so it needs the
   positive-only rule and a watched live cycle.
3. **One scan per visit, with the per-use re-verification** (§1.2, §6.3) — structural, not
   measurement-gated: the plural batch is guaranteed on the first visit after downtime.
4. **The look → first visit hand-off** (§4.2) — certain waste, and the largest structural change.

**Nothing here can be verified without a live run.** One watched cycle with `pet.log` open settles steps
1 and 2; steps 3 and 4 need the bag to compact at least once to prove the re-verify rule.

---

## 9. What was measured, and what was only reasoned about

**Measured from `pet.log`** (25,992 lines, three weeks): 127 runs; 122 looks; 285 visits; **131**
`failed inside the visit`; **30** `given up on after 3 failures`, 20 of them `No queued pet is in the bag`;
**0** `Every row has failed repeatedly — stopped`; **218** `the feeder counts couldn't be read`; **226**
`slot reads EMPTY … reloaded now`; **290** `icon scan:`; **249** `trying the return slot first`; **17**
`close: couldn't reach the X`; **760** avoided tab clicks; the 4-row batch at `2026-10-01 10:08:54`; the
reasonless failure at `2026-10-01 10:09:01`; the empty-queue give-up at `2026-09-19 16:30:16`.

**Read in the source:** the batch selection (`:396-407`), `Visit` (`:1143-1153`), `PlacePet`'s candidate
construction (`:1438-1469`), the look (`:701-725`), the `ReadRows` split (`:737-744`), the rule at
`:310-314`, the close sites (`:723`, `:1196`).

**Reasoned, not measured:** that a fresh read would ever have vetoed a reload — the whole case for step 2,
and it is **one line of instrumentation** away; how often a batch holds more than one row on an ordinary
day; the margin in ⚑ 2; and that the re-verify rule holds when the bag compacts.

**Corrected across the two drafts of this document:** (a) making the scan conditional on the return slot
failing — declined in §1.1 because the scan carries the food re-mark; (b) the "~34 opens with no matching
close" arithmetic — withdrawn in §4.2 because the log spans builds with different logging.

**Not attempted:** no code changed, no live run performed, no plan file outside this one touched.
