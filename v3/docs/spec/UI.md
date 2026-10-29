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

- **Colour.** `ColorScheme.fromSeed(0xFF3B82F6, dark)` is kept. A `ShellColors` `ThemeExtension` adds
  the meanings M3 does not name: `success` #7EE2B8 (+ `onSuccess`, `successContainer`,
  `onSuccessContainer`), `warning` #F2CC7B (+ its pair/container), `info` #9ECAFF. `blocked` is
  `colorScheme.error`. Colour is never the only channel: chips carry a dot + word, selected cells a
  check, banners an icon.
- **Type.** The `textTheme` is declared explicitly: `headlineSmall` 22/600 (page title),
  `titleMedium` 16/600 (card/dialog), `titleSmall` 14/600 (section), `labelLarge` 13/600,
  `bodyMedium` 14/400 (body/prose), `bodySmall` 12.5/400 (meta — never prose alone), `labelSmall`
  11/600 (chips/strip), plus `ShellText.mono` (Consolas 12.5) for coordinates, counts and logs.
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
  nothing to configure.
- **Top bar (48 px):** the compact core status line (`core ready · rev <n> · <window title>`; a success
  dot, not a pill) on the left, the outlined **Logs (N)** button (a list icon), the always-on-top pin
  (`app-bar-always-on-top`) and the product version on the right. There is **no in-app "Seal Tools"
  brand**: the native Win32 caption already carries the window title, so repeating it in the bar only
  competed with the status. **There is no Hold Space toggle.**
- **Status strip (28 px):** a top hairline, core version/rev on the left, the core's root, and
  `Ctrl+L logs` (or `Ctrl+L logs · Esc close` while the drawer is open) on the right. It advertises the
  shortcut; the drawer is opened from the top bar or Ctrl+L.
- **Logs drawer (`LogDrawer`):** 238 px along the bottom of the content, **newest line first at the
  top** (the model appends oldest→newest; the builder walks backwards), a fixed 44 px level badge per
  line (info/warn/error, `note` for shell notes, stderr = error), a fixed origin column (stderr/shell),
  an **All / Errors** filter and a close button. Monospace body. Ctrl+L toggles it; **Esc closes it**
  (the status strip advertises both while open).

## Cards screen

The screen opens with a **Run** page header (22/600 title, prose-capped subtitle, a running-count
status chip on the right) and answers "what is running, and is it healthy?" before configuration.
Screen content is capped at **1240 px and centred** on windows wider than that (the rail keeps its
216 px; a 1920 px window would otherwise stretch the cards), and scrolls in one `ListView`
(`cards-list`). It has two sections:

- **Running now** — one full card per engaged tool: 3 px success accent, title 16/600, a
  `StatusChip` (running/starting/stopping), a `foreground`/`resident` scope word (Hold Space has none —
  it acts through the board, not the foreground window), the core's one-line message, its countdown in
  mono, the core's rows as a table with `ROW / STATUS / DETAIL / NEXT` headers (status is plain
  `onSurfaceVariant`, not a success colour — the protocol has no per-row OK flag), the finished/refusal
  lines verbatim, and **Stop** as the one action (error-container button). The title takes the available
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

## Config surfaces (Phase 2, partly landed)

The shared frame is unchanged in structure — header (22/600 title, prose-capped description, one
dirty/status chip), a scrolling editor, and a save bar — but the pieces the audit found missing are in:

- **Section panels.** Every `EditorSection` is a `Material` panel (surfaceContainerLow, 10 px radius,
  one hairline) with 16 px inside and 16 px between. Panels give each tab the Run screen's structure
  without a per-tab redesign, and they are the container a `ListTile`-based field needs (a coloured
  `Container` would assert).
- **Editable content first.** The scaffold renders the editable child before the `readOnly` block, so
  calibrated geometry is demoted instead of owning the first screenful (§2.1/§2.5.3).
- **Save bar reports outcomes only.** The duplicate "Editing"/"No changes" label is gone; the header
  chip states dirtiness once, and the bar renders save errors/success/warnings.
- **Buy is its own screen** (`BuySellTab(section: buy)` → rail "Buy"): saved presets only (the `presets`
  half of `buy_sell`), dirty-gated Save, no geometry block. The run choice stays on the Buy card.
- **Sell is its own screen** (`BuySellTab(section: sell)` → rail "Sell"): the sell-cap field (the
  `sell_cap` half, saved) plus the per-run slot picker (never saved; started from the Sell card). The
  grid is 26 px cells with a 2 px gutter, a **check + inset ring** on selected cells (`Semantics(selected)`),
  a single count line with Select all / Clear, row shortcuts and the cap status under it; captions are
  prose-capped. Both screens save disjoint fields, so each keeps one primary action.
- The combined (`section: both`) layout remains for the widget tests and is not used by the shell.

## Known interim states in this build

- **The shared frame is only partly applied.** Buy/Sell are split and every tab gets section panels, but
  the per-surface redlines for Spammer, Tuner, Gem, Quest, Pet and the **Calibrate workspace** (capture
  shelf, target tree, stored-coordinate table, reference gallery) are not built; Calibrate is still the
  one-form screen. The plan's Phases 3–5 remain.
- **Named geometry profiles / setup import-export** are not in this pass: one geometry per machine with
  the warn-and-recalibrate story stands; profiles + import/export are recorded as a future
  config-root feature in [OPEN.md](../backlog/OPEN.md).
- Phases 3–5 of the plan (Pet/Calibrate workspace, remaining surfaces, session-shaped calibrations) are
  **not built**.
- **Named geometry profiles / setup import-export** are not in this pass: one geometry per machine with
  the warn-and-recalibrate story stands; profiles + import/export are recorded as a future
  config-root feature in [OPEN.md](../backlog/OPEN.md).
- Phases 2–5 of the plan (shared config frame + Buy/Sell split, Pet/Calibrate workspace, remaining
  surfaces, session-shaped calibrations) are **not built**.
