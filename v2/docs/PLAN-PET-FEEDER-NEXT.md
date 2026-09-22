# Plan — what to improve on the pet feeder next

Written 2026-09-22, after the session that built the visit, the game's time line, the feeding table and
the return-slot-first placement. Four things, in the order I would do them, with what is known and what
still has to be measured.

**§1 (and its 1a) is BUILT** — 2026-09-23, Release build clean, 172 tests pass. Not yet verified in the
launcher, because that needs a restart and a feeder run was live. **§2–§4 are still nothing but a plan.**

---

## 1. Name the pet line on entries that already exist — built 2026-09-23

**The problem, and it is the whole reason this is first: everything built for the computed next-check is
inert for the pets already queued.** Each queue entry needs its **species** — one of eleven lines — and
the dropdown only exists at capture time, so the entries captured before yesterday have none. Every row
therefore falls back to the flat cycle:

```
Row 1: 537 item(s) left in the feeder — reloading in 184 min  [no time line read]
no feeding estimate — no pet line is named for this queue entry
```

Row 1's pet is at `+9`; with the line named its next check is the pet's own figure rather than the food's.

**What to build:** a line selector **per queue entry**, under its thumbnail — no selection state, and it
reads as what it is. Changing it writes `PetQueueEntry.Species` and saves through `PetSessionSave`, which
already exists and already knows this half of the config.

**Why per entry rather than one dropdown plus a "selected" pet:** the queue holds a handful of pets, a
selector each is the same amount of screen, and it removes the question "which one am I editing?" — which
is the class of state that has cost this project the most.

**And not one "set every entry" control instead.** A bulk set is wrong the moment the queue holds two
families: nothing stops a player breeding `鸟蛋类` and `种子类` together, because the rows simply board
whatever is feedable, and a single control would mislabel half of them silently. A *shortcut* over the
rows (one button that loops them) is fine and worth having once they exist; it must not be the only way
in.

### 1a. Make the line visible on the row — added 2026-09-23

**The repair path above is not enough on its own, and the live config proves it.** Three of the five
queue entries hold a blank `species`, and the reason is not carelessness: the **Which pet line** picker
is built once per launcher start pinned to index 0 (`"(no feeding estimate)"`) and is **never seeded from
the config**, so the first capture after any restart writes `null` unless the player remembers to re-pick.
Repairing the entries without fixing that means re-labelling the queue after every restart.

Three things, all small:

- **Seed the capture-time picker from an existing entry** at build time — the last one, or the first with
  a line. Queue entries already persist, so the value is already on disk and **no new config field is
  needed** — which matters, because the projection is the risk (§1's warning below) and this avoids giving
  it another field to drop.
- **Show the line on each row.** Today the queue list prints label + rect + PNG length and the thumbnail
  tooltips print the same — **never the species**. A blank entry is therefore invisible in the launcher,
  and survives exactly because nothing displays it.
- **A per-row ✕.** Only **Remove last** exists today, so an entry that is not the last one cannot be
  removed from the UI at all — repairing the three blanks currently means hand-editing `local.yaml` with
  the launcher stopped, because the Pet tab's save rewrites the whole queue from memory and would clobber
  the edit.

**Mechanism, and it already exists:** [`BuildRulesEditor`](../SealTools.Launcher/MainWindow.xaml.cs) is the
factored shape for an editable row list — a row class holding its own controls plus a mapper back to the
config type, a local `AddRow`, a `✕` that removes from **both** the panel and the backing list, an
`+ Add`, and the caller reading the list back at save time. The Spammer key rows are the same shape, less
factored. The queue editor should be a **third instance of that**, not a new idea — and because it edits
only fields that already exist, it does not touch the projection.

**The projection is the risk, and it is a known one.** `Species` had to be added in **three** places —
`PetQueueEntry`, `LocalPetQueueEntry.ToConfig`, and the save direction — and this projection has silently
dropped a field three times across two sessions. It is already pinned by tests in both directions with a
real non-ASCII value; the UI must not be the fourth.

**Size:** small. One control per entry, one write. **Risk:** low — it sets a value and saves.

**Constraint:** this is launcher UI, so verifying it means rebuilding and **restarting the launcher**,
which ends a feeder run that may be live. Build in Release (safe alongside a running instance); restart
needs the player's agreement at a moment of their choosing.

---

## 2. The intermittent slot read

**The symptom, live:** row 4 read `[0 / — / 300 / 300 / 300] -> 900` where the truth was `1200`. A slot
holding a legible `300` gave the reader nothing, was correctly **not** called empty (the crop showed
food), and so was left out of the sum — understating the row, which reloads it early.

**What is already in place:** the crop is saved on exactly that failure, and only on that one — food in
the slot and no number read. The next occurrence hands us the pixels.

**What to do, in order:**

1. **Wait for a failure, then look.** The last three theories about this read were all wrong and were all
   settled by looking at a crop. There is no reason to expect the fourth to be different.
2. **Then choose between two known-shaped fixes**, depending on what the crop shows:
   - **the crop is mis-aimed** (the number at an edge, or beside it) → **scan fractions**: try the
     configured start, then a step or two either side, and take the first that reads anything. A crop
     that starts *left* of the number reads nothing (the icon dominates), and a crop that starts *right*
     of it clips and **lies** — so the first success is the leftmost crop that contains the whole number,
     and is correct by construction. This was proposed on 2026-09-21 and dropped when the evidence
     stopped supporting it; it becomes right again if the crop shows mis-aiming.
   - **the crop is fine and the reader refused it** → this is the "legible crop finds nothing" trap
     already recorded for the feeder counts, and the answer is a bigger crop in the direction that does
     not clip, not a scan.

**Size:** an afternoon, once there is a crop. **Risk:** a scan multiplies reads per slot; the common case
(three digits) reads on the first try, so only short or mis-aimed slots pay.

---

## 3. The cursor placement failures

**The symptom:** `the cursor wouldn't move to (561,1191) — it stopped at (978,1163)`, hundreds of pixels
off, on a window that had just been reopened. Live at **10:29, 13:58 and 15:36** — three separate runs.
It is the largest single cause of rows going unfed in the whole session, larger than anything that was
fixed.

**And there is no cause attached.** What is known:

- placement lands to within a pixel in a manual test minutes later (the 14:07 trace), and in the icon
  sweep — so the board, the D-counts and the port are all sound;
- it happens **during a run**, worst on a window that has just been reopened;
- the failure is not a small miss that the closed loop should have corrected — it is hundreds of pixels,
  which suggests the moves themselves are not arriving rather than that the aim is wrong.

**The first thing to do is not a fix, it is an instrument.** The cursor trace is the only record of what
the loop asked for and what it got, and **it stops recording earlier than the tool stops acting** — noted
on 2026-09-21 when it went quiet at 14:44 while the scan kept running. Until that is understood, the
evidence is missing exactly where the failures are.

**Order:** fix or explain the trace, then reproduce, then diagnose. **Not** to be guessed at.

---

## 4. The retry policy

```
next[row] = now + RetryMinutes(5)                    a failure
after MaxFailures(3)  →  the row is DROPPED for the day
```

That is right for a transient miss and wrong for a state that will not change in fifteen minutes. Since
yesterday, **"nothing in the bag to board" no longer counts as a failure** and waits 30 minutes instead —
so what is left in this branch is closer to genuinely transient. The open question is whether anything
else belongs on the wait side rather than the failure side, and whether three strikes in fifteen minutes
is the right shape at all.

**This wants a decision before a design**, and the decision is the player's: which failures are worth
retrying soon, and which should wait out a cycle.

---

## 5. The return slot's species — built 2026-09-23

**Why it mattered most.** The return slot is tried **first**, and it is where the pet this row just
finished feeding comes back to — so the gap sat on the *preferred* path. Of 23 feeding-estimate failures
with 0 successes in the live log, **10 were return-slot boardings**; the others were 7 blank-species
entries and 6 rows whose named line has no row at the pet's stage. The summary line said "no pet line is
named for this queue entry" for a pet that had **no entry at all**, which is what sent the first
diagnosis after the wrong cause; the two cases say different things now.

**It cannot be read, so it is remembered.** The hover panel is read off a **bag** cell (one call site),
and a boarded pet is in the loader, not the bag — and `PetPanel` carries only stage/growth/exp anyway,
its own comment recording that a name lookup "could not have worked" across the Simplified/Traditional
split. But the line was **already known** when the pet went in, so `RememberBoarded` now teaches the row
and `MinutesForBoardedPet` falls back to that memory when there is no entry. It only ever **learns**: a
boarding with no line must not erase what the row's previous boarding established, which is the case this
exists for. In memory, not persisted — persisting adds a field to both directions of the projection that
has silently dropped one three times; the cost is one blind cycle after a restart.

## 6. The game's own estimate, and the trap in "just look again" — 2026-09-23

### It is accurate, and that is now measured

**The completion line is right to the minute, and the tool's use of it works.** 2026-09-22:

```
20:54:40  Row 2: 300 item(s) left — reloading in 31 min  [代餐完成预計所需時間：約26分]
21:25:41  visit: opening the breeder once for 1 row(s) — Row 2      ← 31 min later, to the second
21:25:47  Row 2: slot reads EMPTY, so this row is not boarding — it will be reloaded now
```

The pet had finished and been mailed exactly when the game said it would. The level form is the norm for
a row mid-run (`Row 1 … [到9為止…約96分]`, `Row 3 … [到8為止…約67分]`, same visit) and is ignored by
design, because it resets every level.

**And a freshly landed pet shows the level form.** In the same minute as the reload above, Row 2 read
`1500 item(s) left — reloading in 505 min [到1為止预計所需時間：約57分]` — the level form, full-load
fallback. So reading the line right after `StartBoarding` buys nothing, and the earlier proposal to use
it as "another update time" is dropped. This was an inference until this log was read; it is a
measurement now.

**One thing left to instrument.** `FeederEta.Parse` returns on the **first** line carrying `約N分`. If a
window ever states both forms at once, which one the tool gets is decided by OCR order — and a completion
line present but never reached would look exactly like one that is absent. The tool therefore logs **every
raw line** under the slots after landing, plus what Parse picked, so one cycle separates those two. It is
an instrument: it schedules nothing.

### The trap: "look again in 20 minutes" is not a look

**A row that comes due is RELOADED** — end boarding, re-place, load food, start. So "check again sooner"
is **a reload every N minutes**, and every reload **consumes a food cell** from the pool (2 on the free
row, 5 on a paid one; 17 cells per full round). Probing on a short interval drains the bag roughly ten
times faster, and **running out of cells is the one outcome that leaves a pet unboarded** — trading the
recoverable failure for the unrecoverable one.

So if a probe is ever wanted it must be **read-only**: open, read, close — no ending, no placement, no
food. That is the shape of `InspectRows`, which already does it at run start. Not built, and now less
necessary: §5 makes the first estimate right, and §6's measurement shows the line corrects a row at the
moment it matters.

---

## 7. Why Row 1 read nothing, and the fix that is really about seeing — 2026-09-23

**The symptom.** Row 1 returned `[no time line read]` on **four consecutive visits** while all three
neighbours read their lines on those same visits. Its next was therefore pure food arithmetic — `219
items → 62 min` — and its pet finished in 7 minutes. The tool would have noticed about 70 minutes late.

**The cause: a 7-pixel strip error.** Rows 2–4 sit exactly **189 px** apart; Row 1 sat **196** above Row
2. At the uniform pitch its strip should have been at `y = 267`, not `260`. That one number explains the
whole pattern:

| crop | how it is read | size | survives a 7 px strip error? |
|---|---|---|---|
| the feeding counts | inside the slot box | ~60 px tall | **yes** — that is why counts kept reading |
| the time line | `EtaRegion`, derived *below* the strip | `h × 0.40` = **24 px** for a ~10 px line | **no** — and a crop that misses returns NOTHING |

Changing `y` to `267` fixed it on the next look: `[代完成预計所需時間：約5分]`, and the row went from
**62 min to 10**. So the read logic was never wrong; the region was aimed 7 px high, and **nothing on
screen said where the region had landed.** The one broken crop was the one crop that could not be seen.

**Built: draw it.** `PetRedrawOverlay` now draws the time-line region for **every** row (faint for rows
not being edited, like the strips), in cyan, and `RefreshPetGeometry` lists the pixels it resolves to
beside each row's slot/crop columns — the strip *is* the setting, and the readout is what it produces.
This is the same argument the magenta count line already makes in its own comment: *"the read position is
a NUMBER and nothing on this screen said what that number MEANT: a line drawn where it lands turns 0.40
into something you can see."*

**No new config field, deliberately.** `EtaRegion` is a pure function of the row's strip, so the existing
per-row geometry pane is already the control — and Row 1 proves the lever works: nudging `y` fixed it.
Adding a drawn box or an offset field would mean a field in **both** directions of the projection, which
has silently dropped one three times. A hand-drawn box was also considered and rejected on precedent: the
count crop's drawn box was tried and abandoned because its working band measured **three pixels** wide.
The time line is the opposite case — `Parse` pattern-matches and ignores the expiry line, so slack is
free — but visibility plus the existing numbers gets the same result with no new surface at all.

**Still open from this:** the retry belongs in `ReadRows`, not `Visit` (the run-start look — where Row 1
kept failing — calls `ReadRows` directly and has no retry at all), and `EtaRegion` could carry vertical
margin so the *derived* region is forgiving too rather than demanding a pixel-exact strip.

---

## Order, and why

| | scope | size | risk | why here |
|---|---|---|---|---|
| 1 | name the line on existing entries — **built, unverified** | small | low | switches on work already paid for; inert without it |
| 1b | the return slot's species (§5) — **built, unverified** | small | low | the same work, on the path the tool tries *first* |
| — | a read-only "look" instead of a reload (§6) — **not needed yet, and not proposed as a reload** | medium | medium | would trade a recoverable failure for an unrecoverable one |
| 2 | the intermittent slot read | medium | medium | armed with evidence; the last live gap on the food path |
| 3 | cursor placement failures | unknown | — | biggest cost, no cause — instrument first |
| 4 | retry policy | small | low | needs a decision, not a design |

Each stands alone. **1** is the one that makes yesterday's work do anything; **3** is the one that would
repay the most if it were understood.
