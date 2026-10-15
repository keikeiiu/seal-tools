# v2.11 — the Pet Feeder grows up

One row becomes four, the feeder learns to read how much food is left, and it stops being the tool that
anything else kills. Plus two fixes that reach past the pet tool.

**A reflash is optional and nothing here needs it.** Two things wait on firmware 2 — loading food by
drag, and the board reporting its own version — and both fall back to today's behaviour until you flash.
Everything else is live the moment you launch this.

Everything below was verified on the live game on 2026-09-20/21 unless it says otherwise, and the
sections that were not say so plainly.

---

## The Pet Feeder drives four rows

**The boarding window has one free row and three paid ones**, and each is independent: its own
start/end button, its own pet slot, its own food boxes, and — the number that decides everything —
**its own capacity**. The free row holds two food stacks and a paid row five.

That difference is why the schedule is **per row** rather than one clock. Two stacks last 200 minutes
and five last 500, so a single timer would reload the paid rows while they were still half full, or
leave the free row dry for five hours.

Rows are a **list** in the config now rather than four sets of fields, with a migration for a file
written before they existed — because the loader ignores keys it does not recognise, so an old
`local.yaml` does not fail on upgrade, it silently loses every setting nothing maps to any more.

### A pet is found by its icon, not by a position

A returning pet lands in the **first free bag slot**, and the character farms in between, so the cell it
was taken from means nothing by the time it comes back. The queue is a captured crop of the pet's own
portrait, matched across the bag's 64 cells by `IconMatch` — which is where the offset search and the
`MatchLimit` of 0.5 come from: measured against the player's real bag, their pets scored **0.000–0.150**
and everything else **0.831 and up**, and the old limit of 0.12 was throwing away four pets a scan.

**The cursor is parked off the bag before every capture.** The scan reads the screen, so a portrait with
the mouse pointer over it differs at every offset — which is exactly how a pet got written up as "a
genuinely different pet" before the player identified the culprit.

---

## It reads how much food is left

**The reload is scheduled from the number of items in the feeder**, read off the game, rather than from
a timer that assumes a full load. Measured live:

```
feeder counts: [81 / 300] -> 381 (2 of 2 slots read)
Row 1: 381 item(s) left in the feeder — reloading in 132 min
```

which is `381 / 3 per minute + 5 minutes after empty`. When the count cannot be read the tool falls back
to the configured cycle **and says which it did**, so a fallback cannot pass for a reading.

**Reading it took five designs and four of them were wrong**, and what is in here is the one that held:
**40% across each slot to its own right edge, at the slot's full height**, computed from the slot. Two
things about that were measured and are not preferences:

- **A crop cut to the digits' height finds NOTHING** while holding a perfectly legible number — three
  separate crops, all legible, all zero detected boxes. *The mechanism is not known.* The full slot
  height is a measured rule with no explanation, and it is written down as one.
- **The band of read positions that works is about three pixels wide.** Outside it, one direction finds
  nothing and the other returns a **confidently wrong number** — a clipped `138` came back as `3` at
  0.99, the same confidence as a correct read.

Three attempts to let that position be **drawn** — a count box, a count slot read as drawn, and a count
slot applied by offset — all failed, because a hand cannot put a box into three pixels. `Count crop
starts at` is a setting on Calibrate Pet rather than a constant, so a machine that reads nothing, or
reads a clipped number, can move it without a rebuild.

---

## Calibrating a row is four drags

**One strip per row**, divided by that row's stack count into its slots. That replaced fourteen
hand-drawn boxes, and it is the whole calibration.

Three things on Calibrate Pet exist because of what went wrong on the way:

- **The magenta line** on the capture is where the read starts, so `0.40` stops being an abstract
  number and a line through the food icon is visible rather than inferred.
- **Row geometry — nudged by typing** shows each row's strip as four editable numbers with its slots
  and crop positions beside them. A capture cannot answer *"is this row a pixel out from its
  neighbours?"*, and that was a real failure: row 2's crops started 1–2px right of row 3's and its
  widest number ran past the crop edge.
- **Test read moved here**, reads every ticked row in one press, and writes a table under its own
  button. It used to sit on the Pet tab while reading the row selected on Calibrate Pet — state you
  cannot see from where the button is.

**An empty slot reading nothing is the correct answer**, and the table shows the raw reading beside the
number so the two are distinguishable: a slot with no food and a slot whose crop clipped both come back
as "nothing" to the program.

---

## The feeder is resident

**Starting another tool no longer stops the feeding.** It holds nothing between reloads — the game is
needed for about thirty seconds, five times a day — so it is the tool that waits, and everything else
leaves it alone. A `PortGate` in `Core` makes "the game is free" and "take it" one operation, because
the answer can change between the two.

- **A reload that comes due while another tool runs waits**, and says so on its card, including how long
  before the row's food runs out.
- **A Start pressed while the feeder holds the game waits** — bounded, and reported — rather than
  interrupting a reload, which is the one place a half-finished sequence leaves a pet unfed.
- **The Quit hotkey stops the foreground tool only.** It is read from global OS key state, so with two
  loops alive one press used to stop both — quitting a buy run would have ended a schedule that is
  feeding four pets.
- **Two ordinary tools still replace each other**, exactly as before. Only the pet is exempt.

The feeder's card also carries a **standing line** — which rows are boarding and when the next reload is
due, counting down — because a reload is hours apart and a card that only reported the last event said
nothing at all for most of a run. And mini mode keeps **every** running tool's card, so the feeder's
card stays visible while something else runs.

---

## Hold Space — the toggle no longer inverts

**Reported from the second PC: the toggle would not stop it and the spacebar stayed held.** Not a
refusal — an inversion. The guard asked `CurrentId == "holdspace"` **and** `Running == true`, and when a
release has failed `Running` is already false while the key is still down, so the press fell to the
`else`, which **starts hold space again**.

The guard now asks only whether hold space is the loaded tool. And the fix that matters is in the
service: **`StopTool` releases on every stop path** — the card's Stop, the toggle, another tool's Start,
Quit, and shutdown — rather than the toggle being the only one that could. A failed release is reported
on the status line instead of being swallowed.

**Not verified live** — the toggle and the status line are a compile check, as the launcher's window is
not reachable from the test project.

---

## Firmware — the board can be asked its version

Until now **every command went one way**, so "did the board ignore that, or did it act and nothing
happened?" had no answer: both look like nothing. The sketch now writes back, and `V` reports a
protocol level.

**The valuable half is the silence.** The sketch that predates `V` writes nothing at all, so a board
that stays quiet is a definite **no** rather than a failed read — which is also what makes the launcher
safe to ship before you flash: sending `V` to an old board costs nothing and produces exactly that
silence. The Arduino tab now shows what the board said, and the pet run logs it as its first line.

**Requires a reflash**, and a board without `V` cannot report anything, including its own age — so it
does not retroactively answer anything. It makes the *next* flash verifiable.

### And food can be loaded by drag

A right-click drops a food stack into the **earliest empty box** in the boarding window, and every row's
boxes are one queue ordered top-down — so a stack meant for a lower row lands in an upper row's box
whenever that row has run dry. **The tool never noticed, because it has never looked at where the food
went**; it counts the clicks it sent.

`Food load` on the Pet tab picks between **right-click** (the default, works on every board ever
flashed) and **drag**, which names the row's own box. Drag needs firmware 2, and because an old board
*ignores* those letters the tool **refuses to start** a drag-mode feeder on a board that cannot drag,
rather than running one that silently loads nothing.

**Not verified live** — neither the drag nor `V` has run on a board. Both wait on the flash, and both
default to today's behaviour until then.

---

## Fixes that reach past the pet tool

- **A capture reads the SCREEN**, and three things now refuse or correct for it: a foreground guard on
  the feeder read and the bag scan, parking the cursor off the bag, and the Test read refusing to judge
  a capture that is not the game. A diagnostic that reported `天` at 1.00 confidence from a white window
  proved that last one was needed.
- **The window no longer belongs to a single tool.** Mini mode keeps every running tool's card; the
  resident feeder does not shrink the window on its own, because it runs for days.
- **`MatchLimit` is 0.5**, from the measurement above, rather than 0.12 inherited from the composer's
  empty-box check.

---

## Known, not fixed

- **One pet reads low** — 0.320 against the crops, where its neighbours are 0.000. It counts now, so it
  is not urgent, and nobody has confirmed whether it is a rendering difference or something in the way.
- **~57 food cells a day** across four rows against a 64-cell bag holding the pets. Restocking and
  re-marking is a daily chore, and a reload that runs out of cells is the one that leaves a pet
  unboarded.
- **The tuner stays a manual schedule.** Its window and the boarding window cannot both be open, and
  the tuner's must be closed first — so it is out of scope for residency, deliberately.
- **`text_fixes` in `attributes.yaml` still wants filling** from the OCR logs.
