# seal-tools v3 — Code-Level Review

> **Scope:** `v3/` on `main` @ `7f5788e` ("Merge pull request #8 from keikeiiu/v3-core", 2026-11-03), i.e. https://github.com/keikeiiu/seal-tools/tree/main/v3 — Rust core (`v3/core`, 8 crates), Flutter/Dart desktop shell (`v3/shell`), Arduino firmware, protocol fixtures, release script and CI.
> **Version under review:** v3.0.6 — latest release record (`v3/records/RELEASE-v3.0.6.md`, built 2026-10-28); the code is already stamped 3.0.7 (`version.dart`, `pubspec.yaml`), see SHELL-F10. v1/v2 are out of scope; v2 (`v2/`, C#) is cited only as the behavioural parity oracle where v3 regressed.
> **Lenses:** (1) SOTA, (2) Production-grade, (3) Performance & Accuracy.
> **Companion:** [`REVIEW_IMPLEMENTATION.md`](REVIEW_IMPLEMENTATION.md) — per-finding implementation plans, code sketches, tests and acceptance criteria, ordered into phases.

## 0. Method and limits

- Six subsystem slices were reviewed independently, then cross-checked; overlapping findings were merged (see Implementation Appendix B).
- **Static review only.** No Rust or Flutter toolchain was available on the review machine, so `cargo test/clippy`, `flutter test/analyze` and live game runs were **not** executed. Every finding cites `path:line` in the tree above; confidence is stated per finding. Line numbers refer to `7f5788e`.
- Path shorthand: `pet.rs`, `dispatch.rs`, `supervisor.rs`, `tuner.rs`, `quest.rs`, `effects.rs`, `registry.rs`, `runner.rs`… without a crate prefix are under `v3/core/crates/seal-core/src/` unless the finding names another crate (`seal-ocr`, `seal-domain`, `seal-tools`, `seal-config`, `seal-board`, `seal-win`). `PetTool.cs`, `QuestTool.cs` etc. are the v2 references under `v2/`.
- Items already listed in `v3/docs/backlog/OPEN.md` are marked *documented*; they are included only when the review adds a concrete fix.
- Explicitly **out of scope:** anything whose purpose is evading the game's anti-cheat or bot detection. Recommendations are engineering quality only (correctness, safety of the player's own items, robustness, performance).

## 1. Executive summary

v3 is a serious, well-structured rewrite. The architecture is sound and above the norm for a hobby automation suite: a pure decision layer (`seal-domain`) separated from effectful stage machines (`seal-core`), a single-writer NDJSON seam with typed fixtures shared by both sides, an arbiter that is a pure state machine, RAII release guards for held inputs, minimal and documented `unsafe`, and a dependency-free port of the PP-OCRv4 pipeline. Most decision halves have strong v2-oracle tests.

The defects cluster at the **joins** — where config is resolved into a tool, where supervisor and worker hand off, where a loop crosses a long wait — and the tests miss them because each half is tested alone and no test drives a tool past its first long `Wait`. Several are **regressions against v2 that can cost the player items or leave input stuck**:

1. **Tuner ignores its filter** — reads top-level `filter` instead of `tuner.filter` (`tuner.rs:107`); every live run uses `FilterConfig::default()` and rolls past matching items. *(TOOL-F1, Critical)*
2. **Pet Feeder food cursor never advances** (`pet.rs:1608,1925`) and **the timer never forces a reload** (veto path dead) — the pet starves while the card reports success. *(PET-F1, PET-F2, Critical)*
3. **No foreground/window gate before clicks** in the Pet Feeder, and display geometry is measured once per run for all tools — clicks and Enter can land in another application. *(PET-F4, TOOL-F3)*
4. **Quest guard reference taken before the positioning countdown** — practically every quest run stops on step 1. *(TOOL-F2)*
5. **Supervisor invariants:** stopping a tool waiting for the game strands it forever; a worker panic leaks the arbiter slot and hangs shutdown (zero `catch_unwind` in the workspace); a stale deferred-start timer cancels a later start. *(CORE-F1, CORE-F3, CORE-F6)*
6. **Config state has no single owner:** the dispatcher republishes a stale config over worker writes after every command; `config.apply` cannot delete entries (deleted quest flows come back, renames duplicate); a committed two-file save can be silently rolled back on the next load. *(CORE-F4, HW-F1, HW-F2)*
7. **Live `state` events are never pushed during a run** — cards don't update mid-run, and a test locks the gap in. *(CORE-F2)*

On **performance and accuracy**, the OCR port has two deviations from reference DB post-processing (crop without unclip; box score scanning the whole map per contour — consistent with the measured ~26 s full-screen read), no max-side cap on det resize, a rotation bug in corner ordering, raster-order line joining, and no text-level regression corpus. Internally every read pays a PNG→base64→JSON→base64→PNG round trip. Fixing OCR-F2/F3 alone should bring full-screen reads from tens of seconds to ~1–2 s on CPU.

**Recommendation:** land Phase 0 of the implementation plan (21 items, mostly effort S–M) before advertising v3 as the published release; add the integration/simulation test layer in the same pass so these classes of bug stay fixed.

The **Flutter shell** uses sealed protocol types with exhaustive pattern matching and strict analyzer settings, but a client that closes on an undecodable line never kills its core process, so **Restart can leave an orphaned core still driving input** (SHELL-F1). Separately, **CI does not build or test v3 at all**: `.github/workflows/build.yml` is .NET/v2 only (SHELL-F2). The release script audits the zip contents carefully, but it builds from whatever is in the working tree. It doesn't use `--locked`, has no test gate, writes no checksum file and does no signing, and there is no single source of truth for versions: the core reports `0.1.0` while the shell is at 3.0.7 (SHELL-F9/F10).

## 2. Scorecard

| Slice | SOTA | Production-grade | Performance & Accuracy | Headline |
|---|---|---|---|---|
| OCR & vision | ◐ faithful PP-OCRv4 port; no rec-only path, no batching, Simplified-centric model | ◐ explicit errors, SHA-pinned models; stale cached pipeline, no corpus | ○ det post-processing O(map×contours), no unclip, no max-side cap | Biggest latency/accuracy upside |
| Core runtime & protocol | ◐ single-writer NDJSON, typed fixtures; hand-parsed args, 2.6k-line dispatcher | ○ stop strands worker, no panic containment, config drift, no shutdown deadline | ◐ big JSON clones per request, gallery decode on every fetch, unrotated logs | Concurrency edges |
| Pet Feeder | ● pure, oracle-tested decision layer | ○ cursor/timer/foreground/hover-loop regressions vs v2 | ◐ rate trust on single read; tick-sum sleeps | Not safe to run unattended yet |
| Other tools & lifecycle | ● arbiter state machine, one-way cancel, whole-snapshot publish | ○ tuner filter, quest guard order, off-by-one retries, click after Stop | ◐ unbounded/unvalidated inputs | Two in-game-loss bugs |
| Board / Win / Config / FW | ◐ clean seams, documented unsafe; GDI capture, windows-sys 0.52 | ◐ atomic file writes + journal, but delete/rollback/migration semantics wrong | ◐ 2 s reopen per action, 10 ms per firmware loop, damping-only placement | Save semantics |
| Flutter shell & release | ● Dart 3 sealed types, strict lints; ○ no l10n, hand-written protocol types, no CI/signing/SBOM | ○ orphaned core on restart, drafts lost on close, no shell error log | ○ multi-MB base64+PNG decode in `build()`, whole-tree rebuild per log line | Process lifecycle & release pipeline |

● strong · ◐ adequate with gaps · ○ needs work before release

## 3. Cross-cutting themes

1. **Cancellation and supervisor invariants as types.** One `Done` per spawned worker (drop guard + `catch_unwind`), every stashed reply answered on every exit path, generation-tagged timers, a shutdown deadline, a stop check between `decide()` and the effect, and cancellable `GamePort::request`. → CORE-F1/F3/F6/F8, TOOL-F8.
2. **One owner for config.** A `ConfigHub` (revision-stamped `ArcSwap` snapshot) shared by dispatcher, registry and sinks; a single `merge_with_policy` (record maps replaced whole, everything else field-merged) used by validate/store/layout; one `atomic_replace` durable-write primitive; typed per-tool config structs built through the real merge in tests. → CORE-F4, HW-F1..F5, TOOL-F1/F12, PET-F11/F14.
3. **Fail closed on destructive or blind input.** Gate every click on window presence + foreground; re-measure geometry per visit/placement; Retry/Combine/Sell only after a *confirmed* read; uncertainty stops the run. → PET-F4/F5/F6, TOOL-F3/F7, TOOL-F15.
4. **Raw frames inside the core, PNG only at the edges.** A `Frame { w, h, origin, rgba: Arc<[u8]> }` from capture to OCR/vision, one shared OCR pipeline instance, base64 only when the frame crosses the protocol. → HW-F12, OCR-F9, CORE-F10/F11.
5. **OCR fidelity, measured.** Land the eval corpus/harness (OCR-F11) first, then reference-correct DB post-processing (F1–F5), a rec-only path for calibrated single-line boxes (F14), batching/threads (F6), and evaluate PP-OCRv5 / `chinese_cht` rec for Traditional Chinese (F13).
6. **Tests at the joins.** Simulation harness with a fake clock driving tools for days (pet), integration runs through `runner::run` with fake effects, proptest state-machine tests for arbiter/scheduler/veto invariants, a dispatcher-vs-fixture conformance test. The current suite tests halves in isolation.
7. **Observability.** Panic hook → `logs/error.log` with backtrace, `tracing` spans per tool run/request, size-rotated appenders, a structured `pet.jsonl` replacing v2's `pet.log` forensics. → CORE-F3/F12, PET-F7.
8. **Hardware link as a session.** A long-lived reconnecting `BoardSession` actor, readiness polling instead of a fixed 2 s wait, firmware protocol level 3 (keepalive watchdog while holding, abort, optional ack/seq), gain-estimating placement controller. → HW-F6..F11.

## 4. Phase 0 — must fix before publishing v3

Ordered by user impact. All are detailed in `REVIEW_IMPLEMENTATION.md` §Phase 0.

| # | ID | What breaks | Effort |
|---|---|---|---|
| 1 | TOOL-F1 | Tuner filter ignored → rolls past wanted items | S |
| 2 | PET-F1 | Food cursor reset → reloads report success with no food | S |
| 3 | PET-F2 | Timer never forces reload; unknown reads reschedule forever | M |
| 4 | PET-F4 | Clicks/Enter with game not in front | M |
| 5 | TOOL-F2 | Quest guard trips on step 1 of every run | S |
| 6 | PET-F5 / PET-F6 | Infinite hover loop; zero-delay reopen loop | S |
| 7 | TOOL-F7 / TOOL-F8 / TOOL-F6 | Destructive retry on unconfirmed read; click after Stop; one extra attempt | S |
| 8 | CORE-F1 / CORE-F3 | Stop strands waiting worker; panic leaks slot, hangs shutdown | S–M |
| 9 | CORE-F4 | Stale config republished over worker writes | M |
| 10 | CORE-F2 | No live `state` events during a run | M |
| 11 | HW-F1 / HW-F2 | Deletes don't persist; committed save can roll back | M |
| 12 | TOOL-F3 | Geometry measured once per run | M |
| 13 | PET-F7 | Row outcomes never reach card or log | S–M |
| 14 | CORE-F5 | `ocr.scan` grade colour crop uses wrong frame size | S |
| 15 | SHELL-F1 | Closed client never kills its core → orphan still driving input after Restart | M |
| 16 | SHELL-F2 | No CI for v3 (prerequisite T1) | M |

## 5. SOTA gap summary

| Area | Today | Recommended |
|---|---|---|
| OCR model | PP-OCRv4 `ch` rec, ~50 hand repairs for Traditional chars | Evaluate PP-OCRv5 mobile rec / `chinese_cht_PP-OCRv3` on a corpus; CTC forced-alignment scorer over the closed attribute vocabulary |
| OCR runtime | 1 crop/inference, 1 thread, DirectML.dll shipped but unused | Batched rec (6), configurable threads, DML for det with CPU fallback — or drop the DLL |
| Fixed-layout reads | Full det+rec on calibrated boxes | Rec-only `read_line`; NCC glyph templates for bitmap-font digits |
| Capture | GDI `BitBlt` + `CAPTUREBLT`, PNG round trip | Raw frames internally; Windows.Graphics.Capture / DXGI duplication behind the same seam |
| Rust runtime | Hand-rolled std threads/mpsc, ad-hoc logs | `tracing` + rotating appender, panic hook, `ArcSwap` config hub, `recv_timeout`/`select!` supervisor loop |
| Protocol types | Hand-written decoders on both sides, command list in 3 copies | Typed `serde` structs per command → `schemars` JSON Schema → Dart codegen; command table asserted against fixtures |
| Config | JSON-pointer strings + `unwrap_or(default)` | Typed per-tool config, JSON Schema shipped for editor validation, explicit `config_version` migrations |
| Testing | Unit tests per half | Simulation harness with fake clock, proptest state machines, dispatcher conformance test, OCR eval gate |
| Firmware | Fire-and-forget ASCII, failsafe only on DTR | Level 3: keepalive watchdog, abort, optional ack/seq |
| Shell state & perf | One root `ListenableBuilder`, 1,968-line `ShellModel`, frames decoded in `build()` | Scoped notifiers, decode-once `Uint8List` / `Isolate.run`, per-frame log batching |
| l10n | ~456 hard-coded English `Text(` literals, zh-TW guide maintained by hand | `flutter_localizations` + ARB (`app_en`, `app_zh_TW`) |
| Release | Local script, unsigned, hash printed to console, notices incomplete | Tag-triggered workflow: `--locked` build, Authenticode, `SHA256SUMS`, `cargo about` notices, CycloneDX SBOM, `attest-build-provenance` (SLSA L2) |

## 6. Strengths worth keeping

- Pure decision layer with v2-verbatim messages and oracle tests from real logs (`seal-domain`); relative-minute scheduling immune to DST.
- Single-writer stdout, responses always flushed before the events they caused; documented shutdown ordering; poisoned locks recovered with `into_inner`.
- Arbiter as a single-owner state machine; slot cleared only on `finished`; one-way `CancelToken`; `Cleanup` makes "cleanup ignores cancellation" a compile-time property.
- RAII release of held inputs (`BoardEffects::drop`, `SpaceHoldEffects`, `PetTool::drop`); sell is fail-closed (highest slot first, de-duplicated, capped).
- Minimal `unsafe` with SAFETY notes; correct GDI release order; per-thread DPI awareness.
- Atomic per-file writes (temp → `sync_all` → rename) and a write-ahead journal for two-file saves; named-port refusal instead of opening another board.
- OCR port gets the subtle OpenCV conventions right (strict `>` threshold, dilate anchor, pixel-centre resize), dictionary SHA-pinned from model metadata, deterministic thread counts.
- Replay mode, process-level handshake tests, shared protocol fixtures.
- Shell: sealed `IncomingMessage`/`ProtocolEvent`/`Response` hierarchies with exhaustive `switch`, bounded stderr/log buffers, strict analyzer options, fixture cross-checks; `publish.ps1` has an exact allow-list, reparse-point refusal, post-zip audits and a `-Selftest`.

## 7. Spot-verification performed by the lead reviewer

Re-read in source after the slice reports: `tuner.rs:107` (`config.get("filter")`), `pet.rs:1608/1925` (`visit.food_used = self.plan.food_slots_used`), `quest.rs:166` (`phase: Phase::GuardReference`), `version.dart:11` / `pubspec.yaml:19` (3.0.7) vs Cargo `0.1.0`, `build.yml` (.NET only), no production caller of `ShellModel.shutdown()`, `dispatch.rs:405-410` (shadowed `width`/`height` passed to `crop_rgba`), `pet.rs:1601` (`_ => visit.stage = Stage::PickPage` on verify error), `supervisor.rs` Stop arm not touching `pending_reply`, and zero `catch_unwind` in `v3/core/crates/*/src`. All confirmed as described.

## 8. All findings

Severity: Critical (player loss / unsafe input) · High · Medium · Low. Lens: SOTA / PROD / PERF-ACC. Phase: P0 blocker, P1 hardening, P2 performance/SOTA. Merged items point to the ID under which they are implemented.

| ID | Finding | Severity | Lens | Conf. | Phase |
|---|---|---|---|---|---|
| **OCR** | **OCR & screen reading (seal-ocr, vision helpers)** | | | | |
| OCR-F1 | The crop uses the shrunk DB kernel, not the unclipped box | High | PERF-ACC | Medium-High (documented as an "approximation") | P1 |
| OCR-F2 | `filled_mean` scans the whole score map for every contour, so latency is O(map × contours) | High | PERF | High | P1 |
| OCR-F3 | Det resize is missing the max-side cap and C#'s vertical padding, so small or narrow crops become huge det tensors | High | PERF-ACC | High | P1 |
| OCR-F4 | `order_corners` starts at the top-most corner, so a slightly tilted box comes out rotated 90° | Medium | PERF-ACC | Medium | P2 |
| OCR-F5 | The grade line is concatenated in contour (raster) order, not left to right | Medium | PERF-ACC | High | P2 |
| OCR-F6 | Recognition is single-crop and single-threaded; the bundled DirectML runtime is never used | Medium | PERF/SOTA | High | P2 |
| OCR-F7 | Every internal OCR read goes through a PNG encode, base64, JSON, base64 decode and PNG decode round trip *(merged → HW-F12)* | Medium | PERF | High | — |
| OCR-F8 | The cached dispatcher pipeline goes stale after `config.reload` or a variant switch *(merged → CORE-F7)* | Medium | PROD | High | — |
| OCR-F9 | Three independent copies of the ~25 MB model set are loaded | Low | PERF/PROD | High | P2 |
| OCR-F10 | The unused classifier model is a hard requirement | Low | PROD | High | P2 |
| OCR-F11 | No text-level regression corpus; the end-to-end test only checks that something was read | Medium | PROD | High | P1 |
| OCR-F12 | Upscaling uses nearest-neighbour where v2 used cubic or area interpolation | Low-Medium | PERF-ACC | High | P2 |
| OCR-F13 | The recogniser model lacks native Traditional Chinese support | Medium | SOTA | Medium | P2 |
| OCR-F14 | Fixed-layout reads still run free-form text detection | Medium | SOTA/PERF | Medium-High | P2 |
| OCR-F15 | Attribute matching is post-hoc fuzzy string matching over a closed vocabulary | Low-Medium | SOTA/PERF-ACC | Medium | P2 |
| OCR-F16 | Contour tracing uses a per-pixel `BTreeSet` and `HashMap` | Low | PERF | High | P2 |
| OCR-F17 | One threshold serves two different metrics in the gem empty check (parity with v2) *(merged → TOOL-F15)* | Low | PERF-ACC | High | — |
| **CORE** | **Core runtime & shell–core protocol (seal-core main/dispatch/supervisor/logs)** | | | | |
| CORE-F1 | Stopping a tool that is waiting for the game strands its worker forever | High | PROD | High | P0 |
| CORE-F2 | Live `state` events during a run are never sent | High | PROD | High | P0 |
| CORE-F3 | Nothing contains panics: a worker panic leaks the slot and hangs shutdown | High | PROD/SOTA | High | P0 |
| CORE-F4 | The dispatcher's config and revision drift from worker writes, and the stale copy overwrites the fresh one | High | PROD | High | P0 |
| CORE-F5 | `ocr.scan` grade colour crop passes the wrong image size | Medium | PERF-ACC | High | P0 |
| CORE-F6 | A timeout left over from an earlier deferred start cancels a later one early | Medium | PROD | High | P1 |
| CORE-F7 | The cached OCR pipeline is never rebuilt after config changes | Medium | PERF-ACC | High | P2 |
| CORE-F8 | Shutdown has no deadline | Medium | PROD | Medium | P1 |
| CORE-F9 | stdin framing is lenient in the wrong places | Medium | PROD | High | P1 |
| CORE-F10 | Calibration fetches carry the whole gallery and fully decode every PNG | Medium | PERF | High | P2 |
| CORE-F11 | Large JSON values are cloned on every request | Medium | PERF | High | P2 |
| CORE-F12 | Logs grow without limit and every line reopens its file under one global lock | Medium | PROD/PERF | High | P1 |
| CORE-F13 | The OCR fixtures are hand-written, and the command list exists in three copies | Medium | SOTA | High | P2 |
| CORE-F14 | `board.test` and `board.probe` skip the input lease | Low | PROD | High | P2 |
| CORE-F15 | The artifact reference loader accepts absolute and UNC paths | Low | PROD | Medium | P2 |
| CORE-F16 | Tick-based sleeps drift *(merged → PET-F12)* | Low | PERF-ACC | Medium | — |
| **PET** | **Pet Feeder (seal-core pet*, seal-domain tools/pet)** | | | | |
| PET-F1 | The food-cell cursor never advances within a run | Critical | PROD | High | P0 |
| PET-F2 | The timer is not a trigger: unknown readings never force a reload, and the veto path is dead | Critical | PROD/SOTA | High | P0 |
| PET-F3 | Persisted run state is reverted in memory, so Stop→Start in one session restarts from boot values *(merged → CORE-F4)* | High | PROD | Medium-High | — |
| PET-F4 | No foreground or window gate before input, and capture refusals are treated as success | High | PROD | High | P0 |
| PET-F5 | Infinite hover loop when the return-slot pet reads as finished | High | PROD | High | P0 |
| PET-F6 | `reload_on_start` applies on every wake, and with an unknown slot it becomes a zero-delay click loop | High | PROD | High | P0 |
| PET-F7 | Row outcomes, veto notes and give-up reasons never reach the card or `pet.log` | Medium-High | PROD | High | P0 |
| PET-F8 | Any single effect error ends the resident run permanently | Medium | PROD | High | P1 |
| PET-F9 | Placement and drag retries share one counter | Medium | PROD | High | P1 |
| PET-F10 | Wait after the end toggle is shorter than v2's measured value | Medium | PERF-ACC | High | P2 |
| PET-F11 | Mid-run config writes are invisible to the live run and get clobbered by it | Medium | PROD | Medium | P1 |
| PET-F12 | Long waits are a sum of 50 ms relative sleeps: no deadline, no resume detection, ~246k wake-ups per cycle | Medium | SOTA/PERF | Medium | P2 |
| PET-F13 | Rate and pet-minutes accuracy | Medium | PERF-ACC | Medium/Low | P2 |
| PET-F14 | Config values are not bounded | Low | PROD | High | P2 |
| PET-F15 | Cancellation and window hygiene | Low | PROD | High (partly documented) | P2 |
| PET-F16 | Icon sweep allocations | Low | PERF | High | P2 |
| **TOOL** | **Other tools & lifecycle framework (seal-tools, tuner/quest/gem/shop/spammer)** | | | | |
| TOOL-F1 | The tuner run never reads `tuner.filter`; it reads a top-level `filter` | Critical | PROD | High | P0 |
| TOOL-F2 | The quest guard records the cursor before the positioning countdown | High | PROD | High | P0 |
| TOOL-F3 | Effects keep the `DisplayInfo` measured at run start; v2 re-measured on every placement | High | PROD/PERF-ACC | High | P0 |
| TOOL-F4 | A pet worker blocked in `GamePort::request` is never woken when the pet is stopped *(merged → CORE-F1)* | High | PROD/SOTA | High | — |
| TOOL-F5 | A worker panic leaks the slot and the game claim, and hangs shutdown *(merged → CORE-F3)* | High | PROD/SOTA | High | — |
| TOOL-F6 | The tuner makes `max_retries + 1` attempts (off by one compared with v2) | Medium | PROD | High | P0 |
| TOOL-F7 | An unconfirmed OCR read can trigger a destructive retry; the "forced capture" is thrown away | Medium | PROD | Medium | P0 |
| TOOL-F8 | The runner does not check for a stop between `decide()` and the effect | Medium | PROD/SOTA | High | P0 |
| TOOL-F9 | A stale deferred-start timeout cancels a later deferred start; a second deferral overwrites the first *(merged → CORE-F6)* | Medium | PROD | High | — |
| TOOL-F10 | Calibration pointer actions only check the input lease, never hold it; the 21-step cycle blocks the dispatcher with no cancel | Medium | PROD | Medium | P1 |
| TOOL-F11 | A buy preset's row is not checked against `shop_rows`; the buy count is unbounded and the whole plan is built up front | Medium | PROD | High | P1 |
| TOOL-F12 | Spammer cooldowns are not validated; a priority key below the tick period starves every other key | Medium | PROD | High | P1 |
| TOOL-F13 | The `HeldInput` RAII guard and `Cleanup` are not used in production; the ad-hoc guards break their own rule | Medium | SOTA/PROD | High | P1 |
| TOOL-F14 | The card can flip from "Stopping" back to "Running" | Low–Medium | PROD | High | P2 |
| TOOL-F15 | The gem empty threshold falls back to 0.18 where the spec says 0.01; one knob serves two metrics | Low–Medium | PERF-ACC | Medium | P2 |
| TOOL-F16 | Low-severity hygiene: unbounded waits and per-read encode overhead | Low | PERF-ACC | Medium | P2 |
| **HW** | **Board link, Windows layer, config store, firmware** | | | | |
| HW-F1 | `config.apply` (and the portable writer) cannot delete map entries or fields | High | PROD | High (code path), Medium (UX impact) | P0 |
| HW-F2 | The transaction can silently roll back a committed save, and its backups are not durable | High | PROD | Medium | P0 |
| HW-F3 | The migration skips the legacy fold and stamps `legacy_folded: true` | Medium | PROD | Medium-High | P1 |
| HW-F4 | The migration's "verify before retire" checks nothing | Medium | PROD | High | P1 |
| HW-F5 | No retry for Windows sharing violations; legacy writer and artifacts not fsynced | Medium | PROD/SOTA | Medium | P1 |
| HW-F6 | Firmware: `!Serial` runs first on every loop pass, adding about 10 ms per pass | Medium | PERF-ACC | Medium-High (check the ArduinoCore-avr `CDC.cpp` in use) | P2 |
| HW-F7 | Firmware failsafe misses a hung host; long commands cannot be aborted and block host writes | Medium | PROD/SOTA | High | P1 |
| HW-F8 | The board is reopened for every action, with a fixed 2 s boot wait each time | Medium | PERF-ACC | High | P2 |
| HW-F9 | Placement damping only ever grows, with no gain estimate | Medium | PERF-ACC | Medium | P2 |
| HW-F10 | Serial adapter: no write deadline, per-chunk read deadline, no input purge | Low-Medium | PROD | Low (depends on serialport 4.10.1 internals; check `windows/com.rs` `set_timeout` `WriteTotalTimeoutConstant`) | P2 |
| HW-F11 | Move counts are i32 on the host but `int` (16-bit) on the AVR; overlong lines become two commands | Low | PROD | High | P2 |
| HW-F12 | Capture pipeline: PNG encode/decode round trip, `CAPTUREBLT`, undefined alpha | Medium | PERF-ACC/SOTA | Medium | P2 |
| HW-F13 | GDI/FFI layer could be RAII-based; capture backend is legacy | Low | SOTA | High | P2 |
| HW-F14 | Legacy foreign-block parser silently drops non-identifier top-level keys | Low | PROD | High | P2 |
| HW-F15 | No quarantine for a corrupt file; one bad machine file blocks startup | Low-Medium | PROD | Medium | P2 |
| **SHELL** | **Flutter shell, packaging, CI & release** | | | | |
| SHELL-F1 | A closed client never kills its process, so Restart can leave an orphaned core with stdin still open | High | PROD | High | P0 |
| SHELL-F2 | CI does not build or test v3 at all | High | PROD/SOTA | High | P0 |
| SHELL-F3 | Multi-MB base64 frame decoded and PNG re-decoded on every build, including every drag pointer move | High | PERF-ACC | High | P1 |
| SHELL-F4 | The crash view can lose the core's fatal stderr reason and never shows the exit code | Medium | PERF-ACC/PROD | Medium | P1 |
| SHELL-F5 | Every event and log line rebuilds the entire shell, including all kept-alive tabs | Medium | PERF-ACC | Medium | P2 |
| SHELL-F6 | Closing the window silently discards dirty drafts and skips graceful shutdown | Medium | PROD | High | P1 |
| SHELL-F7 | Shell-side exceptions are not recorded anywhere | Medium | PROD | High | P1 |
| SHELL-F8 | Three hand-copied hide→capture→show sequences in `session_panel` lack `finally` and the one-frame wait | Medium | PROD/PERF-ACC | High | P1 |
| SHELL-F9 | The release script packages whatever is in the working tree: no gates, no lockfile pin, no checksum file, no signing | Medium | PROD/SOTA | High | P1 |
| SHELL-F10 | Versions have no single source of truth | Medium | PROD | High | P1 |
| SHELL-F11 | THIRD-PARTY-NOTICES does not satisfy MIT/Apache attribution for the bundled Rust crates | Medium | PROD | Medium (legal reading) | P1 |
| SHELL-F12 | Root `.gitignore` does not cover v3's portable player-state layout | Medium | PROD | High | P1 |
| SHELL-F13 | UI is English-only with about 456 hard-coded `Text(` strings and no l10n infrastructure | Low | SOTA | High | P2 |
| SHELL-F14 | Protocol types are hand-written on both sides; drift is caught only by fixtures | Low | SOTA | Medium | P2 |
| SHELL-F15 | Repo entry docs still describe v2 as current | Low | PROD | High (documented) | P2 |

---
*Implementation detail for every non-merged finding: [`REVIEW_IMPLEMENTATION.md`](REVIEW_IMPLEMENTATION.md).*
