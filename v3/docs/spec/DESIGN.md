# v3 DESIGN — the architecture as built

**Status: current truth, 2026-10-25.** The pure crates and the core skeleton are built and tested; the
Windows and OCR reader crates, the board's OS serial adapter and every tool's effect loop are not (see
§8). This document is the map: why the shape is what it is, which crate owns what, and where the next
piece lands. Siblings own their detail and are linked, never repeated: [PROTOCOL.md](PROTOCOL.md) (the
seam contract), [CONFIG.md](CONFIG.md) (the files), [CALIBRATION.md](CALIBRATION.md) (coordinates),
[TOOL-FLOWS.md](TOOL-FLOWS.md) (per-tool decisions), [OPEN.md](../backlog/OPEN.md) (what is not built),
[PORT-COVERAGE.md](../records/PORT-COVERAGE.md) (the port audit), and v2's
[DESIGN.md](../../../v2/docs/spec/DESIGN.md), which this supersedes for v3.

---

## 1. The shape: two processes, one seam

```
  shell (Flutter/Dart, v3/shell)            core (Rust, v3/core)
  DPI-AWARE · renders · holds a copy        DPI-UNAWARE on purpose · owns the game,
  of config and state · no Win32, no        the board, OCR and the files · decides
  serial, no file writes, no tools          every rule
        │  spawns
        └──► UTF-8 NDJSON over the core's stdin/stdout (one object per line)
             EOF on stdin = "stop safely": cancel the run, release held inputs, exit 0
```

**The seam is a PROCESS, not an interface or an FFI bridge**, for five reasons (the full argument is
[PLAN-V3-CORE.md](../../../v2/docs/plans/PLAN-V3-CORE.md) §2):

1. **The coordinate contract forces it.** The core must stay DPI-unaware — the calibrated physical
   space, the `SetCursorPos` refusal on aware threads (`7f9e1c7`) and the 1:1 physical capture blit are
   built on it ([CALIBRATION.md](CALIBRATION.md) §2). Flutter's Windows runner is PerMonitorV2-aware.
   Two processes each get the awareness they need: core unaware, UI aware and sharp.
2. **A process boundary cannot be violated by an import.** v2's god object existed because the UI could
   reach the domain in-process. Here the only way a widget could decide a pet is unfed is by owning the
   decision — it cannot, because the decision lives behind a pipe.
3. **Crash and stall isolation.** A frozen UI must not kill a feeder that has run for days; the core
   survives, stops safely on EOF, and the next UI launch reconnects to the same files and game.
4. **Each half is testable alone.** The core runs headless from a harness; the shell can run against a
   replay core.
5. **No Win32, no Dart FFI, no third-party desktop plugins in the shell.** The core sends the frame and
   the mapping; Flutter draws.

The protocol is the contract both halves are written against, and the **golden fixtures** are how the
Rust and Dart sides are kept in agreement ([PROTOCOL.md](PROTOCOL.md) §10).

## 2. The crate map

```
seal-proto   the wire types (Request/Response/Event, framing)            serde, serde_json
seal-config  the three-layer loader, fold, atomic writer, foreign blocks serde, serde_json,
                                                                          serde_norway
seal-domain  the pure model and every decision half                      serde, serde_json
seal-tools   lifecycle, arbiter, runner, cancellation, held inputs       seal-domain
seal-board   the board grammar, transport seam, firmware, placement      seal-domain
seal-core    the binary: protocol server, dispatcher, supervisor         seal-proto, seal-config,
                                                                          seal-domain, seal-tools
```

Dependency direction is **downward only**. `seal-core` is the only crate that depends on more than the
domain — today on `seal-proto`, `seal-config`, `seal-domain` and `seal-tools`; `seal-board` is not wired
into it yet. `seal-tools` and `seal-board` depend only on `seal-domain` (their models are the domain's
`Decision`, `ToolId`, geometry). `seal-domain`, `seal-config` and `seal-proto` depend on no workspace
crate at all. The planned `seal-win` and `seal-ocr` sit beside `seal-board` and will follow the same
rule — types from `seal-domain`, nothing above it; the dependency sets are pinned in each `Cargo.toml`,
so changing one is a design event, not a convenience.

| Crate | Owns | Mutable state / files / threads |
|---|---|---|
| `seal-proto` | The wire shapes and line framing | none — pure types |
| `seal-config` | `defaults.yaml.example` base, `local.yaml` overlay + fold, the atomic writer, foreign-block preservation, `attributes.yaml` loading | **files**, one save gate per store; no threads |
| `seal-domain` | Config model + authoring rules, geometry, `Decision`, OCR pure halves, calibration check, the six tools' + Hold Space's decision halves | none — `#![forbid(unsafe_code)]`, no I/O, no Win32 |
| `seal-tools` | `ToolId`/`ToolPhase`/immutable snapshots, the arbiter, the `runner`, cancellation types, held-input RAII | no OS resources; the runner runs on whichever thread calls it |
| `seal-board` | Board command grammar, transport seam, `V` firmware detection, closed-loop placement, port choice | pure over a `Transport`; the OS serial adapter is NOT here |
| `seal-core` | Root discovery, startup, the dispatcher, config revision/save surface, the supervisor thread, the single event writer | owns the process's one writer, config store and supervisor thread |

### The rule that keeps the diagram from eroding

**Nothing above the OS line may reach below it except through a declared seam.** Concretely, and each
is testable:

- `seal-domain` may not read a file, open a port, look at a screen or call unsafe code — it declares
  `#![forbid(unsafe_code)]` and its dependency set is two serde crates. The decision-half convention
  (§5) is what makes that possible.
- A decision half returns data (`Decision` or a small plan type); it never performs the effect. Reader
  and effect contracts are documented input structs, and are the *contract the reader crate implements*
  ([TOOL-FLOWS.md](TOOL-FLOWS.md) names them per tool).
- `seal-tools` is pure: no Win32, no serial, no threads bought from an OS — the loop belongs in
  `seal-core`, implemented against these traits.
- `seal-core` is the **only** writer of `local.yaml` (§4), and the only place a worker's line reaches
  stdout (§3).
- The shell never decides and the core never draws; a rule in a widget is a bug by definition.

## 3. Startup, request and event flow

**Startup (`seal-core/src/main.rs`).** Find the root — `--root`, else walk up from the executable for
`config/defaults.yaml` or `config/defaults.yaml.example` (`root.rs`; the marker rules are
[CONFIG.md](CONFIG.md) §1's). Load the merged config through `ConfigStore`; failure is FATAL (stderr
line, exit 1) rather than a degraded start — a core without config would answer `hello` and then fail
every command. Then spawn the single writer (`EventWriter`, one thread owning stdout) and its
`EventBus`; start the supervisor with the registry; attach it to `CoreState`; enter the stdin loop.
EOF or a read error means `"eof"`.

**Requests.** Each line is decoded (`seal_proto::framing`) and handed to `dispatch::handle`, which returns
a `Response`; the response is queued on the SAME writer channel as events, so no line can interleave
([PROTOCOL.md](PROTOCOL.md) §1). The `hello` gate, protocol-version check, the config command surface,
`ui.window`, `window.find`/`window.measure`/`capture`, `board.probe`/`board.test`, `attributes.get`,
`calibration.save`/`calibration.fetch`, `ocr.scan`,
`tool.start`/`tool.stop` and `shutdown` are the implemented commands today; `tool.start` is live for
the **spammer**, **holdspace**, **tuner** (the first READ-loop tool: capture → OCR → the domain's
`TunerRun::decide` → effects, with the read behind a `TunerSource` seam), **quest** (a pure
config-driven executor — the player positions the mouse during the countdown and a cursor guard stops
a run whose pointer moves; the only seam is the cursor read) and **sell** (a pure transaction-plan
executor over the derived bag/shop geometry). **Buy** is served but refuses at START with the domain's
"No buy preset selected.": its pass needs the run's item and count, and neither a `buy_sell.active`
field nor `tool.start` args exist yet — that protocol decision is named in OPEN and was found by the
shell's card work rather than invented around. The other two ids (gem, pet) refuse by name until
their loops land (the registry is the seam, and its tests pin the refusals). The
rest of PROTOCOL §4. The runner also publishes a tool's own card state (`Tool::status`: message,
countdown, rows) whenever it changes, which is how the tuner's countdown and filter line reach the
`state` event without a second writer. The rest of PROTOCOL §4
is spec-only and is reported in OPEN §3/§4. Every config save runs the domain rules first
(`validate.rs`, "no widget keeps a rule").

**A tool start is a command, not an effect.** `tool_start` asks the supervisor (a channel send); the
supervisor thread owns the arbiter and starts a WORKER whose first act is the opening phase
(`registry.create` — open the board, load the OCR models: the reason it is not on the supervisor's
thread). The `tool.start` response is `Started` (`{}`), `Deferred` (waiting behind the resident feeder),
`Refused` (the arbiter's line as a `log` event), `Unavailable` (no loop in this build) or `Internal`
(a bug, said out loud). The worker reports back exactly twice: `Opened` when it publishes `running`, and
`Done` as its LAST act — `Done` is where `arbiter.finished` runs, so the final snapshot,
`tool.finished`, the slot release and the game hand-over all happen together. A stop during the opening
cancels the token and the worker still ends through `Done`: no slot leaks.

**Events.** Workers only ever send; one writer thread writes. Emitted today: `state` (from an immutable
`ToolSnapshot`), `tool.finished` (from the run's `StopReason` — the same reason that becomes the final
snapshot), `log`, and `core.stopping`. `config.changed` is specified in PROTOCOL §5 but has no emitter
yet — there is no shell to consume it, and a writer-side emitter is the next piece when the UI lands.
Ordering is guaranteed by the single channel: a response the shell asked for and an event a worker
raised can never be seen out of order relative to each other.

**Shutdown order is load-bearing.** Queue the final response FIRST, then `shutdown_tools` (cancels
every run, waits for the workers, emits `core.stopping`), and only then `EventWriter::finish`: the
supervisor is the other `EventBus` holder and the writer stops when the last sender drops. Inverting it
hangs on a sender that outlives the loop (OPEN §2).

## 4. State ownership

- **The core is the sole config writer.** The store owns the save gate and the atomic write; the shell
  gets a COPY of the merged config in `hello` and only ever sends the blocks a tab owns
  (`config.apply {rev, sections}`). It never holds a whole-file copy to write back — that was v2's
  projection-loss class ([CONFIG.md](CONFIG.md) §2/§3).
- **`rev` is a session counter, not an on-disk one.** It starts at 0, and every successful write or
  reload bumps it. The shell echoes the value it was given; a stale `rev` is refused as `conflict` with
  the CURRENT revision and values, so two writers cannot silently fight. The merged view is re-read
  after every successful write, so later commands see the file as it now is, not the startup copy.
- **The supervisor thread owns the arbiter and the worker handles** — one thread, no shared arbiter
  state, which is how v2's two-writer `PROGRESS.md` entry-59 hazard disappears by construction.
- **Published state is immutable snapshots.** `ToolSnapshot` is built by replacing whole values
  (builder methods consume and return), never by mutating a published instance, and readers receive an
  `Arc`; the 750 ms UI tick can never see a half-updated tool. That is the torn-read finding
  (`PROGRESS.md` 63/68): `ToolState`'s "reads are atomic" comment was false for its nullable members.
  `next_action_in_seconds` exists because the crate has no clock: the publish carries seconds from the
  publish, not a timestamp.

## 5. The pure/impure convention, and how a tool is added

The seam inside the core is the same idea as the seam between the processes:

```
        read (the reader crate)                    decide (seal-domain)              perform (seal-core)
  window/capture/OCR/board  ──► typed read structs ──► Decision | ordered plan ──► Effects (board, cursor)
```

- **`Decision`** (`seal-domain/decision.rs`) is `Click(Point)` / `Key(Key)` / `Wait(seconds)` /
  `Stop(StopReason)`. The `StopReason` carries the player's exact line — a reason is not a log line, it
  is `tool.finished.message`. `Wait` is implemented as v2's `SleepCheck`: in chunks, so a stop abandons
  the wait DURING it.
- **Ordered plans where order is the decision.** A spammer tick's presses, a gem turn, a shop pass, a
  quest flow: these are small plan types, not one effect per call — returning one effect at a time
  would move the ordering rules back into the loop (`tools/mod.rs`).
- **Read structs are the reader contract.** Each tool module defines the small types its decisions
  consume and documents, per field, where the core's reader will produce it (`TunerRead`, `ScheduleRead`,
  `PetSetup`, `EmptyRead`, …). Those are what `seal-win`/`seal-ocr`/`seal-board` implement against.
- **Cancellation is a type.** Cleanup receives `Cleanup`, which has no cancellation check; a stop
  abandons the WORK, never the CLEANUP (`cancel.rs`).
- **Held inputs are RAII.** A press that must not survive the tool is a guard released on normal exit,
  early return, panic and cancellation — reverse order of pressing (`held.rs`).
- **The carrier between decide and perform is `runner::run(tool, handle, clock, effects)`:** publish
  `running`, check the stop, `decide()`, perform (`Click`/`Key`/`Wait`), repeat. A failed effect ends
  the run as an error with its line; a cancel ends it as cancelled (`STOPPED_MESSAGE`); a bare effect
  failure uses `A board or capture action failed.` The runner does NOT publish the final snapshot — the
  arbiter does, from the returned reason, so the two cannot disagree.

**Adding a tool end to end** (the page a future contributor needs):

1. **Decision half** in `seal-domain/src/tools/<tool>.rs`: read structs + the decision functions + plan
   types. Port the v2 oracle where it exists; where it does not, the tests are AUTHORED from the v2
   source with line citations ([PORT-COVERAGE.md](../records/PORT-COVERAGE.md) tracks which is which).
2. **Loop**: a type implementing `seal_tools::runner::Tool` (its `decide()` sequence) and an
   implementation of `Effects` for the real board/cursor/capture. The loop is designed to live in
   `seal-core` against the traits — nothing above may call a board directly — and today the binary's
   registry builds no session at all; the fake tools and fake effects in `seal-tools/tests/` are what
   prove the shape.
3. **Registry entry**: `ToolRegistry::create` returns the `Session { tool, effects }` — the opening
   phase. The eight ids are already fixed (`ToolId::ALL`), so the protocol needs no change; until the
   entry exists the placeholder registry refuses the start BY NAME rather than pretending.
4. **Tests**: decision tests in `seal-domain`; the run tested with fake `Tool`/`Effects`/`Clock`
   (`seal-tools/tests/`); the command tested with a fake registry (`seal-core/tests/`).
5. **Doc**: the tool's flow, card lines and guards in [TOOL-FLOWS.md](TOOL-FLOWS.md); its unbuilt
   remainder in OPEN.

## 6. The coordinate and hardware boundary

**Coordinates never appear in this design; they have their own document.**
[CALIBRATION.md](CALIBRATION.md) is the contract: one canonical space (physical, client-relative), the
two-space DPI-unaware measurement, the single `DisplayInfo` the shell may map frames with, capture
rules, and `CalibrationCheck`. The core produces `DisplayInfo`; the shell consumes it; no tool derives
a coordinate from Win32 ([PROTOCOL.md](PROTOCOL.md) §3).

**The board boundary is a grammar plus a seam, not a driver.** `seal-board` owns the exact wire text
(`command.rs`), the closed-loop placement with its tolerance/step budget and settle polling
(`placement.rs`), `V` firmware detection where SILENCE IS AN ANSWER (`firmware.rs`), and the pure port
choice that refuses to open a different board than the one named (`port_choice.rs`). The transport is a
trait (`write a line, read a line, deadline in SECONDS`); tests use `FakeTransport`, a fake cursor and
a fake clock. The OS serial adapter — enumeration, `DtrEnable`/`RtsEnable`, the 2 s boot wait, driver
timeouts, unplug handling, port sharing — is deliberately NOT built; each piece (and what would prove
it) is named in `transport.rs` and OPEN §4.

**What Windows work remains, and where it plugs in:** `seal-win` (window find/measure, the DPI double
read, GDI capture, foreground guard, hotkey polling) and `seal-ocr` (ort + the PP-OCRv4 pipeline) are
blocked on the toolchain pin, not on design ([README §Toolchain](../../core/README.md), OPEN §4). When
they land they implement the read structs and `Effects`; `seal-core`'s registry builds them in a
worker's opening phase. Nothing in `seal-domain` changes.

## 7. Testing and the port discipline

**Gates** (from `v3/core`, details in the [README](../../core/README.md)):

```
.\cargo.cmd test --workspace --offline
.\cargo.cmd fmt --all -- --check
.\cargo.cmd clippy --workspace --all-targets --offline -- -D warnings
```

All three must be clean; a warning is a failure (the core replaces a codebase whose build gate was
already warnings-as-errors). The current test count is whatever `cargo test --workspace` prints; the
audited baseline with its date and per-crate split is [PORT-COVERAGE.md](../records/PORT-COVERAGE.md) §1.

**How a claim is verified:** the audit maps every v2 test file and ported source file to its v3 home
and every gap; the per-tool catalogue lives with the tool's tests; the protocol's golden fixture set
written by the Rust side, decoded by the Dart side when it exists ([PROTOCOL.md](PROTOCOL.md) §10).
Three test families exist that v2 had none of, and they are the port's added value: the protocol
boundary, the arbiter/lifecycle (torn snapshots, RAII release, Cleanup signatures, the runner with fake
effects), and every tool's run-loop decisions that v2 could not reach.

**Oracle discipline — say which one you used.** Where v2 has a test, it is translated with the case
folded (a `[Theory]` becomes a loop, a file becomes a test); where v2 has NO test, the expectation is
AUTHORED from the v2 source with the line cited in the test, and the module docs say so. The audit
records which is which per file: ConfigValidator and Hold Space are the clearest authored oracles, and
the pet's visit decisions and most run loops are the same case — no v2 test existed because those loops
needed a board to construct. **A reviewer checks a claim in three places: the module docs, the test that
pins it, and PORT-COVERAGE's row** — a claim with no home in one of those is a gap, not a fact.

**Deliberate divergences** from v2 are recorded in the [core README](../../core/README.md) — each with
its reason and a test: foreign-comment doubling removed, unmodelled keys kept instead of dropped, the
sorted flow selection after delete, the regex-table refusal (no regex engine), and LIFO held-input
release with two new button lines. The port copies behaviour, not accidents; a divergence without an
entry there does not exist.

## 8. What is not built

Link, do not restate: [OPEN.md](../backlog/OPEN.md) §1 (supervisor behaviours: opening phase details,
quit/pause policy, the 45 s pet wait timeout, tool-side waiting strings), §2 (protocol shapes to settle:
`state.rows`, `config.changed`, hotkey validation, deferred-start replies), §3 (deferred integration:
single-instance lock, binary artifact writer, dictionary switch, `settings:` save path), §4 (the shell
and the Windows/IO crates, including the toolchain blocker). The reader gaps are
[PORT-COVERAGE.md](../records/PORT-COVERAGE.md) §3: **G1 `seal-win`** and **G3 `seal-ocr`** are crates
that do not exist yet, and **G2** is half done — `seal-board` now owns the grammar, placement, firmware
and port choice, but its OS serial adapter is still the adapter's job. Those are why every effect loop
is still "not ported" and why `ProductionRegistry` refuses every start by name.

**Where the plan's status lives:** on the plans board (`v2/docs/plans/README.md`), never in a plan's
own text — a live plan drifts, and this repository has read a shipped plan as pending work before.
