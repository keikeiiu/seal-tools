# Analysis — the launcher UI, measured

Written before any change, from a read of `SealTools.Launcher/MainWindow.xaml.cs` (8,160 lines) and
`MainWindow.xaml` (56 lines). **Nothing here has been changed; this is for a verdict.**

Every number below was counted rather than estimated, and where a claim is a judgement rather than a
measurement it says so. The counts are as of `main` @ `1f42591` (2026-09-22) and will drift.

**How this relates to [PLAN-UI-CLEANUP.md](PLAN-UI-CLEANUP.md).** That plan's WPF-UI rebuild **shipped**
— its Progress table marks steps 0–9 done and the shell attempted and reverted, and the code agrees
(`Section()` returns a `ui:Card`, and the launcher uses `Wpf.Ui.Controls` throughout). Its own
remaining open item is the button label/weight convention. This document is not a second copy of it:
it is the **measurement for the next pass**, and it includes three things that plan never covered
(accessibility, the dispatcher-blocking test handlers, and the thread-safety the status poll depends
on).

---

## 1. What the UI is now

`MainWindow.xaml` holds six named controls — the title bar, two panels, three toggle buttons, the
Hold Space dot, and an **empty** `TabControl`. Everything visible is built in C#: **15 tabs** and
**6 tool cards**, from ~122 methods.

It is worth being precise about how much structure exists, because "8,000-line code-behind" reads
worse than the reality:

| helper | uses | | helper | uses |
|---|---|---|---|---|
| `MakeButton` | 80 | | `UiText` | 28 |
| `Section` | 69 | | `Mono` | 21 |
| `Hint` | 65 | | `MakeTab` | 16 |
| `LabeledField` | 44 | | `MakeInlineButton` | 9 |
| `Place` | 39 | | `MakeStepperButton` / `MakeRowButton` | 3 / 2 |

That is a consistent vocabulary — `Section(title, Hint(...), LabeledField(...))` is applied the same
way across fifteen tabs — so the problem is **not** an absence of structure. It is where the seams
were cut, and a handful of cross-cutting omissions.

## 2. What is already right

Recorded so a later pass does not "fix" these:

- **The section/hint/field vocabulary is real and applied uniformly.** 69 sections, 65 hints, 44
  labelled fields.
- **The comments explain *why*, not *what*.** The 60-vs-70 field width, the `MakeButton` top margin,
  the magenta read line, the reason the queue is captured from a marked cell — this is unusually good
  UI code to read, and it is the strongest argument against a rewrite (see §6).
- **No deadlock patterns.** Zero `.Result`, `.Wait()`, or `GetAwaiter().GetResult()` in 8,160 lines.
- **Nine `catch { }` blocks, eight of them justified** by a comment ("a bad UI block must not stop the
  launcher opening", "diagnostics must never break the click"). The one at
  `MainWindow.xaml.cs:2834` (`catch { return false; }`) is silent and uncommented.
- **The build is clean under `TreatWarningsAsErrors` + `AnalysisMode=Recommended`** — 0 warnings — so
  the unused-symbol and unreachable-branch classes of rot are already impossible.

## 3. The issues

Ordered by what I would fix first, which is roughly by (duplication removed × risk of the bug
recurring), not by severity of user impact.

### 3.1 The calibration canvas is copy-pasted five times — the biggest win

**Observed.** `new Canvas { Background = Brushes.Transparent, MinHeight = 300 }` appears **5×**,
`new Grid { Margin = new Thickness(0, 8, 0, 8) }` **5×**, and there are **18** separate
`MouseDown`/`MouseMove`/`MouseUp`/`SizeChanged` wirings. Each calibrate tab re-implements the same
sequence: capture → `Image` + `Canvas` overlay → redraw → drag-state machine → step counter.

**Consequence.** Four of the five largest methods in the file are calibrate tabs —
`BuildGemCalibrateTab` 351 lines, `BuildPetCalibrateTab` 322, `BuildBuySellCalibrateTab` 129,
`BuildTooltipCalibrateTab` 84, plus `BuildTunerCalibrateTab`. Every fix to canvas behaviour — DPI,
redraw on resize, drag feedback, a too-small drag being rejected — has to be applied five times and
has to *stay* applied. The plan's `MoveRow`/`PointRow` idea sits *inside* these tabs and does not
address the canvas they are drawn on.

### 3.2 Four button factories, one of which exists to undo another

**Observed.** `MakeButton` (MinWidth 84, margin `0,10,6,0`), `MakeRowButton` (height 32, margin
`6,1,0,1`), `MakeStepperButton` (32×32, no margin), and `MakeInlineButton` — whose entire purpose is
to **remove `MakeButton`'s 10 px top margin**, as its own XML doc states.

**Consequence.** A spacing or size change is four edits, and one constructor exists purely because the
default is wrong for one context. The plan's "button label/weight convention" is about *labels*; this
is the *construction*.

### 3.3 Blocking on the dispatcher, quantified

**Observed.** **18** `Thread.Sleep` calls totalling **5,500 ms**, inside **6 `async void` handlers**:
`GemTestFullCycle`, `GemTestClick`, `GemTestMovement`, `GemTestRelativeMove`, `GemTestArduinoRoute`,
`TunerSpringTestClick`.

**Correction to the existing note.** [TODO.md](TODO.md) says "roughly 18 s of frozen window for a full
cycle". The sleeps account for 5.5 s of that; the rest is `HidPointer.To` blocking internally in
`WaitForCursorToSettle`. TODO.md's reasoning for deferring the fix (it is a five-handler refactor, not
a two-line change, because the tools' blocking placement has to move off the dispatcher too) is
correct and this does not change it.

**What is not recorded there:** because they are `async void`, an exception thrown inside any of them
is **unobservable** — it crashes the process rather than surfacing. That is the stronger reason to
touch them.

### 3.4 Accessibility: 2 calls in 8,160 lines

**Observed.** `AutomationProperties` appears **twice**, both `SetName` on two move-route test buttons
(lines 2103, 2159). `AccessKey`: **zero**. `KeyboardNavigation`: **zero**. Across ~80 buttons and 15
tabs.

**Consequence.** There is no keyboard path to any command, and the UI is close to opaque to assistive
technology. This is the one item here the plan does not mention at all. It is cheap to do *while*
tabs are being touched and expensive to retrofit, which is why it belongs in the same pass.

### 3.5 The status poll is hand-written, and its safety assumption is false

**Observed.** The only data binding in the app is the three columns of the Attributes `DataGrid`. Live
status is a **750 ms `DispatcherTimer`** (and a 700 ms UI-save timer) driving a 90-line
`RefreshStatus` that reads service state by hand.

**Consequence.** Two of them:

1. Every new published field needs handler code, and the card can show state up to 750 ms stale.
2. The timer reads `ToolState` **on the UI thread while a tool thread writes it**. That is only safe
   if those fields are safe to read cross-thread — and a separate audit found `ToolState`'s comment
   claiming "reads are atomic" is **not true** of its `int?` / `Nullable<DateTime>` members, which are
   multi-word structs. A torn read shows a garbage countdown rather than a crash, which is the worse
   failure mode.

This is a *deliberate style* (polling, no MVVM) with an *undocumented dependency* on a claim that does
not hold. Either is fine; the pair is not.

### 3.6 Layout constants are inline, ~62 of them

**Observed.** ~62 literal `Width`/`Height`/`MinWidth` assignments; no styles or resources defined in
code. The plan names this exactly — "every grid sets its own `ColGap`/`RowGap`/`76 - ColGap`, which
*is* the inconsistency being complained about" — and its step 0 (palette → theme brushes) shipped, but
the geometric constants did not get the same treatment.

**Consequence.** No single place to tune density, and the narrow-window defects v2.9.1 fixed by hand
(third button running off the edge, a clipped Delete label, a header drawn without its right border)
are the kind that return.

### 3.7 Whole tabs as single methods

**Observed.** `BuildSpammerTab` 429 lines, `BuildPetTab` 392, `BuildGemCalibrateTab` 351,
`BuildPetCalibrateTab` 322, `BuildToolCards` 170.

**Consequence.** Not a bug — but these cannot be reviewed as units, which is why §3.1 is worth doing
first: extracting the canvas shrinks four of them at once.

### 3.8 UI text is not separable from game text

**Observed.** **554** lines contain CJK. Most are game strings the tool must match or draw
(發條, 目錄, 開始代養); the rest of the UI is English.

**Consequence.** Deliberate — the guides say the launcher's own button names stay English — but there
is no resource file, so launcher text and game text live in the same literals. That is part of why the
zh-TW guide tracks the English one so awkwardly: there is nothing to generate it from.

## 4. What this adds beyond PLAN-UI-CLEANUP

The plan already decided the big question — **stay code-built**, adopt shared constants and a config-
wired `Field(...)` helper — with its cost/benefit written down, and most of its steps shipped. This
document corroborates that direction rather than proposing a different one. Three things it does not
cover:

1. **The capture canvas** (§3.1) — the largest duplication in the file, and the thing that would make
   the per-tab pass cheap.
2. **Accessibility** (§3.4) — absent from the plan.
3. **The poll's thread-safety dependency** (§3.5) and **the dispatcher-blocking handlers** (§3.3) —
   the plan treats the UI as a layout question; these two are correctness questions that ride along
   with it.

## 5. Proposed order

Following the plan's own method (one concern per commit, smallest first, no behaviour changes):

1. **The capture canvas**, as one reusable control. Largest duplication, shrinks four big methods,
   and no behaviour change if the extraction is faithful.
2. **One button factory** with a size argument; `MakeInlineButton` disappears.
3. **Accessibility** on the primary actions as each tab is touched — `AccessKey` on the main verbs,
   `AutomationProperties.Name` on icon-only and ambiguous controls.
4. **Layout constants** into one place, per the plan's "shared style constants".
5. **Decide the poll deliberately** (§3.5): either keep polling and make the published fields atomic
   (or immutable snapshots), or move those few values to bindings. Either is small; drift here is what
   is not.

Item §3.3 (the blocking handlers) is deliberately **not** in that order — see §6.

## 6. Non-goals, and one open question

**Not proposed:**

- **An MVVM rewrite.** The imperative style is coherent, the plan priced the alternative, and the
  comments in this file are dense, load-bearing reasoning. The repo has already lost an afternoon to a
  patch that duplicated 535 lines of this file; a rewrite is the same class of move at ten times the
  scale. Incremental extraction keeps the reasoning.
- **Converting tabs to XAML.** The plan left this reversible on purpose (a tab can become a
  `UserControl` in the existing shell). Nothing measured here argues for spending that now.
- **The `ui:NavigationView` rail.** Already tried and reverted, with the failure mode written down.

**Open question for the player, which is not mine to decide:**

- **Is the dispatcher-blocking fix (§3.3) in scope?** It is the only item here that changes observable
  behaviour — the test buttons would stay responsive and could report progress — and it is the one
  TODO.md deliberately deferred. It also touches the window of a feeder that runs live for days, so it
  wants a decision rather than a judgement call.
