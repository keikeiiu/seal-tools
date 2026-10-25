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
- **Hotkey typed-name validation.** The Hotkeys tab parses typed names to VK codes and that rule
  has no protocol path (the config carries codes; validating them would refuse configs v2 ran). A
  `hotkey.parse`-style command is the natural shape. See PROTOCOL §11.5.
- **`tool.start` reply shapes.** A deferred start answers `{}` and publishes the waiting line as a
  `log` (there is no run yet to attach a `state` to — a state event would report a registration
  that has not happened); a `tool_busy` refusal's detail is `{line, tool}`. Neither shape is in the
  spec; pin both in PROTOCOL §4 once the shell exists to consume them.
- **No game-hand-over event.** The supervisor emits an interim `log` line `"<tool>" has the game.`
  when the game passes to a waiting run, "until the tool-side waiting strings land" (§1 item 5). A
  proper event or those strings should replace it.
- **The production registry's placeholder line.** Until the tool loops exist, `ProductionRegistry`
  refuses every tool with a defined "not available in this build yet" line; the shell must show it,
  and it must disappear when the loops land rather than become the new normal.
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
- **Binary artifact writer** — the atomic writer carries text only; the calibration PNGs need a
  temp-and-move sibling when `calibration.save` lands.
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
  wired into the core (`7d1942e`, including the four-frame cache). What still waits: the calibrator
  flow (`calibration.save`/`fetch`, the binary artifact writer), `board.probe`/`board.test` wiring,
  and `ocr.scan` (research in flight). The two live behaviours — a real window measures/captures, a
  covered game is refused — are the `#[ignore]`d tests in `seal-win`.
- **VS Build Tools is INSTALLED (2026-10-25)** — VS Build Tools 2022 17.14 with the VCTools
  workload, MSVC 14.44 and the Windows 10 SDK; the Rust workspace is now on the MSVC host
  (`rust-toolchain.toml` pins `stable`). What remains for `seal-win`, `seal-ocr` and the board's OS
  adapter is the port work itself, not the toolchain. **The Flutter SDK is installed** at
  `C:\src\flutter` (stable 3.47.8, Dart 3.13.7); `flutter doctor` is clean apart from the
  Android/Chrome sections this Windows-only project ignores.
- **Flutter SDK** — not installed; the shell half of step 1 cannot start until both are present.
- **OCR port and parity corpus** — `ort` + the PP-OCRv4 pipeline; the corpus covers the tuner path
  only, and the missing feeder/pet/attribute read paths need a live recording session (PLAN §13.10).
