# v3 — open work outside the pure crates

What the pure crates deliberately do not do, and where each piece lands. Ported from the
`seal-tools` runner hand-over (2026-10-25), the `seal-tools` arbiter/runner work, and the still-open
lists in [CONFIG.md](../spec/CONFIG.md) §5 and [PROTOCOL.md](../spec/PROTOCOL.md) §11. Like v2's
backlog, this is work that has been decided or named — not ideas.

## 1. Lifecycle, to land in `seal-core` (the supervisor)

The pure crate ends at the seam; these are the v2 behaviours it deliberately does not own.

| # | behaviour | where | proves it |
|---|---|---|---|
| 1 | **Start-in-flight / opening phase.** v2 swallowed a second Start during the async 2 s board open and could cancel a start in flight (`_startInProgress`, `_startCancelled`, `StartingId`). v3's arbiter starts synchronously, so the core needs an `opening` phase: a second `tool.start` gets `tool_busy`, and a cancelled startup calls `arbiter.finished` with a cancelled reason. | **LANDED** (`b95efc9`; `supervisor.rs`) | protocol e2e with a slow fake board open: second start refused, cancelled startup ends `cancelled`, no slot leak |
| 2 | **Quit hotkey policy.** v2's `IgnoresQuitHotkey`: a quit press stops the FOREGROUND tool only, never the resident feeder (one press must not end a four-pet schedule). The rule lives where the token is cancelled. | core input loop | arbiter/core test: quit with pet armed stops the foreground, pet stays armed |
| 3 | **Pause vs quit.** v2's `PauseRequested` gave the spammer a graceful pause; v3 models stop/cancel only. If the pause is kept, it needs a decision and a command before any implementation. | protocol decision | — |
| 4 | **45 s pet-reload wait timeout.** The machine models `Deferred` and its timeout line; the constant and the timer belong to the supervisor. | core supervisor | timeout test proves the line and the pending release |
| 5 | **Tool-side waiting strings.** Pet's "`<row>` is due — waiting for {owner} (N min of food left)" / out-of-food, and the spammer's "paused — {waiter} has the game": card strings built by the tool halves from `game_owner()`/`waiting()`. | seam between core and domain | a run where the game is held produces those card lines |
| 6 | **`StopAll(keep: resident)` has no counterpart.** v3 refuses a second start (`tool_busy`) instead of auto-stopping, so shutdown's `stop_all` stops everything. Deliberate divergence, per PROTOCOL §4/§6. | — | recorded here and in the arbiter docs; no implementation |
| 7 | **`ToolId::ALL` is the one accept-and-dispatch list.** v2's defect was a second hand-kept whitelist; the core must use this list for both. | **LANDED** (`dispatch.rs` parses through `ToolId::parse`; `tests/tools.rs`) | a test that every `ToolId` is startable and that no other id is |
| 8 | **`ReleaseHeld` on stop paths.** Replaced by the `HeldInput` RAII guard; a stop path must never call the board itself. | — | held-input tests already cover the guard; core review |
| 9 | **`IsStopping` semantics.** v3 keeps the slot until `finished` instead of deregistering plus a separate stopping set; same observable rule (nothing starts, card says Stopping), simpler state. | — | arbiter tests already pin it |

## 2. Protocol shape to settle

- **`state`'s `phase`** — CLOSED: PROTOCOL §5 states that `phase` is the lifecycle
  (`idle`/`starting`/`running`/`stopping`) and that a tool sub-phase, if one is ever needed, travels
  in `message`/`rows`; the examples use `running`.
- **`state`'s `rows` inner shape** — CLOSED: PROTOCOL §5 now pins the four fields
  (`name`/`status`/`detail`/`next_action_in_seconds`) from `seal-tools::ToolRow`, maps them 1:1 to
  v2's `ToolRowState`, states that seconds are relative to the publish, and pins the empty-list rule;
  `event.state` and `event.state.empty` are the fixtures (50 in total).
- **Event-writer shutdown.** The single writer stops when the last event-bus sender is dropped, so
  the binary must join the tool supervisor — the other bus holder — before finishing the writer;
  otherwise the process can hang on a sender that outlives the loop. Implemented: `main.rs` joins the
  supervisor before `writer.finish()`, covered by the shutdown e2e.
- **Hotkey typed-name validation.** LANDED as `keys.parse`/`keys.names` (PROTOCOL §4/§12.5): the
  domain's `parse_virtual_key`/`virtual_key_name` (v2's `ParseVk`/`VkName`) are now served, so the
  Hotkeys tab can edit names while the config keeps codes — and `config.validate` still does not
  refuse stored codes, because v2 ran configs carrying any code. What remains is the tab itself.
- **Pet feeder follow-ups, named not hidden** (`seal-core/src/pet.rs`). The module is split: increment
  1 (plan, card surface, read seam) is committed with the tool still refusing by name; increment 2
  (the loop, reload sequence, four-row state machine, food-load drag) is in flight, and the registry
  flip only happens when that loop is complete. Two reader-side notes: v2's `IconMatch` (queue/food
  icon vs bag cells) is not in `seal-domain` and is being ported as PURE comparison code in the
  adapter (fed by the capture seam, unit-tested), not as a domain API — it is measurement arithmetic
  like the gem crop comparison, not a decision; and v2's feeder-count read walked a 3x/9x/24x
  upscale ladder that `OcrPipeline::scan_rgba` does not offer, so production does one OCR pass per
  slot plus the v2 warm-colour verdict, with the ladder recorded as a follow-up that does not change
  the `Option<i32>` seam contract.
- **Gem composer follow-ups, named not hidden.** `gem.save_empty_captures` is resolved into the plan
  but nothing is written (it waits for the core's logging/artifact writer and a PNG encode outside the
  run loop); the F9 advance-grade hotkey exists only as `advance_grade_by_hand()` because there is no
  hotkey poller in the runner (the supervisor does not read `GetAsyncKeyState` today); and a live run
  has been STARTED (board + window present, phase running with tally rows, stopped cleanly after one
  smoke) but a full transaction has not been watched against the game. The empty-read failure
  divergence (v3 stops, v2 failed open) is recorded in the core README's divergence list.
- **Buy needs a per-run preset/count path before it can start.** The decision half
  (`ShopPlan::buy`) takes `preset_name` + `count` from the RUN, but the protocol carries neither:
  `buy_sell` has no `active`-style field and `tool.start` has no args (found by the shell's Buy card,
  whose picker is display-only for that reason). `tool.start buy` is therefore SERVED and refuses at
  start with the domain's "No buy preset selected." rather than silently choosing preset #1. The
  options are a `buy_sell.active` config field (a persisted default the card can pick) or
  `tool.start` args (a transient per-run choice); the second needs a protocol shape decision, the
  first a config-schema one. Nothing else in the shop tool is blocked — sell is live.
- **Tool-published message/rows surface.** LANDED as `Tool::status()`: the runner publishes the
  tool's own `message`/`countdown_s`/`rows` whenever they change, BEFORE performing the decision they
  belong to (so a countdown shows during its wait), and a runner test observes the published
  snapshots from inside the effects. The tuner publishes its countdown, the domain's filter line
  (`disabled` when off) and the attempt number; the spareness of the other tools' cards is their own
  lack of status, not a missing seam. The pet feeder's four rows will populate `rows` when its loop
  lands (`seal-tools/src/runner.rs`, `seal-core/src/tuner.rs`).
- **Tuner follow-ups, named not hidden** (`seal-core/src/tuner.rs`): the mouse-guard RECENTER path
  surfaces a failed placement as the effects' placement line rather than v2's composed "cursor
  wouldn't re-centre — <detail>" (the runner performs effects, the tool cannot see their failure);
  the countdown publishes no per-second card message for the same reason as the item above; and the
  `hid` spring placement before the FIRST attempt is implemented, while the guard's recentre during
  the loop relies on the same `Decision::Place`. A live run against the game is still unverified
  (needs the game foreground, a board, and the OCR models).
- **`tool.start` reply shapes.** A deferred start answers `{}` and publishes the waiting line as a
  `log` (there is no run yet to attach a `state` to — a state event would report a registration
  that has not happened); a `tool_busy` refusal's detail is `{line, tool}`. Neither shape is in the
  spec; pin both in PROTOCOL §4 once the shell exists to consume them.
- **No game-hand-over event.** The supervisor emits an interim `log` line `"<tool>" has the game.`
  when the game passes to a waiting run, "until the tool-side waiting strings land" (§1 item 5). A
  proper event or those strings should replace it.
- **The production registry's placeholder line.** Until the tool loops exist, `ProductionRegistry`
  refuses every tool with a defined "not available in this build yet" line; the shell must show it,
  and it must disappear when the loops land rather than become the new normal. **The spammer is the
  first live tool**: `tool.start spammer` resolves the active preset out of the merged config, its
  opening phase opens the serial port, the runner drives the domain's rotation through the board's
  key effects, and the run answers `state`/`tool.finished` like any other. **Hold Space is the
  second**: `tool.start holdspace` presses `P`, the run holds until `tool.stop` cancels it, and the
  effects object's `Drop` sends `U` on every exit path (v2's `finally`, as RAII). The other six ids
  still refuse by name.
- **The `ui:` block's coordinate space.** CALIBRATION.md §1 calls it the shell's own space (machine
  state, never read by a tool) and `ui.window` accepts numbers. If the shell needs exact semantics
  (physical vs logical units for its own window), pin it in PROTOCOL §4 rather than letting it be
  implied by a caller.
- **The config-invalid path is half built.** PROTOCOL §1 states the boot-refusal channel (non-zero
  exit, reason on stderr, nothing on stdout before `hello`) and `config.reload` answers
  `config_invalid` (`af4d9a3`); WHAT REMAINS is the shell rendering both.



## 3. Deferred integration (named, not scheduled)

- **Single-instance cross-process lock** — the save gate is in-process; the protocol's single-
  instance lock is not implemented yet. Two cores on one root can interleave.
- **Binary artifact writer** — LANDED (`calibration.save`): `ConfigStore::write_artifact` writes a
  crop PNG with the same temp-and-rename shape as the text writer, refusing a name that is a path.
- **Variant-dictionary switch** — LANDED (`b6a49a7`): boot and `config.reload` load the dictionary the
  active variant names, all-or-nothing in v2's switch order.
- **`settings:` portable-save caller** — LANDED (`af4d9a3`): `config.apply` carries the portable half
  and projects it through `save_settings`.
- **`config.apply` atomicity** — CLOSED (`46df1ee`): `seal-config::store::apply_sections_with_settings`
  builds the projection first, then one `update_local` merges owned blocks and writes the settings
  projection, so a failed apply writes nothing and consumes no rev.
- **Field-wise validate copy** — CLOSED (`46df1ee`): `validate.rs` merges owned blocks field-wise
  like the writer, so a rule judges the config the write produces.
- **`Clock` trait is duplicated** — `seal-tools::clock::Clock` and `seal-board::clock::Clock` are
  the same one-method, seconds-only trait (seal-board does not depend on seal-tools). Move it to
  `seal-domain` and delete both copies when convenient; the seconds-only unit is deliberate (the
  eleven-minute-sleep incident).
- **CALIBRATION.md landed** (`c5baa65`) — the header's inherited `docs/spec/CALIBRATION.md` pointer
  now resolves; the live measurement side remains `seal-win`'s (gap G1), as the document states.


## 4. The shell and the Windows/IO crates

- **`seal-win` LANDED (`f4e3e9b`)** — `window.find`/`window.measure`/`capture` are implemented and
  wired into the core (`7d1942e`, including the four-frame cache); `board.probe`/`board.test` are
  wired too (they use `seal-board`'s adapter), and `ocr.scan` is implemented (`ocr_unavailable` when
  the models are absent). The calibrator flow is now served too: `calibration.save`/`fetch` with a
  seven-kind table, the one `local.yaml` write gate, derived values (bag-grid pitch), the bag
  slot-vs-grid agreement rule, the environment block and the result-gem crop as the first binary
  artifact. What still waits there: the remaining mark kinds (pet marks/crops, tooltip, quest step
  maps, gem points/MAX/sell slots), the drawn-over reference screenshots, and a `config.changed`
  emitter so a shell with a mirror can see another writer's mark without refetching. The two live
  behaviours — a real window measures/captures, a covered game is refused — are the `#[ignore]`d
  tests in `seal-win`.
- **OCR pipeline follow-ups, named not hidden:** the grade colour fallback and the `remaining`-band
  read are not ported (the result carries `remaining: null` and grade-from-line only); `capture_path`
  stays null until the core's logging writer exists; full-screen scans cost ~26 s per capture on CPU
  (the ten real captures were 265 s in total), so a tool's OCR read should scan its crop, not the
  screen; and end-to-end text parity still needs the raw-crop recording pass
  (OCR-RESEARCH §"Runtime spike results" and §4).
- **OCR shipping/build facts to settle** (measured by the runtime spike, `e106c2b`): the release zip
  must carry **`DirectML.dll`** (~18.5 MB — the `download-binaries` static distribution links
  `onnxruntime.lib` in and ships no `onnxruntime.dll`), and its **redistribution terms must be
  confirmed with Microsoft before release** — the one licence item still open. A clean build needs
  network for the ORT download or a seeded `ORT_CACHE_DIR`; the decision for now is to keep
  `ort = "=2.0.0-rc.15"` and document the seeding rather than switch to `load-dynamic` (+ a shipped
  `onnxruntime.dll`), which would need a Cargo feature change.
- **VS Build Tools is INSTALLED (2026-10-25)** — VS Build Tools 2022 17.14 with the VCTools
  workload, MSVC 14.44 and the Windows 10 SDK; the Rust workspace is now on the MSVC host
  (`rust-toolchain.toml` pins `stable`). What remains for `seal-win`, `seal-ocr` and the board's OS
  adapter is the port work itself, not the toolchain. **The Flutter SDK is installed** at
  `C:\src\flutter` (stable 3.47.8, Dart 3.13.7); `flutter doctor` is clean apart from the
  Android/Chrome sections this Windows-only project ignores.
- **Flutter SDK** — not installed; the shell half of step 1 cannot start until both are present.
- **OCR port and parity corpus** — `ort` + the PP-OCRv4 pipeline; the corpus covers the tuner path
  only, and the missing feeder/pet/attribute read paths need a live recording session (PLAN §13.10).
