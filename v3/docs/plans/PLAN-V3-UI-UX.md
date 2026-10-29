# Plan — v3 shell UI/UX polish (design proposal)

**A proposal, not a description of what exists.** This plan is the design review requested after a
rendered audit of the v3.0.6 shell. It changes no behaviour, no protocol message and no config field:
every item here is presentation, wording, placement or a shared widget. The shell's hard rules
([PLAN-V3-SHELL.md](PLAN-V3-SHELL.md) §5: the shell never decides, the core never draws) are untouched.

> **Status lives only on the board in [README.md](README.md).** This plan's own text is the design and
> the reasoning; it is not the status.

**Evidence (local, uncommitted):** 21 renders of the real shell at 1280×720 and 960×640
(`v3/.scratch/ui-audit/shots/`), a token dump from the live `ColorScheme`
(`v3/.scratch/ui-audit/theme-dump.txt`), three Refactoring UI audits (Workflow D, sweep order
hierarchy → layout → typography → color → depth → finishing): `findings-tabs-a.md`, `findings-tabs-b.md`,
`findings-system.md`, plus `calibration-inventory.md` (all 18 kinds / 35 marks, the capture model and
the 8 required states — verified against PROTOCOL §12.7/§12.8 and CALIBRATION §3/§5/§7). Interactive
demo: `v3/.scratch/ui-audit/ui-demo.html` (open in a browser; `#run`, `#tuner`, `#pet`, `#buy`,
`#sell`, `#calibrate`; "Design notes" toggles rule citations; "Compare with today" overlays the
current build's render; "Demo states" includes all-idle, empty, save-conflict and stale-display).

---

## 0. Decisions taken (2026-10-29, from the review)

These supersede the corresponding recommendations made earlier in this file; where the text below
still reads as a question, the answer is in this table.

| # | Decision | Reference |
|---|---|---|
| D1 | **Navigation is a left rail** (Run / Tools / System). No grouped top tabs. | §3 |
| D2 | **Hold Space becomes its own card**, at the same level as the other tools. The top-bar chrome toggle is removed; its phase, Start/Stop and refusal line live on its card. It has no config screen (nothing to configure). | §6.1 |
| D3 | **Buy and Sell become separate tool screens** in the rail. Buy owns the saved presets; Sell owns the per-run slot selection (which is never saved). One job per screen, each with its own primary action. | §6.3, §6.4 |
| D4 | **Calibrate becomes a workspace**, not a one-form screen: multiple captures (the core keeps 4), all targets grouped by tool, a gallery of calibrated/reference images, and a table of every stored mark with the display environment it was measured on. It must be able to grow into v2's session-shaped calibrations (tooltip hover, tuner 3-box, pet crops) — those are shown with what they need, not hidden. | §6.5 |

What D2–D4 imply before anything is built:

- **D2 needs no protocol change.** `ToolId.holdspace` already has its own state and a `state`
  event (`lib/protocol/types.dart`); only the shell's chrome toggle and the Spammer-card report move.
- **D3 is a layout + config-scope change, not a protocol change.** `buy` and `sell` are already
  separate `ToolId`s with separate run arguments (PROTOCOL §12.6); the only protocol-shaped question
  is that `buy_sell` is one config block today, so the two screens must write disjoint fields of it.
- **D4 splits into two halves, and the split is a protocol fact, not a preference.**
  *Supported today:* 18 kinds / 35 marks (12 untargeted + 6 named-instance kinds expanding to 23
  marks). The core keeps exactly **four frames** in memory (FIFO); every `calibration.save` requires a
  **live** `frame_id`, and an evicted id answers `bad_request: unknown frame_id` — the UI must require
  a new capture, never auto-recapture. A saved mark stores **no frame reference**: `local.yaml` gets the
  mark, the optional artifact and the display environment only; the frame is write-time provenance.
  Each capture carries its **own `DisplayInfo`**, and `calibration.save`'s optional `display` is trusted
  without a frame↔display cross-check, so the capture shelf must pair every frame with its own display
  or record the wrong environment. There is **no profile concept**: a second display setup means a
  second config root, not a UI profile.
  *Not supported yet (separate plan):* `tuner_region`/three-box derive is withdrawn, the tooltip
  two-capture hover session is absent, pet reference crops are live-screen session features, and v2's
  annotated `calib_tuner.png`/`calib_gem.png` are not persisted (CALIBRATION §5/§7). A
  `calibration-inventory.md` (local, uncommitted) enumerates all 18 kinds and the 8 required states.

---

## 1. What the render showed

The audits found no WCAG **text** contrast failure (minimum measured text pair 7.21:1) and no deadlock,
no missing-state crash. The problems are structural and consistent, and three of them are visible in the
first frame the player ever sees:

| # | Problem | Evidence | Severity |
|---|---|---|---|
| 1 | **No hierarchy on the Cards screen.** Seven equal 360 px cards, six showing `No state published yet` / `No rows to show`; five identical filled Start buttons; the running tool is not visually the subject. | `shots/01-cards-live.png`, `shots/30-narrow-cards.png` | P0 (§2.1) |
| 2 | **Reference data leads every config tab.** Pet opens with ten rows of raw arrays (`[100, 200, 300, 400]`) filling the first screen; Tuner/Gem/Buy-Sell lead with "Calibrated geometry — read-only this wave" before any editable control. | `shots/106-tab-pet.png`, `shots/102-tab-tuner.png`, `shots/104-tab-buysell.png` | P0 (§2.1, §2.5.3) |
| 3 | **Prose measure is the full pane** (~1246 px ≈ 190–210 characters/line at `bodySmall` 12 px) against the 45–75 target; the buying/selling "irreversible" sentence is the easiest thing to skim past. | `shots/104-tab-buysell.png`, `shots/106-tab-pet.png` | P0 (§4.3) |
| 4 | **Action rank drifts.** Calibrate shows two filled primary buttons (Capture, Save mark); Buy/Sell's Add is filled while Save is a disabled ghost; seven cards each fill a Start. | `shots/111-tab-calibrate.png`, `shots/104-tab-buysell.png` | P1 (§2.8) |
| 5 | **Status has no language.** `error` carries blocking errors, self-resolving "Loading…", advisories and refusals; success is `primary`; "unsaved" is decorative `tertiary`; logs print every level in one grey. | `shots/02-cards-states.png`, `shots/103-tab-gem.png` | P1 (§5.2.3, §5.7) |
| 6 | **No token layer above M3.** 93 raw width literals (22 values), 5 corner treatments, no declared `textTheme`, no elevation/shadow set, two `bodySmall`/`bodyMedium` ratio 115:14 so the app's information tier is 12 sp. | `findings-system.md` inventory | P1 (§1.5, §3.2, §4.1) |
| 7 | **Empty, disabled and in-flight states are undesigned.** Empty = a grey sentence; disabled controls stay visible and unreadable; no card shows a command in flight while every async config action does. | `shots/01-cards-live.png` | P1/P2 (§8.4, §2.7) |
| 8 | **Twelve flat, scrollable tabs** carry the whole navigation; no grouping, no icons, no memory of the last tab. | `shots/01-cards-live.png` | P1 (§3.1.2, §2.1) |

Deliberate conventions this plan keeps: compact density for a tool used beside a full-screen game;
dark-only theme; one Save per config tab; core sentences rendered verbatim; per-run sell selection;
calibrated geometry read-only in the config tabs; Material 3 component shapes.

---

## 2. Design principles for this pass

The shell is a **companion cockpit**: glanced at during play, configured deliberately between runs.
That makes the priorities:

1. **Running state is the subject.** The window's job is to answer "what is running and is it healthy?"
   before anything else. Configuration is secondary and should get out of the way.
2. **One primary action per surface.** Save on a config tab, Start/Stop on a run surface. Everything
   else is tonal, outlined or text. (§2.8)
3. **The core's state, not the shell's prose.** Prefer a status chip, an icon+label pair and a countdown
   over sentences; when prose is unavoidable, cap the measure. (§4.3, §5.7)
4. **Systems over values.** Every distance, colour, type size and radius comes from a named token. A
   new field or tab must not invent a width. (§1.5, §3.2)
5. **Designed states.** Every state the player is guaranteed to see (first run, nothing running, no
   presets, clean Save, refused command, conflict, core crash) has a designed treatment and a next
   action. (§8.4)

---

## 3. Layout: rail + content, not twelve tabs

**Proposal:** replace the 12-tab `TabBar` with a 216 px left rail in three groups, and keep the current
content pane.

```
RUN      Cards                       (8 tool cards, including Hold Space)
TOOLS    Skill Spammer · Magic Tuner · Gem Composer · Buy · Sell · Quest Hand-in · Pet Feeder
SYSTEM   Attributes · Arduino · Hotkeys · Setup · Calibrate
```

- Grouping is by job, not by tool: *Run* is the live cockpit, *Tools* is one screen per tool, *System*
  is the machine (board, display, keys, calibration).
- **Buy and Sell are two rows, not one** (D3): Buy is a config job (saved presets), Sell is a
  run-prep job (the armed-slot selection, never saved).
- **Hold Space has no rail row** (D2): it has nothing to configure. It exists on the Cards screen and
  the rail's Cards badge counts it as running like any other tool.
- Future tools appear as rows, not as tabs that squeeze the strip; the rail scrolls.
- Keeps the running-state badge on Cards, so the rail answers "anything running?" at a glance.
- Window minimum size becomes enforceable alongside it: **1120 × 680** logical (the rail plus the
  widest rigid row); this is the runner's job (`windows/runner/main.cpp`) and today is missing entirely.

⚑ **Q1 — rail or grouped tabs?** The rail is the recommendation (it fixes 12 flat tabs, the scroll
affordance and the min-size problem at once). The alternative is keeping the top strip and grouping it
into three segmented levels; cheaper, but the 12-tab scan problem stays at 1280.

---

## 4. The token layer (the foundation)

Everything after this section is a substitution; without it the same drift returns. One file
(`lib/ui/theme.dart`) plus one new `lib/ui/tokens.dart`.

### 4.1 Colour — extend `buildShellTheme()` with a semantic layer

Keep `ColorScheme.fromSeed(0xFF3B82F6, dark)` (deliberate, no brand rework). Add a `ShellColors`
`ThemeExtension` for the meanings M3 does not name, then stop using raw roles for state:

| Semantic | Dark value (tuned for `surface #111318`) | Replaces today |
|---|---|---|
| `success` / `onSuccess` | `#7EE2B8` / `#04291C` | `primary` for "ready", raw `Colors.green` in Arduino |
| `successContainer` / `onSuccessContainer` | `#10382B` / `#B9F5DA` | ready banners, "Saved" |
| `warning` / `onWarning` | `#F2CC7B` / `#2A1E00` | `tertiary` for "unsaved", over-cap, cooling |
| `warningContainer` / `onWarningContainer` | `#3E2F10` / `#FFE2A8` | advisory banners |
| `info` | `#9ECAFF` | transient/loading lines (currently `error`) |
| `blocked` | `colorScheme.error` | refusals, validation failures (unchanged role) |

Rules that come with it: **colour is never the only channel** — every chip has a dot + word, every
selected bag cell has a check, every banner has an icon. `error` means "blocked and requires action";
`info` means "working/transient"; success stops being `primary`.

### 4.2 Type — declare `textTheme` once

M3's default `titleSmall` == `labelLarge` (14/500) is why nested group titles look like section titles.
Declare the scale explicitly (Roboto; Windows resolves the same roles):

| Role | Size / weight / line-height | Used for |
|---|---|---|
| `headlineSmall` | 22 / 600 / 1.25 | page title (one per screen) |
| `titleMedium` | 16 / 600 / 1.3 | card/dialog title |
| `titleSmall` | 14 / 600 / 1.35 | section title |
| `labelLarge` | 13 / 600 / 1.3 | strong inline label ("Boarding now") |
| `bodyMedium` | 14 / 400 / 1.5 | body, field values, prose |
| `bodySmall` | 12.5 / 400 / 1.5 | helper, meta, caption — **never prose alone** |
| `labelSmall` | 11 / 600 / 1.3 | chips, status strip |
| mono | 12.5 `Consolas` / 400 | coordinates, counts, logs |

Prose (descriptions, help, warnings) moves from `bodySmall` to `bodyMedium` and is wrapped in a
`ShellProse` widget capped at **`ShellWidth.prose = 620`** (≈60 characters at 14 px). Data, tables and
fields keep full width.

### 4.3 Space, size, radius, elevation

- Keep `ShellSpace` 4/8/12/16/24; add `xxs = 2` (the audits found 17 legitimate 2 px seams).
- Add `ShellWidth`: `prose 620`, `fieldSm 140`, `fieldMd 170`, `fieldLg 220`, `picker 240`,
  `labelColumn 160`, `rail 216`.
- Add `ShellRadius`: `s 6`, `m 10`, `l 14` — one for controls, one for panels, one for cards/dialogs.
  `999` for chips only. Ban new radii at call sites.
- Depth by surface level, not shadow stacks: page `surface`, panel `surfaceContainerLow`, raised card
  `surfaceContainerHigh`, well/inset `surfaceContainerLowest`; 1 px lighter top edge
  (`rgba(255,255,255,.04)`) plus a single contact shadow `0 1px 2px rgba(0,0,0,.35)`. No `BoxShadow`
  written per widget.
- `cardTheme` and `inputDecorationTheme` set once so `Card` and every field are consistent by default;
  `materialTapTargetSize` stays padded, minimum interactive box 36 px (bag cells 28 px + 4 px gap).

---

## 5. Shared components (new `lib/ui/components/`)

These replace the per-tab re-derivations the audits list as one systemic cause. Dart names are
suggestions; behaviour, anatomy and keys matter.

| Component | Anatomy | Replaces |
|---|---|---|
| `AppShell` | rail + top bar + content + status strip + logs drawer | `ShellPage` chrome, 12-tab `TabBar` |
| `PageHeader(title, subtitle, actions, status)` | 22/600 title, prose capped at 620, right-aligned actions, one `StatusChip` | hand-rolled header rows in each tab |
| `SectionPanel(title, description?, actions?, child)` | panel surface, 14/600 title, actions tonal/text | `EditorSection`, ad-hoc `Card`s |
| `StatusChip(kind, label)` | dot + word + semantic colour (success/warning/error/info/muted) | `_PhaseChip`, `Chip`s, raw coloured text |
| `StatusBanner(kind, title, detail, action?)` | icon + container colour + one action | save errors/conflict/external-change lines, Pet readiness |
| `SaveBar(status, dirty, onSave, onDiscard)` | outcome text ("All changes saved · rev 7" / "Unsaved changes"), Discard ghost, Save filled **only when dirty** | `_SaveBar`'s 5 stacked `bodySmall` lines |
| `ReadOnlyPanel(title, rows)` | collapsed by default, `surfaceContainerLow` well, values as `x 120 · y 54 · 60×60` | Pet's ten-row JSON block, read-only geometry everywhere |
| `InfoRow(label, value, mono?)` | 160 px label `bodySmall onSurfaceVariant`, value `bodyMedium onSurface` | `ReadOnlyValue`, Setup's hardcoded 220 px column, Arduino's space-padded rows |
| `EmptyState(icon, title, hint, action?)` | panel-sized, one sentence, one optional action | grey "No rows to show", "0 of 143 attributes", missing-preset dead controls |
| `AsyncButton` | the button + in-flight spinner + disabled-while-pending | every async action except Save |
| `BagGrid(cells, selected, onToggle, columns: 8)` | ≥28 px cells, check + inset ring on selection, `Semantics(selected)` | sell grid, Pet food grid, Pet bag grid |
| `LogDrawer` | 238 px bottom drawer, level badges with colour, All/Errors filter, Ctrl+L | fixed 150 px log panel, uniform grey lines |
| `CaptureStrip` | capture thumbnails (max 4: the core's FIFO), label + time + physical size + scale, "capture" action, note that a fifth drops the oldest | one implicit "last capture" |
| `TargetTree` | tools → kinds → instances, semantic status dot (saved / partly / not set / needs session), mark counts | one flat kind dropdown |
| `StoredMarksTable` | every mark: surface, tool, shape, `x, y · w×h`, artifact, status; filter by tool; "copy as YAML" | none (marks only visible one at a time) |
| `ArtifactGallery` | reference images with thumb, provenance and presence: result-gem crop (present/not captured), pet crops (run-learned), v2 screenshots (v2 only) | none |
| `SessionList` | session-shaped calibrations with what they need and a disabled "Run session" until the protocol supports them | nothing (they were simply absent) |
| `DisplayMatchBanner` | captured vs stored `scale`/client-size/screen, mismatch text + next action | no Detect surface at all (CALIBRATION §6 says the surface is missing) |

`ShellProse` (from §4.2) is used by `PageHeader`, `SectionPanel.description`, `EmptyState.hint` and every
help/caption call site — it is the one-line fix for the P0 measure finding.

---

## 6. Surface redlines

### Cards ("Run")
- **Eight cards, one per tool — including Hold Space** (D2). The top bar loses its Hold Space toggle
  entirely; the card is the only place its state is started, stopped or reported. It has no rail row
  because there is nothing to configure.
- Split by state: **Running now** (full cards, live rows) above **Available tools** (one-line tiles,
  66 px tall, icon + name + one-line state + Start). When nothing runs, an all-idle banner with the next
  step replaces the running section.
- Running card: 3 px `success` accent, `surfaceContainerHigh`, title 16/600, `StatusChip`, one-line
  message, meta line (`next key 4.4 s`), rows table with `Row/Status/Detail/Next` headers, footer with
  the one action (**Stop**, danger container). Hold Space's running card carries its own line
  ("Holding the spacebar · auto-pickup on") and its refusal line — not the Spammer card's.
- Idle tile: outlined Start only; no disabled Stop; no fixed 360 px height; grid
  `minmax(320px, 1fr)` with content-sized rows. Kill `mainAxisExtent: 360`. Hold Space's idle tile says
  "Off · spacebar auto-pickup" — one line, no configuration link.
- Empty/first-run: tiles + `EmptyState("No runs yet — pick a tool and press Start")`; the core's
  "No state published yet" duplicate line disappears (one empty state, not two).
- In-flight: Start shows a spinner + "Starting…" until the core's `state` event arrives (the card still
  never optimistically flips phase — PLAN §5 preserved).

### Config tabs (shared frame)
1. `PageHeader` with prose-capped subtitle, status chip, no action buttons in the header.
2. Editable sections first (`SectionPanel` per group); reference data last, in a collapsed
   `ReadOnlyPanel` titled "Calibrated geometry — drawn on the Calibrate screen" with one "Open Calibrate".
3. One filled action: Save, enabled only when dirty; Discard ghost; status text from `SaveStatus`.
4. Validation: `InputDecoration.errorText` on the field; core refusal in one `StatusBanner` (blocking)
   with the core's sentence verbatim; warnings as info banners; never pre-formatted multi-line text.
5. In-form Add/Rename: tonal; row-level add: text.
6. Dirty state stated once (header chip). Delete the save-bar "Editing/No changes" duplicate.

### Pet
- Header: `Ready to run` / `Not ready — 2 checks` chip; the reason expands into a `StatusBanner` with
  per-item rows (icon + label + detail), not one `\n`-joined `Text`.
- Row card header is the state: `Row 2` + `boarding`/`reloading`/`idle` chip + countdown right.
- `Enabled` and `Food stacks` group as saved fields; `Boarding now` is separated below with an
  `applies immediately` badge, because it writes at once.
- Queue as `InfoRow` table; reference crops/marks in a collapsed `ReadOnlyPanel`.
- Destructive confirm (apply food scan) uses `error`-container affirmative, Cancel focused; label says
  what is replaced.

### Buy (own screen — saved config)
- One job: the saved presets. `InfoRow` table (item, shop row, scroll, count) + tonal Add; Delete
  enabled only when a preset is selected or refusing in the banner; Save is the only filled action and
  is enabled only when dirty.
- The per-run choice (which preset, how many) stays on the Buy **card**, reading the same presets —
  no second copy. The screen says so in one line.
- Collapsed `ReadOnlyPanel` for shop geometry, pointing at Calibrate.

### Sell (own screen — per-run session)
- One job: arm the bag slots for the next run. `BagGrid` with check + inset ring and
  `Semantics(selected)`; one count line under the grid (`12 of 64 selected`, over-cap in `warning`)
  instead of today's two; "next run only · never saved" stated once.
- **No Save bar** — the selection is not config (USER_GUIDE §5.4); the screen's primary action is the
  run (`Sell 12 slots`, danger container, one confirmation because it removes items from the game).
  The card shows the same armed count and runs the same selection.
- Collapsed `ReadOnlyPanel` for bag geometry, pointing at Calibrate.

### Calibrate (workspace — see D4)
Layout at ≥1120 px: **captures + target tree** (250 px) · **canvas** (flexible) · **mark editor +
reference images** (340 px), then **stored coordinates** and **session-shaped calibrations** full width.

- **Captures**: the shelf has at most four frames (the core's FIFO) and shows, per item: thumbnail,
  frame id, physical size/origin, **that capture's own `DisplayInfo` summary**, and an **eviction
  marker on the oldest** ("next capture evicts"). A capture is the game only (`CopyFromScreen`, game
  foreground); the hide → wait one frame → show flow is unchanged. Because every save needs a live
  `frame_id`, a selected mark whose frame has been evicted must say so and require a new capture —
  never auto-recapture.
- **Marks vs provenance**: a saved mark stores the mark, the artifact and the display environment, not
  the frame (verified in the inventory: `calibration.fetch` returns no `frame_id`). The workspace may
  show "measured on frame N" as a hint, but must not claim the frame is persisted; `calibration.save`
  has no `rev` argument (last write wins; `config.changed` arrives after the response).
- **Targets**: grouped by tool — Shop 5/5, Gem 3 kinds/9 marks, Pet 9 kinds/20 marks, Tuner 1/1 —
  18 kinds, 35 marks. Semantic status dot per surface (saved / partly saved / not set / needs session)
  and a mark count; selecting a row drives the canvas and the editor. Named-instance kinds get an
  instance picker (`N/G/DG/Register/Combine`, `ITEM1..3`, row `1..4`).
- **Canvas**: fit-scale with a real zoom control (`fit`, `1:1`, pan) and the factor + display-px-to-
  physical-px line shown; the selected mark is drawn on the frame with its label; `x/y/w/h` typed
  fields are authoritative (CALIBRATION §4's "typed fields are the input") and the drag/arrow-key
  nudge applies to the same mark.
- **Mark editor**: shape-aware fields, target picker, the result-area empty verdict (CALIBRATION §5),
  crop preview for artifact kinds, Revert + Save (Save is the only filled action; Capture is filled
  only while no frame exists — the two never compete).
- **Reference images**: result-gem crop with its state (`present` / `not captured`), pet crops marked
  "run-learned, not a frame mark", and v2's `calib_tuner.png`/`calib_gem.png` marked `v2 only` — the
  gallery tells the truth about what exists (CALIBRATION §5/§7).
- **Stored coordinates**: one table for all 35 marks — surface, tool, shape, value (`x, y · w×h`),
  artifact, status — filterable by tool, with "copy as YAML". This is the "place to store coordinates"
  the one-form screen never had; the source of truth stays `local.yaml` (the core's write gate).
- **Display environment**: the workspace shows the selected capture's `DisplayInfo` (scale, client
  size, screen) and compares it with the stored calibration: a **client-size or screen change
  invalidates every stored point; a scale change alone does not** (CALIBRATION §6/§8). A mismatch is
  `DisplayMatchBanner` with "recalibrate before running a tool" — no auto-anchor, no proportional
  transform. **No profiles:** v3 has one geometry per machine; a second setup is a second config root
  (migration copies `config/`), so the UI warns and recalibrates rather than pretending to switch.
- **States the screen must design** (inventory §4): S1 no capture · S2 capturing (hide → frame →
  capture → unconditional show) · S3 capture refused (window not found / game not foreground / host
  note) · S4 ready-clean (frame + display + shelf) · S5 draft-dirty (validity + eviction risk) ·
  S6 saving/save-refused (`unknown frame_id`, out-of-frame, pitch disagreement, missing target/empty
  verdict) · S7 saved-clean (rev, derived, artifact, `config.changed`) · S8 external/environment
  change. No state renders a raw exception; the core's line stays verbatim.
- **What v2 calibrated that this pass still does not cover** (all need either a protocol command or a
  session; listed, not hidden): tuner three-box derive (grade area + attribute band + remaining band),
  tooltip offset/shape/hover-delay two-capture session, pet capture/crop session (menu 目錄, feed
  icon, row start/end, bag cells; `calib_pet.png`), **Gem movement editor + pointer/cycle tests**,
  **Buy/Sell 64-centre and row overlays + test scroll + "Setup so far" checklist**, and the
  **Setup → Detect** surface (CalibrationCheck is ported and tested; the protocol path/UI is not
  built). All v2 *pixel marks* have parity (`focus_point` → `scroll_point`, `max_button` added).
- **Session-shaped calibrations**: Tooltip offset (two captures: park, hover, box), tuner OCR 3-box
  session, pet reference crops. Each row states what it needs and keeps "Run session" disabled until
  the protocol supports it. **These are not promised by this UI plan** — see Phase 5.

### Attributes / Arduino / Hotkeys / Setup
- Attributes: search + table; an `EmptyState` for zero matches that also hides the unfiltered tables.
- Arduino: port rows as `InfoRow` (icon + port + description + trailing reason), not space-padded
  strings; test actions outlined, cursor-moving click separated and confirmed.
- Hotkeys: accepted-name help once above the group; field helper only on error; raw codes removed from
  the frame (debug tooltip if needed).
- Setup: Detect/Measure/Probe outlined; "Use detected title" is the only filled action.

### Chrome
- Top bar: core status chip (`core ready · rev 7`), Logs, pin, version. **No Hold Space toggle** — it is
  a card now (D2).
- Status strip: core identity left; `Ctrl+L` hint right.
- Logs: drawer, level-coloured badges, All/Errors filter.

---

## 7. Phases

Each phase is one commit series, no behaviour change, screenshots before/after per surface. Widget tests
keep their `ValueKey`s wherever the control survives; tests that assert on removed widgets are updated in
the same commit as the widget.

| Phase | Work | Gate | Est. |
|---|---|---|---|
| **0 · Tokens** | `ShellColors` extension, declared `textTheme`, `ShellWidth/Radius/Size`, `cardTheme`/`inputDecorationTheme`, `ShellProse`, mono style; migrate raw values (no layout change). | `flutter analyze` clean; `flutter test` green; screenshots unchanged except text/colour corrections | M |
| **1 · Chrome + Cards** | `AppShell` rail, `PageHeader`, running/idle split, card + tile components, **Hold Space card** with the top-bar toggle removed, status strip, logs drawer, window min size in the runner. | Cards reviewed at 1280 and 960 with 8 tools — Hold Space included; no overflow; every card Start/Stop still driven only by the core's `state`; the Spammer card no longer reports Hold Space | L |
| **2 · Shared config frame + Buy/Sell split** | `SectionPanel`, `StatusBanner`, `SaveBar`, `ReadOnlyPanel`, `InfoRow`, `EmptyState`, `AsyncButton`; convert Tuner, Gem, Spammer, Quest; **split Buy (saved presets, dirty-gated Save) from Sell (per-run selection, no Save, run action)**, disjoint fields of the `buy_sell` block; per-run choices stay on the cards. | Each screen uses only shared primitives and has one primary action; Buy's Save writes only preset fields; Sell's selection is still never saved; tests updated, `save-*`/`dirty-*` keys preserved | L |
| **3 · Pet + Calibrate workspace** | Pet row headers/checklist/immediate-write separation/queue; Calibrate: capture shelf (4 + eviction), target tree (18 kinds/35 marks), canvas + fit/1:1/pan, mark editor with target/verdict/artifact, stored-coordinate table, artifact gallery, display-match banner, the 8 states. Frame-mark kinds only. | Pet and Calibrate at 1280/960; every state rendered; save still requires a live `frame_id` (evicted frame refuses, no auto-recapture); drag/typed-field flow unchanged | L |
| **4 · Remaining surfaces + polish** | Attributes, Arduino, Hotkeys, Setup, crash/connecting surfaces, empty states, a11y names on icon-only controls, focus order, docs. | Full 13-surface screenshot pass; `flutter test`; USER_GUIDE (and zh-TW twin) updated where labels changed | M |

**Phase 5 — separate plan, requires core/protocol work.** The Calibrate workspace is built to host
these, but does not promise them: session-shaped calibrations (tooltip two-capture hover, tuner
3-box derive, pet capture session), v2's annotated `calib_tuner.png`/`calib_gem.png`, the Gem movement
editor + pointer/cycle tests, Buy/Sell 64-centre/row overlays + test scroll + "Setup so far", the
Setup→Detect surface, and any multi-display profile concept. Each needs a command, a config field or
a session in the core; none can be done by the shell alone.

Out of scope for Phases 0–4: any core/protocol/config change; MVVM/binding rewrite; light theme;
mini-mode geometry (the shell's `MiniModePlaceholder` stays a stub — PLAN-V3-SHELL §6 defers it);
new tools.

---

### 7.1 Phase 0 scope note (2026-10-29)

Phase 0 migrated the token-matching widths (`170 → fieldMd`, `220 → fieldLg`, `240 → picker`,
`160 → labelColumn`, `140 → fieldSm`) across the existing surfaces and replaced the raw
`Colors.green` in the Arduino tab with `ShellColors.success`. A surface's remaining one-off widths
(icon cells, canvas rails, tab-specific field sizes) fall when that surface is rebuilt in Phases 2–4;
they are not re-guessed in Phase 0 because the layout is not supposed to move yet. Renders:
`.scratch/ui-audit/shots/phase0-{1280,960}-*.png` (all 12 surfaces) against the original
`shots/*.png`.


Phase 1 landed 2026-10-29: the 216 px rail (RUN/TOOLS/SYSTEM), the top bar without the Hold Space
toggle, the status strip and the 238 px Ctrl+L logs drawer; running tools as full cards above
content-sized idle tiles (eight tools, Hold Space included as its own card); an empty-first-run state and
the in-flight Start label; the 1120×680 minimum enforced by `WM_GETMINMAXINFO` in
`windows/runner/win32_window.cpp` (the plan said `main.cpp`; Win32 requires the handler in the window
class, so the constant lives there). Buy and Sell rail rows both open the existing Buy/Sell tab until
Phase 2 splits them. Renders: `.scratch/ui-audit/shots/phase1-*.png`.

## 8. Verification for each phase

- `flutter analyze` and `flutter test` from `v3/shell/` (the existing 142-test suite is the regression
  gate; every keyed control that survives keeps its key).
- Screenshot the four run states (idle, running, refused, crash) + the changed tab at 1280×720 and
  960×640; add the shots to the phase commit's message or `v3/.scratch/ui-audit/shots/` (gitignored).
- No hard-coded colours/widths/radii introduced: grep the diff for raw `Color(`, `Colors.`, numeric
  `EdgeInsets`, `fontSize:`; every one needs a token or a comment why not.
- `USER_GUIDE.md` and its `zh-TW` twin updated in the same commit when a user-facing label changes.

---

## 9. ⚑ Still open (D1–D4 above are fixed)

1. **Density — keep compact 12 sp meta, or move body prose to 14 sp?** Recommendation: 14 sp body for
   prose/help (readability at a glance), 12.5 sp for meta/captions, `VisualDensity.compact` kept for
   controls. This changes the feel of every screen.
2. **Card action model — Start always visible on idle tiles, or only on hover?** Recommendation: always
   visible (keyboard paths exist, and the tool is used with a mouse). Cost: seven outlined Starts
   visible at rest.
3. **Logs — bottom drawer (recommended), right-side panel, or separate window?** The drawer keeps the
   form's width; a side panel would eat 320 px permanently; a separate window is more work.
4. **Accent — keep the M3 blue seed, or introduce a warm "seal" accent for the running state only?**
   Recommendation: keep the blue scheme; the semantic layer adds green/amber, which is all the
   differentiation the shell needs.
5. **Calibrate zoom scope — fit + 1:1 + pan in this pass, or fit + 1:1 only?** Recommendation: fit + 1:1
   + pan, because a ±1 display pixel error is ±5.6 physical pixels on a 2865 px frame.
6. **Multi-setup storage — is "one geometry per machine + warn and recalibrate" enough for now?**
   Recommendation: yes for this pass. v3 has no profile key; a second setup means a second config root
   (migration copies `config/`). Named in-workspace profiles would be a core/config feature, not a UI
   change.
7. **Calibrate scope split — ship the frame-mark workspace (Phase 3) and defer the session-shaped kinds
   to Phase 5?** Recommendation: yes. The 18 supported kinds cover every v2 pixel mark; the sessions
   (tooltip, tuner 3-box, pet crops, annotated references, Gem movement editor, Buy/Sell overlays,
   Detect) each need a protocol command.
8. **Sell run placement — start the run from the Sell screen (recommended, one confirmation) or keep
   run only on the Sell card?** The screen owns the selection; starting from it is one click instead of
   a navigational detour, and the card keeps the same action.

---

## 10. What this plan does not claim

- It does not claim the audits' code-only findings are all confirmed: 21 states of the current build
  were rendered, and the findings files mark each row `code-only` or `rendered`; the P0s above are
  rendered.
- It does not claim a new visual identity. The palette, fonts, density and Material 3 shapes stay; the
  plan re-ranks, re-measures and names what already exists.
- It does not touch the core, the protocol, `local.yaml`, or any tool's decision logic.

---

## 11. Protocol / core gaps this design runs into

Found while designing the workspace; each is a fact about today's protocol, not a UI wish. Phases 0–4
must not pretend to fix them.

| Gap | Today (verified) | What the UI does instead | Needs |
|---|---|---|---|
| Marks cannot store a frame | `calibration.save` persists mark/artifact/environment; `fetch` returns no `frame_id` (PROTOCOL §12.7) | Show "measured on frame N" as session hint only; saving still needs the live frame | — (correct as-is) |
| 4-frame FIFO eviction | `win::FrameCache` capacity 4; an evicted id is `bad_request: unknown frame_id`; never auto-recapture (PROTOCOL §8) | Shelf shows the eviction marker; a mark bound to an evicted frame forces a new capture | — (correct as-is) |
| Per-capture `DisplayInfo`, un-cross-checked | each capture returns its own display; `save`'s `display` is trusted | Shelf pairs each frame with its own display and shows it | core-side frame↔display cross-check would be stricter |
| No display profiles | one geometry per machine; no profile key in `local.yaml` (CALIBRATION §8; inventory) | Warn + recalibrate; a second setup = second config root | config-root/profile feature |
| Tuner three-box derive | `tuner_region` withdrawn; `tuner.ocr` hand-edited (CALIBRATION §7) | Target shown as "needs session"; workspace does not fake it | multi-mark session command |
| Tooltip hover | two-capture session absent (CALIBRATION §7) | Listed as "needs protocol" | session command + config fields |
| Annotated reference screenshots | only the result-gem crop exists; `calib_tuner.png`/`calib_gem.png` are v2 only | Gallery marks them `v2 only` | save-side rendering of an annotated frame |
| Detect / CalibrationCheck surface | rule ported and tested; no command/UI (CALIBRATION §6) | Display mismatch banner uses the stored fields the shell has | protocol path for the comparison |
| Gem movement editor, Buy/Sell overlays, setup-so-far | v2 calibration surfaces not ported | Noted as Phase 5 | each is a designer/session feature |
| `calibration.save` has no `rev` | last write wins; `conflict` exists only for `config.apply` | No conflict UI for calibrate; `config.changed` refreshes the mirror | optional rev guard |

### 9.1 Answers (2026-10-29, taken before Phase 0)

All eight were answered: **14 sp body / 12.5 sp meta with compact controls** kept; **Start always
visible** on idle tiles; **bottom logs drawer**; **keep the blue M3 seed**; **fit + 1:1 + pan** on the
Calibrate canvas; **one geometry per machine + warn/recalibrate** now, with named profiles and setup
import/export recorded as a future config-root feature (OPEN §5) rather than a UI-only change;
**frame-mark workspace now, session-shaped calibrations in Phase 5**; and **Sell keeps v2 behaviour**:
the Sell screen arms the per-run slots and the **run starts from the Sell card**, not a primary button
on the screen (this overrides Q8's recommendation; Phase 2 must implement it that way).
