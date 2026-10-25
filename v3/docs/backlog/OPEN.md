# v3 — open work outside the pure crates

What the pure crates deliberately do not do, and where each piece lands. Ported from the
`seal-tools` runner hand-over (2026-10-25), the `seal-tools` arbiter/runner work, and the still-open
lists in [CONFIG.md](../spec/CONFIG.md) §5 and [PROTOCOL.md](../spec/PROTOCOL.md) §11. Like v2's
backlog, this is work that has been decided or named — not ideas.

## 1. Lifecycle, to land in `seal-core` (the supervisor)

The pure crate ends at the seam; these are the v2 behaviours it deliberately does not own.

| # | behaviour | where | proves it |
|---|---|---|---|
| 1 | **Start-in-flight / opening phase.** v2 swallowed a second Start during the async 2 s board open and could cancel a start in flight (`_startInProgress`, `_startCancelled`, `StartingId`). v3's arbiter starts synchronously, so the core needs an `opening` phase: a second `tool.start` gets `tool_busy`, and a cancelled startup calls `arbiter.finished` with a cancelled reason. | core supervisor | protocol e2e with a slow fake board open: second start refused, cancelled startup ends `cancelled`, no slot leak |
| 2 | **Quit hotkey policy.** v2's `IgnoresQuitHotkey`: a quit press stops the FOREGROUND tool only, never the resident feeder (one press must not end a four-pet schedule). The rule lives where the token is cancelled. | core input loop | arbiter/core test: quit with pet armed stops the foreground, pet stays armed |
| 3 | **Pause vs quit.** v2's `PauseRequested` gave the spammer a graceful pause; v3 models stop/cancel only. If the pause is kept, it needs a decision and a command before any implementation. | protocol decision | — |
| 4 | **45 s pet-reload wait timeout.** The machine models `Deferred` and its timeout line; the constant and the timer belong to the supervisor. | core supervisor | timeout test proves the line and the pending release |
| 5 | **Tool-side waiting strings.** Pet's "`<row>` is due — waiting for {owner} (N min of food left)" / out-of-food, and the spammer's "paused — {waiter} has the game": card strings built by the tool halves from `game_owner()`/`waiting()`. | seam between core and domain | a run where the game is held produces those card lines |
| 6 | **`StopAll(keep: resident)` has no counterpart.** v3 refuses a second start (`tool_busy`) instead of auto-stopping, so shutdown's `stop_all` stops everything. Deliberate divergence, per PROTOCOL §4/§6. | — | recorded here and in the arbiter docs; no implementation |
| 7 | **`ToolId::ALL` is the one accept-and-dispatch list.** v2's defect was a second hand-kept whitelist; the core must use this list for both. | core command table | a test that every `ToolId` is startable and that no other id is |
| 8 | **`ReleaseHeld` on stop paths.** Replaced by the `HeldInput` RAII guard; a stop path must never call the board itself. | — | held-input tests already cover the guard; core review |
| 9 | **`IsStopping` semantics.** v3 keeps the slot until `finished` instead of deregistering plus a separate stopping set; same observable rule (nothing starts, card says Stopping), simpler state. | — | arbiter tests already pin it |

## 2. Protocol shape to settle

- **`state`'s `phase`.** PROTOCOL §5's example prints `phase: "visit"` (a tool sub-phase) while
  `ToolPhase` is the lifecycle (`idle`/`starting`/`running`/`stopping`). Decide how a tool-specific
  sub-phase travels — the proposal is `message`/`rows` only — and correct the spec example.
- **`state`'s `rows` inner shape.** §5 prints `rows: [{...}]` without fixing the inner object; v3
  currently serialises v2's `ToolRowState` with `next_action_in_seconds`, and the shell cannot be
  built against "something". Pin the field list in the spec from `ToolRow` once the supervisor's
  publishing path lands.
- **Event-writer shutdown.** The single writer stops when the last event-bus sender is dropped, so
  the binary must join the tool supervisor — the other bus holder — before finishing the writer;
  otherwise the process can hang on a sender that outlives the loop. The supervisor task owns this.
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



## 3. Deferred integration (named, not scheduled)

- **Single-instance cross-process lock** — the save gate is in-process; the protocol's single-
  instance lock is not implemented yet. Two cores on one root can interleave.
- **Binary artifact writer** — the atomic writer carries text only; the calibration PNGs need a
  temp-and-move sibling when `calibration.save` lands.
- **Variant-dictionary switch** — `load_attributes_named` exists and is tested; the dispatcher does
  not call it yet.
- **`settings:` portable-save caller** — the fold and `save_settings` exist; `config.apply` writes
  owned top-level blocks. A portable-save command is the open integration choice.
- **`Clock` trait is duplicated** — `seal-tools::clock::Clock` and `seal-board::clock::Clock` are
  the same one-method, seconds-only trait (seal-board does not depend on seal-tools). Move it to
  `seal-domain` and delete both copies when convenient; the seconds-only unit is deliberate (the
  eleven-minute-sleep incident).
- **CALIBRATION.md landed** (`c5baa65`) — the header's inherited `docs/spec/CALIBRATION.md` pointer
  now resolves; the live measurement side remains `seal-win`'s (gap G1), as the document states.


## 4. The shell and the Windows/IO crates

- **VS Build Tools (`Desktop development with C++`) is the blocker** for `seal-win`, `seal-board`,
  `seal-ocr` (they need a linker and `windows-sys`-class crates) and for Flutter Windows builds.
  The Rust core is pinned to the GNU host until it is installed; see `v3/core/rust-toolchain.toml`.
- **Flutter SDK** — not installed; the shell half of step 1 cannot start until both are present.
- **OCR port and parity corpus** — `ort` + the PP-OCRv4 pipeline; the corpus covers the tuner path
  only, and the missing feeder/pet/attribute read paths need a live recording session (PLAN §13.10).
