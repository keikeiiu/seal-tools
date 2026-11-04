# v3 progress log

A dated, append-only record of the v3 core build and the reasoning that is not visible in the code or
the commit titles. Newest first, like v2's
[PROGRESS.md](../../../v2/docs/records/PROGRESS.md); the entries are grouped by MILESTONE rather than
one per commit (the whole build happened on one day; the entries below continue past the header's
first-session range).

**This is the HISTORY, not the current truth.** How the system works now is
[DESIGN.md](../spec/DESIGN.md); what was ported and what is missing is
[PORT-COVERAGE.md](PORT-COVERAGE.md); what is open is [OPEN.md](../backlog/OPEN.md). When a later
entry supersedes an earlier claim, the spec is what changed — not this file.

**How to use it:** append an entry when a milestone lands. Say what was built, why it is shaped that
way, what it cost or taught, and cite the commits. Do not restate what a spec already says.

**On test counts:** a count quoted in an entry is the count AT THAT MILESTONE. They grow; the command
(`.\cargo.cmd test --workspace --offline` from `v3/core`) is the only current source.

---

## 2026-11-05 (42) — Phase 1 and Phase 2 review work, except engine/library swaps

The player asked to finish the remaining Phase 1 and Phase 2 review findings, leaving out anything
that needs an engine or library swap. The work landed in focused commits (one per finding where the
change was separable; the large cross-crate groups name their ids):

- **OCR** (`0da1994`): F1 unclipped crop, F2 bbox-bounded scoring/contour work, F3 det max-side and
  padding, F4 corner order, F5 left-to-right grade line, F9 one shared model set, F10 optional
  classifier, F12 cubic/area upscale, F14 fixed-layout reads skip detection, F16 allocation-light
  contour tracing.
- **Config/board/HW** (`1b1f1c3`, `f0bfa43`, `5caf9eb`): HW-F3/F4/F5 migration + verify + durable
  fsync/retry, HW-F6/F7/F8/F9/F10/F11 firmware/serial/placement work, HW-F13 RAII cleanup of the
  existing GDI backend, HW-F14 foreign keys (earlier), HW-F15 corrupt-file quarantine; CORE-F15
  rejects absolute/UNC artifact references.
- **Pet/tools** (`e2f2d3d`, `252e458`, `07bbcc6`): PET-F8/F9/F11/F12/F13/F14/F15/F16, TOOL-F10's
  lease check surface, TOOL-F11/F12/F15/F16, TOOL-F13 press-intent-before-write.
- **Core** (`a72cbf4`, plus `ddffd1a`): CORE-F7/F8/F10/F12/F14; CORE-F9 framing (earlier).
- **Shell** (`f8dd35f`, `b6bd31e`, `2012172`, `43daee9`): SHELL-F3 single frame decode,
  SHELL-F6 dirty-draft close guard, SHELL-F9 release gates/checksums/no-signing, SHELL-F10 pubspec as
  version source, SHELL-F11 generated notices.
- Tooling: `991feaf` greens the strict clippy/rustdoc gates, `e088d0a` fixes the frame-cache test's
  harness height.

**Deliberately excluded:** OCR-F6 (DirectML/multi-thread runtime engine), OCR-F13 (recogniser model
swap), SHELL-F13 (l10n framework), SHELL-F14 (protocol codegen), CORE-F13 (typed schema codegen),
HW-F12 (Windows.Graphics.Capture/DXGI), and real code signing (needs an external certificate).

Later in the same round: **SHELL-F5** (`ce09009`) scoped event/log rebuilds to the widgets that
render them (cards/mini/dock self-listen; logs drawer/status/connecting view own their listener) and
**TOOL-F10** (`1aa9ce7`) gave long calibration sequences a real held lease — the supervisor refuses
starts, game requests and other pointer actions while the 21-step gem cycle owns the game. The OCR
regression runner landed as `ocr_eval` with the `fixtures/ocr` schema (`6397fe2`, OCR-F11); the
model-free half of OCR-F15 (`23efff6`) matches one-character substitutions by bounded edit-distance
windows; HW-F7's FW_VERSION 3 is now pinned in the v2 sketch test too (`e968b10`). CORE-F11's frame
layer landed in `aedf627`: FrameCache keeps raw PNG bytes plus a once-decoded RGBA image instead of
base64 `Value`s, so scans/crops no longer re-decode and base64 is re-encoded only for a response.

**Known gaps in this batch:** the OCR-F11 corpus has the runner/schema but no recorded real-game
cases yet, and OCR-F15's full CTC/logit scorer still needs retained model logits; CORE-F11's config
sharing (an `Arc<Value>` mirror) is still not done — only the frame layer; HW-F7's firmware still
needs a real-board reflash/verification; SHELL-F15's README rewrite stays at publish per CUTOVER.

Verification: Rust fmt/clippy/rustdoc clean; `cargo test --workspace --offline` **95 suites, 1,352
passed, 0 failed, 6 ignored**. Shell analyze clean and **371/371** tests. The v2 firmware sketch test
is 4/4. The launcher was rebuilt and restarted on the new binaries.

## 2026-11-05 (41) — the remaining Claude-review Phase 0 fixes

The player asked to work the still-open review items one by one. All ten Phase-0 findings left after
the earlier batch (and TOOL-F7's tuner half, already done in the SEAL-1 work) landed, each with a
regression test and its own commit:

- **PET-F2** (`68f64a5`): the scheduled wait is now subtracted from every row's relative schedule, so
  a timer-due row is acted on even when its slot/feeders stay unreadable; it no longer reschedules the
  configured cycle forever.
- **PET-F4** (`2db798e`): every input step re-checks the game foreground; Open defers, a lost
  foreground drops queued input, failed VerifyEmpty/ScanPage reads abandon the visit, and no Close
  click is sent when the game is gone.
- **TOOL-F3** (`fdb427b`): `BoardEffects` takes a `DisplayProvider`; production tool sessions re-find
  and re-measure the configured window per placement, so a moved/minimised game is the domain's
  `window_missing` line rather than a blind click at stale coordinates.
- **CORE-F3** (`6e365df`): a `DoneGuard` catches worker panics and always sends the exit (slot freed,
  shutdown returns); a panic hook writes the panic + backtrace to `error.log`; the dispatcher is
  wrapped in catch-unwind; a dead supervisor is `Internal`, not `not_running`.
- **CORE-F4** (`fc8b75b`): the dispatcher refreshes `state.config` from the shared mirror when a
  worker bumped the revision and only republishes when the command itself wrote; `bump_revision` uses
  the shared counter.
- **HW-F1** (`f9cb667`): one `merge_with_policy` now routes all three config merges; `presets` and
  `sequences` are replaced whole, so a deleted flow/sequence stays deleted.
- **HW-F2** (`b52c7f1`): backups are fsynced; the transaction marks a successful commit with
  `journal.done` (checked) and recovery never rolls a committed save back; a missing backup refuses a
  partial restore.
- **CORE-F2** (`134e4d3`): `StateCell` publishes through an observer; the worker emits a live `state`
  event for every status change while running (never after Stopping), so cards update mid-run.
- **CORE-F5** (`65795db`): the grade colour crop uses the image dimensions, not the area's, so a
  non-zero-origin grade box no longer silently disables the fallback.
- **PET-F7** (`302ec17`): fold outcomes (strike count included), dropped-row reasons and veto notes
  reach `pet.log`; every-row-failed is an error stop.

Verification: workspace clippy/fmt/rustdoc clean; `cargo test --workspace --offline` **94 suites,
1,254 passed, 0 failed, 6 ignored**. Shell source was not touched by this series.

## 2026-11-04 (40) — Calibrate above the fold, readable light labels, and a quick appearance menu

First UI pass after the capture-once work, driven by the review rather than the backend:

- **Above-the-fold Calibrate:** the fixed workspace now fills the height left after the header/tool tabs.
  The capture shelf, display environment + Detect, and the stored-marks table moved behind the compact
  capture bar's **Captures / Details / Stored marks** dialogs, so the canvas is visible at the default
  720 px window instead of below the panels. The canvas height is responsive, and the page no longer
  scrolls for mark tools.
- **Light-mode labels:** calibration overlay labels pick white or black text from their background's
  luminance; the fixed green/blue/amber palette no longer leaves white-on-amber labels.
- **Tool order/naming:** tabs are now Buy / Sell → Gem Composer → Magic Tuner → Pet Feeder → Tooltip
  (Pet is not the default; the shop tab remains first). The target panel is titled "Buy / Sell targets"
  rather than "Shop targets"; Mark/Gem/Pet titles follow the same naming.
- **Ergonomics:** the target/inspector panes have visible scrollbars with controllers, the three-column
  breakpoint dropped from 980 to 850 so the 1120 minimum still fits with the rail, and a narrow-layout
  hint points at Ctrl+B. The top bar gained a quick appearance menu (Follow system / Light / Dark)
  alongside Setup → Appearance.

Verification: `flutter analyze --fatal-infos` clean, `dart format` clean, shell `flutter test --no-pub`
**356/356**, Windows debug build green. UI spec and both user guides updated.

## 2026-11-04 (39) — Appearance: light / dark / follow-system lands on the calibration branch

The calibration branch had stayed dark-only while the appearance work lived on `v3-core`. The player
asked for the toggle here, so the existing implementation was ported rather than reinvented.

- Both `ThemeData` brightnesses now come from `buildShellTheme(brightness:)`; `ShellColors` has a light
  semantic set, and `ShellApp` maps the model's `ShellThemeMode` to `ThemeMode` on `MaterialApp`.
- **Setup → Appearance** offers Follow system / Light / Dark and saves immediately as the portable
  `settings.ui.theme` (`system|light|dark`; default dark so existing installs do not change on upgrade).
- The native title bar follows the same choice through the runner's `setTitleBarTheme(0|1|2)` host call;
  system mode keeps following Windows when the system setting changes.
- Core `ensure_portable_ui` seeds the `theme` leaf so a hand-edited or reloaded config carries it.

Verification: shell `flutter analyze --fatal-infos` clean, `dart format` clean, `flutter test --no-pub`
**355/355**, Windows debug build green; `seal-config` tests **24/24**. The `seal-core` config integration
test could not link in this environment because the offline ONNX Runtime is absent; the store/config
crate coverage for the new portable leaf is green.

## 2026-11-04 (38) — the mark workspace becomes a fixed canvas with independently scrolling sides

The demo's central promise is that the game frame and the fields editing it never scroll away from
each other. The mark workspace now has that shape at wide widths.

- **Fixed 620 px pane:** target tree, canvas and inspector share one fixed-height row. The left tree and
  the right inspector each scroll inside their own column; the canvas stays put.
- **Pinned actions:** Revert/Save and their result lines were extracted into a separate action bar under
  the scrolling inspector, so Save is always visible without scrolling the fields away from the frame.
  Narrow windows keep the stacked layout with the actions inside the mark panel.
- **Tests:** a new test checks the 620 px pane, scrolls the inspector and asserts the frame's top-left
  does not move and the inner scroll position advanced. Save taps now ensure the button is visible first,
  matching the new scrollable inspector.
- **Docs:** UI spec and both user guides describe the fixed canvas, independent scrolling, and pinned
  actions.

Verification: `flutter analyze --fatal-infos` clean, `dart format` clean, `flutter test --no-pub`
**352/352**, Windows debug build green.

## 2026-11-04 (37) — each mark tool keeps its own live capture; the shelf moves above the workspace

Continuing the capture-once slice: the tool tab now owns its frame, not just its target list.

- **Per-tool capture memory:** `_toolFrameIds` records which live frame each mark tool captured or
  selected. Switching tabs restores that tool's frame; a tool with no mapping adopts the newest live
  shelf frame instead of starting blank, and selecting a shelf frame assigns it to the active tool.
- **Eviction/refusal hygiene:** when the core's four-frame FIFO evicts a frame, or the core refuses an
  id as unknown, the tool mapping pointing at it is removed so no tab can offer a dead frame.
- **Capture shelf moved** from the target-tree column to full width above the mark workspace: the strip
  sits under the tool tabs, matching the design study's capture-first order. The target-tree column is
  now only what the tool can calibrate.
- **Frame provenance visible:** the canvas caption now includes `frame <id>`, so the active frame is
  unambiguous while the test and the player switch tools.
- Tests cover Shop→Pet→Shop frame restoration (f1, f2, f1), plus the moved shelf and existing flows.

Verification: `flutter analyze --fatal-infos` clean, `dart format` clean, `flutter test --no-pub`
**351/351**, Windows debug build green. UI spec and both user guides updated in the same commit.

## 2026-11-04 (36) — Calibrate gets tool tabs; sessions move into their tool workspace

First implementation slice of the approved capture-once UX (`calibrate-capture-once.html` design
study). The player picks what to calibrate first, and the Tuner/Tooltip sessions stop living below the
stored-marks table.

- **Tool tab row:** Buy / Sell (Shop), Gem Composer, Pet Feeder, Magic Tuner, Tooltip, between the page
  header and the workspace.
- **Mark tabs scope the tree:** the target tree now names the active tool and lists only that tool's
  kinds/instances. The kind picker still offers all 17 marks; choosing another tool's kind moves the tab
  with the selection, so the two pickers cannot disagree.
- **Session tabs host the session:** Magic Tuner and Tooltip replace the mark workspace with their
  session panel (one capture, split picture/controls). The page header hides its mark capture/refresh
  actions on those tabs because the session owns the capture.
- **No below-fold sessions:** the global Sessions panel, its header shortcut, and the target-tree
  session rows are gone. Both session workspaces stay mounted in an `IndexedStack`, so switching tabs
  keeps the captured frame and typed fields.
- The scoped tree is guarded for a tool with no mark group (Tuner/Tooltip), and the tuner section now
  renders its status line so a Save result is visible where the action happened.

Verification: `flutter analyze --fatal-infos` clean, `dart format` clean, `flutter test --no-pub`
**350/350**, Windows debug build green. Docs (UI spec and both user guides) updated in the same commit.

## 2026-11-04 (35) — one capture per tuner; calibration units grouped by tool

Three UX corrections after the session design:

- **Tuner is one capture again.** v2 captured the status screen once and drew the three OCR boxes and
  the spring point. v3 had split the spring point into a normal mark, so the player needed one Captures
  capture for it and a second session capture for the OCR boxes. Spring point is no longer a
  presentation mark; it is set inside the Tuner session, which already carries it to
  `tuner.spring_point`. The core still serves the 18-kind table; the shell presents 17 marks.
- **The target tree groups by consuming tool.** Each tool's marks and its sessions now sit under the
  same group header; the separate global SESSIONS section is gone, and Pet's tooltip session lives
  under Pet, Tuner's session under Tuner.
- **The Pet crops/rows session was removed.** It duplicated the nine Pet marks already in the upper
  editor. The mark editor is now the single place for Pet geometry; rows/queue/food cells stay on the
  Pet tab. This also removes one capture and two duplicate Save paths from the screen.

Verification: shell `flutter analyze --fatal-infos` clean, `flutter test --no-pub` 351/351, format
clean; Windows debug/release builds green.

## 2026-11-04 (34) — the Tuner 3-box and Tooltip sessions get a real UI design

The two session-shaped calibrations were already wired to the core, but their panel read as three
anonymous rows of x/y/w/h fields with one mark on the picture. Asked for the missing UI design, this
increment keeps the payloads and tests and rebuilds the presentation:

- **Canvas overlay:** `CalibrationFramePicture` gained `overlayMarks` (id, mark, label, colour), painted
  under the selected/editable mark. It is generic: the tuner passes the two non-selected boxes and the
  tooltip passes the placed cursor point.
- **Tuner OCR 3-box:** numbered Capture → Draw → Check/Save steps; one card per box with its colour dot,
  target chip, status size and x/y/w/h fields; green/blue/amber overlays on the picture. Check now
  renders a result card (region, derived row pitch, grade, remaining, filter verdict, matched rows)
  instead of only a status line.
- **Tooltip offset + shape:** two step cards (hover point, then panel box/offset), the offset formula
  as a mono line (`cursor + offset = panel top-left`), a pink cursor crosshair on the captured frame,
  and a bordered **Read result** card for Test read lines.
- **Discoverability:** the same numbered step strips and cards make both sessions read as guided flows
  under Calibrate's stored table; typed fields stay authoritative and the existing element keys remain.
- **Follow-up — selection and recapture:** the **SESSIONS** rows in Targets are now selectable exactly
  like kind rows (selected state + jump to the matching card), the page header gained a **Sessions**
  shortcut, and the tuner's **Capture/Recapture screen** button moved to the top of its card beside the
  **Target to drag** chips. The selected tuner box also stays drawn after a drag (the first pass hid
  it), and the card carries an explicit drag-on-picture hint.
- **Follow-up — screen beside the controls:** each session is a split workspace at wide widths: the
  captured frame stays fixed on the left (380 px fit) and the target cards/fields scroll in a 470 px
  pane on the right, so a drawing drag and the box it edits are visible together instead of requiring
  a page scroll between the picture and the fields. Narrow layouts stack them. The tuner's
  Capture/Recapture button and target chips sit above the split; the tooltip's placeholder explains
  where the captured panel will appear before the first hover.
- Tests pin the new box cards, step chips, all-box/cursor overlays, the 19 px pitch + 300×200 region
  summary, the Read-result card and the session target selection/scroll; the full shell gate is
  **353/353**, analyze `--fatal-infos` clean, format clean and the Windows debug build OK.
## 2026-11-03 (34) — Appearance: light, dark or follow Windows

The player asked for a Setup choice of light / dark / follow-system. The shell had been dark-only, so
this increment makes both `ThemeData`s first-class instead of flipping a colour: `buildShellTheme` takes
a `brightness`, `ShellColors` gained a light set (semantic roles tuned for the light surface), and
`ShellApp` maps the model's `ShellThemeMode` to `ThemeMode` on `MaterialApp`.

- **Persistence:** portable `settings.ui.theme` (`system|light|dark`), default **dark** so an install
  that predates the setting keeps its look; `ensure_portable_ui` carries the leaf and the core config
  test covers apply/local/reload/hand-written values alongside the other `ui` fields.
- **Setup:** a new **Appearance** section offers Follow system / Light / Dark; the choice saves
  immediately and the shell rebuilds through the model notifier.
- **Native chrome:** the runner gained `setTitleBarTheme(0|1|2)` over `seal/host`. `Win32Window` keeps
  the chosen mode so `WM_DWMCOLORIZATIONCOLORCHANGED` re-applies an explicit light/dark choice instead
  of snapping back to Windows, while system mode keeps following Windows.
- **Tests:** `ShellApp`'s ThemeMode/theme extension mapping, the model's save/restore + title-bar calls,
  and the Setup chip payload. Shell **336/336**, analyze `--fatal-infos` clean, format clean, Windows
  debug build incl. the runner change OK; core config test green.

## 2026-11-02 (33) — the shell lint/quality gate gets sharper

Asked what could be done for lint/code quality on the Flutter frontend. The answer was to make the
analyzer stricter where it is cheap and to make intentional fire-and-forget calls explicit — not to add
a third-party lint stack.

- `analysis_options.yaml`: `strict-casts`, `strict-inference`, `strict-raw-types`, plus
  `directives_ordering`, `prefer_final_locals`, `prefer_single_quotes`,
  `always_declare_return_types`, `unawaited_futures`, `discarded_futures`,
  `use_build_context_synchronously`, `avoid_print`, and
  `no_leading_underscores_for_local_identifiers`. Deliberately not enabled:
  `sort_constructors_first` / `avoid_redundant_argument_values` (style churn across the model classes)
  and `avoid_dynamic_calls` (the JSON boundary is dynamic by design and narrowed by the typed
  decoders).
- `tool/check.ps1`: one command runs `dart format --set-exit-if-changed`, `flutter analyze
  --fatal-infos`, `flutter test` and `flutter build windows --debug`; the shell README documents the
  Windows PowerShell fallback and the shorter inner loop.
- Fixed the findings the stricter gate exposed: three import-order blocks, three strict empty-map
  inferences in `client_test.dart`, one strict-cast map assignment in the Pet test fixture, and 15
  discarded futures made explicit with `unawaited(...)` (dispose cancellations, `initState`
  refresh/load calls, and fire-and-forget card saves).
- No runtime behaviour change. `flutter analyze --fatal-infos` clean, shell **333/333** tests, format
  clean, Windows debug build OK.

## 2026-11-02 (32) — the quick dock: three dock form factors in one launcher

The player asked for a minimal between-runs mode and then for **all three docks from the study** to be
built and selectable. This increment lands the launcher: the compact window that was the 360 × 96 run
bar now has an idle **Quick dock** state, and the same window hands over when a run starts and takes
the window back when a finished bar's dock button is pressed. There is no second topmost window and
no core/protocol change: every Start is the existing `tool.start`, and the Spammer/Quest pickers use
the existing `config.apply` rule.

- **Five placements** (`DockPlacement`): right rail (300 × 480), left rail (300 × 480), bottom right
  (340 × 460), bottom left (340 × 460), top centre (380 × 500). The choice lives in **Setup → Quick
  dock** and in the dock header's placement menu; the dock can be dragged/snapped like the run bar.
  Placement and collapsed state are now persisted through the portable `settings.ui.quick_dock` block
  (`placement` + `collapsed`); the core's `ensure_portable_ui` projection carries the field, verified
  by a seal-core config test (write, preserve, reload, hand-written read). A dragged offset is
  session-only; the preset is what survives.
- **Content, ranked**: seven quick tiles (Spammer, Buy, Sell, Quest, Gem Composer, Magic Tuner,
  Hold Space) with live phase chips only while active, then the Pet Feeder as the only setup-only row
  (Gem Composer and Tuner now quick-start the configured job; Pet remains the long-running
  configure-once tool). Each tile has a dedicated **Start/Stop button** plus a separate **⋮ options**
  button; the whole-row start and the ambiguous `>` chevron were removed after play feedback (⋮ is also
  the full Sell screen's row-button style). Hold Space uses the same Start/Stop button with no options
  or extra toggle label.
  Footer "Open full window"; the options menu opens in-window (Back closes; Start pinned in the footer
  as the only filled action):
  - Buy preset chips + count → per-run `tool.start {preset,count}`;
  - Sell 8 × 8 bag-cell grid with the full screen's `row N` per-line shortcuts (fill/clear the row) →
    per-run `tool.start {slots}`;
  - Spammer rotation / Quest flow → saves the active preset, with a note saying so;
  - Hold Space is an on/off toggle.
- **Model** (`shell_model.dart`): `enterDock`/`exitDock`, `setDockPlacement`, drag/settle helpers,
  `openDockFromMini()` and `_dockToMini()`; the full-window restore target and chrome are carried
  across dock → run bar → full window. `ShellPage` parks the full shell offstage for both compact
  states (tab drafts survive), the top bar gained `app-bar-dock`, and the finished run bar gained
  `mini-dock`.
- **Real-build verification** (150 %): right rail 450 × 720 physical (= 300 × 480 logical) at 16 px
  from the right edge; bottom-right bay 510 × 690 (= 340 × 460); top-centre island 570 × 750
  (= 380 × 500). Each was borderless with `exstyle = WS_EX_NOACTIVATE | WS_EX_TOPMOST`; the right-rail
  capture opened a secondary menu through a real mouse click. Captures:
  `.scratch/ui-audit/dock-real-*.png`.
- **Collapsed resting form** (2026-11-02 follow-up): the header's collapse action shrinks the dock to a
  46 × 150 rail tab, a 220 × 44 bottom pill or a 200 × 36 top island; it keeps three tool icons and the
  running dot glanceable, and clicking, unfolding or hovering it expands.
- **Idle behaviour + hover** (2026-11-02 follow-up): Setup and the dock header's placement menu select
  **Auto-collapse** (eight idle seconds) or **Keep open** (no timer); the choice persists as
  `settings.ui.quick_dock.behavior`. Auto-collapse pauses while the pointer is over the dock; hover
  expansion is transient (`persist: false`) so it never overwrites the player's explicit collapsed
  preference. The timer also no longer persists its own collapse, so a restart opens the dock in the
  chosen mode instead of always starting tiny.
- **Editable tiles + pinned Sell** (2026-11-02 follow-up): the header's Edit action opens an in-window
  editor with up/down reorder, remove and "Add back" for every quick tool; the Sell menu's name field +
  Pin turns the current slot selection into a one-click `Sell · <name>` tile. Tiles and pins persist in
  `settings.ui.quick_dock {tiles, pins}` (core shape + config test extended), so the layout survives a
  restart. Last-used Buy preset/count is remembered for the dock session, and the Pet setup row asks
  the full shell to select the Pet tab when it returns.
- **Gem/Tuner secondary options** (2026-11-02 follow-up): both tiles now carry a ⋮ menu. Gem Composer
  picks the start grade and empty-slot behaviour (Stop / Advance grade / Advance + clear); Magic Tuner
  picks the target grade and a max-retries stepper. Each pick saves the portable `settings.gem` /
  `settings.tuner` field through `config.apply` (the other editable keys are carried so a partial save
  cannot blank them), and Start then runs the saved configuration.
- **Remaining**: only the dragged offset inside a placement is session-only (placement, collapsed
  state, idle behaviour, order and pins persist); live-game Start/Stop verification is still manual.

Gates: shell `flutter analyze` No issues, `flutter test` **333/333** (new `quick_dock_test.dart` covers
tiles, Buy/Sell menus, seven quick tiles, five placements, collapse/expand, auto-collapse and Keep open,
hover expansion, edit remove/reorder/add-back, pinned Sell creation/start, Gem/Tuner option saves,
top-bar entry, full-window return and run-bar hand-back; model tests cover geometry, chrome,
behaviour/preferences/layout restore and both transitions), `dart format` clean, Windows debug build OK;
core `quick_dock_preference_is_accepted_written_and_preserved` green (now includes behaviour),
`cargo fmt --check` clean, clippy clean for seal-config and the core config test.
Truth: [UI.md §Quick dock](../spec/UI.md#quick-dock-landed-2026-11-02).

## 2026-11-01 (31) — the navigation rail collapses (top-bar button + Ctrl+B)

The player asked for the harness-style hideable sidebar. `_ShellPageState` now carries
`_railVisible` (session-only): while true the 216 px `ShellRail` is a `Row` child; while false the row
omits it so the content panel gets the full width, and the selected tab/draft is untouched (the
`IndexedStack` is unchanged). The top bar gained a leading `rail-toggle` `IconButton`
(`view_sidebar`/`view_sidebar_outlined`, tooltip says Hide/Show sidebar (Ctrl+B)) and the page's
`CallbackShortcuts` gained `Ctrl+B` beside the existing Ctrl+L/Esc. The status strip hint now reads
`Ctrl+B sidebar · Ctrl+L logs` (plus `· Esc close` when the drawer is open), so the shortcut is
discoverable without hovering. No persistence: the rail is visible again on the next launch, matching
the mini bar's session-only placement. There is no core/protocol change.

The widget test (`shell_page_test.dart`, "the rail collapses from the top bar and Ctrl+B") pins both
entry points and that hiding keeps `cards-list` while removing `shell-rail`. Full shell gates:
`flutter analyze` No issues, `flutter test` **304/304**, `dart format` clean, Windows debug build OK.
The new chrome is described in [UI.md §Chrome](../spec/UI.md#chrome-phase-1), and the EN/zh-TW
USER_GUIDEs now name the sidebar toggle and Ctrl+B.

## 2026-11-01 (30) — the mini running bar ships: 360 × 96, borderless, one-click Stop

Landed in `2b4c8b2` (shell + runner + tests + USER_GUIDE/spec). The player's requested shape was
"minimal while a tool runs": essential buttons/information only, one click to stop, able to sit beside
the game without covering its information. The design demo
(`.scratch/ui-audit/ui-demo-mini.html`, variant A, eight ⚑ decisions approved by the player) was built
first; the shipping implementation is the 360 × 96 **borderless, always-on-top bar** in
`lib/ui/mini_mode.dart` plus the model/runner plumbing, with no core or protocol change (all bar
values are existing `state` / `tool.finished` facts).

- **Ranked content.** Row 1 tool + phase chip + resident-pet badge + expand; row 2 the core's action
  line, the compact countdown, and exactly ONE primary action: **Stop** while a run is active,
  **Full window** when it is finished/error/core-lost. Rail, tabs, header, save bar, logs, detail rows
  and the native caption are deliberately absent. Footprint is 34,560 px² = 2.7 % of a 1440 × 900
  screen (the full shell is 58.8 %; the earlier WIP card was 340 × 420).
- **Model** (`shell_model.dart`): a foreground run collapses (Pet excluded — it is the resident
  feeder), the bar STAYS on Finished/Error until "Full window", and "Full window" suppresses
  re-collapse for the rest of that run (it collapses again on the next start). A manual top-bar
  action (`app-bar-mini`) collapses a running foreground tool. Position is session-remembered,
  clamped to the work area, and snaps to a 16 px edge inset after a drag. When Pet runs alongside the
  foreground tool the bar shows a paw badge and Stop only stops the foreground run.
- **Shell** (`shell_page.dart`): while the bar is shown the full shell stays mounted offstage inside a
  full-size `OverflowBox` under one `GlobalKey`ed subtree, so the current tab and its draft survive
  the collapse and are still there on expand; the bar itself listens to the model.
- **Runner** (`flutter_window.cpp`, `win32_window.{h,cpp}`): `setMiniChrome` saves style/EX style,
  applies `WS_POPUP` (no caption/thick/min/max/system menu) + `WS_EX_NOACTIVATE` + DWM round corners,
  lowers the minimum size to 360 × 96, and restores the saved chrome, rect and pin on expand. The
  minimum-size guard now accepts the bar's 96 px height (it was 160). Drag is a Flutter pan through
  the existing `setWindowRect` (clamped), not a native caption strip.
- **Real-build verification (the part a widget test cannot prove).** At 150 % scaling the window was
  540 × 144 physical = 360 × 96 logical, `has_caption=False`, `has_thickframe=False`,
  `ex_noactivate=True`, `ex_topmost=True`, docked 16 px from the right edge. A real mouse click on
  Stop sent `tool.stop` (the button showed Stopping…) while the foreground window stayed the game/browser
  — `WS_EX_NOACTIVATE` does not swallow clicks. Expanding restored 1920 × 1080 (1280 × 720 logical)
  with the caption back and no NOACTIVATE. Captures: `.scratch/ui-audit/mini-real-*.png`.
- **Cost/teaching.** The bar's first render exposed two things worth keeping: the shared
  `countdownLabel` says "next action in 11 s" (too wide for 360 px — the bar strips the prefix to
  "in 11 s"), and the 88 px Stop button could not hold a spinner + "Stopping…" (it widens to 112 px
  while busy). The original mini card's 340 × 420 geometry and log tail were replaced rather than
  parameterised; the auto-collapse model and runner min-size/top-most channel were reused.

Gates: shell `flutter analyze` No issues, `flutter test` 303/303 (new model + widget tests for the
collapse/stay-finished/suppress/remember/Stop/resident-pet paths), `dart format` clean. Spec truth:
[UI.md §Mini running window](../spec/UI.md#mini-running-window-landed-2026-11-01).

## 2026-10-28 (29) — the Pet tab can scan the bag again (v3.0.6)

The last automatic half of v2's Pet-tab maintenance workflow: the bag scans, and the explicit write they
feed. `pet.scan_food_page`/`pet.scan_queue_page` (253d7db) are READ-ONLY — input lease, bag-grid/page-tab
validation, one foreground-gated capture, the EXISTING v2 matcher (pet.rs grew one unfiltered
`score_all_scores` sweep that the run's `score_all` wraps, so there is structurally ONE scorer), then
best-effort evidence under logs/reads and a result: food cells 1-based in CELL order, queue `per_entry` in
queue order with score-ordered hits, runner-up, claimed conflicts and v2's deduced "label x n" lines.
`pet.apply_food_scan` (253d7db) is the only writer: 1-based args to 0-based stored pairs, replaces
`food_slots` AND resets `food_slots_used` in one A1 field-precise patch, one `config.changed` after the
response, empty selection refused because nothing-found must not silently clear a live run. The shell
(771a60a) generalised the capture flow into one `_runHiddenWindow` shared by captures and scans (show
always in a finally), added the Scan-the-bag section with the page echo, verbatim report/evidence and
the neutral `hits: []` caption, and the confirmed Apply with REPLACE/RESET/immediate wording, dirty-gate
and mirror refresh. Nothing is auto-applied; the player reviews the report/evidence first.

The check found issues worth recording. The core cannot click or verify the ITEM tab (it validates the
page's mark only), so a wrong page yields a plausible result under the requested page — the scan rows
gained the capture row's page-up/wrong-page caveat (2ec2897) and the docs/UI warn that Apply can then
save wrong-page cells. Two robustness fixes followed (6eba880): the report filename gained the page
(both files are `scan_page{N}_{stamp}.png/.txt`) because a same-second sweep of ITEM1–3 could overwrite
a previous report while the first result's evidence path pointed at the newer one; and the food icon is
decoded BEFORE any capture/evidence, because a missing icon used to cost a hidden capture and leave an
orphan PNG. A final check (task-125) caught two docs nuances now recorded: scans read the SAVED config
(they are not dirty-gated; only Apply is), and Apply acts on the scanned page named in its confirmation,
not the ITEM chip switched afterwards. Test-count note: the shell commit message says 9 new widget
tests/36 in file; the accurate figure verified at the gate is 8 new / 35 in file (suite 264) — recorded
here because a commit message is not rewritten after review.

Gates at `7d2829b`: Rust 79 result lines/0 failed (core lib 176), Flutter 264/264, clippy/fmt/analyze/
format clean, publish stage/app-zip/firmware audits PASS, fixtures 81 (37/38/6) with 0 decoder
mismatches. Artifacts: `dist/SealTools-v3.0.6.zip` 45,073,095 B `21D53399...ACEEE`;
firmware 5,254 B `498E6A4BA...35CCEF3B` (byte-identical, no reflash). Unpublished; CUTOVER has the
checklist. Remaining: the panel test read and the feedable sweep/queue rebuild (increment 4+; the long
sweep's per-page-blocking-vs-background-job surface decision is open), B5 card live detail (v3.1), dry
runs / Quest Test-one-loop, the spammer raw-key editor — OPEN §5.

## 2026-10-28 (28) — the Pet tab can queue pets again (v3.0.5)

The parity audit's bluntest Pet finding was that v3 could not queue a new pet at all: the tab could
remove queue entries but not add one, because v2's queue was populated by capturing the marked bag
cell's icon. Increment 2 restores it as a capture-only primitive rather than a second run path.
`pet.capture_icon` (a10726f) validates the input lease, the calibrated `pet.bag_grid` and the requested
page's 2-point tab mark, takes ONE foreground-gated client capture and crops it in memory — no click, no
cursor move, no board open, no config or log write — using the run's own bag-cell box. The
builder-check's structural note was right: the box expression had been duplicated, so `4d593b8` moved
it to a shared BagGrid helper (`pitch`, `pitch_box`, `cell_box`) used by both the run and the capture,
and removed a validate-then-recompute path. The shell (39ce9fe) hides itself for the shot and shows
again in a finally (so a refusal can never leave it hidden), appends the captured `{label,species,rect,
png}` to the queue DRAFT, and writes it only with the tab's one Save; the food icon goes to
`pet.food_icon_png` the same way. The UI never clicks the ITEM tab — the core cannot verify which bag page
is displayed, only that the requested page's mark exists — so a wrong page gives a plausible wrong
crop; the preview and the echoed page/cell/rect are the only checks, and the docs say so explicitly
after the builder-check caught the phrase "nothing is queued until Save" (the entry is visible in the
draft list immediately; only persistence waits) and an overclaim that the food icon is matched by a
scan (the scans are increment 3+). The check also caught a guard edge — the food-icon source was seeded
once and could survive a Cancel pointing at an unmarked cell — fixed in `35600cd` by reconciling it on
every seed revision (the same seam the return-slot mirror uses).

Gates at `ecfbd2e`: Rust 79 result lines/0 failed (core lib 168), Flutter 251/251, clippy/fmt/analyze/
format clean, publish stage/app-zip/firmware audits PASS, fixtures 75 with 0 decoder mismatches.
Artifacts: `dist/SealTools-v3.0.5.zip` 45,033,252 B `BFB0870C...34BE72`; firmware 5,254 B
`498E6A4BA...35CCEF3B` (byte-identical, no reflash). Unpublished; CUTOVER has the checklist. Remaining:
the bag scans / panel test read / feedable sweep (increment 3+), B5 card live detail (v3.1), dry runs /
Quest Test-one-loop, the spammer raw-key editor — OPEN §5.

## 2026-10-28 (27) — diagnostics back, and the Pet tab can check itself (v3.0.4)

Two audit gaps closed in one release, and both were shaped by the builder-check loop. The Arduino tab
regained v2's Connection card (the audit called its loss a diagnostic regression) but not by opening the
port on tab open — that is what v2 did and it is exactly wrong while a tool runs. `board.details`
(82d640c) answers the same questions read-only: expected IDs/port override, chosen-first port list,
one of three choice states with the core's refusal sentence, and a memory-only protocol-level cache fed
by `board.probe` and the pet session's detect. The shell card (`ea3ae90`) renders it with v2's three
verdict lines and ●/○ markers; the builder-check caught that a silent/legacy board a pet run had opened
still read "not checked yet", so the cache now maps silence to level 1 like probe (110dcc7), and that the
not_found advice was duplicated shell-side (fixed in `1f166e0` to trust `choice.refusal` with a
documented v2 fallback).

The Pet tab got its first session increment: `pet.preflight` is read-only and answers the Setup-so-far /
Ready-to-run facts in v2's lines, and `pet.set_boarding` writes one row's boarding state through the A1
field-precise writer (one field, shared rev, one `config.changed` after the response). The shell renders
the checklists generically and adds per-row **Boarding now** ticks that are disabled while the tab has
unsaved edits — a `config.changed` against a dirty draft would banner the player's own tick as an
external change, so the tick waits for a Save instead. The builder-check found preflight could say ready
while Start refuses: the ready line's per-row format was wrong for >1 row, page tabs were accepted
non-empty instead of for the pages the run uses with 2-point marks, and the food strip was required even
in right-click mode. `17e1ff2` fixes all three against the run's own derivations, adds the
"every row is unticked — nothing to feed" line, and closes the review's cheap gaps (io e2e with a held
local.yaml, raw event-order proof, malformed config). A second check (task-109) then caught that the
docs overclaimed "ready" and the ✗/ℹ markers; the guide was corrected to say ready mirrors Start's
gates for the SAVED, UI-shaped config, that the calibration list mixes gates with facts, and that ℹ is a
shell marker, not a protocol state.

Gates at `7ec3539`: Rust 79 result lines/0 failed (core lib 158, pet_session 13/13), Flutter 243/243,
clippy/fmt/analyze/format clean, publish stage/app-zip/firmware audits PASS. Artifacts:
`dist/SealTools-v3.0.4.zip` 45,005,039 B `51308D18...9385B6`; firmware 5,254 B
`498E6A4BA...35CCEF3B` (byte-identical, no reflash). Unpublished; CUTOVER has the checklist. Remaining:
Pet session increments 2+ (capture/scan/queue/test read), B5 card live detail (v3.1), dry runs / Quest
Test-one-loop, the spammer raw-key editor — OPEN §5.

## 2026-10-28 (26) — the parity audit, v3.0.3, and the calibration foundation

The first two live passes had each found one design error (the Sell grid was persisted, then the tuning
draft resets). The third pass found a different shape of problem: "for the sell grid, there are still
features you missed from the original v2". A single miss could be a bug; a missing row of v2 controls
was evidence the port had silently dropped the launcher work around the tools, so the player asked for
the check to cover everything. The read-only v2→v3 audit (`5ff5b4f`, `records/PARITY-AUDIT-2026-10-28.md`)
is that check: every surface, v2 evidence cited, deliberate differences listed with their spec line, and
the four Calibrate surfaces + Setup Detect compared against the calibration plan rather than counted as
gaps. It found one real safety regression: the Pet Feeder persisted `food_slots_used` and per-row
`boarding_running` mid-run in v2 so a days-long run could not resume on stale state; v3 read them and
never wrote them. `b6394e2` restores that through the config store's single writer (workers cannot use the
dispatcher's rev; CoreState now shares an `Arc<AtomicU64>` and the registry carries a `PetStateSink`), emits
one `config.changed` with no request/response (PROTOCOL `728a15a`), logs a failed write and keeps running.
The close-guard fields have the writer but no producer until the close-probe port — named, not faked.

The rest of the release is the audit's quick batch plus two player asks. `0deda95`/`66d0f22` restore v2's
Sell row/Select-all/Clear controls and make the over-cap guard compare the deduped valid selection rather
than the raw list (the old check could refuse a run that would sell fewer). `b59ef12` adds the
always-on-top pin as a portable `settings.ui.always_on_top`; proving it exposed a latent core bug —
`settings.ui` was silently dropped by the portable projection, so the pin would have looked saved and
vanished — fixed in `b0af630` by making `ui` a real post-template portable section with one writable
leaf. `93d1099` brings the deletion guards, the Spammer card Rotation picker (it saves `spammer.active`
rather than inventing a per-run argument the core does not accept) and the Buy row labelled picker (the
field v2 removed after an off-by-one). `7dc70ea` restores remembered window placement/size: the runner
owns DPI conversion, Dart restores/clamps/saves through the existing `ui.window` command with a debounce,
and the limitations (no clean-shutdown hook, no maximized state) are documented rather than papered over.
`27b9f56`/`10ed375` give the Tuner v2's clean-up-captures as an explicit `logs.cleanup_captures` command
(PROTOCOL §2's one log-housekeeping exception) plus the button. `82d640c` adds read-only
`board.details` — the facts behind the Arduino connection card only v3.0.4's tab will display.

Calibration increment 0 landed on the way: core `pointer.place`/`pointer.click` (flat x/y, closed-loop
placement, per-command input lease with `tool_busy` naming the holder; PROTOCOL §12.9) and the PNG
annotation writer (`1485638`), the shell session draft and pointer actions (`6164497`). Two
planning documents were committed — `PLAN-V3-PET-SESSION.md` (the Pet tab's missing v2 session
surface, on the new run-state writer) and `PLAN-V3-STATUS-DETAIL.md` (card live detail in v3.1 via one
additive `detail` field; Arduino diagnostics in v3.0.4 via `board.details`), the latter with ten Lead
decisions. A builder-check of the B3 plan caught the firmware-cache architecture (worker detection needs
the shared-cell pattern) and the cross-language fixture gate (the shell's test-only decoder must land with
the fixtures) before either was coded.

Gates at `898f861`: Rust 79 result lines/0 failed, Flutter 223/223, clippy/fmt/analyze/format clean,
`flutter build windows --release` 0, publish selftest 26/26 and the stage/zip/firmware audits PASS.
Artifacts: `dist/SealTools-v3.0.3.zip` 44,949,054 B `C19B85AB...441EE4F5`; firmware 5,254 B
`498E6A4BA...35CCEF3B` (byte-identical to v3.0.0-v3.0.2 — no reflash; the v3.0.2 record's firmware hash
had a transcription typo, corrected there with a note). Unpublished; CUTOVER has the publish checklist.
Remaining from the audit is staged in OPEN §5: v3.0.4 (Arduino tab on `board.details`, then the Pet
session surface) and v3.1 (B5 card live detail; dry runs / Quest Test-one-loop ride the command batch).

## 2026-10-28 (25) — the sell selection is per-run, and v3.0.2

The second live pass corrected one design: *"sell selection could be different every sell."* v3.0.1
had made the Sell grid persist its selection into `buy_sell.sell_slots`, which is worse than v2's
memory-only behaviour because a stale selection can sell the slots chosen last time — data loss, not
convenience. The whole seam was re-cut: PROTOCOL now carries the selection as `tool.start`'s `slots`
argument (validated before the supervisor: sell-only, non-empty array of integers 0..63, precise
bad_request sentences; used for that run only, never written), the core builds the sell plan from
those slots and no longer reads the config field at all (a start without slots has no selection and
gets the tool's own "No slots are selected to sell."), the shell keeps the 8×8 grid as process-memory
view state on ShellModel with the Sell card's Start sending it, and both languages' guides, INSTALL,
OPEN and TOOL-FLOWS now say "next run only, not saved" without any fallback claim.

Two gate bugs were caught on the way: the shell test that asserted the app-bar version still expected
v3.0.0 after the 3.0.1 bump (it now reads the `productVersion` constant), and an unused test local.
The release was rebuilt as **v3.0.2**: `SealTools-v3.0.2.zip` 44,838,165 B SHA-256
`7C8D7960…3084`, firmware byte-identical `498E6A4B…EF3B`. Gates: Rust 79 result lines / 0 failed
(core lib 124, tools 13), clippy 0, fmt clean; Flutter analyze clean, 180/180, format clean; all three
publish audits PASS; payload hash-match; staged launch with a fresh migrated v2 config started. It
supersedes v3.0.0/v3.0.1 and awaits the player's second live pass on it (plus the v2 parity worksheet
for the calibration increments).

---

## 2026-10-28 (24) — the first live pass, and v3.0.1

The player ran v3.0.0 and reported real defects. The diagnostic that mattered was not a UI bug: their
migrated v2 config holds quest key steps as integers (`value: 1`), which v3's typed `QuestStep.value`
(String) rejected, so the whole quest block failed to decode and every flow selection, save and hand-in
was refused. The scalar decoder now accepts v2's shapes, the full typed config was audited against the
player's actual file (that was the only decode-class mismatch), and committed sanitised v2-era replay
fixtures now pin typed decode plus the real reader paths — every merged pointer matched the v2 file.
The same audit found a second honest gap: `tuner.ocr.grade_area` was decoded and documented but never
consumed; v2 source proves it is region-relative inside the OCR region, and the reader now uses it with
`grade_y` as fallback (the v2 grade-colour fallback stays OPEN).

The rest of the pass was UI truth: tuner grades fixed/read-only and filter rule names selected from the
dictionary (the player: "no one would remember the options"); gem grades fixed/read-only; the game
window title read-only, detected by Find and saved by the new Use detected title; the Sell 8x8 grid
selectable with its own Save writing `buy_sell.sell_slots` (a deliberate replacement of v2's
memory-only selection); the Pet tab's machine setup editable again (rows enabled/stacks, queue removal,
per-page food cells, return slot, load mode, timings — one Save, capture data preserved byte-for-byte);
and the refusal strings now point at the Pet tab instead of calling it read-only (the genuine
exceptions say so). Two gate-found bugs were fixed on the way: the Sell grid re-baselined a dirty
selection on a refusal, and the Pet tab's State-built count didn't rebuild on draft edits.

Docs were rewritten in both languages (38 = 38), INSTALL/CONFIG/OPEN/TOOL-FLOWS aligned, the product
version became 3.0.1, and the release was rebuilt: `SealTools-v3.0.1.zip` 44,841,808 B SHA-256
`85AAAF98…8961`, firmware byte-identical `498E6A4B…EF3B`. Gates: Rust 79 result lines/0 failed, clippy
0, fmt clean; Flutter analyze clean, 181/181, format clean; all three publish audits PASS; payload
hash-match; the player's v2 config validates clean; staged and clean-unzip launches started. It awaits
the second live pass and the v2 parity worksheet before the calibration-plan increments.

---

## 2026-10-27 (23) — the pre-publish audit: the release tells the truth, then it is re-cut

Before the player could publish, the independent builder-check (plan-reviewer, read-only) compared the
contract docs, the guides and the built payload against the code. It found no packaging drift — the
zip's docs were byte-identical to the repo — but it found truth defects, and a shipped artifact that
lies is worse than one that lacks a feature.

The worst were in the SHIPPED BINARY: refusals sent the player to "Calibrate Gem/Tuner/Pet",
"Calibrate Buy/Sell" and a "Save Composer Moves" button, and told them to edit rows and food load on a
Pet tab that is read-only in v3.0. Those messages now name the MARK to save on the single Calibrate
tab, or the exact `config/local.yaml` key, and say which editor is not built. The shipped guides
repeated the phantom tab, claimed a Hold Space release failure appears on the card (it is
`holdspace.log`-only), claimed a shell-wide Start disable that does not exist, blamed an installer that
does not exist, merged two different capture files, promised a `model_load_failed:<file>` suffix the
code never emits, implied repository-only spec files ship in the zip, omitted the notices file from
INSTALL's tree, told the player to edit a read-only tab, presented unused OCR retries as working,
said Settings where the tab is Setup, and used the hover-panel word for the boarding window in zh-TW.
All fixed in both languages (38 = 38 headings) and INSTALL.

The contract docs had their own contradictions: PROTOCOL's OCR failure reasons and `io` meaning, a
frame cache still marked "not built", `core.stopping`'s `fatal` producer, the hotkey section that
contradicted itself, §12.7's "pending plan" for targets that shipped, §7's v2-era writers paragraph, a
single-instance pointer to the wrong OPEN section and no documented resident/deferred `tool.start`
rules; CALIBRATION's §8 contradicted §6 on display-scale changes; OPEN still called DirectML/capture
unresolved and never listed the missing Detect surface; TOOL-FLOWS quoted the old refusals. And the
golden fixtures covered 26 of 29 commands: `keys.parse`, `keys.names` and `attributes.get` had none,
and both calibration exemplars used a kind the core refuses (`bag`).

Everything was fixed at source, not documented around. The fixture generator now covers all 29 §4
commands with the shell's case-keyed guard extended in test support (**65 fixtures**: 29 request /
30 response / 6 event, 0 mismatches), and `native_assets.json` — the Flutter manifest the re-cut
bundle legitimately carries — joined the publish allow-list after the audit refused it, which is the
audit doing its job.

**The zips were re-cut after all gates**: Rust workspace 76 result lines/0 failed, clippy/fmt clean;
Flutter analyze clean, 165/165, format 81 files; `-Selftest` 26/26, `-Verify` exit 0, `-Release` exit 0
with stage/app-zip/firmware audits PASS. Final artifacts: `SealTools-v3.0.zip` 44,811,539 B SHA-256
`990C2629…F483A`; `SealTools-v3.0-firmware.zip` unchanged 5,254 B SHA-256 `498E6A4B…EF3B`. The release
record carries both cuts, the audit findings and the final hashes; the launch's payload-hash,
packaged-core replay and clean-unzip checks were re-run against the new artifact. Still unpublished,
still the player's live pass and merge/tag/publish.

---

## 2026-10-27 (22) — v3.0 packaged: the stage artifact, the licence file, and the zips

The last stretch of the cut-over. The docs audit's findings were paid off in three docs commits
(PROTOCOL/DESIGN/TOOL-FLOWS statuses made true again, CALIBRATION's counts and a new portability
section, CONFIG's stale bullets, OPEN pruned to genuinely-open work, folder READMEs, a v3.0 release
record); the product announcement stopgap was removed by separating `productVersion 3.0.0` (app bar)
from `protocolClientVersion 0.1.0` (handshake); the capture/hide lifecycle landed so calibration from
the UI is actually possible; naming/version alignment made the binary `SealTools.exe`.

Packaging took four attempts and each failure was a real defect the gates caught: `-Verify` red on
seven inputs at first; the staging attempt blocked by the sandbox's Flutter restrictions (the Lead
session ran it); the first authorized `-Release` refused `data/app.so` and zipped nothing; and before
that an adversarial review of `publish.ps1` found a path-traversal delete via `-Version` in a script
that had never been run. All fixed at cause, with a 26-check `-Selftest` pinning the guards.

**The DirectML licence question is settled by its terms, not by hope:** Microsoft's Software License
Terms for Microsoft DirectML clause 1(a) permit distributing the unmodified DLL inside Windows
applications, with the other clauses applying. `docs/THIRD-PARTY-NOTICES.md` now ships (a fourth
required release doc) with the distribution clause, the shipped copy's FileVersion, the canonical
1.15.4 terms URL, ONNX Runtime's MIT text, the PP-OCRv4 attributions and Flutter's notes pointer.

The release run then passed all three audits and produced `dist/SealTools-v3.0.zip` (44,808,739 B,
SHA-256 B38E673C…C9CA) and `dist/SealTools-v3.0-firmware.zip` (5,254 B, SHA-256 498E6A4B…EF3B);
`RELEASE-v3.0.md` records both, what was verified, and what deliberately is not in v3.0. The staged
release build was launched and self-checked: shell found the core beside it with no environment, the
core seeded `local.yaml`, the window opened. What remains is the player's live pass and their
merge/tag/publish decision — the repo's own rule says a tool is done when it has run.

---

## 2026-10-27 (21) — buy gets its run, the logs become files, calibration is repaired, replay lands

The tool-coverage and infrastructure stretch. **Buy is live**: PROTOCOL §12.6 gave `tool.start`
optional run arguments (`preset`/`count`), the core validates them deterministically before any board
or window is touched, and the Buy card's picker drives the run — the last served-but-refused id now
executes. All eight tools are live.

**File logs (G8) closed**: `seal-core/src/logs.rs` writes v2's per-run files under `<root>/logs/`
(OCR JSONL + optional captures, `error.log`, `holdspace.log`, one `<tool>.log` per run with start,
changed status messages and the final reason), logging never fails a command or a run, and the local
wall clock moved to `seal-win/clock.rs` so `seal-core` has no `unsafe` left. `gem.save_empty_captures`
and holdspace's failed-release line landed with it.

**Calibration repaired from an audit, not from vibes.** The gap audit found `focus_point` writing a
field nothing reads, `tuner_region` writing an unusable shape and `result_gem_area` saving any box as
the "empty" reference. The spec was repaired first (§12.7), then the core and the Calibrate tab: kinds
`scroll_point`/`max_button`, the gem `empty: true|false` verdict with the six-feature signature
sampled through a domain function (the analyzer MOVED to `seal-domain` rather than copied), the tuner
kind withdrawn with its three-box-session reason, and §12.8's `target` extension bringing the pet's
marks and the gem's points (18 kinds total, 1-based wire strings, sibling-preserving per-row writes).
The shell's Calibrate tab follows the same ticket with a target selector and gated Save.

**Replay core**: `seal-core --replay script.json` serves the real protocol from a script with no
config, board, window or OCR (PROTOCOL §13) — a development mode of the same binary, structurally
isolated from machine startup, with a committed example script and ten e2e tests. First live run of
any v3 tool happened in this stretch too: a gem smoke actually started against a real board/window,
published tally rows and was stopped cleanly; no transaction was verified.

**Packaging and docs started**: `v3/publish.ps1` (`-Verify`/`-Stage`/`-Release` with the DirectML
acknowledgement gate and an allow-list zip audit), the shipped config templates,
`docs/pet-data.csv`, `plans/CUTOVER.md` with its twelve decisions, and the docs audit whose findings
this entry's documents now reflect. Next: USER_GUIDE/INSTALL, the staging run, and the live
verification the repo's rule requires.

---

## 2026-10-27 (20) — every tool has a loop, and the game has one owner

The tool-coverage half of the core plan closed in this stretch: **seven tools live** (spammer,
holdspace, tuner, quest, sell, gem, pet) and `buy` the one served-but-refused start. The shapes are
deliberately different because the tools are — the tuner and gem READ (`TunerSource`, `GemSource`),
the quest and shop are pure plan executors, the pet is the resident reader — and each earned only the
effect seams it needed: `Enter`/`Place`/`ClickHere`, then `RightClickHere` for the quest,
`Wheel` for a buy pass, `MoveBy` for the gem's hand-measured tuned counts, and `HoldLeft`/
`ReleaseLeft` for the pet's drag, with `BoardEffects` releasing a held button on `Drop`. Every seam
defaults to REFUSING, because the failure class this suite fears most is a plausible-looking wrong
input.

**Buy is the honest hole.** `ShopPlan::buy` needs the run's item and count, and neither a
`buy_sell.active` field nor `tool.start` args exist; rather than silently buy preset #1 or invent a
field, the tool answers v2's own "No buy preset selected." at START and the protocol decision sits in
OPEN. The sell pass — the irreversible one — validates its whole transaction in `seal-domain` before
any click: filtered, de-duplicated, highest-index-first, capped.

**The pet feeder was the last and worst loop** (v2's 3,520-line `PetTool.cs`): opening LOOK that *is*
the schedule, per-visit foreground deferral, reload steps (toggle/verify, place-pet by return slot or
queue-icon sweep with hover checks, food load by right-click+MAX+Enter or a firmware-2 drag with
before/after bag verification), `consider_veto`, `fold_back` strikes, four independent card rows. Its
icon-match arithmetic is ported as PURE v2-named comparisons in the adapter, not a domain API, and
its two recorded deviations (feeder-count upscale ladder, missing `docs/pet-data.csv`) are in OPEN.

**The last architectural seam: one game, one owner.** The pet work exposed that the arbiter owned
every resident rule but NO running tool could reach it — so the feeder could not yield between visits
and two runs could both be `Started`. `seal_tools::game::GamePort` is the capability (`request` blocks
only its worker, `release`, `should_stand_down`), and the supervisor answers it with the arbiter
(Waiting stashes the reply so the supervisor never blocks; Preempt cancels the spammer through v2's
rule; Granted/StartPending satisfy the waiter or start the deferred foreground tool; shutdown unblocks
stashed requests). The pet asks before its first click in a visit and releases before its long wait.
Registered: the spammer's runner-side yield hook is the one follow-up, and the deferred start's 45 s
timeout is a log warning because the caller was already answered `Deferred`.

**Counts:** Rust workspace 73 result lines green (core lib 118), clippy/fmt clean; shell 142/142
analyze-clean; 140-ish commits on `v3-core`. What remains is live verification against the game and
the small named follow-ups — not unported code.

---

## 2026-10-26 (19) — the read-loop tool, the pure executor, and the shell's last faces

Two tools landed after the first two, and they are deliberately different shapes because the tools
are: the **tuner** (`39ed221`) reads, the **quest** (`b261b45`) does not.

The tuner is v3's first READ-LOOP tool — capture → OCR → the domain's `TunerRun::decide` → effects —
and it defined the shape the remaining reader tools will copy: the read is a seam (`TunerSource`: one
scan + the cursor), so the loop and its conversion are tested with a scripted source (ten tests, no
screen, no board); the production source is `win::capture` (foreground-gated) + `seal-ocr` + the
domain matcher, and `win::CommandError::line()` was extracted so the protocol mapping and the tool
read the same sentence instead of growing two copies. Earning it required three small effect seams,
each committed separately because each is a wire fact: `Enter` (the firmware's `E`, never `K E`),
`Place(Point)` (move without clicking — the HID spring placement and the guard's recentre),
`ClickHere` (the bare `C` manual mode needs), and later `RightClickHere` (`R`).

The quest is the opposite shape and proves the seam is not one mould: its decisions are pure config
(clicks/keys/enters/waits, per-step delays, master loops and sequence passes), its only read is the
cursor, and it may never place the cursor at all — the player positions the mouse during the
countdown and a drift STOPs the run with v2's two stop lines. Both tools validate before anything is
clicked, because a wrong config at a few hundred loops is unattributable afterwards.

**The runner→card seam (`7a1d63d`).** `Tool::status()` (message/countdown/rows) is published as a
whole snapshot when it changes, before the decision it belongs to, so a countdown is visible during
its wait and a read's line appears as soon as it exists. Status equality gates publishing. This is
the gap that had made the rich `state` event a spec-only shape for live tools; now the tuner shows
its countdown, filter line and attempt, and the pet's rows have a place to go when its loop lands.

**The shell finished PLAN §6** while the core grew: `keys.parse`/`keys.names` (`7806a9b`) put the
typed-name rule behind the protocol (PROTOCOL §12.5) and the **Hotkeys tab** (`35c6d89`) edits names
while the config keeps codes — including structural preservation of a code no name covers; the
**card extras** (`62ffc4d`) added persisted Quest pickers, a display-only Buy picker whose protocol
gap is recorded rather than invented around, a read-only 64-slot sell grid, and live-status coverage.

**What the gates caught this stretch:** one stale test expectation (a theme's `quit` code renders
`F11`, not `Escape`); one fake transport that never answered `tool.start`, leaving the client's 10 s
timeout timer pending at teardown (the standing rule is now: a test that presses a command-sending
button answers that command); a 49 px card overflow that forced the extras layout to be
width-adaptive; and the fifth instance of the protocol-vs-Flutter type shadowing in a `ui.` sweep,
which is why the shell README names `calibration_widgets.dart` as the boundary example.

**Counts at this milestone:** Rust workspace green (73 result lines; core lib 54), clippy/fmt clean;
Flutter analyze No issues, 142/142 tests, format clean. **Four live tools** (spammer, holdspace,
tuner, quest); four ids remain refused by name. Next: the shop loops — the first readers besides the
tuner — then gem, then the pet feeder (rows + drag hold, the hardest).

---

## 2026-10-26 (18) — a second live tool, the last silent event, and the shell polish set

**Hold Space is the second live tool (`3004cb2`).** `tool.start holdspace` opens the board, holds the
spacebar (`P`) on the first decision, waits on v2's 50 ms tick, and releases through
`SpaceHoldEffects`' `Drop` — v2's `finally` expressed as RAII, so every exit path (stop, cancellation,
effect failure, shutdown) sends `U` and no code path can forget it. The tool needed the two grammar
commands v2's `HoldSpace.cs` used but the v3 enum had never modelled (`P`/`U`), and they came with a
decision vocabulary (`Decision::HoldSpace`/`ReleaseSpace`, `Effects::hold_space`/`release_space`) —
whose hold default REFUSES rather than approximating with a tap, because a tap looks like the hold
worked while auto-pickup never happens. v2's card-mirror/F12 logic has no v3 counterpart: start/stop
ARE the run lifecycle here, so the module doc records the mapping rather than porting the loop shape.

**`config.changed` has an emitter (this commit).** The last event PROTOCOL §5 specified but the core
never sent. Three writes produce it — `config.apply` (one event per written block, with `settings`
fanning out to the blocks it carries), `ui.window` (`ui`) and `calibration.save` (the marked block) —
and it is sent AFTER the response it belongs to. That ordering is load-bearing rather than tidy: a
shell must never see a `config.changed` caused by its OWN `config.apply` before the answer to that
apply, or every save would look like an external change. Events are therefore queued during dispatch
and flushed by the stdio loop after the response. The e2e harness learned the same rule: `Session::call`
now buffers any event line that arrives while it waits for a response instead of failing on it.

Two real bugs surfaced while earning the tests. The first was the writer-thread shutdown rule from
OPEN §1 taking its revenge: attaching the bus to `CoreState` left a sender alive past
`writer.finish()`, so every e2e test hung AFTER the core wrote its final line — fixed with an explicit
`detach_events()` before finishing the writer. The second was the test harness itself reading a queued
event as a response. Both are the class this project keeps paying for: order between independently
buffered things is a contract, not an implementation detail.

**The shell polish set (`8fdb8ca`).** Spammer preset rename/duplicate across all four name-keyed maps
plus `active` (with the `copy`/`copy N` base rule so duplicating a copy yields `copy 2`), the combos
editor, the live per-second card countdown from one shared ticker (elapsed reset on each `state`
event, disposed on unmount), and the Tuner `(none — use the shipped default)` label. Its four gate
rounds were all test-side: a missing `setState` for a note rendered outside the draft listener (a
real defect), a tap below the fold (the form outgrew the test surface — `ensureVisible` is the
standing pattern), and a 1-based-vs-0-based expectation.

**Counts at this milestone:** Rust workspace green (73 result lines; clippy/fmt clean) with two live
tools and every PROTOCOL §4 command served; Flutter `flutter analyze` No issues, `flutter test`
130/130, format clean. Next tool needs the read/sense seam (capture → OCR → decide), which is the
next architectural piece rather than another adapter.

---

## 2026-10-26 (17) — the last spec-only commands, the first live tool, and the shell's ten tabs

Two milestones that change what v3 IS rather than what it contains.

**`calibration.save`/`fetch` (`ab22a0a`)** closed PROTOCOL §4: there is no spec-only command left.
The shape was forced by the invariants `CALIBRATION.md` had already earned — a refused save writes
nothing and consumes no rev (arguments, frame bounds, the bag slot-vs-grid pitch agreement and the
artifact crop all run before the first byte), the artifact is written before the marks so a mark never
references a missing crop, and the seven-kind table lives in one place in
`seal-core/src/calibration.rs`. The binary artifact writer that OPEN had been carrying since the
config crate landed is `ConfigStore::write_artifact` (temp+rename, bare names only), and the crop is a
pure `crop_rgba` in `seal-win`, so the first artifacts are testable without a screen.

**The spammer is the first live tool (`de016c1`).** `tool.start spammer` resolves the active preset
out of the merged config (published by the dispatcher into a shared cell after every command), opens
the board in the opening phase, and the runner drives the domain's rotation through the board's key
effects. Nothing about the decision half moved: `SpammerPlan`/`SpammerRun` still decide, the runner
still performs, and `seal-core/src/spammer.rs` is only the join (rotation in file order, the domain's
refusal lines, v2's 20 ms tick). The first tool also earned the seam's last missing piece:
`Decision::KeyFast` + `Effects::key_fast`, because a `*key` is a different wire (`k`) and the
spammer must be able to ask for it; the default `key_fast` is the ordinary press, so an effects
implementation that does not model the distinction still sends the key. An unsendable key fails the
START naming every bad key — a recorded divergence from v2's warn-and-skip (core README divergence
10), because the v3 card surface has no per-tick log line to warn into. The other seven ids still
refuse by name, pinned by registry tests.

**The shell reached ten tabs and then eleven.** Waves 1–3 landed as committed checkpoints: the five
config tabs on one draft contract (`11317d1`, `1911ceb`, `6f6ec41`, `112374c`, `8fb84ac`), keep-alive
plus the cross-tab external-change guard (`b974995`), Pet and Setup with multi-block `ConfigDraftGroup`
(`ff4dd99`, `7c007ce`), the Attributes tab over the new `attributes.get` command (`ccf038a`,
`66ef600`), the Arduino tab that moves the real cursor without starting a tool (`8a8f003`), and the
Calibrate tab on `calibration.save`/`fetch` (this session's last commit before this entry).

**What the checkpoints cost, worth remembering:** every shell defect the gate caught was one class —
an import/visibility error that Dart only reports at a consumer (`errorResponse`, `core_actions`,
`DisplayInfo`, `attributes_result` — four times), so the three-namespaces rule is now written in
`v3/shell/README.md` with the new-app-type checklist in `lib/app/README.md`. The other classes were
the fake-async handshake trap, lazy-grid viewports, a stale `TextEditingController` that needed
structural re-seeding (`DraftFieldSlot` + `seedRevision`), and two layout overflows. None were
papered over; each got a test.

**Current counts at this milestone:** Rust workspace `cargo test --workspace --offline` green (73
result lines, ~880 tests), clippy `-D warnings` and `fmt --check` clean; Flutter `flutter analyze`
No issues, `flutter test` 105/105, `dart format` clean. Calibrate is typed-fields-only — the
click-to-set-point / drag-to-set-rect overlay is the next shell increment, deliberately not shipped
unverified because it needs the `BoxFit.contain`/`frame.origin` mapping tested.

## 2026-10-25 (16) — the pipeline reads real captures, and the shell gets its client

The OCR pipeline was assembled (`seal-ocr/src/pipeline.rs`, `modes.rs`) and wired into the core's
`ocr.scan`: det resize 736/32 -> threshold/dilate/contours/min-rect/box-score/unclip -> cubic warp ->
rec -> CTC decode -> the domain's TextCleaner, lines bucketed by v2's configured `row_height`
(floored at 8), and the `lines`/`count`/`attributes`/`grade` modes on the protocol's one result
shape. Two things were learned by running it rather than reading it: ORT allows exactly one
environment per process (the model-gated test failed with "only one environment is allowed" until
det and rec shared a process-wide `OnceLock`), and **the pipeline read all ten of v2's raw
`full_*.png` captures — 10/10 produced text, 508 items in 265 s** — the first end-to-end evidence on
real pixels. Follow-ups named in OPEN: grade colour fallback, `remaining`, capture_path/logging, and
the ~26 s/full-screen cost that says tools must scan crops. The same milestone committed the Dart
protocol client (`263d73d`): 51 fixtures decoded with a coverage guard and recursive shape-drift
check, a client state machine with out-of-order ids/timeouts/events, and a real stdio transport —
the seam now has both halves.

## 2026-10-25 (15) — the hardware surface complete, and OCR proven real

`2528705` wired `board.probe`/`board.test` to the serial adapter: the port choice is seal-board's (a
named port that is absent refuses rather than falling through), the open pays v2's DTR/RTS and 2 s
boot, silence is level 1, and `board.test`'s key argument maps `E`/`enter` to the grammar's Enter
command rather than a letter (the wire letters are the grammar's). `e106c2b` then proved the OCR
runtime on the real models: the character dictionary is the rec model's metadata (6,623 entries; the
output class axis 6,625 confirms it), shapes are recorded, two threads=1 runs are bit-identical,
loads are 173/154 ms and one inference 17 ms, and the shipping payload turned out to be
`DirectML.dll` (static DirectML distribution) with its redistribution terms the one open licence
question. Two findings corrected the plan: the 48-px recogniser height is a preprocessing contract
the model does not enforce, and the det post-processing is the next port (task-29). **Taught:** a
runtime spike that measures beats any amount of spec reading — two assumptions in the research's own
recommendation changed the moment the models ran.

## 2026-10-25 (14) — the toolchain unblocked, the shell scaffolded, the Windows layer built

The player approved the installs, so the machine lost its last excuse: VS Build Tools 2022 17.14 with
the VCTools workload, MSVC 14.44 and the Windows SDK; Flutter stable 3.47.8 at `C:\src\flutter`;
`flutter doctor` clean for Windows. `2ba108c` moved `rust-toolchain.toml` to `stable` (MSVC) and
re-ran every gate on it — the GNU pin existed only because there was no `link.exe`. `207074d` wrote
PLAN-V3-SHELL (hand-written fixture-tested Dart client, built-in state, the wave order, packaging and
the eight-point cut-over gate, five ⚑ player questions), and `15ee238` generated the app at
`v3/shell` with its three Dart gates passing and `serialport` added as seal-board's Windows
dependency. `631ddd3` pinned `state`'s rows in the spec and the fixtures (50 now: `event.state` with
two real rows, `event.state.empty` for the opening snapshot). `6788c0e` gave seal-board its OS
adapter — seconds `Deadline` mapped to driver timeouts, no flush after a failed write, the 2 s boot
wait through the `Clock`, enumeration with the Arduino fallback, four named live-only tests.
`f4e3e9b` built seal-win (window find, the two-space measurement with a thread-scoped aware
context, the foreground refusal, PNG capture), and `7d1942e` wired `window.find`/`window.measure`/
`capture` into the core with the four-frame cache. **Taught:** the two-space contract now exists in
code on both sides, and the covered-game refusal is a guard rather than an error branch to forget.

## 2026-10-25 (13) — the config ruling implemented, the apply made whole, and a docs audit

The ruling from entry (12) became code and then raised two gaps of its own. `af4d9a3` implemented the
portable `config.apply` (settings overlay, dictionary loaded before any write, effective-section
echo), `config_invalid` on reload and the variant-change hook — with a genuine red-first run where
four of eight new tests failed for exactly the ruled reasons. `390411f` extended the golden fixtures
to the new shapes (49 now); `a52c6aa` recorded two defects the work exposed: apply was two atomic
writes under one rev (a projection failure could half-save, against PROTOCOL §4's promise), and
`validate.rs`'s effective copy replaced owned blocks while the writer merges field-wise, so a rule
could judge a config the write would not produce. `46df1ee` closed both — a composed
`apply_sections_with_settings` in `seal-config::store` that builds the projection before one
`update_local`, and a field-wise effective copy — each with red evidence (a half-written `local.yaml`
in the first, a partial `quest` save that validated clean and would have produced a dangling
sequence in the second).

Also this milestone: a read-only docs consistency audit of the whole v3 shelf against the code. It
found fifteen drift points — PROTOCOL still calling itself unimplemented while eight commands
served, `config.changed` and the `phase` examples (`visiting`/`visit`) contradicting the emitter,
CONFIG's §5 bullets describing work that had landed, the divergence log missing four defined lines,
and frame-cache sentences describing unbuilt machinery as current. The fixes (all in this build) are
a warning worth recording: **documentation written while the code is still landing goes stale in
hours**; PROTOCOL §4's "implemented vs spec-only" status paragraph exists so the shell can tell the
difference.

## 2026-10-25 (12) — the rulings: portable `config.apply`, `config_invalid`, the boot channel

The last two commits are a design ruling and the implementation it unblocked. One part was reported to
the Lead as **ambiguous rather than guessed** (does `config.apply` carry the portable `settings:`
block, and how does an invalid or unreadable config reach the shell?), and the ruling arrived as its
own commit (`7c8bfc0`): `config.apply` may carry a `settings` section; the core computes the effective
merged config, runs the rules, resolves the variant and loads the dictionary, then writes owned
blocks and the portable overlay under ONE revision; a failed load is `io` with nothing written and no
revision consumed. PROTOCOL §1 gains the **boot-refusal channel** (non-zero exit, reason on stderr,
nothing on stdout before `hello`), §6 gains **`config_invalid`** for `reload`, §7 states the overlay
semantics, §12.4 records the ruling. The implementation (`b6a49a7`) ran the ported `ConfigValidator`
rules before any command is served — a refusal exits non-zero with v2's `ConfigException` text on
stderr, reusing the crash channel rather than inventing a protocol shape — and made boot and
`config.reload` load the attributes dictionary named by the active variant, all-or-nothing in v2's
`SwitchVariant` order. **Taught:** "invent no protocol shape without a ruling" is cheap and correct —
the ambiguity was named, ruled and recorded on the same day. **Cost:** the portable-apply ROUTING and
the shell rendering are still queued (OPEN), so the ruling has a spec but no caller yet. 74 core
tests at this milestone.

## 2026-10-25 (11) — the spec shelf closes: CALIBRATION, TOOL-FLOWS, DESIGN

Four documents, each the same shape: the v2 record is cited as evidence, the v3 truth is written from
the landed code, and anything not built points at OPEN instead of being restated. **CALIBRATION
(`c5baa65`)** is the document `LOCAL_HEADER` already pointed at and CONFIG §5 listed as the last doc
gap: the canonical space, the DPI-unaware two-space model with the widths measurement, the shell's
`DisplayInfo`-only conversion, capture rules, artifact provenance (the hidden-launcher empty-result
crop, never a live grab), and `CalibrationCheck`'s "stored carries it AND it moved". Its two
unverifiable flags — the `ui:` block's coordinate space and whether `docs/` ships inside an install —
became recorded questions (`bed1c22`) rather than guesses. **TOOL-FLOWS (`070bdc6`)** describes all
eight ids as v3 is: the promise, the flow naming the landed decisions, the verbatim refusal/stop
catalogue grepped from the modules (never invented), the guards, and an explicit loop-not-built
paragraph per tool. **DESIGN (`201d38c`)** completed the shelf: the two-process shape and the five
reasons it is a process, the six-crate map with dependency direction and the erosion rule, the
startup/request/event flow, state ownership, the pure/impure convention and the add-a-tool page, the
coordinate/hardware boundaries, and the port discipline. **Taught:** a doc written FROM the code
catches drift the code review does not — reviewing each claim against the module that owns it is
what let the audit's "authored oracle" distinction reappear in the specs. **Cost:** the docs consumed
a full session's worth of context for no runtime change; the DELIBERATE decision was that the
reasoning must be durable, not that it must be cheap. Also recorded (`25b3a04`): `seal-tools` and
`seal-board` each carry the same one-method seconds-only `Clock`; it moves into `seal-domain` when
the next user appears — a named refactor, not an accident.

## 2026-10-25 (10) — seal-board: the grammar seam and the closed-loop placement

`465fbce` scaffolds, `27f1d25` lands the crate. The board side became a PURE crate over a
`Transport` trait, for the same reason the tools' decision halves did: the interesting part is the
grammar and the loop, not the port. Landed: the level-2 command grammar byte-for-byte (`C/R/E/V/L/l`,
`D dx dy` with RAW counts untouched at 0/+/-1/600/1200/-9600, `K/k`, `F/f` 1-12, `Q/Z` 1-400, one
newline and no carriage return) with refusals for frames the firmware would silently drop; the
transport seam whose `write_line` takes exact wire text and whose `read_line` takes a `Deadline` in
**SECONDS** (the eleven-minute-sleep guard); firmware detection with v2's parse table, two attempts ×
1.5 s past the sketch's `setup()` and SILENCE AS A DEFINITE NO; closed-loop placement with the 2 px
tolerance, 16-step budget, 600 px clamp, divisor damping after a worsening move, settle-before-correct
polling, and a failure that names the goal AND where the cursor stopped; pure port choice that refuses
to open a different board than the one named; and a sketch guard that reads the real `.ino`. **Taught
and recorded as defined deviations:** a transport write failure surfaces as a line instead of
escaping, and an unreadable final cursor says so instead of v2's interpolation. **Cost:** the OS
adapter (WMI enumeration, DTR/RTS open, driver timeouts, anti-cheat sharing, unplug) is named in
`transport.rs`, not faked — it is the same toolchain block as the other reader crates. 53 tests.

## 2026-10-25 (9) — the core takes shape: skeleton, config commands, one writer, the supervisor

The seam's own services, in the order the risk demanded. `6c08abf` (earlier, entry 2) was the
skeleton; this milestone grew it: `8911a05` made the binary boot from the REAL config (`CoreState`
owns a `ConfigStore`, `hello` answers with the merged JSON and a session `rev`, and a root whose
config cannot load exits 1 with stderr instead of running blind); `43c05ea` built the config command
surface (`config.apply` field-wise merge into the owned block, stale `rev` refused as `conflict` with
the current blocks, `config.reload`, `config.validate` shape checks, `ui.window` persisted with the
protocol's height mapping pinned in PROTOCOL by `f777088`); `3bbb60a` made responses and events share
ONE writer thread, so a worker can never split a line (the single-writer test sends 100 of each
concurrently; its first deadlock was a test bug — a cloned `EventBus` kept the writer alive, and the
finish contract is now documented); `b95efc9` landed the supervisor: one thread owns the `Arbiter` and
the worker map, workers report `Opened` and `Done`, and `Done` is the exit path where
`arbiter.finished`, the final snapshot, `tool.finished` and the game hand-over all happen together.
The opening phase is a phase: a start during it is `tool_busy`, a stop during it cancels and ends
`cancelled` with no slot leak, an opening failure frees the slot. **Taught:** the event-writer
shutdown ordering (the writer stops only when the last sender drops, so the supervisor must be joined
first) was REPORTED as open rather than decided silently (`8e976c7`), and so were the rows inner
shape, the deferred-start reply, the `tool_busy` detail and the missing hand-over event
(`e8a6df5`) — a reader should never have to infer a protocol decision from code. **Cost:** the
production registry is a placeholder that refuses every start by name; that line must disappear when
the loops land, not become the new normal (OPEN).

## 2026-10-25 (8) — seal-tools: what v2 could not test, made a pure crate

`30e3aed` scaffolds, then four increments, each one a v2 seam that had no test because it closed over
a port, a config and a runner map at once (`PROGRESS.md` v2 entries 57/59/63/65 are the record).
`f0f0ee9` — tool identity and the immutable snapshot: the eight protocol ids with exact untrimmed
parse (a wire value, not player input), `ToolPhase::engaged()` as the answer to "is it running?"
(entry 57's three windows), and `StateCell = RwLock<Option<Arc<ToolSnapshot>>>` whose publisher swaps
the whole `Arc` — a mixed snapshot is unrepresentable, and a 5,000-publish thread test caught a real
gap in the initial pre-run value. `71df48d` — cancellation as a TYPE: cleanup receives `Cleanup`,
which has no cancellation check in reach, so "a stop abandons the work, not the cleanup" is enforced
by the borrow rather than by remembering; `SleepCheck` ported exactly (50 ms ticks, `Math.Max(1, …)`
floors, seconds as the unit). `0a0974a` — the arbiter: one foreground tool, a stop that only cancels
and holds the slot until `finished`, the resident feeder armed without taking the game and receiving
it on release, `should_stand_down` as `PortGate`'s one live rule (spammer only), v2's `Busy` line
verbatim. `457bd13` — held inputs as RAII, released on normal exit, early return, panic and
cancellation; LIFO instead of v2's fixed U-then-l (divergence #5, `3a921bb`), and two NEW left-button
lines defined where v2 only logged. `debe6f4` — the runner seam: `Tool`/`Effects`/`Clock`,
`RunControl`'s ticked stop, the final snapshot owned by the arbiter so reason and card cannot
disagree, and two lines defined where v2 had none (`Stopped.`, `A board or capture action failed.`).
**Taught:** two real defects were caught by these tests before any tool used them (the busy line
while stopping; a stand-down promise the spammer does not keep) — the point of testing the seam is
that it fails here, not in a live run. **Cost:** the crate is pure, so the `Effects` implementation
that actually drives the board and cursor is still seal-core's missing half.

## 2026-10-25 (7) — the audit, and closing the pure gaps

`0a83467` wrote the audit `PLAN-V3-CORE.md` §8 asked for, and it is the milestone that turned the
port from "done as far as we remember" into a claim a reviewer can check: every v2 test file (27, and
**351 cases re-derived — the documented 324/24 was stale in both numbers**) and every
behaviour-bearing source mapped to its v3 home with real cargo counts; eight gaps named with where
each lands and what proves it; the v3-only test families recorded as the port's added value; and the
honest comparison — decision-bearing coverage, not case-for-case parity. Its sharpest finding was
that **v2 has no test for `ConfigValidator`**, so that oracle had to be AUTHORED. `922c79a` put the
lifecycle hand-over (opening phase, quit-stops-foreground-only, the pause decision, the 45 s pet wait
timeout, tool-side waiting strings) into OPEN rather than implementing it silently. The four pure
gaps then closed, each reported separately: `1c2feba` ConfigValidator (ten annotation + nine
relational checks in v2's order, its exception wrapper, four quirks pinned; every test cites its v2
line), `3cba21a` FoodLoadMode (17 v2 cases), `d80538b` HoldSpace (authored; the double-action quirk
pinned rather than flattened), `f3355cd` CalibrationCheck (7 translated + 4 authored rules). The
audit was NOT rewritten to look better — `7f96a5e` added a same-day addendum pointing at the four
modules and left the 3bd5610 record intact. **Taught:** an authored oracle's RED run usually fixes
the EXPECTATION, not the rule (ConfigValidator failed 3 of 19 first because the fixtures encoded the
wrong null-vs-absent shape; CalibrationCheck 1 of 11 for a valid-but-wrong pair the rule correctly
reports) — that direction is the evidence the oracle is not just the implementation restated.

## 2026-10-25 (6) — the pet feeder's decision half, and the facts-only boundary

The most failure-prone tool, taken in two reported parts. `687cb59` — the window READERS: `FeederEta`
(completion form tested FIRST because it also carries 所需時間; the level form parsed but never
scheduling; the shelf-life 到期 trap skipped by letter; the live OCR mangles accepted),
`FeederLayout` (integer-edged slot boxes so no sliver is lost at the right end where counts are read;
FULL-HEIGHT read regions because the digit-height crop reads nothing; an eta band that is a strict
superset of the old). The module head records the three-way split: readers here, visit decisions next,
**resident/deferral rules in the `seal-tools` arbiter** — the Lead's constraint, because one behaviour
with two owners is a defect. `3bd5610` — the visit decisions: `FeederCount` (digits of the
best-scoring line, the measured gate, the 300 cap) and `PetFeeding` (the unboardable pets dropped so
species+stage is a key, the 14.5 ladder, fraction-of-value minutes, EXP clamped toward done) landed
with their oracles; then `schedule_for`'s six sources (zero means reload
now), `consider_veto` word for word (positive-only; unknown must never cancel a reload), `fold_back`'s
four outcomes with every message verbatim, `build_card`/`schedule_line` with the per-row detail chain,
all thirteen Ready refusals, and `PetFacts` — the plain-data facts the arbiter needs, never the rules.
**Taught:** the domain has no clock, so times are MINUTES FROM NOW and the caller formats `HH:mm`;
that decision fell out of the split rather than being argued. **Cost:** the messages were the largest
part of the work, and they are pinned in `tests/pet_decision.rs` byte for byte. 361 tests at this
milestone (+45).

## 2026-10-25 (5) — the six tools' decision halves, one reported increment each

The pattern proven by SpammerOrder became the plan for every tool: port the pure decision first,
with the v2 tests as oracle, and report each as its own green increment. `d7ab6fb` spammer (16 v2
`SpammerOrderTests` assertions translated + the run-loop rules v2 could not test: opening stagger,
lead-in, preempt as one single per tick with the rest delayed, a combo owning its tick, resume paying
no lead-in; order is an ordered `Vec` because the walk order is the config file's order); `0f471e8`
tuner (stop lines, the repeat signature, mouse guard ported in LOGICAL space with v2's fail-open, the
stop-kind mapping v2 has no vocabulary for, and the `-1 >= -1` unknown-grade quirk pinned rather than
fixed); `9ebbad7` shop (BagGrid 64 row-major centres with C# rounding, ShopGeometry's banker's-rounded
row, `ShopPlan::buy`/`sell` as an ordered plan, sell-slots highest-index-first capped; the unclamped
past-the-bottom row pinned); `77e23e5` gem (11 routes, the empty-box discriminator with the measured
0.294/0.18 pair, `GemRun`'s turns, the empty-streak counter; the auto-check gate ported AS-IS even
though it disables the crop-primary check — recorded, not fixed; `GradeCycles` moved to a shared
module with a re-export); `146d619` quest (card/tab precedence, every refusal, the positioning window,
the loops asymmetry, the flattened pass list, guard rules — it reads NOTHING, which is why it has no
calibration). Two infrastructure commits sit here: `6bea31c` enabled serde_json `preserve_order`
because the spammer walk order is a CONFIG contract (42 golden fixtures differed at the first key and
were regenerated), and `c4c3245` untracked a WIP test file a directory-staged commit swept in.
**Taught:** stage by explicit path, never by directory — the Lead's lesson, recorded in the commit so
the next green snapshot cannot repeat it.

## 2026-10-25 (4) — the domain foundations: geometry, Decision, config model, authoring, OCR

`cdc1e24` scaffolded the crate pair; then the model every tool would build on. `1617385` geometry
(`Dpi.Measure`'s scale from client WIDTHS not heights, `96 × scale`, `ComputeCursorTarget` with
**C# `Math.Round`'s round-half-to-EVEN** pinned — Rust's `round` would click a pixel v2 never
clicked); `17e502a` the `Decision` vocabulary with `Key` limited to what the board can send, the F13+
refusal ported, and `StopKind` reusing PROTOCOL's `tool.finished` reasons; `8939a5d` the eleven
config blocks field-for-field, with v2's initialisers as defaults and the null repairs at decode, and
the fully populated synthetic `local.yaml` round-tripping to identical JSON as the no-field-dropped
proof; `853a5b4` the AUTHORING rules lifted out of the launcher as pure functions with v2's exact
wording (quest dangling-reference gate, step rule, picker rename/re-point/duplicate/delete, preset and
copy names, hotkey VK parse/name, variant switch, Setup's blank client-size refusal — v2 entry 63's
defect); `95cd8c9` wired the rules into `config.validate`/`config.apply` (no widget keeps a rule);
`f153082` then REMOVED the hotkey-code validation the wiring had added — v2's `ConfigValidator` had no
hotkey rule and `GetAsyncKeyState` accepts any code, so refusing a config v2 ran was a parity
regression dressed as validation (RED test first). `e1541de` the attributes dictionary and the pure
OCR halves (`TextCleaner`, `AttrMatcher`, `PetPanel`, `FixSign`), with the regex table refusing
anything beyond the committed literal+`(?!…)` shape **by name** rather than mis-cleaning, and v2's
deliberate matcher quirks pinned. `20fd054` recorded that as divergence #4. **Taught:** moving a rule
out of a widget is also the moment to ask whether the rule was ever right — the hotkey one was not.

## 2026-10-25 (3) — the config crate: three layers, fold, one writer, divergences 1-3

`514cd52` ported `ConfigLoader`'s load order (shipped template → legacy values layer until folded →
`local.yaml` settings then machine blocks, variant resolves the title) and the foreign-block rule:
any top-level block the app does not own survives a write verbatim, comments included. `17d5a20`
landed the writer and the fold, and fixed the review's predicted defect TDD: `update_local` retained
only owned top-level keys before the mutate, because a foreign block also travelled as appended raw
text — the red test showed `my_notes:` twice after one save and three times after two, exactly the
compounding the review named. The fold compares per leaf against the template, carries per section
with v2's `MergeSettings` semantics, writes its marker unconditionally in the same gated save,
falls back to the legacy values layer if the write fails, and never writes the legacy file. The
constant header now says an unmodelled key inside an owned block is **KEPT** (v3 has no typed
projection to drop it); comments are still lost, and the header still says so. The three deliberate
divergences landed as README entries with tests and reasons (`bd87d3a`, `10f456d`, `7e5cedf`), and
`9862c61` wrote [CONFIG.md](../spec/CONFIG.md) as the layer's spec. **Taught:** "the port copies
behaviour, not accidents" needs a written home or the first convenient regression becomes policy. 75
tests at this milestone.

## 2026-10-25 (2) — workspace, gates, protocol spec, golden fixtures, the first skeleton

`5b95318` wrote [PROTOCOL.md](../spec/PROTOCOL.md) — transport/lifecycle, envelope and error codes,
the command and event surface, config merge semantics (core is sole writer and merger), the frame
cache, and the rulings where the spec refines the plan — and moved the plan's status to the plans
board, where it belongs. `c1b3212`/`b75b3f8`/`9df1fdf` scaffolded the cargo workspace, the envelope
types and one-message-per-line framing. `01635ca` recorded the player's decision to keep v2 on .NET 8
past its support date until v3 cuts over (so the plan could stop claiming the bump was forced).
`6c08abf` the first real binary: root discovery, `hello`/`shutdown` through a testable dispatcher, the
NDJSON loop, and an integration test that spawns the process and speaks the handshake over pipes.
`e98b501` then added `cargo.cmd` (a `CARGO_HOME` under `.scratch/` so builds run inside the sandbox),
documented the three gates, and made the first pass of them clean — a warning is a failure, matching
the v2 build gate. The **42 golden fixtures** (`a1779b3`) came later in the day, once the types had
settled: byte-compared against the real types and decoded back, with coverage lists that fail when the
protocol grows a shape — the mechanism that keeps the future Dart side from drifting. **Taught:** the
sandbox denies test-process writes to the system temp dir, so test scratch lives in Cargo's
`target/tmp` — a small thing that shaped every later test helper. 18 tests at the skeleton milestone.

## 2026-10-25 (1) — the plan lands: two processes, one seam

`352c4da` added [PLAN-V3-CORE.md](../../../v2/docs/plans/PLAN-V3-CORE.md), superseding
PLAN-V3-RUST's shape: two processes with a typed protocol, not an in-process C ABI behind WPF or
Tauri. The diagnosis that forced it was measured, not asserted — 77 `.cs` files / 26,650 lines with a
9,415-line launcher growing ~3,800 lines a fortnight, zero interfaces, and the DPI contract requiring
a DPI-unaware process while Flutter's runner is PerMonitorV2. The plan's five reasons the boundary is
a process, the crate shape, the protocol sketch and the port sequence are what this build executed;
the plan's status lives on the plans board, and its reasoning graduates into
[DESIGN.md](../spec/DESIGN.md) now that it is built. **Cost at the time:** the plan is 564 lines
written before anything ran; **taught:** writing the coordinate contract and the protocol down BEFORE
the code is what made "which half owns this?" decidable for the rest of the day.

---

## Process notes that live only in the build conversation

Recorded here as named gaps rather than invented history — the Lead can fill these in:

- **Green-snapshot commits.** Each increment was reported to the Lead with the three gate outputs, and
  the Lead committed it as its own snapshot; a red in-flight test once got swept into a commit by
  directory staging (`c4c3245` documents the correction: stage by explicit path).
- **Dependency rule.** No new crate dependencies without asking the Lead; the workspace pins serde,
  serde_json and serde_norway, and a fetch is needed before `--offline` builds return.
- **Report discipline.** Per-increment reports name files, counts from cargo output, RED evidence, and
  translated-vs-authored oracles; the Lead verifies and commits, and the audit (`PORT-COVERAGE`) is
  the record of what that process produced.

### The Lead's supplement (filled in from the build conversation, 2026-10-25)

- **Who proposed the config ruling.** The portable `config.apply` shape, the boot-refusal channel and
  `config_invalid` were proposed by the core-side author (`config-author`) when task-17's increment 3
  hit the ambiguity; the Lead accepted it with two refinements — the result echoes the effective
  sections for the names in the request in the request's structure, and an invalid on-disk config is
  split into boot refusal (stderr channel) versus reload response (`config_invalid`) — and committed
  the ruling as `7c8bfc0`. Proposal and decision are separate facts on purpose.
- **Branch intent.** All of this is local commits on `v3-core`, branched from `main` (v2); nothing is
  pushed and the cut-over/merge is the player's call. The agent task board used during the build is
  process tooling, not repository history, so it is not recorded here.
- **The audit numbers.** The 27-files / 351-cases figure is the committed audit's re-derivation
  (`0a83467`), not a fresh count taken for this record; the audit also says why the documented "324
  across 24/25 files" is stale in both numbers.

