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
- **Top bar (48 px):** `Seal Tools`, a core status chip (`core ready · rev <n>`), **Logs (N)**, the
  always-on-top pin (`app-bar-always-on-top`) and the product version. **There is no Hold Space toggle.**
- **Status strip:** core version/rev and the core's root on the left, `Ctrl+L logs` on the right. It
  advertises the shortcut; the drawer is opened from the top bar or Ctrl+L.
- **Logs drawer (`LogDrawer`):** 238 px along the bottom of the content, a level badge per line
  (info/warn/error; stderr is treated as error), origin shown for stderr/shell lines, an
  **All / Errors** filter and a close button. Monospace body. Ctrl+L toggles it.

## Cards screen

The screen answers "what is running, and is it healthy?" before configuration. It has two sections:

- **Running now** — one full card per engaged tool: 3 px success accent, title 16/600, a
  `StatusChip` (running/starting/stopping), the core's one-line message, its countdown, the core's rows
  as a table with `ROW / STATUS / DETAIL / NEXT` labels, the finished/refusal lines verbatim, and
  **Stop** as the one action (error-container button).
- **Available tools** — one-line tiles: tool icon, name, one-line state (Hold Space says
  "Off · spacebar auto-pickup"; a tool whose snapshot message exists shows it), optional card-specific
  settings (Buy's per-run picker, Quest target, Spammer rotation) and an outlined **Start**. Start is
  always visible (the eight recommendations taken with this plan).
- When nothing runs, an **empty state** ("No runs yet") with the next action replaces the running
  section; the tiles follow. There is no fixed card height: the running cards are content-sized and the
  tiles wrap 3/2/1 columns by width.
- **In-flight:** a Start/Stop disables and labels itself ("Starting…"/"Stopping…") with a spinner until
  the core's next `state` arrives. The card's phase never moves optimistically.
- **Hold Space is a card** at the same level as the other tools: Start holds the spacebar, Stop releases
  it, its phase/message/refusal render on its card, and it has no config screen. (Its failed release is
  still only in `holdspace.log` — the release has no protocol event.)

## Known interim states in this build

- **Buy and Sell rail rows both open the existing Buy/Sell tab.** Phase 2 splits them into a Buy screen
  (saved presets, dirty-gated Save) and a Sell screen (per-run slot arming, no Save). The player's
  decision recorded from the plan's questions: **Sell keeps v2 behaviour — the run is started from the
  Sell card**, not a button on the Sell screen.
- **Named geometry profiles / setup import-export** are not in this pass: one geometry per machine with
  the warn-and-recalibrate story stands; profiles + import/export are recorded as a future
  config-root feature in [OPEN.md](../backlog/OPEN.md).
- Phases 2–5 of the plan (shared config frame + Buy/Sell split, Pet/Calibrate workspace, remaining
  surfaces, session-shaped calibrations) are **not built**.
