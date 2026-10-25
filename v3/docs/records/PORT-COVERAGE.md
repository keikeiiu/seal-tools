# Port coverage — what v2 behaviour has a v3 home, and what does not

Written 2026-10-25, at the commit where all six tools' decision halves landed (`3bd5610`). This is
the audit `PLAN-V3-CORE.md` §8 asks for: a map from v2's tests and behaviour-bearing sources to their
v3 homes, the gaps, and the tests that are the port's own added value.

**Method.** Every count in this file was produced on 2026-10-25 by running the crate's own suite
(`.\cargo.cmd test -p <crate> --offline` from `v3/core`) and reading the resulting per-binary output;
the v2 side was derived from the files (`[Fact]` + `[InlineData]` cases, no `[MemberData]`/
`[ClassData]` found) and by reading each v3 test file named below, not from memory. The counts are
**as of `3bd5610`**: if a crate has grown since (a parallel increment, e.g. seal-core's event
surface), re-derive that one row with the command above rather than trusting this table. Crate
totals:

| crate | tests | what it is |
|---|---|---|
| `seal-proto` | 13 | the wire types: envelope, framing, the golden fixtures |
| `seal-config` | 75 | the three-layer loader, the fold, the writer, foreign blocks (7 of the 75 are unit tests in `src/yaml.rs`) |
| `seal-domain` | 361 | geometry, decisions, config model, OCR pure halves, authoring rules, the six tools' decision halves |
| `seal-tools` | 74 | arbiter, cancellation types, held inputs, snapshot/phase, the runner seam |
| `seal-core` | 38 | root discovery, handshake, dispatch, the config command surface |
| **total** | **561** | |

---

## 1. Test-file map

`v2/SealTools.Tests/` holds **27** `.cs` files today (not the 25 `v2/README.md` mentions — see §5). Each
row names the v3 file that carries the assertions today, or why it was not translated. "v3 tests"
is the count of `#[test]` functions in the destination file(s), not of translated cases: the
translation is usually one v3 test per v2 *decision*, with the v2 `[Theory]` rows folded into a
loop.

| v2 test file | v2 cases | v3 home | v3 tests there | notes |
|---|---|---|---|---|
| `AttrMatcherTests.cs` | 6 | `seal-domain/tests/ocr_attr_matcher.rs` | 23 | all six translated (filter rules + the overflow finding); the rest are v3-only matcher cases |
| `BagGridTests.cs` | 8 | `seal-domain/tests/shop.rs` | 31 (file total) | 8 translated, including the 47-vs-51 px gap case and the invalid-rect list |
| `CalibrationCheckTests.cs` | 7 | — | — | **gap G6**: `CalibrationCheck` not ported |
| `ConfigLoaderTests.cs` | 66 | `seal-config/tests/{load,fold,foreign,write}.rs`; `seal-domain/tests/config_local.rs` (+ the variant-switch cases in `seal-domain/tests/authoring_rules.rs`) | 68 + 8 (+ 19 in `authoring_rules.rs`, shared with the launcher rules) | split by concern: three layers/merge → `load`; fold → `fold`; foreign preservation → `foreign`; save/atomic/apply → `write`; the local overlay + null repairs → `config_local`; variant switching → `load` + `authoring_rules`. The v2 file's 1,448 lines are the single largest translation |
| `CursorTargetTests.cs` | 3 | `seal-domain/tests/geometry.rs` | 8 | 3 translated; the banker's-rounding pin is v3-only detail |
| `FeederCountTests.cs` | 11 | `seal-domain/tests/pet_decision.rs` | 45 (file total) | 5 v3 tests loop the 11 v2 cases |
| `FeederEmptySlotTests.cs` | 4 | partial | — | the 0.85 "not the boarding window" threshold is carried as `PET_SLOT_NOT_THE_WINDOW_ABOVE`; the pixel comparison itself is seal-ocr → **G3** |
| `FeederEtaTests.cs` | 33 | `seal-domain/tests/pet_reader.rs` | 22 (file total) | 8 eta tests looping 33 cases; the layout cases of the same file became the layout rows below |
| `FeederLayoutTests.cs` | 10 | `seal-domain/tests/pet_reader.rs` | (same 22) | all translated into the layout tests of that file |
| `FirmwareSketchTests.cs` | 4 | — | — | **gap G2**: firmware is still v2's sketch; the test reads `arduino/` |
| `FirmwareVersionTests.cs` | 23 | — | — | **gap G2**: the `V` reply and its messages belong to seal-board |
| `FoodLoadModeTests.cs` | 17 | — | — | **gap G5**: `FoodLoadMode.Complaint`/`IsDrag` not ported; only the drag-vs-right-click gate appears as a private rule in `pet::ready` |
| `GemColorAnalyzerTests.cs` | 6 | `seal-domain/tests/gem.rs` | 24 (file total) | 1 translated (the measured Euclidean-vs-mean pair); 5 are image-level → **G3** |
| `GradeCyclesTests.cs` | 6 | `seal-domain/tests/tuner.rs` | 33 (file total) | all 6 translated |
| `HeldKeysTests.cs` | 3 | `seal-tools/tests/held.rs` | 14 | 3 translated; the RAII/LIFO behaviour is v3-only |
| `IconMatchTests.cs` | 6 | — | — | **gap G3**: icon matching is capture-side |
| `InputArbiterTests.cs` | 9 | `seal-tools/tests/arbiter.rs` | 24 | 9 translated into the start/stop/claim/booking tests |
| `PetFeedingTests.cs` | 9 | `seal-domain/tests/pet_decision.rs` | (45) | 8 translated into the table/remaining-minutes tests; the 9th reads the shipped CSV by walking up from the test binary — kept for the loader side, not translated (file I/O) |
| `PetPanelTests.cs` | 23 | `seal-domain/tests/ocr_pet_panel.rs` | 19 | 23 v2 cases → 19 v3 tests (the `[Theory]` rows loop; the null-lines case is the empty slice, documented in-file) |
| `PortChoiceTests.cs` | 9 | — | — | **gap G2**: COM-port resolution is seal-board |
| `PortGateTests.cs` | 26 | `seal-tools/tests/arbiter.rs` | (24) | the game-claim rules are translated as the arbiter's one-game/one-tool + booking tests; the serial-port specifics belong to G2 |
| `PresetCopyTests.cs` | 11 | `seal-domain/tests/authoring_names.rs` | 13 | translated, including the i32-overflow copy-name case |
| `QuestConfigTests.cs` | 13 | `seal-domain/tests/authoring_quest.rs` | 25 | translated, plus the picker rename/re-point/duplicate rules the launcher kept inline |
| `ShopGeometryTests.cs` | 7 | `seal-domain/tests/shop.rs` | (31) | translated, including the banker's-rounded row 9 |
| `SpammerOrderTests.cs` | 16 | `seal-domain/tests/spammer.rs` | 33 | translated, plus the run-loop tick rules v2 could not test |
| `ToolBaseCancellationTests.cs` | 10 | `seal-tools/tests/cancel.rs` | 13 | translated; the `Cleanup` type replaces the mute flag |
| `ToolToggleTests.cs` | 5 | `seal-tools/tests/tool.rs` + `tests/arbiter.rs` | 14 + 24 | re-expressed: the five "engaged combos" cases are `ToolPhase::engaged` + the arbiter's busy refusal; v3 has no `Decide()` because start/stop are explicit protocol commands |

**Files with no v3 home: 6** (`CalibrationCheckTests`, `FirmwareSketchTests`, `FirmwareVersionTests`,
`FoodLoadModeTests`, `IconMatchTests`, `PortChoiceTests`), plus three partials
(`FeederEmptySlotTests`, and the image-level halves of `GemColorAnalyzerTests`, plus one
`PetFeedingTests` case that reads a file). Every one is also in §3.

---

## 2. Source-file map

`Status` is about the PURE half: **complete** = every decision in the file is ported and tested;
**partial** = the pure half is ported and the named remainder is a reader/effect; **gap** = nothing
in v3 yet. Commits are the ones that landed the v3 side (from `git log` on `v3-core`).

### `v2/SealTools.Core` (31 files)

| v2 source | v3 home | status | commit | remainder belongs to |
|---|---|---|---|---|
| `AppConfig.cs` | `seal-domain/src/config/{shared,quest,local}.rs` (local overlay + shared value types); merged values carried as JSON by `seal-config` | partial (by design) | `8939a5d` | the resolved portable blocks are opaque JSON on purpose; a decision that needs a field gets a typed input struct (see each `tools/*` module). The typed `AppConfig` graph is NOT rebuilt — record, not a gap |
| `ConfigLoader.cs` | `seal-config/src/{load,store,fold,foreign,yaml}.rs` | complete | `514cd52`, `17d5a20` | file I/O — this is its home |
| `ConfigValidator.cs` | — | **gap G4** | — | seal-config (or seal-core boot): load-time geometry/AllowedValues checks and their `ConfigException` texts |
| `AttributesConfig.cs` | `seal-domain/src/config/attributes.rs` | complete | `e1541de` | — |
| `TextCleaner.cs` | `seal-domain/src/ocr/text_cleaner.rs` | complete (no v2 tests existed) | `e1541de` | — |
| `PetPanel.cs` | `seal-domain/src/ocr/panel.rs` | complete | `e1541de` | — |
| `AttrMatcher.cs` (Tuner project) | `seal-domain/src/ocr/matcher.rs` | complete | `e1541de` | — |
| `GemColorAnalyzer.cs` | `seal-domain/src/tools/gem.rs` (Distance/IsEmpty) | partial | `77e23e5` | `seal-ocr`: `Analyze`, `WarmFraction`, `DiffFraction` |
| `BagGrid.cs` | `seal-domain/src/tools/shop.rs` | complete | `9ebbad7` | — |
| `ShopGeometry.cs` | `seal-domain/src/tools/shop.rs` | complete | `9ebbad7` | — |
| `GradeCycles.cs` | `seal-domain/src/tools/grade_cycles.rs` | complete | `77e23e5` | — |
| `FeederEta.cs` | `seal-domain/src/tools/pet.rs` | complete (re-implemented without a regex dep; committed shapes) | `687cb59` | — |
| `FeederLayout.cs` | `seal-domain/src/tools/pet.rs` | complete | `687cb59` | — |
| `FeederCount.cs` | `seal-domain/src/tools/pet/decision.rs` (parse + constants) | partial | `3bd5610` | `seal-ocr`: `WarmFraction` |
| `PetFeeding.cs` | `seal-domain/src/tools/pet/decision.rs` | complete pure (CSV `parse`, key, remaining minutes) | `3bd5610` | `Load` (file read) → seal-config/seal-core |
| `SpammerOrder.cs` | `seal-domain/src/tools/spammer.rs` | complete | `d7ab6fb` | — |
| `QuestAction.cs` | `seal-domain/src/config/quest.rs` + `authoring/quest.rs` + `tools/quest.rs` | complete | `853a5b4`, `146d619` | — |
| `GemRoutes.cs` | `seal-domain/src/tools/gem.rs` | complete | `77e23e5` | — |
| `Dpi.cs` | `seal-domain/src/geometry.rs` (`from_rects`, `cursor_target`) | partial | `1617385` | `seal-win`: the two-space measurement |
| `WindowFinder.cs` | geometry types + cursor mapping in `seal-domain` | partial | `1617385` | `seal-win`: find/foreground/frame measure |
| `ScreenCapture.cs` | — | **gap G1** | — | `seal-win` |
| `Hotkeys.cs` | `seal-domain/src/authoring/hotkey.rs` (names ↔ VK) | partial | `853a5b4`, `f153082` | `seal-win`: the `GetAsyncKeyState` poll; the run-loop interleaving (pause/quit) G8 |
| `HidPointer.cs` | — | **gap G2** | — | `seal-board` |
| `Arduino.cs` | — | **gap G2** | — | `seal-board` |
| `PortGate.cs` | `seal-tools/src/arbiter.rs` | complete (game claim) | `0a0974a` | — |
| `PortChoice.cs` | — | **gap G2** | — | `seal-board` |
| `FirmwareVersion.cs` | — | **gap G2** | — | `seal-board` |
| `HeldKeys.cs` | `seal-tools/src/held.rs` | complete (stronger: RAII) | `457bd13` | — |
| `InputArbiter.cs` | `seal-tools/src/arbiter.rs` | complete | `0a0974a` | — |
| `ToolBase.cs` | `seal-tools/src/cancel.rs` | partial | `71df48d` | the hotkey interleaving (quit pause) and the beeps stay in the loop → G8 |
| `ToolState.cs` | `seal-tools/src/tool.rs` | complete (snapshot replaces the torn-read `ToolState`) | `f0f0ee9` | — |
| `ToolToggle.cs` | `seal-tools/src/tool.rs` (`ToolPhase::engaged`) + arbiter busy | re-expressed | `f0f0ee9`, `0a0974a` | — |
| `FileLogger.cs` | — | **gap G8** | — | `seal-core` (log files and paths) |
| `OcrEngine.cs` | `seal-domain/src/ocr/matcher.rs` (`fix_sign`) | partial | `e1541de` | `seal-ocr`: recognition, grouping, `IsConfirmed`, grade-line cut, logs |
| `RootDir.cs` | `seal-core/src/root.rs` | complete | `6c08abf` | — |

### Tool projects (9 files)

| v2 source | v3 home | status | commit | remainder belongs to |
|---|---|---|---|---|
| `SealTools.Tuner/AttrMatcher.cs` | see Core table above | complete | `e1541de` | — |
| `SealTools.Tuner/SealTuner.cs` | `seal-domain/src/tools/tuner.rs` | partial (decision half complete) | `0f471e8` | effect loop → seal-core; OCR reads → seal-ocr; cursor placement → seal-board/seal-win |
| `SealTools.GemComposer/GemComposer.cs` | `seal-domain/src/tools/gem.rs` | partial | `77e23e5` | capture/OCR/board → seal-ocr/seal-board; loop → seal-core |
| `SealTools.Shop/ShopTool.cs` | `seal-domain/src/tools/shop.rs` | partial | `9ebbad7` | board/capture/foreground → seal-board/seal-win; loop → seal-core |
| `SealTools.Spammer/SpammerOrder.cs` | see Core table above | complete | `d7ab6fb` | — |
| `SealTools.Spammer/SkillSpammer.cs` | `seal-domain/src/tools/spammer.rs` | partial | `d7ab6fb` | board writes/gate wait → seal-board/seal-tools loop; hotkey toggle → seal-win |
| `SealTools.Spammer/HoldSpace.cs` | — | **gap G7** | — | seal-tools (decision) + seal-board (press/release); the guard side exists in `held.rs` |
| `SealTools.Pet/PetTool.cs` | `seal-domain/src/tools/pet.rs` + `pet/decision.rs` (readers + visit decisions) | partial (decision half complete) | `687cb59`, `3bd5610` | OCR/capture/board → seal-ocr/seal-board; loop → seal-core; resident/deferral → `seal-tools` arbiter (`0a0974a`, facts-only boundary in `PetFacts`) |
| `SealTools.Quest/QuestTool.cs` | `seal-domain/src/tools/quest.rs` | partial (plan/guard complete) | `146d619` | board/loop → seal-board/seal-core |

### Launcher (rule-bearing parts only; the UI itself moves to `v3/shell`)

| v2 source | v3 home | status | commit |
|---|---|---|---|
| `MainWindow.xaml.cs` — Save gates, pickers, preset naming, variant switch, client-size rule, hotkey-name parsing, step validation | `seal-domain/src/authoring/*` (the rules) + `seal-core/src/validate.rs` (their wiring to `config.validate`/`config.apply`) | complete (rules); UI deliberately not ported to the core | `853a5b4`, `95cd8c9` |
| `LauncherService.cs` — `SwitchVariant` order-of-safety, `SaveConfig`, attribute load | `authoring::variant::plan_variant_switch`; `seal-config` save + `load_attributes` | complete | `853a5b4`, `514cd52` |
| `LauncherService.cs` — `ResolveArduinoPort`, probe/diagnose, held-release service glue, tool registry | — | **gap G2/G8** | — |
| `MainWindow.xaml.cs` — human (non-decision) interaction: layout, canvases, timers, InfoBars | `v3/shell` (Flutter) — intentionally NOT in the core | n/a (design) | — |

---

## 3. Gaps — v2 behaviour with no v3 home yet

Eight, and each one is a crate boundary rather than a forgotten function. Nothing is listed twice;
"proves it" names the test that would.

| # | behaviour | where it lands | what proves it |
|---|---|---|---|
| **G1** | **The screen halves of Win32:** DPI double-read measurement, window find/foreground/frame measure, GDI capture. The pure coordinate contract (types, `from_rects`, `cursor_target`) is already in `seal-domain`. | `seal-win` (blocked: the workspace pins the GNU target until VS Build Tools installs, per `README.md`) | `Dpi.Measure` against the recorded reference machine (scale 1.5, DPI 144, client 2865×1789, no height-ratio drift) plus a live-checked find/foreground/capture on the game; v2 has no test for these, so v3 must add them where a window exists and mark them live |
| **G2** | **The board and the serial port:** Arduino command writes, level-2 command set, `V` query parsing, port discovery/choice (including "a named port that is missing refuses rather than falling through"), firmware version messages, HID placement/click/drag, wheel notches. | `seal-board` (same toolchain block) | translated `PortChoiceTests` (9), `FirmwareVersionTests` (23), the level/command-shape assertions of `FirmwareSketchTests` (4, shared with the sketch) — all pure over a port mock; plus one hardware-in-the-loop check listed in the release notes |
| **G3** | **Recognition and image matching:** `OcrEngine` (ort + the three PP-OCRv4 models, line grouping, `IsConfirmed`, the grade-line cut, debug logs), `IconMatch`, `GemColorAnalyzer.Analyze/WarmFraction/DiffFraction`, the empty-slot pixel comparison, `FeederCount.WarmFraction`. | `seal-ocr` | translated `IconMatchTests` (6) + the five image-level `GemColorAnalyzerTests` cases + `FeederEmptySlotTests` (4) on synthetic images (v2's own fixtures were synthetic too, since the real crops are personal state) |
| **G4** | **Load-time typed validation:** `ConfigValidator` — the tuner OCR geometry ordering/containment checks, the `[Range]`/`[AllowedValues]` annotations, and their descriptive `ConfigException` texts. | `seal-config` load (or a seal-core boot check if it needs the typed graph) | NEW v3 tests written from `ConfigValidator.cs`: v2 has **no test for it** — `ConfigLoaderTests.cs` contains no `ConfigException`/`Throws` assertion — so this is the one gap whose oracle must be authored, not translated |
| **G5** | **`FoodLoadMode`:** `Complaint` (the "the drag did nothing / this board is too old / what to do" rule) and `IsDrag`'s public form, plus the picker's mode list. | `seal-domain/src/tools/pet` (pure) — the board's drag-level fact comes from G2 | translated `FoodLoadModeTests` (17); the board-level cases need the level number G2 supplies, so they land with it |
| **G6** | **`CalibrationCheck`:** the machine-mismatch comparison between the stored `CalibrationInfo` and a fresh `DisplayInfo`, naming every differing field (PLAN-DEPLOY §step 6). | `seal-domain` (pure, next to `geometry`/`config`), surfaced by seal-core's Setup/`config` commands | translated `CalibrationCheckTests` (7), including "what the stored calibration does not carry is not a mismatch" |
| **G7** | **Hold Space's decision half:** the F12/hold/release rule, its player-facing texts, and the "a press during cold start is swallowed" class of bug. The effect guard exists (`seal-tools/src/held.rs`), the tool decision does not. | `seal-tools` (decision) + `seal-board` (press/release), sharing `HeldInput` | v3 tests for press-during-starting, held-until-stop, release-on-any-stop (the arbiter/cancel tests give the harness) |
| **G8** | **Logging and the non-decision service glue:** `FileLogger` and the log paths (`logs/`, `logs/reads/`, `logs/captures/`, `gem.log`, `pet.log`, `ocr_log.jsonl`), plus the run loop's hotkey interleaving (pause = graceful stop, quit skipped by the pet, checked before cancellation). | `seal-core` (paths/logging; the loop gains the pause/quit check) | v3 tests for path layout and rotation-free append; for the stop signals, v2's ignores-quit case is the oracle and **pause has no v2 test** — that half is authored |

Not gaps, recorded so a future reader does not "fix" them: **ToolToggle** is re-expressed
(`ToolPhase::engaged` + explicit start/stop commands), the **typed `AppConfig` graph** is
deliberately not rebuilt (values stay JSON; decisions get typed input structs), and the **launcher
UI** moves to the Flutter shell rather than the core.

---

## 4. v3-only tests — the port's added value

Behaviour v2 could not test (or had no test for), now pinned. One line each; the crate totals are in
the header.

- **The protocol boundary (`seal-proto` 13, `seal-core` 38):** newline framing, envelope encode/decode,
  the 42 golden fixtures, root discovery/walking, the `hello` handshake over real stdio, EOF-as-stop,
  refusal before handshake, unknown command, `shutdown`, and the whole config command surface —
  `config.apply`'s field-wise merge, stale-revision conflict, hand-edit reload, ownership refusal.
  v2 had no process boundary, so none of this could exist.
- **Config writer (`seal-config` 75):** two sequential writers and two concurrent writers keeping each
  other's block; an unparsable `local.yaml` failing the save instead of being overwritten; a failed
  write reporting its path and leaving no litter (temp-name delivery); CRLF normalisation; a foreign
  block's comment/quoting/list style surviving verbatim; the fold never re-running; the
  header text pinned sentence by sentence.
- **Arbiter and lifecycle (`seal-tools` 74):** a concurrent reader never sees a torn snapshot (the
  `PROGRESS.md` 63/68 torn-read finding); a published snapshot is immutable after a new publish;
  held inputs released LIFO and on every stop path via RAII (v2's entry-65 list of paths that did
  not); cancellation as a *type* — `Cleanup` has no cancellation check, so "this is cleanup" is
  visible at the call site and nesting cannot un-mute; the runner loop with a fake tool and fake
  effects (stop between decisions, effect failure becomes the error line, stop-before-start does no
  work); the resident feeder's booking/deferral lines, now testable without the game.
- **Decisions v2 could not reach (`seal-domain`):** every tool's run loop was behind a serial port and
  a window, so the following are v3-only by construction — the spammer tick (opening stagger,
  lead-in, preemption, combo owns the tick, resume), the tuner stop kinds/order and the mouse guard's
  fail-open, the shop transaction plan and sell-order/cap, the gem turn sequence and empty-streak
  gate, the pet schedule (six sources), veto, fold-back and card, and the quest plan/guard/batch-stop
  lines. Also: the authoring rules that lived in a 9,415-line launcher (57 tests), the text-cleaner
  order and its `\b` rule (v2 had none), the attributes/model hand-edit tolerance, the decision
  vocabulary's protocol-coded stop kinds, and the geometry's round-half-to-even port pin.

---

## 5. The honest number

- **v2 today:** **27** files and **351** derived cases (`[Fact]` 287 + `[InlineData]` 64 across the
  files listed in §1; no `[MemberData]`/`[ClassData]` exists, so no case is invisible to that count).
  `v2/docs/spec/DESIGN.md` says "324 tests" and `v2/README.md` says "324 cases across 25 files": both
  understate the current tree. The docs were not re-derived here — this file is the re-derivation.
- **v3 today:** **561** `#[test]` functions across five crates (table at the top).
- **Not comparable case-for-case, deliberately.** Several v2 `[Theory]` rows became one v3 test with a
  loop (so the v3 number is lower per case), while v3 has whole test families v2 could not have (the
  protocol itself, the arbiter, concurrency, and the untestable run loops), so the total is higher.
  The claim this file supports is the one the plan makes: **every v2 test that pins a decision or a
  message has a translated home — 17 of 27 files directly, 4 re-expressed or partial, 6 with no v3
  home and named as gaps** — not numerical parity.
- **What could not be verified:** anything requiring a live game window, a serial board or the OCR
  models (gaps G1-G3): these have no v3 tests by definition yet. The shipped-CSV assertion in
  `PetFeedingTests` is kept for the loader side rather than translated, and the fixture-based cases
  were. No personal data (`v2/config/local.yaml`) was read or copied.

---

## 6. Update — four gaps closed the same day (task-13)

This file is the audit as written at `3bd5610`; §2 and §3 are kept as the record. Since then the
four pure gaps have been ported into `seal-domain`, each reported and reviewed before commit:

- **G4 `ConfigValidator`** → `src/config/validate.rs` (19 tests; oracle AUTHORED, every rule citing
  its `ConfigValidator.cs` line).
- **G5 `FoodLoadMode`** → `src/tools/pet/food_load.rs` (11 tests carrying all 17 v2 cases).
- **G6 `CalibrationCheck`** → `src/calibration.rs` (11 tests: 7 translated, 4 authored rules).
- **G7 the HoldSpace decision half** → `src/tools/hold_space.rs` (13 tests; oracle authored).

The remaining gaps are the reader crates (G1 `seal-win`, G2 `seal-board`, G3 `seal-ocr`) and the
service/logging glue (G8) — blocked on the Windows toolchain or the core loop, and each named with
its landing place in §3. `seal-domain` is now 415 tests (was 361 when this file was written); the
other crate totals in §5 are unchanged except `seal-core`, which grew on the day too.

---

## 7. Update 2 — after the audit, the same day

- **Golden fixtures: 49** (was 42). The portable `config.apply` and `config_invalid` shapes landed
  (`390411f`), so the cross-language contract grew; the compare-only generator is the reader.
- **The workspace is six crates.** `seal-board` landed (`27f1d25`, 53 tests): G2's grammar, transport
  seam and closed-loop placement are done and only the OS serial adapter remains. `seal-core` grew
  the supervisor and single event writer (`b95efc9`), boot validation and the variant dictionary
  (`b6a49a7`), the portable apply (`af4d9a3`), and the atomic-apply/field-wise-validate fix
  (`46df1ee`).
- **The remaining gaps are unchanged in kind:** G1 (`seal-win`), G2's OS adapter, G3 (`seal-ocr`),
  G8 (service/logging glue). No gap was closed by the audit itself — it was read-only by design.

---

## 8. Update 3 — the Windows layer landed (after the installs)

- **G1 `seal-win` is partly closed** (`f4e3e9b`): window find, the two-space measurement (thread-scoped
  aware context, width-ratio scale) and the PNG client capture exist, with the pure halves tested and
  the two live behaviours `#[ignore]`d; the core serves `window.find`, `window.measure` and `capture`
  (`7d1942e`) with the four-frame cache. Still G1: the calibrator flow (`calibration.save`/`fetch`,
  the binary artifact writer) and live verification against the game.
- **G2's adapter landed** (`6788c0e`): serial transport with the seconds `Deadline`, enumeration with
  the Arduino fallback, the boot sequence through the `Clock`; four behaviours stay live-only
  (cold-plug, unplug, anti-cheat sharing, driver timings).
- **G3 `seal-ocr` is unchanged** — the research task is in flight; the models' provenance and the
  runtime choice are the next gate.
- **The shell exists** (`15ee238`) and its protocol client is in progress; `PLAN-V3-SHELL.md` is the
  map and `v3/docs/plans/README.md` is its status.
