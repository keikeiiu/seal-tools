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

