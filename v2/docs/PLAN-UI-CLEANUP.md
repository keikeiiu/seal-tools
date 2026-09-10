# Plan — UI cleanup (tabs, buttons, text-box layout) — STUDY, not started

Planning note, 2026-09-10 (revised with the answers so far). The goal is a launcher that reads
cleanly: each tab doing one job, controls grouped by intent, one row builder instead of hand-rolled
grids, and buttons whose labels say what they do.

## Answers so far

| question | answer |
|---|---|
| `Settings` tab | rename to **`Hotkeys`**; `Arduino` keeps its name |
| Advanced toggle | **decide per tab, as we work through them** — no global rule up front |
| structure (tabs vs collapsible sections) | **pending this document** — see the grouping below |
| XAML vs code-built | **pending** — see the trade-off section |

## Progress

Updated in the same commit as each tab. ✔ = done, ▸ = in progress, ☐ = not started.

| # | Tab / step | State | Commit |
|---|---|---|---|
| 1 | `Settings` → **`Hotkeys`** (rename + focus caveat + guides) | ✔ | `b43a6d1` |
| 2 | `Arduino` — split into **Connection** / **Input test**, own result line, button renamed | ✔ | *(this commit)* |
| 3 | `Setup` (display environment / save) | ☐ | |
| 4 | `Gem` settings (run / advanced) | ☐ | |
| 5 | `Spammer` (preset / keys / save) | ☐ | |
| 6 | `Attributes` (dictionary / save) | ☐ | |
| 7 | `Tuner` settings (goal / timing / filter rules / overrides / save) | ☐ | |
| 8 | `Calibrate Tuner` (capture / steps / save) | ☐ | |
| 9 | `Calibrate Gem` — split into Capture / Points / Tests / Moves / Full-run / Advanced | ☐ | |
| 10 | Shared row builder + style constants (introduced with the first tab that needs it) | ☐ | |
| 11 | Button label/weight convention applied across the remaining tabs | ☐ | |

## What exists today (inventory)

Nine tabs, all built imperatively in `MainWindow.xaml.cs` (~2,700 lines; ~900 are UI):

| Tab | Holds |
|---|---|
| **Arduino** | port list, expected VID/PID, Refresh, "Test Click (C)" + status light |
| **Setup** | display environment (scale, client size), Detect, Save; client-size mismatch warning |
| **Tuner** | target grade, require grade, max retries, two delays, save-captures, filter enable/match-mode, rules grid, override grid, Save |
| **Gem** | start grade, on-empty-result mode, save-empty-captures, Save |
| **Spammer** | preset picker + name/New/Rename/Delete, key/cooldown rows + Add Key, Save |
| **Attributes** | OCR dictionary table (+ variants), Add, Save |
| **Calibrate Tuner** | instructions, Capture, canvas, three drag steps, Save |
| **Calibrate Gem** | capture + diagnose, coordinate grid (X/Y/W/H), from/to point tests, Test Click / Test Move, Check Result Colour, Test Result Gem, two Debug Cursor buttons, **tuned moves grid (11 rows)**, Save Composer Moves, **mode combo + arduino moves grid (11 rows)**, Test Full Cycle, Save Gem Composer |
| **Settings** | four hotkey boxes, Save |

## Proposed grouping, tab by tab

Sections are collapsible groups inside the tab (implementation choice still open — see below).
**⚑** marks a candidate for that tab's Advanced decision.

### Arduino — *no change beyond spacing*
1. **Connection** — port list, expected VID/PID, Refresh.
2. **Input test** — Test Click (C), status light/text.

### Hotkeys (was Settings)
1. **Keys** — start/stop, quit, advance grade, pause; Save Hotkeys.
2. The "hotkeys need the launcher focused" caveat becomes a hint line under the fields, not just the guide.

### Setup
1. **Display environment** — scale, client size, Detect, the mismatch warning.
2. **Save** — Save Setup.

### Gem (settings)
1. **Run** — start grade, on-empty-result; Save Gem Config.
2. ⚑ **Advanced** — save-empty-captures (debug only).

### Tuner (settings)
1. **Goal** — target grade, require grade, max retries.
2. **Timing** — click→enter delay, OCR delay.
3. **Filter: rules (main goal)** — enabled, match mode, rules grid, Add Rule.
4. **Filter: overrides (stop immediately)** — override grid, Add Override.
5. ⚑ **Advanced** — save captures.
6. **Save** — Save Tuner (primary).

### Spammer
1. **Preset** — picker, name box, New / Rename / Delete.
2. **Keys** — key/cooldown rows, Add Key.
3. **Save**.

### Attributes
1. **Dictionary** — the table + variants, Add.
2. **Save**.

### Calibrate Tuner
1. **Capture** — Capture, canvas.
2. **Steps** — the three drags, with the step indicator.
3. **Save**.

### Calibrate Gem — the one that needs the split
1. **Capture** — Capture gem window. ⚑ Diagnose capture.
2. **Points** — the coordinate grid, Save Coordinates. ⚑ the raw X/Y/W/H grid (the drag workflow sets these; typing them is the fallback).
3. **Tests (read-only)** — from/to + Test Click, Test Move (rel), Check Result Colour, Test Result Gem. *Nothing here changes config or clicks in the game.*
4. **Moves** — the mode combo, then the **active** set's grid (the other one collapsed under "show the other set"), and Save Composer Moves / Save Gem Composer.
5. **Full-run test** — Test Full Cycle, visually distinct (it makes 21 clicks in the game).
6. ⚑ **Advanced** — Debug Cursor (logical / physical), Diagnose capture, the raw coordinate grid.

### Button labels and weight (applies to every tab)

| today | proposed |
|---|---|
| `Test` (tuned move row) | `Send this move` |
| `Test` (arduino move row) | `Run this route` |
| `Test Click` | `Place cursor + click` |
| `Test Move (rel)` | `Test tuned move` |
| `Test Full Cycle (Arduino)` | `Run one full cycle` |
| `Save Composer Moves` | `Save tuned counts` |

Weight: **Primary** = the tab's save/commit action. **Secondary** = read-only tests. **Caution** =
anything that clicks repeatedly in the game (Run one full cycle), ideally with a confirm.

## XAML vs code-built — the trade-off

**What XAML buys**

1. **Styles and templates.** Spacing, label alignment and field widths get defined **once** in a
   `ResourceDictionary`; today every grid sets its own `ColGap`/`RowGap`/`76 - ColGap`, which *is*
   the inconsistency being complained about.
2. **Bindings.** Two-way binding to config would delete most of the per-tab Save handlers (each one
   currently reads its controls by hand and writes `AppConfig`).
3. **Reusable controls.** One `MoveRow` control shared by both move grids, one `PointRow` for the
   coordinate grid.
4. **Structure is readable and diffable** without reading C#.

**What it costs**

1. A rewrite of ~900 lines of *working* UI, including the stateful bits (capture → drag steps → save).
2. The dynamic parts (config-driven rows, calibration flows) stay in code — so it's two places to
   look, not one.
3. Bindings need view models; today the code reads/writes `AppConfig` directly. That is an MVVM
   refactor on top of a layout change.
4. No visual designer in this environment, so XAML's editing advantage is smaller than usual.

**Conclusion: stay code-built for this cleanup**, but adopt the two ideas that actually fix the
complaint — a **single row builder** and **shared style constants** — plus a small `Field(...)`
helper that returns a control already wired to a config get/set, so Save handlers shrink the same way
bindings would. XAML stays available later: a tab can be rewritten as a XAML `UserControl` hosted in
the existing code-built shell, so this decision is not irreversible.

## Method (unchanged)

1. Work **one tab per commit**, smallest first — `Hotkeys`, `Arduino`, `Setup`, `Gem`, then the
   bigger ones — so each diff is reviewable and revertable alone.
2. Pure moves/renames first; the shared row builder second (introduced with the first tab that needs
   it, then applied as tabs are touched); Advanced toggles last, decided per tab.
3. Screenshot before/after per tab into `logs/captures/ui/`, and update the matching
   [USER_GUIDE.md](USER_GUIDE.md) rows in the same commit — button names must match the guide.
4. No behaviour changes, no config keys renamed, no calibration flow reordered.

## Non-goals

- No theme/branding work, no new colours or fonts beyond WPF-UI's.
- No new tabs for planned features ([PLAN-TUNER-SPRING.md](PLAN-TUNER-SPRING.md) has its own plan).
