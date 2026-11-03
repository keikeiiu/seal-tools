# seal-tools v3 — Review Implementation Handover

> Companion to [`REVIEW.md`](REVIEW.md). Target tree: `v3/` on `main` @ `7f5788e` (https://github.com/keikeiiu/seal-tools/tree/main/v3), v3.0.6 (code stamped 3.0.7).
> Every item below has: evidence (`path:line` at `7f5788e`), problem & concrete failure scenario, fix plan with code sketch, tests to add, acceptance criteria and effort (S ≤ ½ day, M ≤ 2 days, L > 2 days).
> Static review — nothing was compiled or run. Re-check each cited line before editing; numbers drift as soon as the first fix lands.

## How to use this document

1. **Work phase by phase.** Phase 0 items are correctness/safety blockers (player item loss, stuck input, stranded workers, lost saves). Phase 1 is production hardening. Phase 2 is performance, accuracy ceiling and SOTA upgrades.
2. **One finding → one PR** where possible; the IDs (`PET-F1`, `CORE-F4` …) are stable so commit messages and issues can reference them (`fix(pet): advance food cursor within a run (PET-F1)`).
3. **Duplicates are merged.** Where two slices reported the same defect, it is implemented once under the primary ID (Appendix B).
4. **Do the enabling work first** (§Prerequisites); several Phase 0 fixes are only verifiable with it.

## Prerequisites — test & tooling scaffolding (do first)

| # | Work item | Unblocks | Effort |
|---|---|---|---|
| T1 | **CI for v3** (= SHELL-F2): GitHub Actions `windows-latest` job running `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `flutter analyze`, `flutter test`, plus protocol fixture drift check; cache `~/.cargo` and pub; add `cargo-deny` (licences + advisories) and Dependabot for cargo/pub/actions. | Everything | M |
| T2 | **Fake-clock simulation harness** for resident tools: drive `PetTool` through `runner::run` with `FakeClock` (jumpable), scripted world model (tray drains at rate, pet finishes, slot reads, foreground flips), fake effects recording decisions. | PET-F1/F2/F5/F6/F12, TOOL-F2/F6/F8 | M |
| T3 | **Supervisor test kit**: `FakeRegistry` with tools that block in `GamePort::request`, panic in `decide`, or never finish; assertions "exactly one `tool.finished` per start", "shutdown returns within budget". | CORE-F1/F3/F6/F8 | S |
| T4 | **Real-merge config fixtures**: build test configs from `defaults.yaml` + overlay through `seal-config`'s real merge instead of hand-written JSON (the hand-written shape hid TOOL-F1). | TOOL-F1, CORE-F4, HW-F1 | S |
| T5 | **OCR eval corpus + `ocr_eval` example** (OCR-F11): ~50 recorded crops per read path with `.expected.json`; prints CER, exact-match, p50/p95 latency to a JSON baseline. | OCR-F1..F6, F12..F15 | M |
| T6 | **Proptest** dev-dependency for state-machine invariants (arbiter, spammer scheduler, pet veto/scheduler). | CORE/TOOL/PET invariants | S |

Suggested PR sequence: T1 → T4 → TOOL-F1, TOOL-F2, PET-F1 (tiny, high value) → T3 → CORE-F1, CORE-F3, CORE-F6 → T2 → remaining PET Phase 0 → CORE-F4 / HW-F1 / HW-F2 (config ownership, do together) → CORE-F2 → T5 → OCR-F2, F3, F4 → rest.

## Definition of done (every item)

- Regression test that fails on `7f5788e` and passes after the fix.
- `cargo clippy -D warnings` / `flutter analyze` clean; fixtures regenerated if wire shapes change, with `PROTOCOL.md` updated in the same PR.
- User-visible strings keep the v2-verbatim wording where the domain layer pins it.
- `docs/backlog/OPEN.md` updated when an item closes a documented limitation.

## Phase 0 — correctness & safety blockers (fix before publishing v3)

21 items.

### PET-F1 — The food-cell cursor never advances within a run

**Severity:** Critical · **Lens:** PROD · **Confidence:** High

**Evidence:** `pet.rs:1608` `visit.food_used = self.plan.food_slots_used;` (PickPage) and `pet.rs:1925` (Fold) both reset the cursor from the start-of-run snapshot (`pet.rs:419`). `self.plan.food_slots_used` is never assigned anywhere else. `FoodDone` increments only the visit-local counter (`pet.rs:1867-1871`). v2 mutated the live config instead (`PetTool.cs:3375` `cfg.FoodSlotsUsed++`).

**Impact:**
- Row 1 empties cells 0–1, then row 2 (and every later visit) right-clicks cells 0–1 again.
- In right-click mode there is no verification (`FoodDone` follows `Enter` directly, `pet.rs:1833-1835`), so the reload is reported as `Reloaded` with no food loaded. The pet starves while the card shows a full 205-minute cycle.
- The persisted `food_slots_used` is rewritten with the same small numbers, so the "No food cells left" stop never triggers.

**Fix:**
- Move `food_slots_used: i32` into the tool's mutable run state (for example `PetTool::food_used`). Initialise it from the plan, read it at `FoodPick`, increment it at `FoodDone` (or claim the cell before the load, as v2 did), and never reset it from the plan.
- Test: 2 rows × 2 stacks with right-click → the 4 `Place` decisions hit 4 distinct cells and the sink records 1, 2, 3, 4. Also test a second visit after a long `Wait`.

**Effort:** S

### PET-F2 — The timer is not a trigger: unknown readings never force a reload, and the veto path is dead

**Severity:** Critical · **Lens:** PROD/SOTA · **Confidence:** High

**Evidence:**
- `next_minutes` is never decremented. The only writes are at `pet.rs:1376,1393,1402,1422,1435,1487,1538`.
- `Phase::Wait` sleeps the soonest interval and goes back to `Open → Look` (`pet.rs:1994-2005`).
- In `look`, `was_due = next_minutes <= 0.0` (`pet.rs:1388-1390`) is therefore never true after a wait, so `consider_veto` never runs. A row becomes due only when a fresh read says `<= 0` (`pet.rs:1392`).
- The unknown-slot branch (`pet.rs:1407-1423`) never pushes the row into `due` and hard-codes `boarding_running: true`.
- A row without an empty reference always answers unknown (`pet.rs:755-757`).
- v2 compared absolute times (`PetTool.cs:549` `next[r] <= now`) and acted on any reading without food evidence (`PetTool.cs:1803-1806`).

**Impact:**
- When counts or time line are unreadable (`CycleFallback`), or the window failed to open (slot ≥ 0.85 → unknown), the row is rescheduled 205 minutes from "now" on every wake, indefinitely. The pet is never fed and there is no error.
- A row with no `pet_slot_empty_png`, which preflight explicitly says is "not a start gate" (`pet_session.rs:199-202`), is never reloaded. If it ever were, `VerifyPlaced`/`VerifyStarted` would fail it anyway (`pet.rs:1728,1884-1885`).

**Fix:**
- Keep absolute deadlines: `RowState { due_at: Option<Instant /*via injected clock*/>, … }`. In `look`, compute `timer_due = now >= due_at` and run `consider_veto(&fresh, …)` for every timer-due row; skip the reload only on `(true, _)`.
- For an unknown slot, keep the ticked belief and read feeders as v2 did (`PetTool.cs:1119-1154`), with `slot_occupied: false` so the veto can never skip.
- Make `pet_slot_empty_png` and `boarding_pet_slot` start gates in `ready()` and preflight, or define an explicit no-reference mode.
- Tests: scripted source whose counts are unreadable after the reload → the second wake reloads; unknown slot plus timer due → reload; positive count above 30 minutes → veto.

**Effort:** M

### TOOL-F1 — The tuner run never reads `tuner.filter`; it reads a top-level `filter`

**Severity:** Critical · **Lens:** PROD · **Confidence:** High

**Evidence:** `seal-core/src/tuner.rs:107` `let filter: FilterConfig = match config.get("filter").cloned() {`. Everywhere else the filter lives inside the tuner block:
- the config registry owns `"tuner.filter"` (`seal-config/src/registry.rs:136`);
- `defaults.yaml:109` puts `filter:` under `tuner:`;
- the calibration check reads `config.get("tuner").and_then(|t| t.get("filter"))` (`tuner_session.rs:206-212`);
- the shell's Tuner tab edits `filter` inside the tuner block (`tuner_tab.dart:56`).

The unit-test configs put `"filter"` at the top level (`tuner.rs:692`, `registry.rs:531`), which hides the bug.

**Problem & impact:** Every live run gets `FilterConfig::default()` (`enabled:false`), so `require_grade` and the attribute rules are ignored.
- Filter-only setups ("stop when the attributes match, at any grade") and setups where `require_grade` is below `target_grade` keep rolling past matching items until `target_grade`. That destroys exactly the roll the player wanted: a real in-game loss.
- With the shipped defaults, the run stops at the first DG with `REACHED (no filter)` whatever the attributes are.

**Fix:**
- Read `config.pointer("/tuner/filter")` in `tuner::plan`.
- Better: share one `fn filter_config(config) -> Result<FilterConfig, String>` between `tuner_session.rs` and `tuner.rs`. Refuse a malformed filter instead of falling back to the default.
- Change the test configs to the real merged shape.
- Add a test that loads `defaults.yaml` through the real merge and asserts `plan(..).settings.filter.enabled == true`.
- Acceptance: a run with a filter and no `require_grade` stops with `FILTER MATCHED at grade …`.

**Effort:** S

### CORE-F1 — Stopping a tool that is waiting for the game strands its worker forever

**Severity:** High · **Lens:** PROD · **Confidence:** High · **Also reported as:** TOOL-F4

**Evidence:** `supervisor.rs:340-351`: the `Stop` arm calls `arbiter.stop(tool)` and never touches `pending_reply`. `arbiter.rs:301-303` clears `self.waiting` when the stopped tool is the waiter. After that, `handle_game_release` (supervisor.rs:546) only answers the stashed reply on `GameRelease::Granted(waiter)`, which can no longer happen. The pet calls `self.game.request(ToolId::Pet)` inside `decide()` (pet.rs:1292).

**Scenario:** The tuner is running. The resident pet asks for the game and blocks in `answer.recv()`. The player clicks Stop on Pet. The card shows `stopping` for good, and any later `tool.start pet` is refused "still stopping". Every other tool's `request` is then refused "another tool is already waiting" (supervisor.rs:496) until the core restarts. The Preempt path (supervisor.rs:513-526) has the same gap.
**Fix:** In the `Stop` arm, after `Stop::Cancelling { tool, .. }`:
```rust
if pending_reply.as_ref().is_some_and(|(t, _)| *t == tool) {
    let (_, r) = pending_reply.take().unwrap();
    let _ = r.send(Err(STOPPED_MESSAGE.to_owned()));
}
```
Do the same in `stop_all` and in the Preempt stop. Test: a fake resident blocks in `request` behind a running foreground tool; `stop_tool(Pet)` must produce `tool.finished{reason:cancelled}` within 1 s, and the next pet start must succeed. **Effort S.**

### CORE-F2 — Live `state` events during a run are never sent

**Severity:** High · **Lens:** PROD · **Confidence:** High

**Evidence:** `runner.rs:325` publishes message, countdown and rows into `handle.state` (a `StateCell`, tool.rs:333) and nowhere else. The bus only receives `state_event` at state transitions (supervisor.rs:344, 363, 524, 598, 613, 630). PROTOCOL.md:274 says `state` is pushed "whenever the published state changes — the whole 750 ms-poll surface". `tests/supervisor.rs:313-316` asserts exactly `["starting","running","idle"]`, so the test locks the gap in. No command exists to read state.

**Impact:** Pet rows, countdowns and tool messages never update on the card during a run. Replay scripts hide this. OPEN.md §1 notes no card has been watched end-to-end yet.
**Fix:** Give `StateCell` an optional observer. Either `RunHandle` carries a `Sender<WorkerEvent::State(Arc<ToolSnapshot>)>`, or `spawn_worker` wraps the cell so that `publish` also sends. The supervisor emits the event only if `is_running && !is_stopping` (the same guard as `Opened`). Coalesce to at most one event per ~100 ms per tool. Update the supervisor test to expect the mid-run message. **Effort M.**

### CORE-F3 — Nothing contains panics: a worker panic leaks the slot and hangs shutdown

**Severity:** High · **Lens:** PROD/SOTA · **Confidence:** High · **Also reported as:** TOOL-F5

**Evidence:** There is no `catch_unwind` or `panic::set_hook` anywhere in `crates/*/src`. `worker_loop` (supervisor.rs:646-706) sends `Done` only on normal return. The shutdown loop `while !workers.is_empty() { receiver.recv() }` (supervisor.rs:366) never ends: the supervisor keeps its own `worker_sender` clone, so `recv()` never returns an error.

**Scenario:** A tool panics, say an index bug in OCR or a `Value` cast. The effects' `Drop` does run during unwinding, so held inputs are released. But the arbiter slot stays `running` forever (every start answers `tool_busy`), and EOF or `shutdown` hangs the process while it holds the COM port.
- If the supervisor thread itself panics, workers are never cancelled. `writer.finish()` (main.rs:141) then blocks on the `EventBus` clone held by the pet sink inside the registry `Arc`, so the process hangs with a tool still driving input.
- `stop_tool` reports `NotRunning` when the supervisor is dead (supervisor.rs:196-198), which is misleading.
- v2 wrote unhandled exceptions to `logs/error.log`. v3 only has the default stderr hook.

**Fix:**
1. Wrap the body of `worker_loop` (both `create` and `run`) in `std::panic::catch_unwind(AssertUnwindSafe(..))`. On `Err`, log `"{tool} panicked: {msg}"` to `error.log` and send `Done{reason: StopReason::error("internal error — see error.log")}`. Use a guard struct whose `Drop` sends `Done` if it wasn't sent, so the send survives even nested panics.
2. In `main`, install `panic::set_hook` that writes `logs/error.log` with `std::backtrace::Backtrace::force_capture()`, then chains to the default hook.
3. Wrap `handle()` in `catch_unwind` so a dispatcher panic answers `internal` instead of killing the core.
4. Have `stop_tool` return a distinct `Internal` when the supervisor is gone.
5. Add a test with a `FakeRegistry` tool that panics in `decide`: expect `tool.finished{error}`, the slot freed, and `shutdown` returning.

Never set `panic = "abort"` in a profile; it would silently disable every release guard. Document that next to the guards. **Effort M.**

### CORE-F4 — The dispatcher's config and revision drift from worker writes, and the stale copy overwrites the fresh one

**Severity:** High · **Lens:** PROD · **Confidence:** High · **Also reported as:** PET-F3

**Evidence:**
- Workers write pet run state, refresh the shared mirror and bump the revision: `pet_state.rs:81-88` (`*shared = config.clone(); self.revision.fetch_add(1)`).
- The dispatcher only re-reads the revision number (`dispatch.rs:523`), never the config, then republishes its own old `state.config` after every command (`dispatch.rs:610` → `617-621`).
- The registry builds the next run from that mirror (`registry.rs:131`), and the pet reads `food_slots_used` from it (`pet.rs:419`).
- `pet_calibration.rs:340-349` writes back `boarding_running` from the stale `state.config`.
- `calibration.rs:895` builds the new config from the stale copy.
- `bump_revision` (dispatch.rs:276-280) does `self.rev += 1; store(..)`. If a worker's `fetch_add` lands mid-command, two different writes get the same revision number.

**Scenario:** The feeder persists `food_slots_used=5`. The shell's connection card sends `board.details`, and the mirror goes back to 2. The player restarts Pet and it feeds from slots already used. This defeats the "run-state persistence" OPEN.md §5 lists as restored, within a single session.

**Fix:**
1. At the top of `handle`, if `revision.load() != state.rev`, refresh `state.config` from the store or the mirror.
2. Make `bump_revision` use `self.rev = self.revision.fetch_add(1, SeqCst) + 1`.
3. Only `publish_config` when this command changed the config (a dirty flag), and store an `Arc<Value>` stamped with its revision so an older revision can never overwrite a newer one.
4. Longer term, make the config one `ArcSwap<(u64, Arc<Value>)>` owned by a small `ConfigHub`.

Test: a sink write, then `board.details`, then `registry.config()` must contain the written field. **Effort M.**

### PET-F4 — No foreground or window gate before input, and capture refusals are treated as success

**Severity:** High · **Lens:** PROD · **Confidence:** High

**Evidence:**
- `PetSource::game_in_front` is defined (`pet.rs:624,779-781`) but has no caller in the loop.
- `Phase::Open` clicks the menu and feed icon blind (`pet.rs:1958-1965`). `BoardEffects::click` has no foreground check (`effects.rs:101-103`).
- `VerifyEmpty` turns `Err` (for example `NotForeground`) into "empty" and proceeds: `_ => visit.stage = Stage::PickPage` (`pet.rs:1601`).
- `ScanPage` silently skips a failed capture (`pet.rs:1625`).
- The right-click load sends `Decision::Enter` (`pet.rs:1833`).
- Look errors retry every 5 minutes and click the close button each time (`pet.rs:1432-1440`).
- `DisplayInfo` is measured once at run start (`registry.rs:317-318`) and reused for days.
- v2 deferred with no clicks before and during visits (`PetTool.cs:554-591,1786-1792`). This was the 2026-10-01 incident guard (57 of 90 strikes).

**Impact:** With another window in front, or the game closed or moved, the feeder clicks and right-clicks at stale screen coordinates and presses Enter into whatever app has focus. This repeats every 5 minutes, forever, with no strike.
**Fix:**
- Check `game_in_front()` (and that the window exists) in `Phase::Open` before `ensure_game`, and again at `Stage::Begin` of each row. If it fails: no decisions; rows go to `RETRY_MINUTES`; show `window_not_in_front_message` / `window_left_front_message`, which already exist and are unused.
- Treat `Err` from `slot_state` in the verify stages as "abandon visit" rather than "empty".
- Re-measure `DisplayInfo` at each visit start, for example with a `Decision::Remeasure` or a measuring `Effects::begin_visit`.
- Tests: `FakeSource::game_in_front = false` → zero Click/Enter decisions; front lost mid-visit → the remaining rows keep their schedule.

**Effort:** M

### PET-F5 — Infinite hover loop when the return-slot pet reads as finished

**Severity:** High · **Lens:** PROD · **Confidence:** High

**Evidence:** `ChooseCandidate` pushes the return-slot candidate whenever `candidates.is_empty()` (`pet.rs:1657-1673`). `HoverRead` with `finished` goes back to `ChooseCandidate` (`pet.rs:1709-1713`), which pushes the return slot again. Candidates are cleared only at PickPage and Fold.

**Impact:** A pet at +9/100% (or a misread panel) in the return slot, or any run where every queue match is refused, loops `Place` plus hover-wait forever. The visit never folds and the game claim is never released, so foreground tools are deferred indefinitely.
**Fix:**
- Add `return_slot_tried: bool` to `Visit` (or build the candidate list once, return slot last).
- When all candidates are exhausted, produce `RowOutcome::NothingToBoard` with `all_refused_message(&refused)` and collect refused labels as v2 did.
- Test: the hover says `+9 [100%]` → the row folds `NothingToBoard` within N decisions.

**Effort:** S

### PET-F6 — `reload_on_start` applies on every wake, and with an unknown slot it becomes a zero-delay click loop

**Severity:** High · **Lens:** PROD · **Confidence:** High

**Evidence:**
- `schedule_for` returns `StartOverride` with `next_minutes: 0.0` whenever `settings.reload_on_start` is set (`decision.rs:332-338`), and `schedule_settings()` passes the plan flag on every look (`pet.rs:1346-1351`).
- Unknown branch: `next_minutes = Some(0.0)` but the row is not added to `due` (`pet.rs:1411-1422`).
- `Phase::Wait` with `minutes <= 0.0` reopens immediately (`pet.rs:1995-1998`).

**Impact:**
- Every row is reloaded on every wake, burning food regardless of the tray. v2 has the same latent flaw (`PetTool.cs:1230`).
- In v3 specifically, one unknown row (no reference, or the window did not open) makes the tool click menu, feed and close roughly every 2.7 s, forever.

**Fix:**
- Make the override one-shot: `self.start_override_pending = plan.reload_on_start` and clear it after the first Look.
- Guard `Phase::Wait` with a minimum interval (for example `max(minutes, 1.0)` whenever no row was acted on in this pass).
- Proptest invariant: two consecutive `Open` phases are always separated by a `Wait ≥ 60 s` unless a reload happened in between.

**Effort:** S

### TOOL-F2 — The quest guard records the cursor before the positioning countdown

**Severity:** High · **Lens:** PROD · **Confidence:** High

**Evidence:** `seal-core/src/quest.rs:166` `phase: Phase::GuardReference`. At `:245-248`, `GuardReference => … self.start = Some(start); self.phase = Phase::Positioning(0);`. v2 does it the other way round: `QuestTool.cs:99` `if (!Position(…))`, then `:102` `TryReadGuardReference(…)`.

**Problem & impact:** The reference point is wherever the cursor was when Start was clicked, which is the shell window. The player then moves the mouse into place during the countdown, so the first step's `guard_problem` sees the move and stops the run with "The mouse moved". In normal use every quest run (and Test one loop) dies on step 1 unless `guard_px` is huge. Tests miss it because `ScriptedCursor` never moves during positioning (`quest.rs:498`).

**Fix:**
- Reorder the phases to `Positioning → GuardReference → Steps`.
- Test: cursor at (0,0) during positioning and (500,500) afterwards must click and not trip the guard.

**Effort:** S

### TOOL-F3 — Effects keep the `DisplayInfo` measured at run start; v2 re-measured on every placement

**Severity:** High · **Lens:** PROD/PERF-ACC · **Confidence:** High

**Evidence:** `effects.rs:33` holds `display: DisplayInfo`, used at `:73` `self.display.cursor_target(at.x, at.y).logical`. Nothing ever refreshes it. v2's `ShopTool.cs:323-330` `var display = HidPointer.Display(_cfg.Window.Title); if (display == null) { error = "Game window not found (or minimized)…"`. `geometry.rs:100-101` itself says "Measured per call, never cached".

**Problem & impact:** If the game window is moved, minimised or changes DPI mid-run, every closed-loop placement aims at the old screen position and then clicks. This affects Gem (hours long), Buy/Sell and the pet. On Sell, "somewhere unknown" can be an arbitrary stack, which v2 explicitly guarded against. The domain's `shop::window_missing()` refusal is never reached.

**Fix:**
- Add a `DisplayProvider` trait, injected into `BoardEffects`, with `fn current(&mut self) -> Result<DisplayInfo, EffectError>`. The production version calls `find_window` + `measure`; tests use a fixed version.
- Call it in `place_at`, and map a missing or minimised window to the domain's `window_missing()` line.
- Test: a fake provider that changes the origin between two placements must change the logical target.

**Effort:** M

### HW-F1 — `config.apply` (and the portable writer) cannot delete map entries or fields

**Severity:** High · **Lens:** PROD · **Confidence:** High (code path), Medium (UX impact)

**Evidence**
- `seal-config/src/store.rs:634` in `apply_sections` (and `:688` in `apply_sections_with_settings`): `Some(slot) => merge_overlay(slot, value)`.
- `store.rs:1298-1319` `merge_overlay`: objects merge, `null` is skipped.
- `store.rs:1276` `changed_fields` only walks `after`'s keys.
- `layout.rs:209-213` merges the fragment into the on-disk file again with another `merge_overlay`.
- `seal-core/src/validate.rs:104-110` builds the effective config the same field-wise way.
- The shell deletes by omitting the key: `shell/lib/ui/tabs/quest_tab.dart:224` `asJsonMap(value['presets']).remove(name);`.
- The spec contradicts itself: `PROTOCOL.md:239` says "owned blocks replace", `PROTOCOL.md:416` says "merges field-wise".

**Problem.** Deleting a Quest flow, sequence, spammer/buy-sell preset, or a field inside a flow sends the block without that key. Every merge layer keeps the old key, so the deletion never reaches disk. The core echoes the merged block with the flow still in it, and it reappears on reload. Renaming (`authoring::quest::rename_flow` = remove + insert) leaves a **duplicate** flow. Clearing a value with `null` is impossible too. No test covers deletion: `tests/new_layout_store.rs`, `layout_write.rs` and `seal-core/tests/config*.rs` have none.

**Fix.**
- Define deletion explicitly. Recommended: a registry of "record maps" (`quest.presets`, `quest.sequences`, `spammer.presets`, `buy_sell.presets`, `pet.queue`…) that are **replaced whole** when present in a section. Everything else stays field-merged.
- Alternative: adopt RFC 7386 JSON Merge Patch inside owned blocks (`null` = delete), with a protocol version bump since `null` currently means "not stated".
- Apply the same rule in three places: `validate::effective_config`, `ConfigStore::apply_sections*` and `layout::write_sections`. Put one shared `merge_with_policy(target, overlay, path)` in seal-config and delete the three copies of `merge_overlay`.
- Tests: delete a flow → `config.reload` → absent (legacy and portable layouts); rename a flow → exactly one key; partial `quest {guard_px}` save still keeps the presets.
- Fix PROTOCOL §4/§7 so they agree.

**Effort:** M.

### HW-F2 — The transaction can silently roll back a committed save, and its backups are not durable

**Severity:** High · **Lens:** PROD · **Confidence:** Medium

**Evidence**
- `transaction.rs:165`: `let _ = fs::remove_dir_all(&directory);` and then `Ok(())`.
- Every `layout::load` and every `commit` calls `recover`, which rolls back any directory that still has `journal.json` (`:190-231`).
- The backups are `fs::copy(&target, directory.join(&backup_name))` (`:101`) with no fsync.
- During recovery, a missing backup is skipped silently: `if let Some(backup) = backup && backup.is_file()` (`:223`).

**Problem.**
- On Windows, Defender, the search indexer or OneDrive often hold a just-created file open for a moment. If `remove_dir_all` fails, the core answers OK and bumps rev, but `journal.json` survives. The next load restores the `.bak` files and **reverts the Setup/Pet save the player saw succeed**.
- After a power loss during the commit window, the un-fsynced `.bak` can be zero-filled, and recovery then writes those zeros over the good target.
- A missing `.bak` leaves one file new and one rolled back, which is a half-applied save.

**Fix.**
- Make the commit marker removal explicit and checked. After the last rename, `fs::remove_file(journal.json)` (or rename it to `journal.done`), and propagate any error from that step. Only then clean up the rest best-effort, with failures logged.
- Have `recover` ignore directories without `journal.json` (it already does).
- fsync the backups: copy via `File::create` + `io::copy` + `sync_all`.
- Use `MoveFileExW(MOVEFILE_REPLACE_EXISTING|MOVEFILE_WRITE_THROUGH)` for the renames, or call `FlushFileBuffers` on the parent directory handle opened with `FILE_FLAG_BACKUP_SEMANTICS`.
- In recovery, a missing backup for `existed:true` should be an error that keeps the journal, not a skip.
- Tests: inject a cleanup failure through a filesystem seam and assert the next load keeps the new content. Truncate a `.bak` and assert recovery refuses it rather than restoring it.

**Effort:** M.

### SHELL-F1 — A closed client never kills its process, so Restart can leave an orphaned core with stdin still open

**Severity:** High · **Lens:** PROD · **Confidence:** High

**Evidence**
- `client.dart` `_markClosed` only cancels subscriptions: `unawaited(_linesSubscription.cancel()); unawaited(_diagnosticsSubscription.cancel());`. There is no `closeInput()` or `terminate()`.
- `close()` starts with `if (_closed) { return; }`.
- `shell_model.dart:434-447` `restart()`: `await previous.close();` then `connect()`.
- `framing.dart` `decodeLines` is an `async*` wrapper around a throwing `decodeLine`. Any `ProtocolDecodeException` ends the stream. The strict enum decoders in `types.dart:233,261,286,311,508,549` throw "unknown tool id/phase/stop reason/…", and `utf8.decoder` (with `allowMalformed` defaulting to false) rejects any invalid byte.
- `_onCoreOutputClosed` (`shell_model.dart:1049-1061`) then reports "the core exited".
- OPEN.md §2 (documented): "Single-instance cross-process lock … specified, not built".

**Problem and impact**
1. Something the shell cannot decode reaches stdout: a new phase string, a malformed field, or a stray non-UTF-8 byte.
2. The client marks itself closed and stops reading stdout and stderr, but the core is still alive with its stdin open.
3. The crash view says "the core exited", which is wrong.
4. The user clicks Restart. `previous.close()` is a no-op, and a second core spawns on the same root.
5. The first core keeps running whatever tool was active, including Arduino input, with nobody able to stop it. It eventually blocks writing to a stdout pipe nobody reads, possibly while holding keys down. Both cores can also interleave config writes.
6. The only way out is Task Manager.

Write errors on a dead core make this worse. `writeLine` uses `stdin.writeln`, whose failure only surfaces on the never-observed `stdin.done`, so the request waits out the full 10 s timeout.

**Fix**
- `client.dart`: `_markClosed` should call `unawaited(_transport.terminate())` unless it was reached through a clean `close()`. Alternatively, `close()`, when already closed, should still call `_transport.terminate()`.
- `process_transport.dart`: in `terminate()`, close stdin, wait for `exitCode.timeout(2s)`, then `kill()`. Listen to `_process.stdin.done.catchError(...)` and fail pending requests at once. Use `Utf8Decoder(allowMalformed: true)` for stderr.
- `framing.dart`/`client.dart`: make a single undecodable *event* non-fatal. Log it and emit `UnknownEvent`/`MalformedEvent`; keep a fatal close only for responses that cannot be correlated.
- Give the failure its own `ConnectionFailure.protocol` message (that enum value already exists) instead of `coreExit`.
- Tests: a `FakeTransport` that emits a garbage line should end with `terminated == true`, and `restart()` after that should terminate the old transport before spawning.
- Acceptance: no code path leaves a live process unreferenced.

**Effort:** M

### SHELL-F2 — CI does not build or test v3 at all

**Severity:** High · **Lens:** PROD/SOTA · **Confidence:** High

**Evidence**
- `.github/workflows/build.yml` runs only `dotnet restore/build/test v2/SealTools.sln`.
- The v3 gates exist only as hand-entered lines in `records/RELEASE-v3.0.6.md:68-74`: "264/264 tests… clippy -D warnings 0… Protocol fixtures: 81".
- `AGENTS.md` still says v2 is "the current build" and lists "the same three commands CI runs" (all .NET).

**Problem and impact**
- The main branch of the shipping product has no automated gate. Fixture drift between `seal-proto` and the Dart decoders, clippy regressions and broken Flutter tests are only caught if the owner remembers to run them before a release.
- There is no dependency auditing and no dependabot.

**Fix**
- Add `.github/workflows/v3.yml` on `windows-latest` with path filters `v3/**`:
  - Job `core`: `dtolnay/rust-toolchain` pinned by `rust-toolchain.toml`, `Swatinem/rust-cache`, then `cargo fmt --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, and `cargo deny check licenses advisories bans` (or `cargo audit`).
  - Job `shell`: `subosito/flutter-action` pinned to the version in `.metadata`, cache enabled, then `pwsh v3/shell/tool/check.ps1 -FlutterRoot $env:FLUTTER_ROOT`.
  - Job `publish-selftest`: `pwsh v3/publish.ps1 -Selftest`.
- Add `.github/dependabot.yml` for `cargo` (`/v3/core`), `pub` (`/v3/shell`) and `github-actions`. Pin actions by SHA.
- Update `AGENTS.md` with a v3 section.
- Acceptance: a PR touching `v3/` shows three green checks.

**Effort:** M

### PET-F7 — Row outcomes, veto notes and give-up reasons never reach the card or `pet.log`

**Severity:** Medium-High · **Lens:** PROD · **Confidence:** High

**Evidence:**
- `fold` sets `self.message = folded.message` (`pet.rs:1549`). Then, in the same `decide_once` loop and with no decision queued in between, the message is overwritten: by the schedule line for the last row (`pet.rs:1931-1936`), or by "reloading…" for the next row's `Begin` (`pet.rs:1578-1580`).
- The look's `unread` and veto notes are overwritten the same way (`pet.rs:1442-1451`).
- `LoggedTool` logs only the published status (`supervisor.rs:736-739`).
- `FoldResult::dropped_log` is never consumed.
- "Every row failed" ends as `StopReason::done` (`pet.rs:1982`), so the log says "finished".

**Impact:** "Row 1 failed 2/3: The pet did not go in…" and "given up on after 3 failures — <reason>" are invisible, and `pet.log`, v2's main diagnostic tool, loses the reload history.
**Fix:**
- Give `PetTool` a `Logs`-backed `note(&str)` sink, or a `Vec<String>` drained by `LoggedTool`. Write `folded.message`, `dropped_log`, the veto note, the slot difference and the raw time line there.
- Keep the outcome on the card by appending it to the schedule line or putting it in the row's `detail`.
- Use `StopReason::error` for every-row-failed.
- Test: after a failed reload, the recorded log lines contain "failed 1/3".

**Effort:** S-M

### CORE-F5 — `ocr.scan` grade colour crop passes the wrong image size

**Severity:** Medium · **Lens:** PERF-ACC · **Confidence:** High

**Evidence:** `dispatch.rs:405-410`:
```rust
let width = area.x2.saturating_sub(area.x1)... ; let height = ...;
seal_win::pure::crop_rgba(width, height, &rgba, rect)
```
These local `width`/`height` shadow the frame's dimensions. Any grade area not starting at (0,0) crops out of bounds, so the colour fallback (G25) silently never runs in `ocr.scan`. The live tuner does it correctly (`tuner.rs:544-547` uses `crop_width` and the frame's `width`). Calibrator test reads therefore disagree with real runs.

**Fix:** Rename to `crop_w`/`crop_h` and pass the frame's `width as u32, height as u32`. Better, move the grade crop into a shared helper used by both `tuner.rs` and `dispatch.rs`. Add a test with a synthetic frame whose grade area is offset. **Effort S.**

### TOOL-F6 — The tuner makes `max_retries + 1` attempts (off by one compared with v2)

**Severity:** Medium · **Lens:** PROD · **Confidence:** High

**Evidence:**
- The tool's countdown step calls only the tool-level `self.begin_attempt()` (`tuner.rs:351`); `self.run.begin_attempt()` runs only after a `Retry` (`:432`).
- So the first read is decided with `attempt == 0`, and the check is `self.attempt >= self.settings.max_retries` (domain `tuner.rs:286`).
- v2 increments before every click (`SealTuner.cs:121` `attempt++`, `:253`). The domain test calls `run.begin_attempt()` before its first decide (`tests/tuner.rs:351`), so it models v2.
- The card hides the gap with `.max(1)` (`tuner.rs:256-260`).

**Problem & impact:** The spring ceiling is exceeded by one, which consumes one extra in-game spring per capped run. Attempt numbers on the card are also wrong after the first retry.

**Fix:**
- Call `self.run.begin_attempt()` when the countdown ends (`Step::Countdown` → `begin_attempt`) and remove the `.max(1)` workarounds.
- Add a seal-core test: `max_retries: 2` must give exactly two `ClickHere` decisions.

**Effort:** S

### TOOL-F7 — An unconfirmed OCR read can trigger a destructive retry; the "forced capture" is thrown away

**Severity:** Medium · **Lens:** PROD · **Confidence:** Medium

**Evidence:**
- `tuner.rs:597` `last = Some(Ok(read));`: after `ocr_retries` unconfirmed reads, the last half-read is still returned and decided on.
- In the domain, `grade_ok` is false when `grade` is `None` (`tuner.rs:262-265`), so the decision is `Retry` and the next click re-rolls.
- `tuner.rs:593-596` `let _ = crate::win::capture(...)` discards the frame; nothing is written, although the comment says "leaves a frame behind" (v2 saved it: `OcrEngine.cs:237-245`).

**Problem & impact:**
- If the grade line or one attribute row is unreadable on the roll that actually hit the target or matched the filter, the tool clicks again and the result is lost.
- Retries re-capture immediately with no wait, so a transient animation is likely to be read the same way three times.
- The post-mortem frame never exists.

This is v2 parity for the decision, but Retry is the only irreversible action, so the safe choice is to stop.

**Fix:**
- Add `TunerRead::confirmed` (or return `Result<Confirmed, Unconfirmed>` from the source).
- In `TunerRun::decide`, when the read is unconfirmed and the decision would be `Retry`, stop with `StopReason::error("Couldn't confirm the read — stopped rather than roll again")`. Make this configurable if parity is wanted.
- Add a ~100–150 ms gap between OCR retries.
- Actually write the last capture through `Logs::capture`.
- Tests: an unconfirmed read with grade `None` must stop, not retry.

**Effort:** S–M

### TOOL-F8 — The runner does not check for a stop between `decide()` and the effect

**Severity:** Medium · **Lens:** PROD/SOTA · **Confidence:** High

**Evidence:** `runner.rs:313-334`: `control.check()` runs, then `tool.decide()` (the tuner does up to `ocr_retries` capture+OCR passes inside it), then the click, Enter or placement is performed with no second check.

**Problem & impact:** A Stop pressed during a slow `decide` (tuner OCR, a gem read) is followed by one more click, for example one more spring spent after the player pressed Stop. Stop latency is effectively the length of `decide` plus one effect.

**Fix:**
- After `let decision = tool.decide();`, add `if control.check().is_err() && !decision.is_terminal() { return StopReason::cancelled(STOPPED_MESSAGE); }`.
- Runner test: a tool whose `decide` cancels the token and returns `Click` must produce zero clicks.

**Effort:** S


## Phase 1 — production hardening

28 items.

### OCR-F1 — The crop uses the shrunk DB kernel, not the unclipped box

**Severity:** High · **Lens:** PERF-ACC · **Confidence:** Medium-High (documented as an "approximation")

**Evidence:** `seal-ocr/src/pipeline.rs:309-316`
```rust
let Some(_offset) = unclip_quad(&quad, UNCLIP_RATIO) else { continue; };
let mapped = Quad(quad.0.map(|point| Pt { x: (point.x * scale_x)...
```
The header (`pipeline.rs:14-16`) acknowledges the shortcut.

**Problem:** DB is trained to predict a shrunk text kernel; with shrink ratio 0.4 it is trimmed by about 0.42 × text height on each side. Unclip at 1.5 is what restores the glyph extent. Cropping the raw min-rect therefore cuts off ascenders, descenders and stroke ends. It also stretches a thin slice to 48 px with no margin, which the rec model never saw in training. The expected effect is more confusables (攻擊→攻孽/攻擎, 體→艘, 禦→票), which is exactly what the `text_fixes` table keeps patching. Separately, contour points are pixel indices, so the quad also under-covers by one det pixel on the right and bottom.

**Fix:**
- In `detect_and_read`, follow PaddleOCR `boxes_from_bitmap`:
  1. `mini = min_area_rect(contour)`; skip when `short_side(mini) < 3` (`min_size`).
  2. `box_score(...) < 0.6` → skip.
  3. `poly = unclip_quad(&mini, 1.5)?`.
  4. `quad = min_area_rect_f64(&poly)`; skip when `short_side < 5` (`min_size + 2`).
  5. Map to the original image, clamp, crop.
- Add `min_area_rect_f64(&[Pt])` (the hull already works on f64 internally).
- Tests: a synthetic score map with a 20×6 kernel should give a crop quad of about 20+2d × 6+2d, with d = area·1.5/perimeter.

**Acceptance:** on the recorded tuner crops, character accuracy does not regress and the `text_fixes` hit count drops.

**Effort:** S.

### OCR-F2 — `filled_mean` scans the whole score map for every contour, so latency is O(map × contours)

**Severity:** High · **Lens:** PERF · **Confidence:** High

**Evidence:** `seal-ocr/src/detgeom.rs:228-235`
```rust
for y in 0..map.height { for x in 0..map.width {
    if point_in_polygon(&coords, x as f64 + 0.5, y as f64 + 0.5) {
```
Every contour gets this, including holes and 1-pixel specks; there is no `max_candidates` (1000) cap and no `min_size` filter before scoring. On a 1920×1088 map with a few hundred contours, that is about 10⁹ point-in-polygon tests. This is consistent with the measured "~26 s per full-screen capture" (OCR-RESEARCH, spike results).

**Fix:**
- Restrict the scan to the quad's clamped integer bounding box, as reference `box_score_fast` does: `xmin = floor(min x).clamp(0, w-1)`, `xmax = ceil(max x).clamp(...)`, and the same for y.
- Apply `min_size` before scoring, and cap contours at 1000.
- Optionally rasterise with a scanline fill instead of per-pixel point-in-polygon.

**Acceptance:**
- An identical score on the existing `filled_mean` tests.
- A bench: a full-screen v2 capture under 1.5 s on CPU (det plus post-processing), and a tuner region under 150 ms.

**Effort:** S.

### OCR-F3 — Det resize is missing the max-side cap and C#'s vertical padding, so small or narrow crops become huge det tensors

**Severity:** High · **Lens:** PERF-ACC · **Confidence:** High

**Evidence:** `pipeline.rs:406-416` only enforces "short side → 736":
```rust
let ratio = if short < DET_LIMIT_SIDE { DET_LIMIT_SIDE / short } else { 1.0 };
```
OCR-RESEARCH §3.1 documents two more rules in the C# library: max side 2000 / min side 30, and black vertical padding when `h<=30` or `w/h>8`. Neither is implemented.

**Problem:** feeder count regions (`FeederLayout::read_region`, e.g. 15×40) become about 736×1984 det inputs. An eta strip of about 188×48 becomes 2880×736. A wide 400×30 strip would become about 9800×736, roughly 7 M pixels and about 90 MB of f32 input. Combined with F2 this is seconds per read. Extreme anisotropic upscaling also moves text far outside DB's training scale.

**Fix:**
- Port `ResizeImageWithinBounds`: reduce the max side to 2000, raise the min side to 30, and apply `ApplyVerticalPadding`.
- Keep the original→det scale per axis. Padding is top-anchored, so only the scale changes.
- Unit tests:
  - 15×40 gives ≤ 2000 on the long side.
  - 400×30 is padded before the resize.

**Effort:** S.

### SHELL-F3 — Multi-MB base64 frame decoded and PNG re-decoded on every build, including every drag pointer move

**Severity:** High · **Lens:** PERF-ACC · **Confidence:** High

**Evidence**
- `ui/tabs/calibration_widgets.dart:131-134`:
  ```dart
  Widget build(BuildContext context) { … final bytes = decodePngBase64(frame.data);
  ```
- The same State has `onPanUpdate: (details) => setState(() {` (line 247), and `_pictureArea` builds `Image.memory(bytes, … gaplessPlayback: true` (line 409).
- The same pattern appears in `calibration/capture_shelf.dart:192` (per shelf item, in `build`) and `calibration_widgets.dart:620`.
- The frames are full-client PNGs (PROTOCOL `capture`, `hover_capture` "captures the full client").

**Problem and impact**
- Each drag move base64-decodes megabytes on the UI isolate.
- It also creates a new `Uint8List`. `MemoryImage` equality is by bytes identity, so the image cache misses and the PNG is fully decoded again, about 8 MB RGBA at 1080p per decode.
- `gaplessPlayback` hides the flicker, not the cost. Results: jank while dragging calibration boxes, ImageCache churn up to its 100 MB cap, and cache eviction of other images. The shelf multiplies the cost by the number of captures.
- The whole response, including the base64 string, is also `jsonDecode`d on the UI isolate (`process_transport.dart` → `client.dart`).

**Fix**
- Decode once when the frame arrives. Store `Uint8List` (or a `MemoryImage`/`ui.Image`) on the capture model, e.g. `CalibrationCapture.bytes` as a `late final`, and key widgets on `frame_id`.
- For shelf thumbnails use `ResizeImage(MemoryImage(bytes), width: 160)`.
- Optionally move `jsonDecode` and `base64Decode` of responses larger than ~256 KB to `Isolate.run`.
- Test: a widget test that pumps 50 pan updates and asserts the decode counter equals 1 (inject the decoder).

**Effort:** S

### OCR-F11 — No text-level regression corpus; the end-to-end test only checks that something was read

**Severity:** Medium · **Lens:** PROD · **Confidence:** High

**Evidence:** `seal-core/tests/ocr_captures.rs:58-62` asserts `files > 0` and `non_empty > 0`. The raw-crop recording pass is documented as missing (OPEN.md §1, OCR-RESEARCH §5).

**Problem:** F1, F4 and F5, and any model upgrade (F13), cannot be measured or protected against regression.

**Fix:**
- Add `v3/core/fixtures/ocr/<path>/<name>.png` and `.expected.json` (`lines`, `grade`, `remaining`, `attributes`). Gate them on `SEAL_OCR_MODELS` like today.
- Add a `seal-ocr` example binary, `ocr_eval`, that prints CER, exact-match rate per read path, and p50/p95 latency, written to a JSON baseline. CI-on-demand fails on a regression larger than ε.
- Record about 50 crops per read path: tuner region, feeder count, eta, tooltip panel, grade box.

**Effort:** M.

### CORE-F6 — A timeout left over from an earlier deferred start cancels a later one early

**Severity:** Medium · **Lens:** PROD · **Confidence:** High · **Also reported as:** TOOL-F9

**Evidence:** `schedule_pending_timeout` (supervisor.rs:566-572) sends a bare `Command::PendingTimeout`. `arbiter.pending_timeout()` (arbiter.rs:402-403) `take()`s whatever start is pending at that moment.

**Scenario:** Quest is deferred at t=0 and starts at t=10. Buy is deferred at t=40. The quest's timer fires at t=45 and refuses Buy after 5 s instead of 45 s.
**Fix:** Keep a `pending_generation: u64` in `supervise`, use `PendingTimeout { generation }`, and ignore a mismatch. Better still, drop the per-timer thread and use a `recv_timeout` deadline in the supervisor loop. Test with an injectable timeout duration. **Effort S.**

### CORE-F8 — Shutdown has no deadline

**Severity:** Medium · **Lens:** PROD · **Confidence:** Medium

**Evidence:** `supervisor.rs:366-409` waits for workers with no time limit, and `main.rs:133` waits on that. The shell's `terminate()` only runs on a shutdown it started itself. If the shell crashes, the only signal is EOF.

**Impact:** A worker stuck in a blocking call (serial, OCR, Win32) leaves an orphaned core holding the COM port, and the next launch cannot open the board.
**Fix:** Drain with `receiver.recv_timeout(deadline - now)` and a budget of about 5 s. On timeout, log the tools that are still running to `error.log`, emit `core.stopping`, and return. `main` then exits with a non-zero code. Optionally put the core in a Job Object (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`) created by the shell. **Effort S/M.**

### CORE-F9 — stdin framing is lenient in the wrong places

**Severity:** Medium · **Lens:** PROD · **Confidence:** High

**Evidence:** `main.rs:105-106`: `Ok(0) | Err(_) => break "eof"`. One invalid UTF-8 byte (`InvalidData`) shuts the core down. `read_line` has no size limit. Unparseable lines are answered with `Response::err("", ..)` (main.rs:115-122), although PROTOCOL.md §2 says "`id` always echoed". A request with a numeric `id` or a missing `v` therefore times out in the shell after 10 s instead of failing at once. Requests whose `v` is not `PROTOCOL_VERSION` are never refused.

**Fix:** Read with `read_until(b'\n')` into a `Vec<u8>` through `.take(MAX_LINE)` (e.g. 64 MiB, because frames can be inline). Answer an over-long line or invalid UTF-8 with `bad_request`. Decode in two steps: first `serde_json::Value`, pull out `id` if it is a string or number, then the typed `Request`. Check `request.v` in `handle`. Add tests that send an invalid byte, a missing `v` and a numeric id to a spawned binary. **Effort S.**

### CORE-F12 — Logs grow without limit and every line reopens its file under one global lock

**Severity:** Medium · **Lens:** PROD/PERF · **Confidence:** High

**Evidence:** `logs.rs:20-21` ("appended and never truncated"). Every line does `create_dir_all` plus open, write and close while holding the single `write_lock` shared by all files (logs.rs:298-325). `ocr_log.jsonl` gets one record per scan. `<tool>.log` gets one line per changed status message (supervisor.rs:738-741), so a message containing a countdown means one line per tick. There is a cleanup command for `logs/captures` but not for `logs/reads`.

**Impact:** Long tuner or pet sessions produce hundreds of MB of logs. With Defender on Windows, each open/close costs about 0.1–1 ms and serializes the workers.
**Fix:**
- Size-capped rotation (e.g. 10 MB × 3 per file: rename to `.1` when over the cap, checked every N writes).
- Keep one `BufWriter<File>` open per file inside a `Mutex<HashMap<..>>`, flushed per line, so the "durable per line" promise still holds.
- Have `logs.cleanup_captures` also cover `reads/`, or add an age limit.
- Strip numbers from status text before comparing it for `LoggedTool`.

**Effort M.**

### PET-F8 — Any single effect error ends the resident run permanently

**Severity:** Medium · **Lens:** PROD · **Confidence:** High

**Evidence:** `runner.rs:335-399`: every effect failure returns `StopReason::error`. Placement failures come from `effects.rs:80-83`.

**Impact:**
- A cursor nudged by the player, a COM hiccup after resume, or a USB re-enumeration stops a multi-day feeder.
- A placement failure between `HoldLeft` and `ReleaseLeft` releases the stack wherever the cursor is, via the `Drop` guard.

**Fix:**
- Add a per-tool effect-failure policy, for example `Tool::on_effect_error(&EffectError) -> Recovery { Stop | Retry{after_s} }`. The pet maps errors to a row strike with `RETRY_MINUTES`, and reopens the transport on write errors.
- Keep the drag hold/place/release as one effect (`Effects::drag(from, to)`) so a failed second placement can release over the source cell.

**Effort:** M

### PET-F9 — Placement and drag retries share one counter

**Severity:** Medium · **Lens:** PROD · **Confidence:** High

**Evidence:** `visit.attempts` is incremented in `ClickCandidate` (`pet.rs:1725`), reset only at PickPage and Fold (`pet.rs:1606,1923`), and reused by `FoodDragCheck` (`pet.rs:1851-1853`).

**Impact:** A pet placed on the 3rd try leaves zero drag retries. Retries are also shared across stacks.
**Fix:** Add a separate `drag_attempts` field, reset per stack in `FoodPick`. Test: placement succeeds on attempt 3 and the first drag "doesn't move" → one retry still happens.

**Effort:** S

### PET-F11 — Mid-run config writes are invisible to the live run and get clobbered by it

**Severity:** Medium · **Lens:** PROD · **Confidence:** Medium

**Evidence:**
- `PetPlan` is a snapshot (`pet.rs:392-440`).
- `pet.set_boarding` and `pet.apply_food_scan` write while a run is live (`dispatch.rs:1182-1205,1849-1886`).
- The run's next `fold` persists its own `boarding` (`pet.rs:1544-1548`), and `FoodDone` persists its own counter.
- The pet scan and hover commands use a snapshot lease (`dispatch.rs:1524-1539`; `supervisor.rs:203-205` "SNAPSHOT, not a held lease"), so a feeder wake can interleave with a roughly 45-second hover sweep.

**Fix:**
- Either refuse these commands while the pet run is armed (`tool_busy`), or route them to the worker through a command channel so the run applies them to its own state.
- Make scan commands take a real lease that `GamePort::request` honours.

**Effort:** M

### TOOL-F10 — Calibration pointer actions only check the input lease, never hold it; the 21-step cycle blocks the dispatcher with no cancel

**Severity:** Medium · **Lens:** PROD · **Confidence:** Medium

**Evidence:**
- `tooltip_session.rs:59-73` `check_input_lease` only asks whether a tool holds the game.
- `gem_routes_session.rs:358-395` then runs 21 real clicks with `thread::sleep(500/800ms)` on the request thread.

**Problem & impact:** The check passes while the pet is armed but not holding the game. Mid-cycle the pet calls `request_game`, the arbiter grants it, and two drivers move the same mouse with interleaved clicks. About 12 s of dispatcher blocking also delays `tool.stop` and every other request, if dispatch is single-threaded (verify).

**Fix:**
- Add `Arbiter::lease(Calibration)`, a pseudo-holder of the game, with an RAII `LeaseGuard` that releases through the supervisor on drop.
- Run long pointer sequences on a worker thread with a `CancelToken`, and answer progress through events.

**Effort:** M

### TOOL-F11 — A buy preset's row is not checked against `shop_rows`; the buy count is unbounded and the whole plan is built up front

**Severity:** Medium · **Lens:** PROD · **Confidence:** High

**Evidence:**
- Domain `shop.rs:251-264`: `row_centre` rejects only `rows < 1 || index < 0`, and says "that is the caller's to report". `ShopPlan::buy` (`:375-380`) never checks `preset.row < shop_rows`.
- Core `shop.rs:132` refuses only `count < 1`, and `for _ in 0..count { steps.extend(transaction(..)) }` builds 12 steps per count eagerly.
- The parity audit's B9 item (documented) says v2 replaced the free row field with a labelled picker after an off-by-one bought the wrong item.

**Problem & impact:**
- `row: 10` on a 10-row list right-clicks half a pitch below the list, which can be a wrong purchase.
- There is no spend cap analogous to `sell_cap`.
- A huge count allocates `12·count` steps; `i32::MAX` aborts the process.
- One malformed preset makes `from_value::<BTreeMap<..>>().ok()` wipe all presets, and the player sees the misleading "Buy preset 'x' no longer exists" (`core shop.rs:64-68`).

**Fix:**
- Refuse `row >= shop_rows` with a line naming the row.
- Add `buy_cap` (or a fixed maximum).
- Generate transactions lazily: keep `remaining: i32` in `ShopTool`.
- Parse presets individually and report the bad one.
- Tests at the boundaries: `row = rows-1` passes, `row = rows` is refused.

**Effort:** S

### TOOL-F12 — Spammer cooldowns are not validated; a priority key below the tick period starves every other key

**Severity:** Medium · **Lens:** PROD · **Confidence:** High

**Evidence:**
- `core spammer.rs:76` `cooldown.as_f64().unwrap_or(0.0)`.
- No cooldown validation anywhere (grep finds none in `validate.rs`).
- Domain `spammer.rs:357-366`: with `preempt`, the first due key `break`s the walk every 20 ms tick.

**Problem & impact:**
- A cooldown typed as a string, or 0 or a negative number, becomes "due every tick", which is about 50 presses/s.
- With a priority order, the lower keys never fire (silent starvation).
- Without priority, keys due on the same tick are sent back-to-back with no gap (`SpammerTool::decide` drains its queue). The module itself says the game swallows all but the first, yet each swallowed key's cooldown is restarted. This last point is v2 parity.

**Fix:**
- Validate at start: a cooldown must be a finite number ≥ the tick period, otherwise refuse with a named line.
- Optionally insert `Wait(startup.step)` between same-tick presses.
- Add proptest properties over random rotations: (a) every key with cooldown > 0 fires at least once in `max(cooldown)·N` seconds of simulated ticks; (b) no key fires before its cooldown.

**Effort:** S–M

### TOOL-F13 — The `HeldInput` RAII guard and `Cleanup` are not used in production; the ad-hoc guards break their own rule

**Severity:** Medium · **Lens:** SOTA/PROD · **Confidence:** High

**Evidence:**
- Grep finds `HeldInput`/`without_cancellation` only inside `seal-tools`.
- `holdspace.rs:110-113`: `self.write(HoldSpaceAction::Hold)?; self.held = true;`
- `effects.rs:141-144`: `hold_left` sets `held_left` only after a successful write.
- `held.rs:150-163` says intent must be recorded *before* the write, so that a failed press is still released.

**Problem & impact:**
- A write that errors after the byte reached the board (timeout or partial write) leaves the spacebar or left button held, with no release on `Drop`; only the firmware fallback remains.
- Two hand-rolled guards duplicate a tested one, and that drift is the class of v2 bug `held.rs` was written to prevent.
- `domain::hold_space::tick`, `SpammerRun::resume` and `GamePort::should_stand_down` are also unwired. The last two are a documented follow-up.

**Fix:**
- Implement `BoardInput` for the board transport and have both effects own a `HeldInput`.
- Or at minimum set `held = true` before the write.
- Delete or `#[cfg(test)]`-gate the unwired APIs.
- Test: a press whose transport returns `Err` must still send `U` on drop.

**Effort:** S–M

### HW-F3 — The migration skips the legacy fold and stamps `legacy_folded: true`

**Severity:** Medium · **Lens:** PROD · **Confidence:** Medium-High

**Evidence**
- `store.rs:127-131`: on a shipped install, `migrate` runs **before** the legacy path that reads `defaults.yaml` (`:178`) and folds it.
- `migrate.rs:216-286` reads only `local.yaml`.
- `migrate.rs:158-163` `meta(folded)` is always called with `meta(true)` (`:286`).
- `tests/migration.rs:61` only covers a `local.yaml` that already has `legacy_folded: true`.

**Problem.** A player who upgrades from a pre-fold install still has their saved settings (hotkeys, tuner behaviour, presets) in the old `defaults.yaml`. After migration the new layout reads only the template, so those values silently vanish. They are still on disk, but never read again. `adopt_spammer_presets` is skipped as well.

**Fix.**
- In `ConfigStore::load`, run the legacy load (fold plus preset adoption) first, then migrate. Or call `fold_into_local` inside `migrate` when `!is_folded(local)` and a legacy `defaults.yaml` exists.
- Write `meta(is_folded_after)` rather than a hard-coded `true`.
- Test: a shipped tree with an unfolded `local.yaml` and a legacy `defaults.yaml` holding `hotkeys.start: 99` should give `hotkeys.start == 99` after migration.

**Effort:** S-M.

### HW-F4 — The migration's "verify before retire" checks nothing

**Severity:** Medium · **Lens:** PROD · **Confidence:** High

**Evidence**
- `migrate.rs:272-279`: `let loaded = layout::load(root, base)?; if loaded.config.is_null() {…}`.
- `layout::load` always starts from `base.clone()` and turns a non-object into `{}` (`layout.rs:49-52`), so the result can never be null.
- The doc at `:208` claims it verifies that the new tree "loads exactly as the old one did".

**Problem.** A routing bug that drops a block (a registry gap, or an owner mismatch between `settings.X` and `X`) passes the check. The code then deletes `local.yaml` (`:282`). The backup exists, but the player is not told.

**Fix.**
- Before writing, compute `old = legacy_merged_config(root, base)` (factor the legacy branch of `ConfigStore::load` into a pure function).
- After writing, compute `new = layout::load(...).config`, and only retire the old file if `old == new` modulo documented exclusions such as crop strings → `artifacts/…`. On a mismatch, roll back the written files from the backup and return an error that names the differing JSON pointers.
- Test: drop a registry owner in a test build and expect a migration refusal.

**Effort:** M.

### HW-F5 — No retry for Windows sharing violations; legacy writer and artifacts not fsynced

**Severity:** Medium · **Lens:** PROD/SOTA · **Confidence:** Medium

**Evidence**
- `file.rs:155` and `transaction.rs:152` use a bare `fs::rename`.
- `store.rs:930-932` `write_atomically`: `fs::write(&temp, text).and_then(|()| fs::rename(&temp, path))` with no fsync.
- `write_artifact` (`store.rs:~595`) also has no fsync. The durability gap is already documented in `ANALYSIS-CONFIG-STORE.md §7.1`.

**Problem.**
- If the destination is open without `FILE_SHARE_DELETE` (an editor, OneDrive sync, some AV), the rename fails with `ERROR_ACCESS_DENIED`/`ERROR_SHARING_VIOLATION` and the save is lost on a transient lock.
- A temp file left by a crash (`*.<pid><nanos>.tmp`) is never swept, and `unknown_files` ignores it.
- The calibration PNGs and dev-tree saves are not crash-durable.

**Fix.**
- One `atomic_replace(path, bytes)` in `file.rs` used by every writer (store, artifacts, transaction): create temp → `sync_all` → `MoveFileExW(…, MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)`, or `ReplaceFileW` when the destination exists.
- Retry about 5 times with 20→320 ms backoff on raw OS errors 5/32/33.
- Sweep `*.tmp` older than one hour in `setup/`, `machine/` and `artifacts/` at load.
- Test with a fake renamer that fails twice and then succeeds.

**Effort:** S.

### HW-F7 — Firmware failsafe misses a hung host; long commands cannot be aborted and block host writes

**Severity:** Medium · **Lens:** PROD/SOTA · **Confidence:** High

**Evidence**
- The failsafe only fires on `!Serial` (`ino:153`), which follows DTR/line state.
- `Q/Z` loop up to 400×25 ms ≈ 10 s (`:259-267`).
- `D` walks the move with `delay(1)` per 10 counts (`:185-192`).
- There is no ack. The comment at `:89` says "The app holds the port open for its whole lifetime", but v3 reopens per action (F8).

**Problem.**
- If the core deadlocks or freezes with the port still open while `P`/`L` is held, space or the left button stays down indefinitely.
- While a 10 s wheel frame runs, Stop cannot preempt it. Later writes pile up in the CDC endpoint, and the host's `write_all` can block (see F10).

**Fix (protocol level 3, backward-compatible).**
- **Keepalive watchdog:** `static uint32_t lastRx; if ((spaceHeld||leftHeld) && millis()-lastRx > 3000) releaseAll();`. The host sends a no-op such as `N\n` every second while holding (seal-tools `held.rs`). Older boards ignore unknown letters, so this is safe to send.
- **Abort:** inside the `D`/`Q`/`Z` loops, `if (Serial.peek()=='!') { Serial.read(); break; }`.
- **Optional ack mode:** `A 1` enables `"K <seq>"` after each completed frame, so the host knows when a blocking command has finished.
- Bump `FW_VERSION` to 3. Update `firmware.rs` `CURRENT_LEVEL` and `tests/sketch.rs`.

**Effort:** M.

### SHELL-F4 — The crash view can lose the core's fatal stderr reason and never shows the exit code

**Severity:** Medium · **Lens:** PERF-ACC/PROD · **Confidence:** Medium

**Evidence**
- `_markClosed` cancels `_diagnosticsSubscription` as soon as stdout ends (`_onStreamDone`).
- `CoreProcessTransport.exitCode` exists but `grep exitCode v3/shell/lib` finds no consumer outside the transport.
- PROTOCOL §1: a fatal startup error "exits non-zero with the reason on stderr and nothing on stdout". The (designed) lock refusal is "exit code 2 and one stderr line; the shell shows it".
- `_onCoreOutputClosed` builds its message from `tail.last` at that moment.

**Problem and impact**
- stdout EOF and the final stderr chunk are separate pipes read concurrently. If stdout's `done` is processed first, stderr is cancelled before the reason line arrives. The user then sees "the core exited" with an empty tail for exactly the most important case: bad config or a dictionary that will not load.
- If the core dies during the handshake, `_fail` runs twice (once from `_onCoreOutputClosed`, once from the handshake `catch`), so `crashCount` is doubled.

**Fix**
- `Transport` should expose `Future<int> exited`.
- On stdout `done`, wait for `Future.wait([exitCode, stderrDone]).timeout(1s)` before `_markClosed`.
- Include `exit $code` in `ConnectionModel.markFailed`.
- Guard `_fail` with a per-client "already failed" flag.
- Test: a fake transport where stdout closes before stderr emits its line; assert the tail contains it.

**Effort:** S–M

### SHELL-F6 — Closing the window silently discards dirty drafts and skips graceful shutdown

**Severity:** Medium · **Lens:** PROD · **Confidence:** High

**Evidence**
- `ShellModel.shutdown()` exists (`shell_model.dart:450-466`), but nothing in `lib/` calls it.
- `main.dart` has no `AppLifecycleListener` and no `onExitRequested`.
- The runner `win32_window.cpp:201-205` handles `WM_DESTROY` with `PostQuitMessage(0)`.
- `ConfigDraft.isDirty` exists (`config_draft.dart`), but no close guard reads it.

**Problem and impact**
- A player edits a spammer preset or a pet queue and closes the window. The edits are gone without a prompt.
- The core only stops through implicit stdin EOF. This is acceptable per PROTOCOL §1, but the explicit `shutdown` path that exists in code is never exercised in production.

**Fix**
- In `ShellApp`, add `AppLifecycleListener(onExitRequested: () async { if (model.hasDirtyDrafts) { …show dialog… return AppExitResponse.cancel; } await model.shutdown(); return AppExitResponse.exit; })`.
- `ShellModel.hasDirtyDrafts` aggregates the tab draft groups.
- Test: a widget test with a dirty draft where the exit request returns cancel.

**Effort:** S

### SHELL-F7 — Shell-side exceptions are not recorded anywhere

**Severity:** Medium · **Lens:** PROD · **Confidence:** High

**Evidence**
- A search for `FlutterError.onError|PlatformDispatcher|runZonedGuarded|openWrite|writeAsString` in `v3/shell/lib` returns nothing.
- The release build is a GUI subsystem with no console.

**Problem and impact**
- A widget exception, a `TypeError` from a lenient `asString` decoder, or an unhandled `stdin.done` error (see F1) is lost.
- Field bug reports from players carry only the in-memory 200-line log, and only if they screenshot it.

**Fix**
- In `main.dart`, set `FlutterError.onError` and `PlatformDispatcher.instance.onError`, and append to `<root>/logs/shell-YYYYMMDD.log` (rotate at 1 MB, keep 5).
- Mirror shell notes into the same file.
- Add a "Copy diagnostics" button to the crash view (shell and core version, exit code, stderr tail, last 200 log lines).

**Effort:** S

### SHELL-F8 — Three hand-copied hide→capture→show sequences in `session_panel` lack `finally` and the one-frame wait

**Severity:** Medium · **Lens:** PROD/PERF-ACC · **Confidence:** High

**Evidence**
- `ui/tabs/calibration/session_panel.dart:155-161`, `:245-251` and `:478-484`:
  ```dart
  try { await widget.host.hideWindow(); } catch (_) {}
  final result = await widget.model.captureFrame();
  try { await widget.host.showWindow(); } catch (_) {}
  ```
- Compare `calibrate_tab.dart:308-329` and `pet_tab.dart:628-645`, which use `try/finally` plus `await _waitOneFrame()` with the comment "One frame for the hide to be composited before the core checks the foreground window".

**Problem and impact**
- `_coreAction` catches the protocol exceptions but not, for example, a `TypeError` from a decoder. Any throw leaves the app window hidden with no tray icon, so the user must kill it.
- Without the frame wait, the core can sample the foreground before DWM removes the shell, giving intermittent `game_not_foreground` or the shell appearing in the capture.

**Fix**
- Add one `Future<T> withHiddenWindow<T>(HostActions host, Future<T> Function() body)` in `app/host_actions.dart` (hide, wait for the end of frame, `try body finally show`), and use it at all five sites.
- Test with `FakeHost`: when the body throws, `showWindow` is still called.

**Effort:** S

### SHELL-F9 — The release script packages whatever is in the working tree: no gates, no lockfile pin, no checksum file, no signing

**Severity:** Medium · **Lens:** PROD/SOTA · **Confidence:** High

**Evidence**
- `publish.ps1:588` runs `& cargo build --release -p seal-core` without `--locked`. Line 597 runs `& flutter build windows --release`.
- `Write-Environment` (`:422-437`) prints `git status --short` but does not fail on a dirty tree.
- `Write-Hash` (`:707-713`) only uses `Write-Host` for the SHA-256. Nothing is written to `dist/`.
- No `signtool` or Authenticode step anywhere.
- No `cargo test`/`flutter test`/fixture check before `-Release`.

**Problem and impact**
- An uncommitted experiment, or a Cargo.lock that drifted silently, can ship.
- The hash in `CUTOVER.md` is copied by hand from console output.
- The unsigned `SealTools.exe` plus `seal-core.exe`, a tool that synthesises input and drives a COM device, will trigger SmartScreen and antivirus heuristics. Players have no way to verify provenance.

**Fix**
- `-Release` should:
  - refuse a dirty tree unless `-AllowDirty` is passed;
  - run `cargo test --locked` and `tool/check.ps1`, or require a green CI run for `HEAD`;
  - build with `--locked`;
  - write `dist/SHA256SUMS.txt` plus a `build-info.json` (commit, toolchain versions, timestamp) into the stage, and add that file to the allow-list;
  - optionally `signtool sign /fd sha256 /tr <tsa>` when a cert env var is present.
- Longer term: a tag-triggered GitHub workflow that builds, attests (`actions/attest-build-provenance`, SLSA L2) and uploads assets with checksums.
- Extend `-Selftest` with checks for the dirty-tree refusal and checksum-file presence.

**Effort:** M

### SHELL-F10 — Versions have no single source of truth

**Severity:** Medium · **Lens:** PROD · **Confidence:** High

**Evidence**
- `lib/app/version.dart`: `productVersion = '3.0.7'`. `pubspec.yaml:19`: `version: 3.0.7+1`. These are two hand-edited copies.
- `publish.ps1:31`: `[string]$Version = "v3.0"`. It is checked against a regex only, never against pubspec, so the default would produce `SealTools-v3.0.zip` holding a 3.0.7 binary.
- `v3/core/Cargo.toml` `[workspace.package] version = "0.1.0"`, which is what `hello.coreVersion` reports. The shell logs "core 0.1.0 connected" for every release.
- The latest record is `RELEASE-v3.0.6.md` while the code already says 3.0.7, with no "unreleased" marker.

**Problem and impact**
- Mislabelled zips are possible, and bug reports cannot tell core builds apart.
- The "About" version in `shell_page.dart:393` can disagree with the zip name.

**Fix**
- Make pubspec `version` canonical. Generate `version.dart` from it (`--dart-define=PRODUCT_VERSION` set by publish, or a small `tool/gen_version.dart`), and set the Cargo workspace version to the same `3.0.x`.
- `publish.ps1` should derive `$Version` from pubspec, or fail if `-Version` disagrees with pubspec, `Cargo.toml` or `version.dart`. Add a selftest case for this.

**Effort:** S

### SHELL-F11 — THIRD-PARTY-NOTICES does not satisfy MIT/Apache attribution for the bundled Rust crates

**Severity:** Medium · **Lens:** PROD · **Confidence:** Medium (legal reading)

**Evidence**
- `v3/docs/THIRD-PARTY-NOTICES.md:67-71`: "dependencies are licensed MIT or Apache-2.0 (the `ort` stack, `windows-sys`, `serde`, `sega`-style utilities). The authoritative per-crate licences are in … the repository's `Cargo.lock`".
- `Cargo.lock` has 119 packages. Cargo.lock holds no licence texts and is not shipped.
- "`sega`-style utilities" reads like placeholder text.

**Problem and impact**
- MIT and Apache-2.0 require the copyright and licence text to accompany binary distributions. The zip contains statically linked code from about 119 crates without their notices. Some may not be MIT/Apache (for example Unicode-3.0 or BSD in transitive dependencies), and nothing verifies this.

**Fix**
- Add `cargo about generate about.hbs > THIRD-PARTY-RUST.md` (or `cargo-bundle-licenses`) to `-Stage`, and add the file to the allow-list.
- Add a `deny.toml` licence allow-list enforced in CI (F2).
- Remove the placeholder sentence.

**Effort:** S

### SHELL-F12 — Root `.gitignore` does not cover v3's portable player-state layout

**Severity:** Medium · **Lens:** PROD · **Confidence:** High

**Evidence**
- `.gitignore:11-15` ignores only `v3/config/local.yaml`, `local.yaml.backup-*` and `local.yaml.corrupt-backup`.
- CUTOVER "Portable config layout packaging (2026-10-31)" lists player state under `config/setup/`, `config/machine/`, `config/artifacts/`, `config/user.yaml`, `config/meta.yaml`, `config/.migration-backup-*` and `config/.seal-txn/`.
- The dev core root is `<repo>/v3` (`core_path.dart:196`).

**Problem and impact**
- The first dev run migrates `local.yaml` into `v3/config/setup/…` and `machine/…`. A `git add -A` then commits the owner's calibration and personal state, which AGENTS.md explicitly forbids.
- `publish.ps1` guards the zip, but not the repo.

**Fix**
- Add those seven patterns, plus `v3/config/.seal-core.lock`, under `v3/config/`.
- Add a CI step: `git ls-files v3/config` must equal the shipped-template allow-list.

**Effort:** S


## Phase 2 — performance, accuracy ceiling & SOTA upgrades

38 items.

### OCR-F4 — `order_corners` starts at the top-most corner, so a slightly tilted box comes out rotated 90°

**Severity:** Medium · **Lens:** PERF-ACC · **Confidence:** Medium

**Evidence:** `detgeom.rs:199-212` rotates the angle-sorted corners to start at the minimum-y corner. `pipeline.rs:438-450` then takes `quad_width = |q0q1|`.

**Problem:** when `min_area_rect` returns a box tilted a degree or two counter-clockwise (right end higher), the top-most corner is top-right. The order becomes TR, BR, BL, TL, so width and height swap and `perspective_crop` hands rec a glyph rotated 90°. Short blobs, such as a single grade letter or count digits, are where the calipers most often pick a slightly tilted rectangle. The existing test only covers an exactly axis-aligned rectangle (`det_postprocess.rs` `box_points_are_clockwise...`).

**Fix:** use PaddleOCR's `get_mini_boxes` ordering:
1. Sort the four points by x.
2. Of the two leftmost, the smaller y is TL and the other is BL.
3. Of the two rightmost, the smaller y is TR and the other is BR.

Add tests with ±2° and ±10° rotated 30×8 rectangles asserting `quad_width > quad_height`.

**Effort:** S.

### OCR-F5 — The grade line is concatenated in contour (raster) order, not left to right

**Severity:** Medium · **Lens:** PERF-ACC · **Confidence:** High

**Evidence:** `seal-ocr/src/modes.rs:296-300` joins `band` straight from `scan.items`, which come in `find_contours` order (`pipeline.rs:298`). `detgeom::sort_boxes` exists but is never called (grep finds only tests).
```rust
let text: String = band.iter().map(|item| item.text.as_str()).collect::<String>().to_uppercase();
```

**Problem:** v2 built the grade line from `BuildLines`, which sorts by x. If the grade letter's box starts a pixel higher than the "GRADE：" box (a larger glyph or badge), v3 builds `"DGGRADE："`. After `rfind(':')` the rest is empty, so the text read finds no grade and the result depends on the colour fallback. `read_attributes` and `pet.test_read_panel` (`dispatch.rs` ~1433 uses `scan.items`) have the same ordering problem. v2 also filtered grade candidates (`conf > 0.1`, `len > 1`, `!= "300"`, de-duplicated), and those filters are dropped here.

**Fix:** sort `items` with `sort_boxes` in `detect_and_read` before returning, which also makes the order deterministic. In `read_grade`, re-add v2's conf/len/"300" filters and de-duplication. Test: items given in reverse x order must still read `DG`.

**Effort:** S.

### OCR-F6 — Recognition is single-crop and single-threaded; the bundled DirectML runtime is never used

**Severity:** Medium · **Lens:** PERF/SOTA · **Confidence:** High

**Evidence:**
- `pipeline.rs:357-361` makes one `rec.infer` per box. The C# library batched 6 (OCR-RESEARCH §3.3).
- `pipeline.rs:205,207` loads sessions with `threads = 1` in production.
- `model.rs:138-142` registers no execution provider, yet `lib.rs:30-34` says the release must ship `DirectML.dll` (18.5 MB). Cargo has no `directml` feature.
- `model.rs:323`: `decode_ctc` rebuilds `class_list` (6,625 `String` clones) for every crop, even though `RecModel` already stores `class_list`.
- `infer` copies its input (`data.to_vec()`) and its output (`logits.to_vec()`, T×6625 f32).

**Fix:**
- Add `RecModel::infer_batch(&[RecInput])`: sort crops by aspect ratio, take chunks of 6, pad to the chunk's max width (padding 0.0 is parity), and decode each row.
- Pass `class_list` into `decode_ctc` by reference.
- Make the thread count a config field (default `min(4, physical cores)`). Keep 1 in the parity runner, as the docs require.
- Then either enable `ort`'s `directml` feature and register DML for **det only**, with CPU fallback (DML needs memory pattern off and sequential execution), or stop shipping `DirectML.dll`.
- Add one warm-up inference at load so the first live read does not pay arena allocation.

**Acceptance:** a tuner read under 100 ms on CPU, with byte-identical text on the corpus between batched and unbatched runs.

**Effort:** M.

### OCR-F13 — The recogniser model lacks native Traditional Chinese support

**Severity:** Medium · **Lens:** SOTA · **Confidence:** Medium

**Evidence:**
- `MODEL_FILES` pins `ch_PP-OCRv4_rec_infer.onnx` (dictionary `ppocr_keys_v1`, 6,623 entries, Simplified-centric).
- `v3/config/attributes.yaml:105-155` carries about 50 hand-maintained repairs for Traditional characters: 攻孽/攻擎→攻擊, 艘力→體力, 防票力→防禦力, 幸蓮→幸運, 地/国/盘→每 (the last is a global plain replace that also rewrites any real 地).

**Problem:** if characters such as 擊, 體 or 禦 are not in the dictionary, the model cannot ever output them and the fixes table can only grow. The game is Traditional Chinese.

**Fix:**
- First, check: `RecModel::dictionary()` lookup for 擊體禦運殺傷 (a one-line test).
- Then evaluate PP-OCRv5 mobile rec, which unifies Simplified, Traditional, English and Japanese with a ~18k dictionary, and `chinese_cht_PP-OCRv3` rec, on the F11 corpus. Both have ONNX exports (RapidOCR / paddle2onnx). Note that v5 ONNX exports often ship the dictionary as a separate file.
- Make `RecModel::load` accept an optional dictionary path when the metadata is absent.
- Add a load-time check that the output class axis equals `dict.len() + 2`; today `decode` silently skips out-of-range indices (`model.rs:350`).

**Acceptance:** lower CER on the corpus, and the `text_fixes` table can shrink.

**Effort:** M.

### OCR-F14 — Fixed-layout reads still run free-form text detection

**Severity:** Medium · **Lens:** SOTA/PERF · **Confidence:** Medium-High

**Evidence:** count (`pet_calibration.rs:639-641`), remaining, grade and eta are all calibrated single-line boxes, yet each goes through the full `scan_rgba` (det, contours, crops).

**Fix:**
- Add `OcrPipeline::read_line_rgba(img) -> Decoded`: resize the region to height 48 (cubic), pad with `rec_input`, run rec and decode, with no det. This is PaddleOCR's `det=False` path.
- Use it for count, remaining and the grade letter. Keep `scan_rgba` for the tooltip panel and attribute block.
- For digits, `+`, `%` and `.` in the game's fixed bitmap font, an even cheaper and deterministic option is glyph templates: harvest them once during calibration, then match with zero-mean NCC over a column-segmented strip. Keep OCR as the fallback when the best NCC is below a threshold.

**Acceptance:** count/remaining reads under 20 ms each, with accuracy at least equal on the corpus.

**Effort:** M.

### CORE-F7 — The cached OCR pipeline is never rebuilt after config changes

**Severity:** Medium · **Lens:** PERF-ACC · **Confidence:** High · **Also reported as:** OCR-F8

**Evidence:** `state.ocr` is built once (`dispatch.rs:370-388`, `1477-1489`) with `attributes.text_fixes` and `row_height` baked in (`seal-ocr/src/pipeline.rs:202-217`). Nothing ever sets it back to `None` (grep found no `ocr = None`). The config changes that should rebuild it are:
- a variant switch (`config.apply`, `dispatch.rs:752-755`),
- `config.reload`,
- `calibration.tuner.save` (which derives `row_height`).

**Impact:** After a tuner calibration or a variant switch, the calibrator's test reads use the old dictionary and row height until the core restarts.

**Fix:** Store `(dictionary, row_height)` next to the pipeline. In `ensure_ocr`, rebuild when they differ, or only swap the cheap parts (cleaner, row height) without reloading the ONNX sessions, e.g. a `pipeline.reconfigure(&attributes, row_height)`. Also route `ocr_scan` through `ensure_ocr`, which removes duplicated code. **Effort S.**

### CORE-F10 — Calibration fetches carry the whole gallery and fully decode every PNG

**Severity:** Medium · **Lens:** PERF · **Confidence:** High

**Evidence:** `session_gallery` (calibration.rs:918-941) reads up to four full-frame PNGs, calls `decode_png` on each just to get width and height, and base64-encodes all of them. Every `calibration.fetch` includes it (calibration.rs:1006), and so does `fetch_all` (1124). The shell's per-kind fallback is about 35 fetches, i.e. about 35× (4 frames, several MB each) on the stdout pipe.

**Fix:**
- Read width and height from the IHDR chunk (bytes 16..24) instead of decoding.
- Return the gallery only from `fetch_all`, or from a dedicated `calibration.gallery`.
- Cache by file mtime.

**Effort S.**

### CORE-F11 — Large JSON values are cloned on every request

**Severity:** Medium · **Lens:** PERF · **Confidence:** High

**Evidence:**
- `publish_config` clones the whole merged config after every command, reads included (dispatch.rs:610, 619).
- `ocr_scan` clones a cached frame, a multi-MB base64 string (dispatch.rs:340).
- `capture_command` clones the frame it just inserted (dispatch.rs:2124).
- `echo_sections(&state.config.clone(), ..)` clones for no reason (dispatch.rs:775).
- `FrameCache` stores frames as base64 inside a `Value`, so every user re-decodes base64 then PNG (`frame_rgba`, dispatch.rs:503).

**Fix:**
- Share the config as `Arc<Value>`, with the dirty flag from F4.
- Make `FrameCache` hold `struct Frame { meta, png: Arc<[u8]>, rgba: OnceCell<Arc<RgbaImage>> }` and base64-encode only when writing the response.
- Remove the clones at 340, 775 and 2124.
- `Vec::remove(0)` → `VecDeque`.

**Effort M.**

### CORE-F13 — The OCR fixtures are hand-written, and the command list exists in three copies

**Severity:** Medium · **Lens:** SOTA · **Confidence:** High

**Evidence:** The command names live in the `handle` match (dispatch.rs:535-600), in `REQUEST_COMMANDS` in seal-proto `tests/fixtures.rs:30+`, and in the Dart shell. The fixture *payloads* are written by hand (for example the `ended_at` literals at fixtures.rs:1419), because seal-proto cannot depend on seal-core. A change to the dispatcher's output can drift from the fixtures without any test failing. Arguments are parsed by hand (`.get().and_then(as_str)`) with `(String, String)` or `Box<Response>` errors.

**Fix:**
- Add a test in `seal-core/tests` that runs every request fixture through `handle()` on a seeded `CoreState` with fake win/board, and checks the response shape against the response fixture.
- Replace the match with a `static COMMANDS: &[(&str, Handler)]` table and assert it equals the set of fixture commands.
- Use typed `#[derive(Deserialize)]` structs per command (optionally `schemars` for JSON Schema / Dart codegen), plus a `thiserror` `CommandError { code, message, detail }` with `impl From<CommandError> for Response`.

**Effort L, incremental.**

### PET-F10 — Wait after the end toggle is shorter than v2's measured value

**Severity:** Medium · **Lens:** PERF-ACC · **Confidence:** High

**Evidence:** `pet.rs:1586-1587` waits `self.action_wait()` (1.2 s by default). v2 waits `Math.Max(EndWait, ActionWait)` with `EndWait = 2.5` (`PetTool.cs:1935,3389`), raised after live misses where the pet had not yet reappeared in the bag. v2 also persisted `BoardingRunning=false` right after the press (`PetTool.cs:1933-1934`); v3 persists only at fold.

**Fix:** Add `const END_WAIT: f64 = 2.5`, use `self.wait(END_WAIT.max(self.action_wait()))`, and persist `boarding_running=false` after the end click.
**Effort:** S

### PET-F12 — Long waits are a sum of 50 ms relative sleeps: no deadline, no resume detection, ~246k wake-ups per cycle

**Severity:** Medium · **Lens:** SOTA/PERF · **Confidence:** Medium · **Also reported as:** CORE-F16

**Evidence:** `cancel.rs:126-131` (`steps = seconds / 0.05`, one `clock.sleep_seconds(tick)` per step) and `clock.rs:28-31` (`thread::sleep`).

**Impact:**
- Per-tick overshoot accumulates: small with high-resolution timers, around +25% if a 15.6 ms timer resolution applies.
- Whether time spent in suspend counts depends on the platform. In either case nothing detects resume: after a PC sleep the loop clicks into a client that is probably disconnected, with no check.

**Fix:**
- Sleep to a deadline: `let end = clock.now() + d; while clock.now() < end { sleep(min(remaining, tick)); check()? }` with `Clock::now()` injected.
- Compare wall clock with the monotonic clock at wake. On a jump larger than a threshold, run a Look first and verify that the window exists and is in front (F4).
- Simulation test with a fake clock that jumps.

**Effort:** M

### PET-F13 — Rate and pet-minutes accuracy

**Severity:** Medium · **Lens:** PERF-ACC · **Confidence:** Medium/Low

**Evidence:**
- One read of `parse_rate` replaces the configured rate for that row (`pet.rs:1481`, `decision.rs:270-272`). An OCR 3→1 misread triples the food minutes, the unsafe (late) direction.
- `fold` sizes the next load with the rate read before the reload, which describes the previous pet (`pet.rs:1521-1526`).
- `minutes_to_finish` scales by the table's own minutes column (`decision.rs:197-208`). The implied items per minute in `pet-data.csv` varies from 1 to 5 per row and ignores the row's measured rate. (Low confidence: verify against a live finish.)

**Fix:**
- Accept a measured rate only when two consecutive reads agree. Otherwise use `max(measured, configured)` for scheduling, since the earlier reload is the safe direction.
- Re-read the rate after the reload (VerifyStarted already reads `row_lines`, `pet.rs:1887`).
- Proptest: `schedule_for` is monotone non-increasing in `rate`, and never exceeds the cycle fallback when `items_left <= load_items`.

**Effort:** S-M

### HW-F6 — Firmware: `!Serial` runs first on every loop pass, adding about 10 ms per pass

**Severity:** Medium · **Lens:** PERF-ACC · **Confidence:** Medium-High (check the ArduinoCore-avr `CDC.cpp` in use)

**Evidence.** `seal_mouse.ino:153`: `if (!Serial && (spaceHeld || leftHeld))`. In ArduinoCore-avr, `Serial_::operator bool()` ends with `delay(10)`.

**Problem.** Every `loop()` sleeps 10 ms before `Serial.available()`. That adds up to 10 ms of latency to every command, including each closed-loop `D` step and each spammer key press, even when nothing is held.

**Fix.**
- Swap the operands: `if ((spaceHeld || leftHeld) && !Serial)`, so the delay is paid only while something is held. Or rate-limit the check with `millis()`.
- This changes no behaviour, so no protocol level bump is needed. Keep `tests/sketch.rs` green.

**Effort:** S.

### HW-F8 — The board is reopened for every action, with a fixed 2 s boot wait each time

**Severity:** Medium · **Lens:** PERF-ACC · **Confidence:** High

**Evidence**
- `seal-core/src/pointer.rs:113` (and registry.rs:148-250, buysell_session.rs:218, gem_routes_session.rs:114) call `board::open_board` per command or run.
- `serial.rs:127` `boot_wait(clock)` = 2.0 s every time.

**Problem.** Every pointer test, `pet.select_page` or route test pays enumeration plus 2 s, and `board.probe` adds up to 3 s of `V` windows. The 32U4 does not reset on DTR (`serial.rs:9` says so), so the wait only matters on a cold plug. Every close also toggles DTR, which conflicts with the firmware's assumption that the port is held for the app's lifetime.

**Fix.**
- A long-lived `BoardSession` actor in seal-core (a thread that owns `SerialTransport` and an mpsc of commands): open lazily, keep the port, and reopen after a `TransportError` with re-enumeration.
- Replace the fixed wait with readiness polling: send `V` every 250 ms until it answers or 5 s pass, then treat the board as silent.
- Accept when a second pointer command within a session completes in under 200 ms (measured live).

**Effort:** M.

### HW-F9 — Placement damping only ever grows, with no gain estimate

**Severity:** Medium · **Lens:** PERF-ACC · **Confidence:** Medium

**Evidence**
- `placement.rs:123-126`: `if error >= last_error { divisor *= 2; }`. It never decreases.
- `step_count` divides by the divisor (`:188-197`).
- The settle wait burns 24×15 ms when a move produces no visible motion (`:171-183`).

**Problem.** One bad sample, such as a missed settle, the game warping the cursor, or "Enhance pointer precision" swallowing a 1–2 count move, permanently halves the gain. From 600 px with divisor 8, convergence is about (7/8)^n, which needs around 40 steps, more than `MAX_STEPS = 16`, so the placement fails. Steps that produce no motion cost 360 ms each.

**Fix.**
- Estimate the gain from measured motion: `g = clamp(moved/asked, 0.25, 4.0)` per axis, smoothed (EMA), then `ask = round(error / g)` clamped to ±600.
- Fall back to halving only on a sign flip (overshoot).
- Exit the settle wait early when `now == before` after 3 polls for asks of 3 counts or fewer.
- Tests in `tests/placement.rs` with fake cursors at gain 0.5, 1.5 and nonlinear EPP-like curves, plus one spurious reading: each should converge in 8 steps or fewer.

**Effort:** M.

### HW-F12 — Capture pipeline: PNG encode/decode round trip, `CAPTUREBLT`, undefined alpha

**Severity:** Medium · **Lens:** PERF-ACC/SOTA · **Confidence:** Medium · **Also reported as:** OCR-F7

**Evidence**
- `window.rs:321-322`: `blit_screen_bgra` → `encode_png`.
- Consumers immediately call `decode_png` again: `seal-core/src/pet.rs:277`, `gem.rs:319`, `dispatch.rs:511`, `tuner_session.rs:122`, `calibration.rs:766`.
- `window.rs:450` `SRCCOPY | CAPTUREBLT`.
- `pure.rs:62` copies the GDI reserved byte as PNG alpha.

**Problem.**
- A full-client deflate at the default level plus a decode costs tens of milliseconds per scan, plus 3 extra full-frame copies.
- On DWM, `CAPTUREBLT` is slower and makes the cursor flicker, which can disturb hover/tooltip captures.
- GDI's reserved byte is not a defined alpha (often 0), so saved PNGs can render transparent. OCR ignores alpha (`seal-ocr/src/pipeline.rs:246`).
- The PNG-vs-BGRA choice for the *protocol* payload is a documented open decision (OPEN §6). The *internal* path is not covered by that decision.

**Fix.**
- Add `pub struct Frame { width, height, origin, rgba: Vec<u8> }` and `capture_client_frame()` that returns raw pixels, swizzling in place with alpha forced to 255.
- Encode PNG only at the persistence/protocol boundary, as RGB with `png::Compression::Fast`.
- Drop `CAPTUREBLT` unless a layered overlay must be captured.
- Bench target: a 1920×1080 capture-to-OCR handoff under 15 ms.

**Effort:** M.

### SHELL-F5 — Every event and log line rebuilds the entire shell, including all kept-alive tabs

**Severity:** Medium · **Lens:** PERF-ACC · **Confidence:** Medium

**Evidence**
- `shell_page.dart:4`: "One `ListenableBuilder` at the root renders the whole shell". At line 158 that builder listens to `widget.model`.
- `shell_model.dart:1946-1951` `_onSubmodelChanged` → `notifyListeners()`, and it is subscribed to `logs`/`tools`/`config`/`connection`.
- `LogBuffer._add` calls `notifyListeners()` per line, and `entries` copies the list on each call.
- The UI is about 20.6k LOC (`pet_tab.dart` alone is 2,717 lines). The `IndexedStack` children are re-instantiated each build, so every built tab runs `build()` as well.
- No event throttling is specified in PROTOCOL.

**Problem and impact**
- During a running tool, `state` and `log` events, plus core stderr chatter, trigger whole-tree rebuilds several times per second, even while the offstage full shell sits behind mini or dock mode.
- This is CPU drain on a machine that is also running the game and OCR. It amplifies F3.

**Fix**
- Scope listeners: wrap the logs drawer and status strip in their own `ListenableBuilder(listenable: model.logs)`, cards in `model.tools`, and so on. Make the root listen only to `connection` plus mode flags.
- Batch log notifications to one per frame with `SchedulerBinding.scheduleFrameCallback`.
- Make the tab list `const`/cached widgets.
- Acceptance: a test with `debugPrintRebuildDirtyWidgets` or a build counter shows that a log line does not rebuild `PetTab`.

**Effort:** M

### OCR-F12 — Upscaling uses nearest-neighbour where v2 used cubic or area interpolation

**Severity:** Low-Medium · **Lens:** PERF-ACC · **Confidence:** High

**Evidence:**
- `seal-core/src/pet_calibration.rs:666-680`: `upscale_rgba` repeats pixels for the 3×/9×/24× count ladder. v2 used `InterpolationFlags.Cubic` (`OcrEngine.cs:187-188`). The production runner has no ladder at all (OPEN.md §2, documented).
- `seal-core/src/pet.rs:912-935`: `area_resize` claims "box-average downscale" but samples one source pixel. v2 used `InterpolationFlags.Area` (`IconMatch.cs:50`).

**Problem:** nearest-neighbour ×24 gives the recogniser blocky glyphs it was not trained on. The ladder only ever changes the rec crop's source quality, because det resizes to 736 regardless. After a grid recalibration changes the pitch by ±1 px, nearest sampling inflates the differing fraction for every icon.

**Fix:** add `imageops::resize_cubic`, or reuse the cubic kernel from `perspective_crop`, and expose `OcrPipeline::scan_rgba_upscaled(factor)`. Use it in both the calibration ladder and the runner, so the two paths read identically. Implement a real area average in `area_resize`. Test: area-downscaling a 2×2 checkerboard gives the mean grey.

**Effort:** S.

### OCR-F15 — Attribute matching is post-hoc fuzzy string matching over a closed vocabulary

**Severity:** Low-Medium · **Lens:** SOTA/PERF-ACC · **Confidence:** Medium

**Evidence:** `seal-domain/src/ocr/matcher.rs:122-136`. `all_chars_in` (characters present anywhere, ignoring order and count) scores 0.8×len, and the threshold is `best_score <= 1.0`. There is no confusable-character model; confusions go through `text_fixes`.

**Fix:** keep this as parity behaviour. Add a second scorer, used when no exact `contains` hit occurs: for each dictionary variant, score with CTC forced alignment (the log-probability of the variant under the line's logits, via the CTC forward algorithm over T×C), normalised by length. This needs `RecognizedLine` to keep its logits, or the top-k per timestep. The closed attribute vocabulary is a few hundred strings, so this is cheap and removes most confusable fixes. Gate it behind config and compare on the corpus.

**Effort:** M-L.

### TOOL-F14 — The card can flip from "Stopping" back to "Running"

**Severity:** Low–Medium · **Lens:** PROD · **Confidence:** High

**Evidence:**
- `runner.rs:325-332` publishes `.running(ToolPhase::Running)` whenever the status changes.
- `arbiter.rs:443-449` `publish_phase` does a read-modify-write and can lose an update.

**Problem & impact:** After Stop, the tuner's or pet's next status change republishes `Running`, so the card shows running until `finished`. The read-modify-write can also lose either writer's update.

**Fix:**
- Have the runner publish `if control.is_cancelled() { Stopping } else { Running }`.
- Better: give `StateCell` an `update(|s| …)` method that holds the write lock for the whole change, and make the phase monotonic (never lower than `Stopping` once stopping).

**Effort:** S

### TOOL-F15 — The gem empty threshold falls back to 0.18 where the spec says 0.01; one knob serves two metrics

**Severity:** Low–Medium · **Lens:** PERF-ACC · **Confidence:** Medium · **Also reported as:** OCR-F17

**Evidence:**
- `gem.rs:205` `float(config, "/gem/empty_distance").unwrap_or(0.18)`.
- `local.yaml.example:52` sets `0.18`.
- `CALIBRATION.md:200`: "gem 0.55, gate `empty_distance` 0.01".
- v2 `AppConfig.cs:995` defaults to `0.01`.
- The same value gates both the pixel-diff fraction and the six-feature distance (`domain gem.rs:312-314`).

**Problem & impact:** At 0.18 the margin to a gem's diff is about 2x instead of 35x. A small or translucent gem can read as empty and advance the grade early (wasted cycles; in `advance_grade_clear` mode, resource right-clicks).

**Fix:**
- Split the setting into `empty_diff_max` (default 0.01) and `empty_signature_distance` (default 0.18), migrating the old key to both.
- Log both values in `record_empty_check`.

**Effort:** S

### HW-F10 — Serial adapter: no write deadline, per-chunk read deadline, no input purge

**Severity:** Low-Medium · **Lens:** PROD · **Confidence:** Low (depends on serialport 4.10.1 internals; check `windows/com.rs` `set_timeout` `WriteTotalTimeoutConstant`)

**Evidence**
- `serial.rs:155-162` `write_all` + `flush` with no write timeout set.
- `read_line` sets the full deadline per `device.read` call inside the loop (`:166-203`), so data that trickles in can stretch the total wait.
- `open` (`:111-129`) does not call `clear(ClearBuffer::All)`.

**Problem.** If COMMTIMEOUTS has write constant 0, a board that is busy (F7) or wedged blocks the worker thread inside `WriteFile` indefinitely. Stop and the supervisor cannot interrupt it, and an unplug mid-write depends on the driver failing the IRP.

**Fix.**
- Track an `Instant` deadline across the read loop and set the remaining time per read.
- After `open`, call `port.clear(ClearBuffer::All)`.
- Verify the write timeout. If it is unset, enforce one (a write thread with `recv_timeout`, or extend serialport via `SetCommTimeouts` on the raw handle).
- Map write-timeout errors to a "board busy / not responding" `TransportError`.

**Effort:** S-M.

### HW-F15 — No quarantine for a corrupt file; one bad machine file blocks startup

**Severity:** Low-Medium · **Lens:** PROD · **Confidence:** Medium

**Evidence.** `file.rs:55-58` returns `ConfigError::Parse`, which `layout::load` propagates (`layout.rs:58`) and fails the whole load. Nothing in seal-config handles corruption. v2 kept `*.corrupt-backup` files (repo `AGENTS.md`).

**Fix.** For `Class::Machine` files only, rename the unparsable file to `<name>.corrupt-<stamp>` and continue with a warning in `Loaded.warnings`; the tool then reports the calibration it needs. For setup files, keep failing loudly, but name the file and the line. Test with a truncated `machine/pet.yaml` and check that the load succeeds with a warning and a quarantined file.

**Effort:** S.

**Documented, not new.** There is no cross-process single-instance lock (OPEN §2; `store.rs` doc). This matters more once F2's journal exists, because two cores can each recover the other's live transaction. Building `config/.seal-core.lock` (`LockFileEx`, exclusive) closes both.

### OCR-F9 — Three independent copies of the ~25 MB model set are loaded

**Severity:** Low · **Lens:** PERF/PROD · **Confidence:** High

**Evidence:** `dispatch.rs:371`, `tuner.rs:488-490` and `pet.rs:655-656` each call `OcrPipeline::load_for_root`. Each load is about 330 ms (det 154 + rec 173 ms measured) plus ORT arenas.

**Fix:** keep one `Arc<Mutex<OcrPipeline>>` in `CoreState` and hand a clone to the tool sources. Sessions are `Send`; `run` needs `&mut`, hence the mutex. This also lets F8's invalidation happen in one place.

**Effort:** S.

### OCR-F10 — The unused classifier model is a hard requirement

**Severity:** Low · **Lens:** PROD · **Confidence:** High

**Evidence:** `pipeline.rs:95-99` lists `ch_ppocr_mobile_v2.0_cls_infer.onnx`. `missing_models` refuses a directory without it, but nothing ever loads it (grep for `cls_infer` finds only this constant).

**Problem:** an install missing a file that is never used reports `ocr_unavailable`.

**Fix:** split into `REQUIRED_MODELS` (det, rec) and leave cls optional or remove it. Game UI text is never upside down. Update `tests/models_resolution.rs`.

**Effort:** S.

### OCR-F16 — Contour tracing uses a per-pixel `BTreeSet` and `HashMap`

**Severity:** Low · **Lens:** PERF · **Confidence:** High

**Evidence:** `seal-ocr/src/detmask.rs`, `find_contours`/`boundary_cycles`: `BTreeSet<(i32,i32)>` per component and `component.contains` per pixel edge. The edges `HashMap` is keyed by start vertex, so a pinch vertex with two outgoing edges is overwritten. The 2×2 dilate makes this rare.

**Fix:** use a label image, `labels[idx] == label`, for the inside test. Store edges in a `Vec` indexed by vertex with up to 2 slots. Or port Suzuki-Abe border following, which also gives OpenCV parity.

**Effort:** S-M.

### CORE-F14 — `board.test` and `board.probe` skip the input lease

**Severity:** Low · **Lens:** PROD · **Confidence:** High

**Evidence:** `dispatch.rs:2038-2069` opens the board without `input_lease()`, unlike `pointer.*` and the calibration tests. The pet holds the port for the whole run (registry.rs:284), so a call during a run fails with a raw OS "access denied" as `board_unavailable` instead of `tool_busy`.

**Fix:** Call `check_input_lease` first in `board_test`. `board_probe` can stay read-only but should report `tool_busy` naming the tool that holds it. **Effort S.**

### CORE-F15 — The artifact reference loader accepts absolute and UNC paths

**Severity:** Low · **Lens:** PROD · **Confidence:** Medium

**Evidence:** `pet_artifacts.rs:58-65` rejects only `..` and `\`. A value such as `artifacts/C:/x.png` or `artifacts///host/share/x.png` makes `root.join(name)` resolve to an absolute path, i.e. it reads any local file or reaches out to an SMB share (which leaks NTLM credentials).

**Fix:** Require `Path::new(name).components()` to be exactly one `Normal` component, the same rule `write_reference` should use. Test with drive-letter and UNC inputs. **Effort S.**

### PET-F14 — Config values are not bounded

**Severity:** Low · **Lens:** PROD · **Confidence:** High

**Evidence:**
- `wait_after_empty_minutes` is unvalidated (`pet.rs:425`). A negative value gives `next <= 0` in `fold_back` (`decision.rs:516`) and a back-to-back reload loop.
- `stacks` up to `i32::MAX` → `Vec::with_capacity(stacks)` in `slot_boxes` (domain `pet.rs:335`), reached from preflight and `ready()`, aborts the core on allocation. `load_items` multiplies without checking (`decision.rs:277`).
- `ready()` checks drag mode with `eq_ignore_ascii_case` (`decision.rs:930-932`) while the run uses a trimmed comparison (`food_load.rs:44-47`), so `"drag "` passes Start but fails at `FoodPlace`.

**Fix:** Clamp in `plan()` (stacks 1..=8, wait 0..=120, rate 1..=30), use `food_load::is_drag` everywhere, and add `seal-config` schema bounds.
**Effort:** S

### PET-F15 — Cancellation and window hygiene

**Severity:** Low · **Lens:** PROD · **Confidence:** High (partly documented)

**Evidence:**
- `stopped_during()` (`decision.rs:1075`) is unused, so a stop mid-visit says only "Stopped." (`runner.rs:285`).
- `boarding_window_may_be_open` and `close_button_png` are loaded but unused (`pet.rs:421-423`). The missing open-window probe is documented in OPEN.md §2.
- `PetTool` implements no `summary()`.

**Fix:** Track the in-flight batch, use `stopped_during` for the cancelled message via `Tool::summary`, and persist `boarding_window_may_be_open=true` on Open and `false` after Close.
**Effort:** S

### PET-F16 — Icon sweep allocations

**Severity:** Low · **Lens:** PERF · **Confidence:** High

**Evidence:** `best_offset_score` allocates a crop for each of 49 offsets × 64 cells × queue entries × pages, and `differing_fraction_images` clones it again when the sizes match (`pet.rs:960-964,1022-1031`). The full sweep is repeated for each row in a batch.

**Fix:** Compare in place against bag offsets without copying, and reuse one sweep per visit.
**Effort:** S

### TOOL-F16 — Low-severity hygiene: unbounded waits and per-read encode overhead

**Severity:** Low · **Lens:** PERF-ACC · **Confidence:** Medium

**Evidence and impact:**
- `cancel.rs:127-131`: a huge or infinite `seconds` saturates `steps` to `usize::MAX`. `RealClock` then refuses the overflowing `Duration`, so the loop busy-spins at 100 % CPU until Stop.
- Every tuner or gem read encodes the capture to PNG, then base64, then a JSON `Value`, then decodes base64 and PNG back to RGBA (`win.rs:121-133` plus `dispatch::frame_rgba`), and does it up to three times per attempt.

**Fix:**
- Clamp waits to, say, 24 h.
- Add `capture_rgba()` returning raw pixels for in-process readers; keep PNG only for the protocol.

**Effort:** S

### HW-F11 — Move counts are i32 on the host but `int` (16-bit) on the AVR; overlong lines become two commands

**Severity:** Low · **Lens:** PROD · **Confidence:** High

**Evidence**
- `command.rs:90` `move_by(dx: i32, dy: i32)` is unchecked.
- `ino:180-181` `int dx…; sscanf(buf, "D %d %d", …)`, and `int` is 16-bit on the ATmega32U4.
- `ino:161` `readBytesUntil('\n', buf, 31)`.

**Problem.** Today only callers that bound their values exist (placement ±600, gem legs ±9999 at `gem_session.rs:61`). Any future caller over ±32767 wraps, so the move goes the wrong way. A line of 32 bytes or more is split, and its tail is parsed as a fresh command whose first byte picks the action.

**Fix.**
- Make `move_by` fallible with `±9999` (`MOVE_MAX`), or split large moves into several frames in `encode`.
- In the firmware, use `long` with `%ld` and reject out-of-range values.
- On overflow, discard up to the next `\n`: `if (n == sizeof(buf)-1 && buf[n-1] != '\n') { while (Serial.read() != '\n'); return; }`.

**Effort:** S.

### HW-F13 — GDI/FFI layer could be RAII-based; capture backend is legacy

**Severity:** Low · **Lens:** SOTA · **Confidence:** High

**Evidence.** Manual release sequences at `window.rs:400-498` are correct today but fragile to edit. `windows-sys = "0.52"` (`seal-win/Cargo.toml`), while 0.61 is already compiled into the tree.

**Fix.**
- Small Drop guards: `struct ScreenDc(HDC)`, `struct MemDc(HDC)`, `struct Bitmap(HBITMAP)`, `struct Selected<'a>{dc, prev}`. That removes about 60 lines and makes early `?` returns safe.
- Moving seal-win to windows-sys 0.61 costs no extra build, since serialport keeps pinning 0.52 anyway.
- Longer term: Windows.Graphics.Capture (per-window, works when the game is occluded, `IsBorderRequired=false` on Win11) or DXGI Desktop Duplication (GPU, about 1 ms per frame) behind the same `capture_client_frame` seam. Keep GDI as the fallback.

**Effort:** S (RAII), L (WGC).

### HW-F14 — Legacy foreign-block parser silently drops non-identifier top-level keys

**Severity:** Low · **Lens:** PROD · **Confidence:** High

**Evidence.** `foreign.rs:98`: `if key.chars().all(|c| c.is_alphanumeric() || c == '_')`. `store.rs` `update_local_checked` then keeps only owned keys from the parsed YAML.

**Problem.** In `local.yaml` (legacy/dev layout), a top-level block named `my-notes:` or `"notes":` is not recognised as foreign text, and it is also removed from the parsed object, so it is deleted on the next save. The migration path is unaffected because it uses parsed values for `user.yaml`.

**Fix.** Get the foreign key set from the parsed mapping (`parsed.keys().filter(!owned)`). Locate each block's text span by matching `^(?:"key"|'key'|key)\s*:` at column 0. Fall back to re-serialising the parsed value (without comments) rather than dropping it. Test with hyphenated, quoted and BOM-prefixed keys.

**Effort:** S.

### SHELL-F13 — UI is English-only with about 456 hard-coded `Text(` strings and no l10n infrastructure

**Severity:** Low · **Lens:** SOTA · **Confidence:** High

**Evidence**
- `pubspec.yaml` has no `flutter_localizations` and no `l10n.yaml`.
- `lib/ui` contains about 456 `Text(` calls with literals, across about 20.6k LOC.
- `USER_GUIDE.zh-TW.md` is maintained by hand "when a user-facing label changes" (PLAN-V3-UI-UX:411).
- Only about 41 `Semantics`/`semanticLabel`/`tooltip:` usages.

**Problem and impact**
- zh-TW players get a translated guide but an English UI, and the guide has to describe English labels.
- Labels in Dart and both guides drift with no tooling to catch it.

**Fix (incremental)**
- Add `flutter_localizations` + `intl` and `l10n.yaml` with `app_en.arb`/`app_zh_TW.arb`. Migrate per tab, starting with crash and connecting surfaces, cards and the status strip.
- Add a test that fails on missing ARB keys.
- Pass `Locale` from Windows (`PlatformDispatcher.locale`).

**Effort:** L (incremental)

### SHELL-F14 — Protocol types are hand-written on both sides; drift is caught only by fixtures

**Severity:** Low · **Lens:** SOTA · **Confidence:** Medium

**Evidence**
- `lib/protocol/types.dart` (740 lines) and `test/support/fixture_decoders.dart` (22 KB) are maintained by hand alongside `seal-proto`.
- The only drift guard is 81 JSON fixtures, run manually (see F2).

**Problem and impact**
- Any field added to a Rust type but not to a fixture is invisible to the shell until runtime, which feeds into F1.

**Fix**
- Emit a JSON Schema from `seal-proto` (`schemars`) into `v3/protocol/schema/`, and add a CI check that the schema is up to date.
- Either generate Dart types (`quicktype`/`json_serializable` + `freezed` sealed unions) or at least validate every fixture against the schema in both test suites.

**Effort:** M–L

### SHELL-F15 — Repo entry docs still describe v2 as current

**Severity:** Low · **Lens:** PROD · **Confidence:** High (documented)

**Evidence**
- `AGENTS.md:3-8`: "C# / .NET 8 WPF … `v2/` is the current build". Its layout table has no `v3/`.
- The root README still advertises v2.16. CUTOVER step 3 (rewrite README for v3.0.6) is documented as pending.

**Problem and impact**
- Coding agents and contributors follow the wrong gates and conventions for v3, and the PR "CI" gives false assurance.

**Fix**
- Add a v3 section to AGENTS.md now (gates: `tool/check.ps1`, `cargo clippy/test`, `publish.ps1 -Selftest`; layout; no-`local.yaml` rule for the portable layout). Do the README rewrite at publish, per CUTOVER.

**Effort:** S


## Appendix A — Slice-level architecture recommendations (verbatim from slice reviews)


### A.OCR — OCR & screen reading (seal-ocr, vision helpers)

1. **Split "where" from "what".** Let `seal-ocr` expose three read primitives:
   - `scan` (det + rec), for free-form panels;
   - `read_line` (rec only), for calibrated single-line boxes (F14);
   - `match_glyphs` (NCC templates), for the bitmap-font digits.

   Each tool read path in `seal-core` should state which primitive it uses. The sessions should be one shared, `Arc`-held instance (F8/F9). Text cleaning and row bucketing should be per-call options, not pipeline state.

2. **Create one `vision` module**, in `seal-domain` or a small `seal-vision` crate, for the pixel-comparison primitives now duplicated three times: `pet.rs::differing_fraction`, `differing_fraction_images` and `gem.rs::diff_fraction`, plus the resize helpers in `pet.rs`, `pet_calibration.rs` and `seal-ocr::imageops`. Give it a single `RgbaView<'a>` borrowed-image type to remove the per-offset `Vec` allocations and clones in `best_offset_score` (`pet.rs:1012-1025`).

3. **Raw pixels on internal paths, PNG only at the protocol edge** (F7).

4. **Make the eval harness the gate for OCR changes** (F11). Land it before F1, F4, F13 and F15 so each change has a measured before and after. Suggested order:
   1. F2, F3 and F4: small changes with large latency and accuracy effects.
   2. F11, the harness.
   3. F1 and F5, measured on the harness.
   4. F6, F7 and F14 for latency.
   5. F13 and F15 for the accuracy ceiling.


### A.CORE — Core runtime & shell–core protocol (seal-core main/dispatch/supervisor/logs)

1. **Split `dispatch.rs` (2.6k lines)** into `commands/{config,calibration,pet,board,win,tool}.rs`, each registering handlers in one table (F13). Move `CoreState` into `state.rs`. Remove the duplicated OCR-loading code.
2. **One owner for config state.** A `ConfigHub { store, current: ArcSwap<Snapshot{rev, value}> }` shared by the dispatcher, the registry and the pet sink. Revisions come only from the hub, and writes go through it. This removes F4 by construction.
3. **Supervisor as an explicit state machine with timers.** Use a single `recv_timeout` loop that holds the deadlines (deferred timeout, shutdown budget) and counts stashed replies, with an invariant check in tests that "every stashed reply is answered on every exit path" (F1, F6, F8). Consider `crossbeam-channel` `select!` with `after()` for the timers. Since Rust 1.67 std `mpsc` is crossbeam-based, so this is about ergonomics, not speed.
4. **Panic and crash policy** (F3): a panic hook writing to `error.log`, `catch_unwind` at the worker and dispatcher boundaries, and a "never `panic=abort`" note next to the release guards. Optionally write a minidump via `MiniDumpWriteDump` for native crashes in the OCR/ONNX runtime.
5. **Structured logging:** adopt `tracing` with a `tool`/`run_id` span per worker and per request (`cmd`, `id`), and a rotating file appender (`tracing-appender`). Keep the v2 line formats as a custom formatter. This also solves F12.
6. **Backpressure on outgoing messages:** a bounded `sync_channel` (e.g. 1024) for events, coalescing `state` per tool (keep only the latest) while responses are never dropped. This becomes important once F2 is fixed.
7. **Frame transport:** keep binary frames inside the core (`Arc<[u8]>`) and base64-encode only at the wire. The PNG-vs-BGRA payload question is documented in OPEN.md as a decision to be made.
8. **Long blocking commands** (pet scans, gem cycle) make `tool.stop` wait behind them because the dispatcher is single-threaded. This is a documented decision (OPEN.md "Pet sweep protocol"). Revisit once live passes exist, by handing them a cancel token through a job id.


### A.PET — Pet Feeder (seal-core pet*, seal-domain tools/pet)

1. **Make the schedule a pure, clocked state machine in `seal-domain`.** For example `PetScheduler { rows: Vec<RowSchedule{due_at, strikes, belief}>, food_cursor, start_override_pending }` with `fn on_look(now, readings) -> Vec<Action>` and `fn on_outcome(now, row, outcome)`. Deadlines are absolute (an injected monotonic clock; wall clock only for HH:mm labels). Decision rule: `timer_due && !veto ⇒ reload`. Persist this struct as one patch. This fixes F1, F2, F6 and F12 at the root.
2. **Replace the flat `Stage` enum plus shared `Visit` counters with per-step sub-states** that own their counters (`Placing{attempt}`, `Loading{stack, drag_attempt}`, `Choosing{remaining, return_tried}`). Add a progress invariant: every stage transition either emits a decision or advances a bounded counter.
3. **Treat foreground and window presence as preconditions on input.** Either an effects-level gate (`Effects::click` refuses when not in front, returning a distinct `EffectError::NotForeground` that the pet maps to a deferral rather than a stop), or a check at the start of each visit and row. Re-measure `DisplayInfo` at visit start.
4. **Give the run one owner for its mutable state.** The live tool owns `food_slots_used` and `boarding_running`. Shell edits go to the worker through a channel, and the dispatcher re-reads config when the shared revision moves (F3, F11).
5. **Add a structured `pet.jsonl` event stream** (look reading, veto decision, outcome, strikes, food cell used) separate from card messages, rotated by size. It is the v3 replacement for v2's `pet.log` forensics.
6. **Testing:**
   - A simulation harness that drives `PetTool` through several days with a fake clock and a scripted world model (tray drains at rate, pet finishes, slot reads, foreground flips).
   - Proptest the domain invariants: veto is positive-only, the food cursor is monotone, the next reload is never later than the cycle fallback when evidence is missing, and there is no zero-interval reopen.
   - Today no test drives past the first long `Wait` (`pet.rs:2798-2819` `run_to_wait`), which is why F1, F2, F5 and F6 went unnoticed.


### A.TOOL — Other tools & lifecycle framework (seal-tools, tuner/quest/gem/shop/spammer)

1. **Per-run context with cancellation everywhere.** Pass a `RunCtx { token, clock, display: DisplayProvider, port }` to the session. Blocking calls (`GamePort::request`, closed-loop `place`, OCR retries) then take `&CancelToken` and check it in their loops. This fixes F4 and F8 and the stop latency during placements (placement internals not reviewed; low confidence).
2. **Supervisor and worker invariants as RAII.** Use a `DoneGuard` (F5), a `LeaseGuard` (F10) and generation-tagged timers (F9). Encode "exactly one `Done` per spawn" as a type and test it with a panicking fake tool.
3. **Typed config at the tool boundary.** F1 and F12 come from JSON-pointer string paths and `.unwrap_or(default)`. Deserialize each tool's block into the `seal-domain` structs once (`serde`, with `deny_unknown_fields` where possible), refuse malformed values at start, and build test configs through the real merge, not hand-written JSON.
4. **Fail closed on the destructive side.** Write the rule into each read-loop tool: Retry/Combine/Sell must only follow a confirmed read (F7, F15). Uncertainty should stop the run.
5. **Integration-level tests on top of the unit tests.** Run each tool through `runner::run` with fake effects, a fake clock and an injected cancel. Add proptest state-machine tests for the arbiter (random start/stop/request/release/finished sequences; invariants: at most one game holder, `current` is never the resident, every stop eventually yields `finished`) and for the spammer scheduler (F12). F2 and F6 would have been caught by a scripted end-to-end test asserting the click count and guard timing.
6. **Wire or delete the unused safety primitives** (`HeldInput`, `Cleanup`, `SpammerRun::resume`, the stand-down hook). The spammer yield is documented in `OPEN.md §2`: implement it with `GamePort::should_stand_down` polled each tick, plus `SpammerRun::resume`, instead of cancelling the spammer.


### A.HW — Board link, Windows layer, config store, firmware

1. **One durable-write primitive and one merge policy** in seal-config. Today there are three writers (`file::write_text`, `ConfigStore::write_atomically`, `write_artifact`) and four `merge_overlay` copies (store, layout, seal-core validate, the projection). Consolidate them into `atomic_replace` (F5) and `merge_with_policy` driven by registry metadata (record-map vs field-merge, F1). Then the protocol's merge semantics can be tested in one place.
2. **Config schema.** Add `schemars`-derived JSON Schema from the seal-domain typed configs, checked at load and at `config.apply`. Ship the schema in the release so editors such as VS Code + YAML LSP validate hand edits. Put a `config_version` with explicit migration steps alongside it (ANALYSIS §7.3).
3. **Comment preservation (optional).** If prose in setup files matters, move only the setup-file writer to a CST-preserving editor such as `yaml-edit` or `saphyr`'s span/marker API. Otherwise keep `serde_norway` and leave the explicit "comments are lost" header in place, which is honest as it stands.
4. **Board session actor plus protocol level 3** (F7, F8, F10): a long-lived, reconnecting serial owner with a write deadline, keepalive while holding, abort, and an optional ack/sequence number. Framing stays line-based ASCII. A CRC is unnecessary over USB CDC, but ack plus sequence gives command-completion and Stop semantics that the current fire-and-forget protocol cannot.
5. **A raw `Frame` type through capture → OCR**, with PNG only at the edges (F12). A future WGC/DXGI backend then drops in behind `capture_client_frame` without touching consumers.
6. **Placement as a small controller** with per-axis gain estimation and settle detection tuned to move size (F9), with fake-cursor tests for nonlinear gain. This is where the most user-visible accuracy and latency is available.


### A.SHELL — Flutter shell, packaging, CI & release

1. **Process supervisor object.** Move spawn, handshake, close, terminate, exit-code and stderr-drain into one `CoreSupervisor` that owns the `Process` for its whole life, with the invariant "every spawned process is either referenced or killed". `ShellModel` should only see states. This fixes F1 and F4 and makes Restart provably single-process. On the native side, consider assigning the core to a Windows Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, set in the runner, so a shell crash or hang can never orphan a core.
2. **Split `ShellModel` (1,968 lines).** Separate the connection/supervisor, tool commands, config drafts, window/dock placement and calibration actions into smaller `ChangeNotifier`s, each with its own listeners (fixes F5). Expose them through `InheritedNotifier`s or a lightweight DI layer.
3. **Heavy payloads off the UI isolate.** Treat `png_base64` as a typed `Uint8List` that is decoded once, preferably in `Isolate.run`. Longer term, add a binary side channel (a file path in `<root>/captures/tmp` or a named pipe) for frames instead of base64 inside NDJSON. PROTOCOL already notes "`bgra` is a later, measured option".
4. **One release pipeline.** CI on every PR covers fmt, clippy, test, deny, `flutter analyze/test` and `publish -Selftest`. A tag-triggered workflow then builds with `--locked`, signs, generates the SBOM (`cargo cyclonedx` plus a pub list) and notices, writes SHA256SUMS and provenance attestation, and creates a draft GitHub release. The local `publish.ps1` audits stay as the shared library both paths call.
5. **Robustness policy for the protocol.** Unknown enum values in events should map to `unknown` variants, not fatal errors. Only an unparseable envelope or a response that cannot be correlated should close the connection, and that close must also kill the process.
6. **l10n and accessibility pass alongside the label work** already planned in PLAN-V3-UI-UX phase 4, so the zh-TW guide and UI share ARB keys.

No finding here touches anti-cheat or detection evasion. All recommendations are engineering-quality only.


## Appendix B — Merged duplicates

| Reported as | Implement under |
|---|---|
| TOOL-F4 | CORE-F1 |
| TOOL-F5 | CORE-F3 |
| PET-F3 | CORE-F4 |
| OCR-F8 | CORE-F7 |
| TOOL-F9 | CORE-F6 |
| CORE-F16 | PET-F12 |
| OCR-F7 | HW-F12 |
| OCR-F17 | TOOL-F15 |
