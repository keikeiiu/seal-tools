# The bag sweep — once per visit, with every stored cell re-read before it is used

> **ARCHIVED 2026-10-16.** Shipped in v2.15 (2026-10-13): the bag sweep, once per visit, with every stored cell re-read before it is used (PROGRESS 74). A design record for PLAN-PET-FLOW.md's step 3.
>
> It lives here with the plan it belongs to, and is kept for its REASONING — a design decision
> without its reasoning gets reversed by whoever finds it inconvenient next. How it works now is
> [DESIGN.md](../DESIGN.md); what is still open is [TODO.md](../TODO.md).

Design record, 2026-10-11, branch `feature/pet-flow`. Step 3 of
[PLAN-PET-FLOW.md](PLAN-PET-FLOW.md) §8 ("one scan per visit, with the per-use re-verification" —
§1.2 and §6.3). Two structures were designed independently and judged adversarially; the judge chose
the one below and named six amendments, all applied.

**Built:** the sweep is taken by a visit's FIRST PLACEMENT and shared; every stored cell is re-read
before it is clicked; a row whose own boarding just ended may run one fallback sweep. Commit with
this record.

---

## Why the sweep moves at all

The sweep (`FindQueuedPetCandidates`) does two jobs on one set of page captures: it matches the queued
pet icons, and it re-marks the food cells. On 2026-10-11 it lived inside `PlacePet`, so **a plural
batch paid one full three-page sweep per row** — every restart after downtime puts several rows past
due at once, which is the visit with the most rows and the longest work. The plan's §1.2 calls the bag
*visit-level state*, and it is: it belongs to the window that is open, not to the row being reloaded.

## Where it runs, and why not where the plan's diagram puts it

PLAN-PET-FLOW §5's target flow shows the sweep at visit scope, before the row loop. **This record
deviates deliberately, on evidence.** At visit scope the sweep runs *before any row has ended its
boarding* — and a boarding row's pet is in the LOADER until its own end press, so the sweep would look
at a bag that provably does not contain the pet every one of those rows needs. Its saving would then
depend on the fixed return slot, which `pet.log` measures as empty **173 of 258 tries (67%), and
~100% recently** (271 scan-sourced placements against 26 return-cell boards) — or on a refresh that
re-runs the sweep after the end press anyway, which is the placement this record uses directly.

So the sweep is taken inside `PlacePet`, by the visit's FIRST placement — for a boarding first row,
*after its own end press*, so the list includes the pet that just came back and any spare the bag
holds. Consequences:

- **The ordinary single-row reload keeps the sweep exactly where it has always been.** PLAN-RELOAD-VISIT
  §4's "the single-row case must behave exactly as today" holds literally, not by argument.
- **A plural batch pays at most today's cost**: one shared sweep, plus at most one fallback per row.
- **The food re-mark happens exactly ONCE per visit and is never skipped** on a visit that acts —
  fallback sweeps carry no food write (`remarkFood: false`). This is the amendment the judges demanded:
  §6.2's invariant is about the re-mark happening, and a second write from a fallback would reset a
  counter the other rows are still counting against. The re-mark also now runs for a setup with **no
  pet queue but a food icon**, which previously got no re-mark at all — a latent §6.2 hole.

## The per-use verification (§6.3)

The sharing saves the SWEEP, never the CHECK. Two new Core-supported single-cell reads, both
`bool?` — **match / positive mismatch / unknown** — and unknown never removes anything:

| what | asked | primitive |
|---|---|---|
| a stored PET cell | "does this cell still hold any queued pet?" | `CellHoldsQueuedPet` → `CaptureBagCell(centre, SearchRadius)` + `IconMatch.ScoreCell` against every queue icon, judged at `MatchLimit` |
| a marked FOOD cell | "does this cell still hold food?" | `CellHoldsFood` → the same, against the food icon |

`IconMatch.ScoreCell` is the sweep's own comparison (same inset, tolerance and offset search) applied
to one cell, so verify and sweep cannot disagree about what a match is. The crop carries the search
radius on every side, because a one-cell crop cannot host the ±3 px search the sweep ran — a shifted
sprite would otherwise read as "the pet is gone". A **positive mismatch** skips that cell and says so
in `pet.log`; an **unreadable** one (no icon, failed grab) carries on exactly as before.

## The fallback sweep, at most one per row

A boarding row's pet is not in the visit's sweep (it was in the loader then). The walk handles that
three ways: the return slot first (unchanged), then the shared list with verification, and — if the
list yields NOTHING USABLE and this row's boarding just ended — **one fallback sweep**, taken after
its own end press, whose list replaces the shared one for the rows after it. It fires only on
exhaustion, never on a hard failure: "the pet did not go in after 3 right-clicks" is today's failure
and no sweep fixes it. Bound: one fallback per row ⇒ at most N sweeps per visit, today's bound.

## What the logs now show

`the visit's bag sweep: N queued pet candidate(s), shared by every row of this visit`, `the visit's
bag sweep was already taken — N candidate(s) to re-verify before they are used`, a line per
verification skip (page, cell, score), and `the shared list had nothing to board and this row's own
pet came back after it was swept — sweeping once more for this row`. The `icon scan:` count in
`pet.log` changes meaning (per visit, not per row) — noted so a future baseline knows.

## Verified, and owed

268 tests pass, with three new `IconMatchTests` pinning the single-cell verification's three outcomes;
Pet + Tests build clean. **Owed: a live run**, and the plan's own gate for this step — the bag
compacting at least once to prove the re-verify rule — which is also what would show whether the
fallback fires as often as this record assumes.
