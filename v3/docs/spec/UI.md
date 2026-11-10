# UI — the shell's token layer, chrome and Cards screen

**This is the current truth** for how the v3 shell looks and behaves at the presentation layer. The
design reasoning lives in [PLAN-V3-UI-UX.md](../plans/PLAN-V3-UI-UX.md) (a proposal); the rendered
before/after evidence is local (`v3/.scratch/ui-audit/`). Phases 2–5 of that plan are not described
here yet.

The shell's hard rule is unchanged: **the shell never decides, the core never draws**
([PLAN-V3-SHELL.md](../plans/PLAN-V3-SHELL.md) §5). Everything below is presentation and wording; the
phases, rows, counts and sentences still come from the core's `state`/`log`/`refusal` messages.

## Tokens

`lib/ui/tokens.dart` is the single place a new value comes from (`lib/ui/theme.dart` re-exports it and
builds the `ThemeData`).

- **Colour.** `ColorScheme.fromSeed(0xFF3B82F6, brightness)` builds both themes. A `ShellColors`
  `ThemeExtension` adds the meanings M3 does not name: dark uses `success` #7EE2B8 (+ `onSuccess`,
  `successContainer`, `onSuccessContainer`), `warning` #F2CC7B (+ its pair/container), `info` #9ECAFF;
  light installs the matching darker `success`/`warning`/`info` plus inverted containers so a StatusChip
  keeps AA contrast on the light surface. `blocked` is `colorScheme.error`. Colour is never the only
  channel: chips carry a dot + word, selected cells a check, banners an icon.
- **Brightness.** Setup's **Appearance** section offers **Follow system / Light / Dark**; the shell
  installs both `ThemeData`s on `MaterialApp` and maps the choice to `ThemeMode`. The choice is the
  portable `settings.ui.theme` (`system|light|dark`, default `dark` so the original shell does not
  change appearance on upgrade). The native title bar follows the same choice through the runner's
  `setTitleBarTheme` host call (`0` system, `1` light, `2` dark), and re-follows Windows when the
  system setting changes in system mode. The top bar also carries a quick appearance menu (system/light/
  dark) so the choice is reachable without opening Setup.
- **Type.** The `textTheme` is declared explicitly: `headlineSmall` 22/600 (page title),
  `titleMedium` 16/600 (card/dialog), `titleSmall` 14/600 (section), `labelLarge` 13/600,
  `bodyMedium` 14/400 (body/prose), `bodySmall` 12.5/400 (meta — never prose alone), `labelSmall`
  11/600 (chips/strip), plus `ShellText.mono` (Consolas 12.5) for coordinates, counts and logs.
  The typeface is **Roboto bundled with the shell** (`assets/fonts/`, weights 400/500/700 + italic):
  Windows does not ship Roboto, so naming it without the asset silently fell back to a system font and
  the shipped pixels differed from the approved renders (this was true until the 2026-10-30 real-build
  check; `flutter build windows` output now lists the faces in `FontManifest.json`). Mono stays
  Consolas, a standard Windows font.
  `ShellProse` caps prose at `ShellWidth.prose` (620 ≈ 60 characters at 14 px); data, tables and fields
  keep full width. `VisualDensity.compact` stays.
- **Space/size/radius.** `ShellSpace` 2/4/8/12/16/24; `ShellWidth` prose 620, fieldSm/Md/Lg
  140/170/220, picker 240, labelColumn 160, rail 216; `ShellRadius` 6 (controls), 10 (panels), 14
  (cards/dialogs), `chip` 999; `ShellSize` min logical window 1120×680, idle tile 66, icon steps.
- **Defaults.** `cardTheme` (surfaceContainerHigh, radius 14, no elevation/surface tint),
  `inputDecorationTheme` (filled surfaceContainerLow, radius 6, primary focus ring) and `dividerTheme`
  are set once so a new card/field does not re-invent them.

Phase 0 migrated the token-matching widths and all raw colours; a surface's remaining one-off widths
fall when that surface is rebuilt in Phases 2–4 (recorded in the plan §7.1).

## Chrome (Phase 1)

- **Window minimum:** 1120×680 logical, enforced by the runner's `WM_GETMINMAXINFO` in
  `windows/runner/win32_window.cpp` (DPI-scaled; the plan named `main.cpp`, but the message handler is
  where Win32 requires it). The size is the rail plus the widest rigid form.
- **Rail (`ShellRail`, 216 px):** RUN → Cards; TOOLS → Skill Spammer, Magic Tuner, Gem Composer, Buy,
  Sell, Quest Hand-in, Pet Feeder; SYSTEM → Attributes, Arduino, Hotkeys, Setup, Calibrate. Each row has
  a `rail-<key>` ValueKey; the Cards row shows a badge with the running-tool count (Hold Space included).
  Selecting builds the destination lazily and keeps it alive in an `IndexedStack`, so drafts survive
  navigation exactly as they did with the old kept-alive tabs. **Hold Space has no rail row** — it has
  nothing to configure. **The rail can be hidden** with the top bar's `rail-toggle` sidebar button or
  **Ctrl+B**; hiding it removes the rail from the row so the content panel takes the full width (the
  selected tab and its draft are unchanged). The choice is session-only — the rail is visible again on
  the next launch.
- **Top bar (48 px):** the `rail-toggle` sidebar button (`view_sidebar`, tooltip Hide/Show sidebar
  (Ctrl+B)) leads; then the compact core status line (`core ready · rev <n> · <window title>`; a success
  dot, not a pill), the outlined **Logs (N)** button (a list icon), the always-on-top pin
  (`app-bar-always-on-top`) and the product version on the right. There is **no in-app "Seal Tools"
  brand**: the native Win32 caption already carries the window title, so repeating it in the bar only
  competed with the status. **There is no Hold Space toggle.** While a foreground tool runs (Pet
  excluded) an `app-bar-mini` picture-in-picture action appears to collapse into the mini bar; when
  idle, the `app-bar-dock` action opens the quick dock.
- **Status strip (28 px):** a top hairline, core version/rev on the left, the core's root, and
  `Ctrl+B sidebar · Ctrl+L logs` (plus `· Esc close` while the drawer is open) on the right. It
  advertises the shortcuts; the drawer and rail are opened from the top bar or their shortcut.
- **Logs drawer (`LogDrawer`):** 238 px along the bottom of the content, **newest line first at the
  top** (the model appends oldest→newest; the builder walks backwards), a fixed 44 px level badge per
  line (info/warn/error, `note` for shell notes, stderr = error), a fixed origin column (stderr/shell),
  an **All / Errors** filter and a close button. Monospace body. Ctrl+L toggles it; **Esc closes it**
  (the status strip advertises both while open).

## Mini running window (landed 2026-11-01)

A foreground run (every tool except the resident Pet Feeder) collapses the whole window to a
**460 × 168 borderless, always-on-top bar** (`MiniRunningCard`, `lib/ui/mini_mode.dart`). Approved
design demos: `v3/.scratch/ui-audit/ui-demo-mini.html` and `v3/docs/design/finished/mini-bar-text.html`; real-build evidence
`v3/.scratch/ui-audit/mini-real-*.png`. The core and protocol are unchanged: every bar value is an
existing `state` / `tool.finished` fact.

- **Window chrome** — `setMiniChrome` (shell channel) → `Win32Window::SetMiniChrome`: saves the
  style/EX style, applies `WS_POPUP` (no caption/thick frame/min/max/system menu),
  `WS_EX_NOACTIVATE` and always-on-top, and rounds the popup on Windows 11; "Full window" restores the
  saved chrome, the saved full rect and the saved pin. The minimum size is lowered to 460 × 168 while
  mini and restored to 1120 × 680 on expand. `WS_EX_NOACTIVATE` keeps the game's keyboard focus when
  Stop is clicked while the click is still delivered to Flutter (verified on the real 150 % build:
  690 × 252 px, `has_caption=False`, `ex_noactivate=True`, foreground window unchanged after a Stop
  click).
- **Content, ranked** — row 1: drag grip, tool icon + name (14 px), phase chip (dot **and** word),
  resident-pet paw badge, expand icon. Row 2: the core's action/outcome sentence as one 13 px semibold
  line (two lines only when there are no facts; full text on hover). Row 3: the OCR attributes as one
  `name=value` line each, full width and without cards; normal attributes stay bright on-surface and
  only an attribute named by the enabled filter (and inside its min/max) takes the primary colour.
  The long Filter value follows as its own full-width ellipsized line. Row 4: Grade/Left/Try as compact
  metric chips in the footer, then clock + countdown (`in 12 s`) and exactly one primary action. At most
  three metric chips are shown; the tooltip carries the full fact set. Rail, tabs, header, save bar,
  logs, finished summaries and the native caption remain absent.
- **Actions** — an active run shows **Stop** (88 × 32, one click, no confirmation; enabled while
  `running` and `starting`; disabled with a spinner while `stopping` or while a stop command is in
  flight, so the bar never invents a phase). A finished/error/lost bar shows **Full window**
  (112 × 32); the expand icon is always present.
- **States** — `running`, `starting`, `stopping`, `finished`, `stopped` (cancelled), `error`, and
  `core lost`; colour is always paired with the chip word. The finished/stopped/error result bar keeps
  the last live OCR attribute lines for inspection but drops the live metrics and Filter line, matching
  the approved result design. The bar stays on Finished/Error until "Full window"; it never pops the
  shell over the game on its own. Expanding restores the exact full
  rect; the full shell stays mounted offstage behind the bar, so the current tab and its draft survive
  the collapse.
- **Resident Pet Feeder** — Pet never collapses the window; it is the resident feeder and keeps the
  full window usable. If pet runs while a foreground tool is mini'd, the bar shows a paw badge
  (tooltip "Pet Feeder is running in the background"); Stop stops only the foreground tool and the
  pet keeps running. The full window still shows both running cards.
- **Placement** — default dock right edge, 16 px inset, 18 % down the work area (below a typical
  minimap). Drag is a Flutter pan that moves the window through `setWindowRect`, clamped to the work
  area; on release the bar snaps to an edge within 28 px (16 px inset). The position is remembered for
  the session and re-clamped when the next run collapses; it is not persisted across restarts (a
  machine-state `ui` field would be the follow-up).
- **Entry/exit** — automatic on the foreground tool's first non-idle `state`, plus the top-bar
  `app-bar-mini` action while a foreground run is active. "Full window" suppresses auto-collapse for
  the rest of that run; the next start collapses again. The bar has no Esc/Tab contract: with
  `WS_EX_NOACTIVATE` it is pointer-only by design (keyboard would mean taking focus from the game).
- **Not built** — cross-restart position persistence; a native `HTCAPTION` drag strip (the Flutter pan
  above is the implementation); a topmost fallback for exclusive-fullscreen games (the usual Windows
  topmost limitation).

## Quick dock (landed 2026-11-02)

The idle companion window: a compact launcher for between runs, not the full Cards page in miniature.
It is the idle state of the same borderless, non-activating window that becomes the run bar; opening
the full shell restores its saved placement and pin.

- **Collapsed resting form** — the header's `dock-collapse` action shrinks the dock to its form-factor
  tab: a 46 × 150 vertical rail tab, a 220 × 44 bottom pill or a 200 × 36 top island. It keeps three
  tool-icon shortcuts (Skill Spammer, Buy and Sell) and a running dot. Clicking an icon expands the
  launcher and opens that tool's options; clicking the rest of the tab or its `dock-expand` button just
  returns to the full launcher. Hovering only pauses Auto-collapse — it does not replace the tab, so the
  icon clicks are reachable.
- **Idle behaviour selectable** — Setup → Quick dock and the dock header's placement menu both offer
  **Auto-collapse** (collapse after eight idle seconds) and **Keep open** (stay expanded until the
  player collapses it, the default for new configs). The header also carries a one-click
  `dock-keep-open` toggle so stopping the timer never requires opening the menu. The choice is persisted.
  Auto-collapse is paused while the pointer is over the dock and resets on any interaction; resizing or
  moving the dock restarts the idle countdown. Only Auto-collapse or an explicit collapse shrinks an open
  launcher again.
- **Entry/exit** — the top-bar `app-bar-dock` action (or Setup's "Open quick dock") collapses the shell
  into the dock. The full shell stays mounted offstage (the same keyed subtree as mini mode), so tab
  drafts survive. "Open full window" in the dock restores the exact full rect. The finished run bar's
  **Quick dock** action uses the same Keep-open rule as `enterDock`, so it returns expanded under Keep
  open and only uses the resting form when that preference was explicitly collapsed.
- **Five placements, three sizes** — `DockPlacement` gives **right rail**, **left rail**, **bottom
  right**, **bottom left** and **top centre**, at 300×480 (rails), 340×460 (bays) and 380×500 (island).
  `DockSize` scales the chosen shape: **Compact** (0.9×), **Standard** (1.0×) and **Large** (1.25×),
  including the collapsed tab/pill/island. A matching `DockMetrics` table scales the content too — title
  12.5/13/15 px, summary 10/10.5/11.5 px, row 40/44/54 px and the fixed Start
  column 58×26/64×28/76×34 px — so a larger window gets larger labels and action targets rather than
  empty space. The choice can be made in Setup → Quick dock or from the dock header's placement menu
  (which now lists the three sizes); the dock can also be dragged (pan through `setWindowRect`) and snaps
  to its nearest edge. **Placement, size, collapsed state, idle behaviour, tile order and pinned Sell
  selections are persisted** in the portable `settings.ui.quick_dock` block
  (`{placement, size, collapsed, behavior, tiles, pins}`) through the normal validate → apply path;
  the core's portable projection carries the fields (seal-config `ensure_portable_ui`). A dragged
  offset inside a placement is session-only — the preset is what survives a restart.
- **Content** — the quick tiles (Skill Spammer, Buy, Sell, Quest Hand-in, Gem Composer, Magic Tuner,
  Hold Space by default), then one setup-only row (Pet Feeder) that opens the full window **on its
  Pet tab**; footer "Open full window". Each quick tile has a **dedicated Start button** and, where it
  has options, a **separate ⋮ options** button — the row itself is not a Start target. The header's
  **Edit** action opens an in-window editor: up/down reorder, Remove, and "Add back" for removed
  tools; pinned Sell tiles appear there too. The dock remembers the order (empty is allowed). Idle rows
  show no phase chip; a running/starting row shows its chip and the button becomes Stop. Hold Space
  uses that same Start/Stop button and has no options menu or extra "toggle" label.
- **Pinned Sell actions** — in the Sell menu a name field + **Pin** turns the current per-run bag
  selection into a named tile (`Sell · <name>`); tapping the tile starts Sell with that exact `slots`
  list. Pins are saved in the dock preference, load into the current selection from the menu chips, and
  are removed through Edit. Names must be non-empty and unique.
- **Secondary menus** — one at a time, Back closes; Start is the single filled action pinned in the
  footer:
  - **Buy** — preset chips + count, sent as the existing per-run `tool.start {preset,count}`; the
    last-used preset/count is remembered for the dock session (not config), so a re-entry after a run
    keeps it.
  - **Sell** — an 8×8 bag grid with the full screen's **per-line shortcuts**: every row has a
    `row N` button that fills all 8 cells when any are off and clears them when all are on, beside
    All/None and the live count. Sent as per-run `tool.start {slots}`.
  - **Spammer** — rotation list; picking one saves the active preset with `config.apply`, exactly like
    the Cards picker, and the menu says so.
  - **Quest** — flow list, same saved-active rule.
  - **Gem Composer** — start-grade chips (from the fixed grade ladder) and the empty-slot behaviour
    (Stop / Advance grade / Advance + clear); each pick saves that portable field with `config.apply`,
    then Start runs the saved job.
  - **Magic Tuner** — target-grade chips (from `grade_order`) and a max-retries stepper; both save the
    portable `settings.tuner` fields before Start.
  - **Hold Space** — no menu; the tile is an on/off toggle (Start/Stop).
- **Run transition** — a foreground tool's first non-idle `state` turns the same compact window into
  the run bar (with the same full-window restore target). A finished/error bar offers the
  `mini-dock` action back to the launcher in addition to "Full window".
- **Verification note** — every behaviour above is covered by widget/model tests and the core config
  test; the live game path (real Start/Stop clicks against the game) is still a manual check.

## Cards screen

The screen opens with a **Run** page header (22/600 title, prose-capped subtitle, a running-count
status chip on the right) and answers "what is running, and is it healthy?" before configuration.
Screen content is capped at **1240 px and centred** on windows wider than that (the rail keeps its
216 px; a 1920 px window would otherwise stretch the cards), and scrolls in one `ListView`
(`cards-list`). It has two sections:

- **Running now** — one full card per engaged tool: 3 px success accent, title 16/600, a
  `StatusChip` (running/starting/stopping), a `foreground`/`resident` scope word (Hold Space has none —
  it acts through the board, not the foreground window), the core's one-line message, its countdown in
  mono (ticking from the grid's ONE ticker), then **one scrollable body (max 168 px)** shared by:
  the core's rows as a table with `ROW / STATUS / DETAIL / NEXT` headers (status is plain
  `onSurfaceVariant`, not a success colour — the protocol has no per-row OK flag; the NEXT cell uses the
  core's own `next_action` word when it published one, e.g. `feed in 30 m`), and below a hairline the
  additive **`detail` list** — `label: value` lines (label `onSurfaceVariant`, value `onSurface`) plus
  label-less attribute lines rendered exactly as the core wrote them. A body with no rows and no detail
  keeps the one quiet "No rows to show" line. The finished/refusal lines are verbatim, and **Stop** is
  the one action (error-container button). The title takes the available
  width with `Expanded`, so it no longer truncates against the chip's and scope word's fixed widths.
- **Available tools** — one-line tiles (min height 66): a 34 px icon well, the name, a "Ready · …"
  summary computed from the same config/run state Start reads (preset/count, armed slots, active
  rotation, target grade, grade chain, pet row count from `pet.slots`…), and an outlined **Start**.
  Buy/Quest/Spammer additionally show a small **tune** button that reveals their choices inside the card
  (`options-<tool>`); collapsed tiles stay one line. Start is always visible. Hold Space's tile says
  "Off · spacebar auto-pickup" and has no options.
- When nothing runs, a banner replaces the running section: **No runs yet** while no tool has ever
  published a `state`, **Everything is idle** once they have. There is no fixed card height: running
  cards are content-sized and tiles wrap 3/2/1 columns by width (3 only when the content is wide enough
  for the demo's ~380 px tiles).
- **In-flight:** a Start/Stop disables and labels itself ("Starting…"/"Stopping…") with a spinner until
  the core's next `state` arrives. The card's phase never moves optimistically.
- **Hold Space is a card** at the same level as the other tools: Start holds the spacebar, Stop releases
  it, its phase/message/refusal render on its card, and it has no config screen. (Its failed release is
  still only in `holdspace.log` — the release has no protocol event.)

## Config surfaces (landed 2026-10-30)

The shared frame: a `PageHeader` (22/600 title, prose-capped description, one status chip that states
"Unsaved changes" or "All changes saved"), a scrolling editor, an optional **300 px aside**, and a save
bar that reports outcomes only (the dirty state is stated once, in the chip). One filled action per
screen: Save.

- **Section panels.** Every `EditorSection` is a `Material` panel (surfaceContainerLow, 10 px radius,
  one hairline) with 16 px inside/between. Material, not a coloured `Container`, because `ListTile`
  fields assert against a decorated background.
- **Aside.** `ConfigTabScaffold.aside` (300 px, own scroll) carries reference data and context beside
  the form: Tuner/Gem/Buy/Sell put a collapsed `ReadOnlyPanel` "Calibrated geometry" there plus a short
  "how this screen saves/run" panel. Nothing reference-shaped leads the form anymore. Below ~900 px of
  content width (a 960 window beside the rail) the aside stacks below the form instead of squeezing
  every row; Tuner rule rows wrap the same way.
- **Foldable advanced sections.** `FoldableSection` holds diagnostics that are not part of a normal
  run: Tuner (OCR retries, save captures, capture cleanup), Gem (save empty captures), Quest (cursor
  guard). Folded by default; the controls keep their keys.
- **Tuner** (`spammer`-style panels): Grades (the shared **GradeLadder** path `N → G → DG → XG → SG`,
  read-only; the target grade uses the primary fill and the filter's `require_grade` a warning
  border/fill, with no extra icon or word inside the node. The **Target grade** and
  **Minimum grade** selectors sit directly beneath the ladder; the old Require-grade dropdown in
  Filter moved here so both roles have their controls in the same place.
  Timing, Springs, Filter (rules/overrides with tonal Add, outlined per-row delete), Advanced (folded);
  geometry in the aside. The aside also carries the demo's **Last run**
  panel once a run has finished: the core's `tool.finished` `summary` (Grade, Attempts, Captures saved,
  Stopped by) plus the local `ended_at`; the panel is absent before the first finish and never invents
  a placeholder run.
- **Spammer**: Presets (editing/active pickers, tonal Add/Rename, outlined Duplicate/Delete; selection
  actions hidden when the preset list is empty) · Keys & cooldowns (opening, priority key rows) ·
  Rotation — combos.
- **Gem**: Grades (the same **GradeLadder**; the start grade uses the info fill, no extra word or
  icon inside the node), Empty result, Movement; calibrated positions/slots/result area in
  the aside; save-empty-captures folded.
- **Quest**: Active selection (empty sequence reads as "flow only"), Flows, Sequences; empty lists are
  `EmptyState` with a next action; cursor guard folded.
- **Buy / Sell are split screens** (`BuySellTab(section: buy|sell)`): Buy edits/saves the `presets`
  field; Sell edits/saves `sell_cap` and arms the per-run selection (never saved; run from the card).
  Both put their calibrated geometry in the aside. The Sell grid is 26 px cells with a 2 px gutter, a
  check + inset ring and `Semantics(selected)` on armed slots, one count line with Select all / Clear,
  row shortcuts, prose-capped captions. The combined (`both`) layout remains for the widget tests and is
  not used by the shell.

## Pet (landed 2026-10-30)

- **Layout:** readiness and a one-line summary (rows · queued · food cells · next pet) span the page.
  Below, at wide widths, the setup is two columns: **rows + queue + add-by-icon** on the left;
  **bag scan + food cells + return slot + food loading** on the right. **Timing & read thresholds** and
  the portable **Feeding** fields are folded into one **Timing & thresholds** disclosure, collapsed by
  default; narrow windows stack the columns. This removes the previous single 10-panel scroll without
  dropping any editor.
- **Readiness first:** a `StatusBanner` whose kind is the core's verdict (`pet.preflight`: success ready,
  warning not-ready, error on a refusal, info while reading) with **Refresh checklists** as a text
  action. Below it, a panel of per-item checklist rows (icon + label + detail) and the core's readiness
  lines as individual `Text`s — never one `\n`-joined block.
- **Row cards:** a `SurfacePanel` containing one card per breeding row. The header is the state —
  `Row N` + `StatusChip` (boarding/reloading/idle/disabled) + "next: Save" while dirty. When the feeder
  is running, the card also renders the core's live row under the header
  (`Live · <status> · next <word> <time>`), joining the saved setup to what the run is doing; the saved
  fields stay the config. Saved fields
  (Enabled, Stacks) group together; the **immediate A1 control** ("Boarding now") is separated below with
  an "applies immediately" cue, because it writes at once and is not part of the draft. Row marks read
  `start/end x 10 · y 20 · 100×30` (no raw arrays).
- **Queue** is a compact table; Add-by-icon / Scan / Food cells / Return slot / Loading / Timing /
  Feeding keep their behaviour, with prose through `ShellProse` and `EmptyState` for no rows/no queue.
- **Calibrated marks & reference crops** are demoted into a collapsed `ReadOnlyPanel` showing
  `x · y · w×h` values and a configured-count summary.
- **Diagnostics:** the Scan-the-bag section also carries **Test read one pet panel**
  (`pet.test_read_panel`, OPEN §6 G13 now shipped): pick the ITEM page at the top of the section and the
  1-based bag cell, press once, and the tab renders the core's `region`, raw OCR `lines`, its own parsed
  `panel.line` (or "Not a pet panel"), the `numbers` it saw and the best-effort evidence path. It only
  hovers — no click, no config write — and states that the game must be in front with the page up.

## Calibrate workspace (landed 2026-10-30)

The one-form screen became the demo's workspace. Layout: at **content width ≥ 850** (the default 1280
window leaves ~1064 px beside the rail, and the 1120 minimum still leaves ~872) — target tree 250 px ·
flexible canvas · mark editor + reference gallery 340 px; below the breakpoint the panels stack in one
scroll column with a "collapse the sidebar" hint. A compact **capture bar** sits between the tool tabs and
the fixed panes: active frame/display summary, its own **Capture** action (filled when no screen exists),
**Captures**, **Details**, and **Stored marks** buttons (those three open dialogs, so the canvas is above
the fold at 720 px). The page header keeps the display/match chips and Refresh. A **tool tab row** sits
between the header and the workspace: **Buy / Sell** (Shop), **Gem Composer**, **Magic Tuner**, **Pet
Feeder**, **Tooltip**. The three mark tabs scope the target tree and editor to that tool; the two session
tabs replace the mark workspace with that session's one-capture workspace. A **Screens strip** under the
capture bar lists every live capture (up to the core's twelve), newest first, labelled by the tool that
took it (`Shop · 1`) with the count of targets assigned to it.

- **Per-target capture assignment:** each mark target has its own assigned screen, keyed by
  `kind`/`kind@target`. A target with no assignment starts empty and Save stays disabled; it never borrows
  another tool's newest frame. Selecting a target restores its assigned screen. Clicking a screen in the
  strip only previews it; the editor's **Screen** row and **Use “Shop · 1” for this target** action perform
  the assignment. A capture auto-assigns only to the selected target that has no screen yet. Assignments
  are session state; saved marks never store a frame id.
- **Tool-scoped target tree:** the active mark tool's 17 presentation marks / 34 instances are listed
  (Shop 5/5, Gem 3/9, Pet 9/20), with saved/partial/not-set dots and counts; named-instance kinds expand
  their targets (N/G/DG/Register/Combine, ITEM1–3, row 1–4). Selecting a kind/target drives canvas and
  editor. Switching the kind picker to another tool's kind moves the tool tab with it.
- **Capture shelf:** behind the capture bar's **Captures** dialog. At most twelve frames (the core's
  FIFO). Each item: label/time, physical size, that capture's **own `DisplayInfo`**, frame id, and an
  eviction marker on the oldest; the thirteenth capture evicts the oldest and a save against an evicted id
  is refused — the screen says so and never auto-recaptures. Selecting a shelf frame previews it; assigning
  it is the editor's explicit action. The hide → one frame → show flow and its status are unchanged; the
  capture result/refusal stays visible in the compact capture bar.
- **Session capture reuse:** Tuner and Tooltip show the same capture bar/Screens strip above their
  existing right-side editors. Tuner draws its three boxes and Spring point on the selected shared
  screen; a screen change clears the Check result and disables Save until **Check boxes** runs again,
  and Save sends that screen's `frame_id`. Tooltip reuses the selected screen as the visual background
  for its offset/size fields, but **Capture hover** remains the live cursor-anchor/Test-read action.
  The sessions' right-side layouts and controls are otherwise unchanged; the Tuner's private
  **Capture screen** button is not shown in shared mode because the shared bar above owns capture.
  Screen selection itself has no target assignment because each session has one calibration record.
- **Delete a screen:** each Screens tile has a small **× Delete screen** action. It opens two
  confirmations (usage/consequences, then the irreversible confirmation); only the second one calls
  `capture.discard`, removes the tile and clears the session-only assignments that pointed at it.
  **Saved calibration marks and coordinates are config data and are never changed by a delete** — the
  dialog says so and the saved mark remains fetchable.
- **Canvas:** fit / 1:1 / zoom ± / pan, zoom % and "1 display px = N physical px"; the selected mark is
  drawn with its label. The typed x/y/(w/h) fields stay authoritative and the mapping function is
  unchanged and single. Overlay labels choose white or black text from the label background's luminance,
  so the fixed grade/attribute/remaining palette stays legible in light mode.
- **Fixed mark workspace:** at wide widths the target tree, canvas and inspector fill the height left
  under the capture bar. The target tree and the inspector scroll inside their own columns with visible
  scrollbars; the canvas does not move while the inspector scrolls, so drawing and editing the selected
  mark happen in one view. Revert/Save are pinned below the scrolling inspector, and the gallery scrolls
  with the fields. The page itself does not scroll for mark tools; **Details** (display environment +
  Detect) and **Stored marks** are dialogs. Narrow windows keep the stacked layout.
- **Editor:** surface + target pickers, shape-aware fields, the result-gem-area empty verdict, stored
  crop preview, Revert + Save (Save is the only filled action and enables only with a live frame/target/
  verdict); save refusals render verbatim, including "capture again" for an unknown frame id.
- **Stored coordinates:** all 34 instances — surface, tool, shape, `x, y` / `x, y · w×h`, artifact, status —
  filterable by tool. Marks do **not** persist a frame reference; the frame is write-time provenance.
  **Copy as YAML** copies the saved marks as a small YAML document to the clipboard (a read of values
  already on screen; it writes nothing and is not a config import).
- **Reference images:** result-gem crop (present/not captured), pet crops marked "run-learned", v2
  annotated screenshots written by the v3 session saves and marked present/not captured — truthful states, no invented artifacts, and no v2-only state.
- **Display environment:** the capture's DisplayInfo and the stored environment; a mismatch raises a
  `StatusBanner` "recalibrate before running a tool" (client-size/screen invalidates every point, scale
  alone does not; no auto-anchor, no proportional transform).
- **Sessions:** **Magic Tuner** and **Tooltip** are top-level tool tabs (not a panel below the stored
  table). Each is one capture-owned workspace: every session starts with a numbered step strip
  (Capture → measure → save), states what it needs, and keeps the typed fields authoritative. At wide
  widths each session is a **split workspace**: the captured picture stays fixed on the left (fit to
  380 px) while a 470 px right pane scrolls its target cards/fields independently, so a drag and the
  box being edited are visible together; narrow widths stack the two columns. Switching tabs keeps both
  session states and typed fields mounted.
  - **Tuner OCR 3-box + Spring point** — the top of the card pairs **Capture screen / Recapture
    screen** with the required capture state and the **Target to drag** chips (grade, attributes,
    remaining, spring). One capture covers the three OCR boxes AND the optional spring point (v2's
    one-capture flow); there is no separate Spring-point mark. After capture, the picture shows every
    drawn box in its own colour (grade green, attributes blue, remaining amber) with labels. A card
    per box carries the colour dot, label and its x/y/w/h fields; dragging on the picture writes the
    selected box and advances grade → attributes → remaining (v2 order). **Check boxes** adds a result
    card with the derived region, row pitch, grade, remaining count and filter verdict plus the
    matched-row count; **Save tuner** writes `tuner.ocr`/`spring_point`.
  - **Pet tooltip offset + shape** — the picture sits left, the two step cards right: **1 · Hover
    point** (x/y/delay + *Capture hover*; the core places the cursor and captures the panel) and
    **2 · Panel box & offset** (offset and panel size; a mono formula line shows `cursor + offset =
    panel top-left`). The picture overlays the placed cursor as a pink crosshair under the selected
    panel box; a **Read result** card shows the Test read lines in a bordered surface instead of loose
    prose. The nine Pet marks are calibrated in the mark editor; the removed Pet rows/crops session
    duplicated them.

## System surfaces (landed 2026-10-30)

- **Attributes:** search field over a real dictionary table (Name / OCR variants, category counts); zero
  matches swaps the table for an `EmptyState` with Clear; the per-level/negative/text-fix tables render
  only while the search is empty (they are not searchable).
- **Arduino:** a Board panel first — `StatusChip` + verdict + VID/PID/Baud/port as `InfoRow`s; ports as
  icon + port + description + trailing reason rows (not space-padded strings); firmware and input tests
  in their own panels; all three test actions are outlined and the cursor-moving Click is separated with
  a "moves the cursor" warning. No filled action on the page.
- **Hotkeys:** the accepted-name help appears once above the group; raw stored codes moved into a
  section-header tooltip; the four fields keep their key-name/parse/save commands.
- **Setup:** **Appearance** chooses Follow system / Light / Dark and saves it immediately; **Quick dock**
  chooses its placement and idle behaviour; Detect/Measure/Probe are outlined probes under a caption
  that names the one writing action; **Use detected title** is the single filled action with "applies a
  config save" under it; the detected-vs-configured mismatch raises a warning banner.

## Crash / connecting surfaces

The crash view is a `StatusBanner` (error) with the one Restart action when recovery is possible, the
core's failure sentence as its own keyed line, the root as an `InfoRow`, the stderr tail in a monospace
`SurfacePanel`, and a warning banner (not a restart) for a protocol mismatch. The connecting surface
keeps its spinner + status text, with the stderr tail only when there is one.

## Known interim states in this build

- **Phase 5 (session-shaped calibrations) is not built**, because it needs core/protocol work: tooltip
  two-capture hover, tuner three-box derive (withdrawn `tuner_region`), pet capture/crop session, v2's
  annotated reference screenshots, the Gem movement editor + pointer/cycle tests, Buy/Sell 64-centre/
  row overlays + test scroll + "Setup so far", and the Setup → Detect surface (the ComparisonCheck rule
  is ported; no command exists). The workspace lists each with what it needs; it does not fake them.
  **The backend ledger for every one of these is [OPEN.md §6](../backlog/OPEN.md#6-backend-work-the-finished-ui-needs).**
- **Frame persistence / calibration rev / Detect**: a saved mark stores mark + artifact + display
  environment, never the frame; `calibration.save` has no `rev`; there is no batch "fetch all" (the
  workspace fetches the kinds on open). These are protocol facts, not UI choices (OPEN §6 G04–G09).
- **Named geometry profiles / setup import-export** are still a future config-root feature
  ([OPEN.md](../backlog/OPEN.md)); the UI warns and recalibrates, it does not switch setups.
- `StatusBanner` has no widget-detail slot yet (Pet's checklist rows sit in the panel below the banner);
  noted for the next primitive pass, not a behaviour gap.

Gates for this build: `flutter analyze --fatal-infos` No issues (strict casts/inference/raw-types + async hygiene lints); `flutter test` 394/394; `dart format` clean. Every
destination plus the empty/logs/all-running/crash states was rendered at 1280 and 960
(`.scratch/ui-audit/shots/final-*.png`) with no layout overflow. The mini bar was additionally verified
on a real Windows build at 150 % (460 × 168 logical / 690 × 252 physical, borderless + non-activating,
Stop click and expand restore exercised). The quick dock was verified on the real runner at all three
form factors (right rail 450 × 720 physical, bottom-right bay 510 × 690, top-centre island 570 × 750 —
all borderless, `WS_EX_NOACTIVATE|TOPMOST`), including opening a secondary menu.
