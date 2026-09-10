# Plan — UI cleanup (tabs, buttons, text-box layout) — STUDY FIRST, not started

Planning note, 2026-09-10. The goal is a launcher that reads cleanly: fewer "what does this button
do?" moments, consistent controls, and each tab doing one job. **No code is written until the
structure below is agreed** — layout churn is the most expensive kind of work to redo, and the current
UI grew feature by feature.

## What exists today (inventory)

Nine tabs, built imperatively in `MainWindow.xaml.cs` (~2,700 lines, of which ~900 are UI):

| Tab | Holds | Notes |
|---|---|---|
| **Arduino** | port list + "Test Click (C)" + Refresh | diagnostics; a whole tab for one action |
| **Setup** | display environment (scale/client), Detect, Save | fine |
| **Tuner** | target grade, max retries, two delays, filter rules + override rules editors | two rule grids, each with an "+ Add" |
| **Gem** | start grade, empty mode/streak, colour gap, save captures | fine, short |
| **Spammer** | preset picker + name/rename/delete, key/cooldown grid | preset management mixed with the grid |
| **Attributes** | OCR dictionary table + variants | fine |
| **Calibrate Tuner** | 3-step drag flow + Save | fine |
| **Calibrate Gem** | capture + diagnose, coordinate grid (X/Y/W/H), from/to point tests, Test Click / Test Move, result-colour + result-gem tests, two Debug Cursor buttons, **tuned moves grid (11 rows × dx/dy/Test)**, Save Composer Moves, **mode combo + New Gem Composer Moves grid (11 rows × Test)**, Test Full Cycle | the problem child — see below |
| **Settings** | four hotkey boxes + Save | named "Settings" but only holds hotkeys |

## Concrete problems (observed, not guessed)

1. **Calibrate Gem is five jobs in one scroll.** Capture/diagnose, coordinates, point tests, tuned
   moves, arduino moves, full-cycle. Reaching the bottom means scrolling past everything else.
2. **Two grids look identical but mean different things.** "Composer moves" (dx/dy + Test) and "New
   Gem Composer Moves" (destination + Test) sit one above the other with the same "Test" label; only
   the heading distinguishes them, and "New" will age badly.
3. **Same-looking buttons, different risk.** "Test" (moves the cursor), "Save" (writes config),
   "Test Full Cycle" (21 clicks in the game) and "Diagnose capture" (writes a PNG) all look like
   equally weighted secondary buttons.
4. **No grouping by intent.** Safe read-only tests, config writers, and things that act in the game
   are interleaved.
5. **Layout is hand-rolled.** Each grid sets its own column widths and margins (`ColGap`, `RowGap`,
   `76 - ColGap` boxes); the same "label + box + button" pattern is rebuilt several times.
6. **Tab names don't pair up.** `Tuner` (config) vs `Calibrate Tuner`; `Gem` (config) vs
   `Calibrate Gem`; `Settings` holds only hotkeys; `Arduino` is really "connection status".

## Principles to agree before writing any code

- **One tab, one job**, and a tab's name should say which job. Candidate: rename `Tuner`/`Gem` to
  `Tuner settings`/`Gem settings`, or merge the two calibration tabs into one `Calibrate` tab with a
  tool selector.
- **Group by intent, in this order**: *read-only tests* → *what this tab changes* → *actions that
  touch the game* (with the heaviest last and visually distinct).
- **Progressive disclosure**: advanced/diagnostic controls (Debug Cursor buttons, the tuned-move
  editor when `move_mode` is `arduino`, the coordinate X/Y/W/H grid) sit behind an **Advanced**
  toggle, collapsed by default.
- **One row builder.** A single helper for `label + control(s) + optional button`, so every grid
  shares the same column rhythm and spacing.
- **Say what a button does.** "Test" becomes "Place cursor", "Move there", "Run one cycle"; saves say
  what file they write.
- **No behaviour changes.** This is layout, naming and grouping only — the wiring stays, so a diff
  can be read as "moved" rather than "changed".

## Open questions (need your call before coding)

1. **Split or collapse?** Do you want more tabs (e.g. `Calibrate Gem` split into *Points* / *Moves*),
   or fewer tabs with **Expander** sections inside?
2. **How much behind Advanced?** Is the tuned dx/dy editor allowed to hide when the mode is
   `arduino` (it does nothing then), or must it always be visible?
3. **Settings naming.** Rename `Settings` → `Hotkeys`, or keep it as the home for future options?
4. **Diagnostics.** The Arduino tab, Debug Cursor buttons, and Diagnose capture are one-off
   diagnostics — keep them visible, hide them, or move them to a single `Diagnostics` tab?
5. **WPF-UI components.** The app already depends on WPF-UI (FluentWindow, Button, ComboBox). Are
   `Expander` / `Card` / `InfoBar` acceptable, or should everything stay a plain `StackPanel`?

## Method (so the effort isn't wasted)

1. Answer the five questions above; freeze the target structure in this doc.
2. Do it **one tab per commit**, smallest first (`Settings`, `Arduino`, `Setup`, `Gem`), so each
   diff is reviewable and revertable on its own.
3. Pure moves/renames first, the row-builder refactor second, Advanced toggles last.
4. Screenshot before/after per tab into `logs/captures/ui/` and update the relevant
   [USER_GUIDE.md](USER_GUIDE.md) rows in the same commit — the guide's button names must match.
5. Only after the layout is settled, revisit the deferred polish items (launcher parking, the tuned
   editor, Test Full Cycle) so they land on the new structure rather than the old one.

## Non-goals

- No theme/branding work, no new colours or fonts beyond what WPF-UI already provides.
- No change to what any button does, no config keys renamed, no calibration flow reordered.
- No new tabs for planned features (the tuner spring work has its own plan:
  [PLAN-TUNER-SPRING.md](PLAN-TUNER-SPRING.md)).
