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

A **"Scan bag for feedable pets"** button. For each calibrated page, for each cell in the grid:

1. move the cursor onto the cell (a MOVE — never a click);
2. wait `hover_delay_ms` for the game's tooltip;
3. OCR the calibrated tooltip region;
4. `PetPanel.Parse` it.

Then classify:

- **finished** — parsed, and `IsFinished`;
- **feedable** — parsed, and not finished (this is where a +0 pet lands; a +0 renders no `+N` at all
  and the parser reads the missing growth as 0, which is measured, not assumed);
- **not a pet** — `Parse` returned null. Empty cells and food items land here, which is exactly why
  the parser is the filter: the icon matcher only knows pets already queued, and the +0 ones are
  precisely the ones not known yet.

Report a line per pet plus a total, and save nothing.

## Cost

About **1.5 s per cell** (hover delay + OCR), so ~90 s per page and ~4½ min for three. It is a
deliberate button, not something a run does. The launcher hides **once per page** rather than once per
cell: the per-crop hide flashes the window and pays a 300 ms compositor wait each time, which over 192
cells would cost a minute of pure flicker for nothing.

Rejected for v1: an empty-cell pixel skip (the trick the boarding slot uses). It would cut most of the
time, but it needs an empty-cell reference crop, and there is no reason to add a calibration before
measuring whether the plain scan is fast enough.

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
