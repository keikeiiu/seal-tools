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

## Direction (decided 2026-09-10)

**Adopt WPF-UI properly, tab by tab.** The project already references WPF-UI 4.3 and merges its theme
dictionaries, but only ever used `FluentWindow`, `ui:TitleBar` and `ui:Button` — every tab is plain
WPF with a second, hard-coded palette beside the theme. That is why the UI reads as generic WPF.

| | today | with WPF-UI |
|---|---|---|
| Navigation | `TabControl`, nine tabs across the top | `ui:NavigationView` — left rail with icons, one content pane |
| Sections | bare `TextBlock` heading | `ui:Card` (nothing hidden — still "headings only") |
| Inputs | `TextBox` / `ComboBox` / `CheckBox` | `ui:TextBox` / `ui:ComboBox` / `ui:ToggleSwitch` — consistent padding and focus rings, so far fewer hand-set margins |
| Messages | `MessageBox.Show` inside save handlers | `ui:InfoBar` / `ui:ContentDialog` |
| Colour | eight hex brushes duplicating the theme | semantic theme brushes → theme-aware, light mode becomes one line |

**What this changes about the plan:** WPF-UI's `Card` replaces the section-heading helper, and its
controls carry the spacing the shared row builder was going to fix — so this **subsumes** steps 10–11
below rather than following them. The information-architecture work is unaffected: the grouping
(Capture / Points / Tests / Moves / Advanced), the renames and the `Hotkeys` rename all still apply,
because those are decisions about *what goes where*, not about which control draws it.

The two tabs already rebuilt (`Hotkeys`, `Arduino`) are marked for redo below — they are the two
smallest, so the cost is minutes, but doing the cleanup twice on every tab is exactly what the
sequencing existed to avoid.

### Two gotchas found while rebuilding the first tab (apply to every later tab)

1. **Use `ui:Card`, not `ui:CardControl`.** `CardControl`'s template measures its content with
   unbounded width, so a hint paragraph never wraps — it runs past the border and gets clipped.
   `Card` is a bordered `ContentControl`; put our own header `TextBlock` inside it and the width is
   constrained as expected.
2. **Tabs' `ScrollViewer` needs `HorizontalScrollBarVisibility = Disabled`.** With horizontal
   scrolling available the content is measured with infinite width, which is the other way to make
   every `TextWrapping.Wrap` hint silently stop wrapping. Applied to all eight tab ScrollViewers.

Also worth noting for the shell step: in a 720-tall window a card plus its save button can push the
primary action below the fold (the Hotkeys tab does). A sticky footer — or a taller default window —
should be part of the shell decision.

Icons (`ui:SymbolIcon`) are deliberately not used yet: `CardControl.Icon`/`SymbolIcon.Symbol` take a
`SymbolRegular` value, and those enum member names can't be verified from here (PowerShell 5.1 can't
reflect a .NET 8 assembly). Pick them from WPF-UI's icon list, or log the enum once from the app.

### Ordering (revised after checking the API)

WPF-UI's `NavigationView` is built for Frame/Page navigation (`NavigationViewItem.TargetPageType`,
`GoBack`/`GoForward`/`ClearJournal`, `FrameMargin`, an internal content presenter). It also raises
`SelectionChanged` / `ItemInvoked`, so it can be driven directly and host our own panels instead of
`Page` classes.

That makes the shell the **last** step, not the first: the tab rebuilds don't depend on it, and doing
them first means the shell decision is a single change to `MainWindow.xaml` plus how the builders are
registered — with a fallback (a plain rail + content host) if `NavigationView`'s template fights us.

1. **Palette** — one `App.xaml` change, sets the tone for every tab that follows.
2. **Tabs, one commit each** — ui: controls, `Card` sections, in the order below.
3. **Shell last** — rail instead of the tab strip, once we know it works.

## Progress

Updated in the same commit as each tab. ✔ = done, ▸ = in progress, ☐ = not started.

| # | Tab / step | State | Commit |
|---|---|---|---|
| 0 | **Palette**: eight hex brushes deleted; the UI now uses WPF-UI's semantic brushes | ✔ | *(this commit)* |
| 1 | `Hotkeys` (was Settings) — **redone** with `ui:Card`, `ui:TextBox`, `ui:InfoBar` | ✔ | *(this commit)* |
| 2 | `Arduino` — **redone**: Connection / Input test as cards | ✔ | *(this commit)* |
| 3 | `Setup` — Display environment / Stored calibration cards | ✔ | *(this commit)* |
| 4 | `Gem` settings — Run / Advanced cards | ✔ | *(this commit)* |
| 5 | `Spammer` — Preset / Keys / Advanced cards, key rows on a header grid | ✔ | *(this commit)* |
| 6 | `Attributes` — Dictionary card (read-only table) | ✔ | *(this commit)* |
| 7 | `Tuner` — Goal / Timing / Filter-rules / Filter-overrides / Advanced cards | ✔ | *(this commit)* |
| 8 | `Calibrate Tuner` (capture / steps / save) | ☐ | |
| 9 | `Calibrate Gem` — Capture / Points / Tests / Moves / Full-run-test / Advanced | ☐ | |
| 10 | **Shell last**: `TabControl` → `ui:NavigationView` rail (driven via `SelectionChanged`) or a plain rail + content host | ☐ | |
| — | ~~Shared row builder~~ — largely obsolete: `ui:` controls carry their own spacing (kept only if the move grids still need one) | – | |
| — | Button label/weight convention — still applies, per tab as it is rebuilt | ☐ | |

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
3. Screenshot before/after per tab into `logs/captures/ui/` (gitignored — local review only), and update the matching
   [USER_GUIDE.md](USER_GUIDE.md) rows in the same commit — button names must match the guide.
4. No behaviour changes, no config keys renamed, no calibration flow reordered.

## Non-goals

- No theme/branding work, no new colours or fonts beyond WPF-UI's.
- No new tabs for planned features ([PLAN-TUNER-SPRING.md](PLAN-TUNER-SPRING.md) has its own plan).
