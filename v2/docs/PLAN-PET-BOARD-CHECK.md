# Plan — finding the feedable pets in the bag

**Goal.** The bag holds a mix of pets: some at **+9 / 100%**, which cannot be fed any more, and some at
**+0**, which need food. The tool has to say which is which.

**Why it matters more than "wasted food".** Boarding a pet that is already finished raises an **error
dialog**, and a blocking modal wedges the rest of the run — everything after it is clicking into a
dialog. So a finished pet is not a wasted reload, it is a stuck run.

---

## What already exists

Nothing here is new machinery. The pieces have been sitting unused since 2026-09-18:

| Piece | Where |
|---|---|
| the rule | `PetPanel.IsFinished` — `Growth == 9 && Exp >= 100`, tested in `PetPanelTests` |
| the parser | `PetPanel.Parse(lines)`, reads growth from `+N` and EXP from the panel's only `%` |
| the hover | `FocusThenHover` — a MOVE, never a click (a click on a bag pet switches the equipped pet) |
| the tooltip region | calibrated: cell centre + `offset_x/offset_y`, `width × height`, `hover_delay_ms` |
| the cells | `BagGrid.Centres(pet.BagGrid)` — 64 per page |
| the pages | `pet.PageTabs`, already calibrated |
| the read | `_service.ReadText(region, 3)` over a hidden launcher |

`PetPanel.Parse` is called from exactly one place today — the marked-pet diagnostic — and
`IsFinished` from nowhere in the run at all.

## Design

**Find with the matcher that already exists, then hover only what it found.**

`IconMatch.ScoreAll(bag, grid, icon)` already answers "which bag cell holds this pet" — one capture per
page, matched against the stored queue icon, no hovering. The run uses it before every boarding, and
"Scan the bag for these pets" already exposes it on the same tab. **There is deliberately only one
answer in this codebase to "where is this pet"**: an earlier revision of this plan added a second one
(a pixel detector that decided which cells looked occupied), it was a duplicate of a working
mechanism, and it came back out.

So, for each calibrated page:

1. switch to the page, park the cursor off the bag, and capture the client **once**;
2. `ScoreAll` each queued icon against that capture, and keep the best cell per icon under
   `PetTool.MatchLimit` — the run's own limit;
3. **hover only those cells** (a MOVE — never a click), wait `hover_delay_ms`, OCR the calibrated
   tooltip region, and `PetPanel.Parse` it.

Then classify:

- **finished** — parsed, and `IsFinished`;
- **feedable** — parsed, and not finished (a +0 pet renders no `+N` at all and the parser reads the
  missing growth as 0, which is measured, not assumed);
- **matched but unreadable** — `Parse` returned null on a cell the matcher found. Reported as neither:
  it is never called finished, because skipping a pet that needed feeding cannot be undone.

Report a line per pet plus a total, and save nothing unless the queue is being rebuilt.

## Cost

A handful of hovers per page — the pets in the queue, not 64 cells. Seconds, not minutes. The launcher
hides **once per page** rather than once per hover, and the capture is taken before any hovering
because the tooltip follows the cursor and would lie over the neighbouring cells.

## Scope, stated plainly

This reports on **pets already in the queue**. It cannot discover a pet that has never been captured —
that needs its icon, which is the manual "Mark a pet to queue" step. Finding unknown pets was the
premise of the duplicate detector, and it came back out with it.

## The failure direction — the part that matters

**A read that fails must not answer "finished".** Skipping a pet that needed feeding is the one
outcome here that cannot be undone, and it is the same rule the parser already follows: `IsFinished`
is `==` and not `>=` precisely so a garbage read does not stop the tool feeding a pet that still needs
it. A null panel, an empty read or a low-confidence one is **not finished** — it is "unknown", and the
report says so rather than guessing.

This is a report, so it is read by a person. **The guard comes later, and only once this has been
seen working on live pets** — the same gate the marked-pet diagnostic was waiting behind.

## Not in v1

- **Queueing the feedable pets.** It would make the run board exactly those, and it is the thing that
  actually prevents the dialog — but it needs the scan trustworthy first.
- **The run-side guard.** Skipping a finished pet mid-reload, and the blind-`Enter` dismissal the open
  already uses for its out-of-food message. Unverified for *this* dialog: Enter may confirm it rather
  than dismiss it, and that wants one live check.
