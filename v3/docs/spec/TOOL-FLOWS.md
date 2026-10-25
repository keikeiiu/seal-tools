# v3 TOOL-FLOWS — what each tool does now

**Status: decision halves landed and tested in `seal-domain`; effect loops not built.** This is the v3
truth for what each tool decides and promises. v2's
[TOOL-FLOWS.md](../../../v2/docs/spec/TOOL-FLOWS.md) is the RECORD it supersedes — read it for how v2's
loops actually ran. Every string quoted here exists in the landed code; the full catalogues are the
`seal-domain/tests/*` files named per tool. What is NOT built is named per tool and collected in
[OPEN.md](../backlog/OPEN.md) §1 and [PORT-COVERAGE.md](../records/PORT-COVERAGE.md) §3 (gaps G1–G3).

The shared lifecycle is [PROTOCOL.md](PROTOCOL.md) §5/§9 and the `seal-tools` crate; the shape of a
decision is [PLAN-V3-CORE.md](../../../v2/docs/plans/PLAN-V3-CORE.md) §6.4: the tool half **decides**
(`Click`/`Key`/`Wait`/`Stop(reason)` or an ordered plan type), the loop **performs** effects, and the
reason carries the player's line.

---

## The shared frame (one fact, one place: `seal-tools`)

| Piece | Where | What it gives a tool |
|---|---|---|
| One game, one tool | `arbiter.rs` | A second `tool.start` while another runs is **refused**, not auto-stopped (a deliberate v2 difference — PROTOCOL §4/§6). Refusal lines: `"{holder}" is using the game right now — stop it, then try again.` and, for a start arriving as the last run leaves, `"{tool}" is still stopping — wait for it to finish, then start again.` |
| Phases | `tool.rs` | `idle` / `starting` / `running` / `stopping`; `engaged()` = anything but idle, which is the toggle rule (`ToolPhase::engaged`, v2 `ToolToggle`'s re-expression). |
| Resident exception | `arbiter.rs` | The pet feeder is the one tool whose start stops nothing and whose run another start does not stop. Its card keeps a standing line while it defers; the DEFERRAL RULES are the arbiter's, the pet half supplies only facts (`PetFacts`). |
| Held inputs | `held.rs` | A press that must not survive the tool is an RAII guard: released on normal exit, early return, panic and cancellation. Released **LIFO** (v2 wrote `U` then `l`; recorded divergence, [README §divergences](../../core/README.md)). Failure lines: `Could not hold the spacebar: {error}`, `Could not hold the left mouse button: {error}`, `Could not release the spacebar — tap it once in game ({error})`, `Could not release the left mouse button — click once in game ({error})`. |
| Cancellation | `cancel.rs` | Stop abandons the WORK, not the CLEANUP — and it is a TYPE: cleanup receives `Cleanup`, which has no cancellation check. |
| The seam | `runner.rs` | `run(tool, effects, clock)`: publish running, decide → perform → check stop. An effect failure ends the run as an error with its line; a bare failure uses `A board or capture action failed.` |
| Ids | `tool.rs` | `spammer`, `holdspace`, `buy`, `sell`, `tuner`, `gem`, `quest`, `pet` (PROTOCOL §9). |
| Not wired yet | OPEN §1/§3; `registry.rs` | `ProductionRegistry` refuses every tool with `"{tool}" is not available in this build yet — its tool loop is not ported.` until its loop exists; the supervisor's interim hand-over line is `"{waiter}" has the game.` (OPEN §1 item 5/§2). |

Each loop still needs: the reading side (seal-win/seal-board/seal-ocr, G1–G3), its own loop body
(OPEN §1 items 1–5 cover opening phase, quit policy, pause decision, the pet wait timeout and the
tool-side waiting strings), and the card's live rows (inner shape still unpinned — OPEN §2).

---

## Skill Spammer — `spammer`

*Presses a cooldown-governed rotation — every viable combo first, then the singles — and stands down for
another tool when one is waiting; it has no natural end.*

**Flow (decisions in `tools/spammer.rs`).** `SpammerRotation::new` (preset's ordered `(key, cooldown)`
list) → `SpammerPlan::plan_or_stop` (refuses a missing preset) → `SpammerRun::start`/`resume` →
`SpammerRun::tick(now)` repeatedly: the opening (`lead_in_remaining`/`opening_at`) runs the ordered
openings, then a combo that is due owns the tick until `runnable_combos` says its last step went out,
`priority` allows at most one press per tick (the rest are delayed, not dropped), and `singles` are the
preset minus every key a combo owns. `SpammerPress`/`BoardPress` carry the write; `unsupported_message`
reports keys the board cannot send.

**Player lines (verbatim).**

- `The active preset '{active}' was not found — pick one in the Spammer tab.`
- `No spammer presets are configured — add one in the Spammer tab.`
- `Spammer key '{stripped}' unsupported — use one letter or digit, or F1–F12.`

**Guards.** Combos are atomic ("no filler, no other combo" between their steps); an unusable key is
advanced past rather than retried every tick; a leading `*` is the fast tap (same press, lowercase).
**Yield** is `arbiter.should_stand_down` (landed) — the spammer is the only holder that acts on it.
**Loop not built:** the board writes (G2), the yield/claim calls at the boundary, and the dead-port stop
(a write error becomes the runner's error line). Card rows: current key/attempt are the loop's.

## Magic Tuner — `tuner`

*Clicks the 發條 button through the Arduino, OCRs the result panel, and repeats until the target grade
and filter are met — stopping on success, an unreadable panel, a repeating result, the retry budget, a
cursor-drift guard, a write fault, or Stop.*

**Flow (`tools/tuner.rs`).** `TunerRun::begin_attempt` starts the 5-second `START_COUNTDOWN` and the
attempt; `TunerRun::decide(&TunerRead{grade, remaining, attributes})` returns `TunerOutcome{decision,
filter_status}` where `decision` is `TunerDecision::Retry` or `Stop(StopReason)` — the stop kind and its
line are the decision half's, so the card text is testable without a game. `SpringMode::parse` +
`spring_step` place the cursor when `spring_mode: hid`; `mouse_guard_step`/`after_recenter` own the
`off`/`stop`/`recenter` behaviour.

**Player lines (verbatim; the `>>>`/`[!]` wording is v2's and is kept).**

| Outcome | Line |
|---|---|
| target + filter | `>>> {grade} REACHED + FILTER MATCH <<<` |
| target, no filter | `>>> {grade} REACHED (no filter) <<<` |
| filter, no grade requirement | `>>> FILTER MATCHED at grade {grade} (no grade requirement) <<<` |
| out of springs | `>>> OUT OF SPRINGS <<<` |
| same result three times | `>>> STUCK / OUT OF SPRINGS (same result x3) <<<` |
| retry budget | `[!] Max {n}` (v2's short form, unchanged) |
| unreadable panel | `Read failed — nothing recognised in the OCR region.` |
| guard stop | `mouse moved — stopped` · `cursor keeps drifting — stopped` |
| guard recenter failure | `cursor wouldn't re-centre — {detail}` |
| hid without a point | `spring_mode is hid but tuner.spring_point isn't calibrated — click the 發條 button in Calibrate Tuner` |
| window/placement | `game window not found — couldn't place the cursor on the spring` · `couldn't place the cursor on the spring — {detail}` |

**Guards.** Cursor guard modes and the recenter budget; a signature repeated three times stops; the
retry budget stops; the filter's `any/all/per_attr` result gates a success. **Loop not built:** capture
and OCR (G3), `C`/`E` writes (G2), the countdown ticker and card rows (grade/attempt).
`GradeCycles` is landed for the per-grade tally.

## Buy — `buy` and Sell — `sell`

*One pass, then stop — counted jobs, not loops: running them forever would spend a fortune or empty a
bag by leaving the room.*

**Flow (`tools/shop.rs`).** Both are built as an ORDERED PLAN, not one decision at a time.

- **Buy:** `ShopPlan::buy(settings, preset, count)` → `ShopGeometry` derives every row centre from the
  calibrated list region; `focus_steps` clicks the focus point; one `ShopStep::Scroll{notches, down}`
  applies the preset; then `transaction(row, max)` per run count — `Place`, right `Click`, `Place(MAX)`,
  `Enter`, `Enter` — finished by `Bought {count}x '{name}'.`
- **Sell:** `ShopPlan::sell(settings)` → `sell_slots_to_sell` (deduped, sorted high-index-first, capped by
  `sell_cap`) → the same transaction per slot, finished by `Sold {n} slot(s).`

**Start refusals (verbatim).**

- `The bag grid isn't calibrated — Calibrate Buy/Sell.` · `No slots are selected to sell.` ·
  `{n} slots selected but the per-run cap is {cap}. Raise the cap on purpose, or narrow the selection.`
- `The focus point isn't calibrated — Calibrate Buy/Sell. The game ignores a click while it is unfocused, and it is unfocused because you just clicked this window to start the run.`
- `No buy preset selected.` · `Buy preset '{name}' no longer exists.` ·
  `Shop rows aren't calibrated — {problem}.` (problem: `the shop list region isn't drawn` /
  `the configured row count is 0`) · `The scroll point isn't calibrated — Calibrate Buy/Sell.`
- Mid-run focus failure (a DIFFERENT sentence): `The focus point isn't calibrated — the game has to be focused before it will take a click or a wheel notch. Mark it in Calibrate Buy / Sell.`
- `The MAX button isn't calibrated — Calibrate Buy/Sell.` ·
  `Couldn't work out the shop row — recalibrate the two row marks.` ·
  `Game window not found (or minimized) — open and restore the game first.` ·
  `couldn't place the cursor at ({x},{y})` (fallback placement detail).

**Guards.** The sell cap is enforced in the PLAN, not advised; the focus click happens before anything
that needs focus; a preset's scroll means "notches down from the top" and the list being at the top is
the player's setup (CALIBRATION.md §1); one MAX mark serves buy/sell and the pet count dialog. **Loop not
built:** board clicks/keys/wheel (G2), the foreground check (G1), and the card's live attempt line; the
Buy card's item/count controls are the shell's (they seed the run's `count`, not the preset's).

## Gem Composer — `gem`

*Selects a grade, moves to Register, combines, and on an empty result box (or a cleared slot) advances
N → G → DG once; it ends by itself when the last grade is done.*

**Flow (`tools/gem.rs`).** `GemRun::new` (grades configured) → `select_grade_turn` (route
`radio_<grade>`, from `GemRoutes::resolve`) → `combine_turn` (register→combine, click) → the caller reads
the result and calls `finish_turn(empty: Option<bool>)`: an empty result increments `empty_streak` and,
once it reaches the setting, the run advances (or clears the resource slots first per `EmptyMode`);
`advance_grade_by_hand` is the explicit Next button. Last grade done →
`All grades done (last was {grade}) — composer stopped.` `MoveMode` chooses HAND-TUNED HID counts or the
`arduino` move set (cursor placed on each route's destination point). Empty detection is
`result_box_is_empty` (crop diff) with `composition_is_empty` the fallback; the crop's provenance and the
6-px inset rule are CALIBRATION.md §5.

**Player lines (verbatim).**

- `No gem grades are configured — the composer has nothing to work through.`
- `All grades done (last was {grade}) — composer stopped.`
- `{what}: unknown route '{route_key}'.` · `No route Resource3 → {other}.`
- `{what}: '{destination}' isn't calibrated yet — open Calibrate Gem and save it.`
- `{what} isn't saved yet — open Calibrate Gem and press Save Composer Moves.`
- `Grade position '{grade}' isn't calibrated yet — open Calibrate Gem and save it.`
- `Game window not found (or minimized) — open and restore the game first.`
- `Couldn't move the cursor onto the {grade} button — {detail}. Stopped instead of clicking blind.`
- `{what}: couldn't move the cursor to {point_name} — {detail}.`

**Guards.** Every point/route is checked before use; `empty_mode: stop` never auto-advances; the
`empty_streak` gate prevents one read from advancing a grade; a placement failure stops rather than
clicking blind; the game gate is claimed by the launcher (resident boundary). **Loop not built:** the
moves themselves (`D` counts or placed moves, G2), capture/colour analysis (G3), and `gem.log` cycle
lines; `GradeCycles` is landed.

## Quest Hand-in — `quest`

*Replays a fixed, player-built flow of clicks and keypresses at the quest NPC a set number of times, and
ends by itself when the loops run out — or when the mouse moves past the guard, or on Stop.*

**Flow (`tools/quest.rs`).** `QuestPlan::plan(quest, sequence_override, flow_override, loops_override)`
validates first (nothing has been clicked yet): a sequence's entries must name existing flows, a flow
must have steps, each step is checked by `quest_step_complaint`. The plan carries the schedule, the
positioning countdown (`positioning_counts`/`positioning_text`), the passes, and the guard width. At run
start `guard_reference` reads the cursor once; each step is a `QuestCommand`
(`Click`/`RightClick`/`Enter`/`Key`/`NoInput`) with its delay; `guard_problem` stops on drift;
`stopped_on_loop` / `stopped_in_sequence` name where it stopped; a clean finish is
`DONE — {n} loop(s) of "{name}"` or `DONE — {n} master loop(s) of "{name}"`.

**Player lines (verbatim).**

- `There is no sequence called "{name}" — pick another on the card, or reload the config.` ·
  `There is no flow called "{name}" — pick another on the card, or reload the config.`
- `The sequence "{name}" has no entries — add some on the Quest tab.` ·
  `The sequence "{name}" names a flow "{flow_name}" that does not exist.` ·
  `The flow "{flow}" has no steps — add some on the Quest tab.` ·
  `Flow "{flow}" step {n}: {problem}`
- `move the mouse into place — {seconds_left}s`
- `The cursor couldn't be read, so it can't be watched — stopping rather than clicking at a place nothing is checking.` ·
  `The cursor stopped being readable, so it can't be watched any more.` ·
  `The mouse moved — it is at ({x},{y}) and started at ({start_x},{start_y}).`
- `{reason} Stopped on loop {loop}/{loops}.` · `{reason} Stopped during {flow} {pass}/{times} of master loop {master}/{loops}.`

**Guards.** It READS NOTHING — no capture, no OCR, no window lookup — which is why it has no
calibration; the positioning window is the only positioning, and the cursor guard is the only
side-effect check. A wrong flow does `loops` wrong actions and only the player can notice, which is what
the test loop is for. **Loop not built:** board writes for the five commands and the interruptible
waits (G2/Clock), plus the self-end teardown rule (OPEN §1; v2 TOOL-FLOWS §1.3).

## Pet Feeder — `pet`

*Keeps each ticked breeding row fed on its own per-row clock by reloading the feeder before the pet runs
out; it loops for days, and it is the one tool that ignores the quit hotkey.*

**Flow (decision half: `tools/pet.rs` + `pet/decision.rs` + `pet/food_load.rs`).** `ready` refuses a
half-calibrated setup (below). The opening look sets the schedule: per row, the slot reads empty /
occupied / unreadable (`ReadRows` contract), `FeederEta` reads the game's own time line (completion form
schedules, level form is display-only), `FeederCount` reads the tray under its score gate and two-crop
agreement, `FeederLayout` derives the crops. `schedule_for` turns one the row's read into a
`RowReading` (six sources — StartOverride, NotBoarding, CompletionRead, CountRead, CountEmpty,
CycleFallback) in **minutes from now**, not a wall clock. A due row goes through `consider_veto` (skip
only on food SEEN) and then the reload's steps; `fold_back` folds the outcome into the next time and the
card: reloaded / deferred / nothing-to-board / failed (three strikes drop the ROW, not the run).
`build_card` publishes the standing line and one line per row; `PetCard::facts()` is the ONLY thing the
arbiter gets (wants the game now, next action, card) — the resident/deferral rules stay in `seal-tools`.

**Player lines (verbatim; every one is pinned in `tests/pet_decision.rs`).**

- Start refusals (13, in `ready`, in this order): `目錄 isn't calibrated — Calibrate Pet.` ·
  `The pet feed icon isn't calibrated — Calibrate Pet.` ·
  `The boarding window's X isn't calibrated — Calibrate Pet.` ·
  `The boarding bag's grid isn't calibrated — Calibrate Pet. (Its own, not the shop's.)` ·
  `The bag page tabs aren't calibrated — Calibrate Pet.` ·
  `No breeding row is set up — Calibrate Pet. Each row needs its start/end button and its pet slot marked.` ·
  `Every row is unticked — nothing to feed. Tick a row on the Pet tab.` ·
  `Row {n}'s start/end button isn't marked — …` · `Row {n}'s food strip isn't drawn — …` ·
  `{what} is marked on bag page {p}, but only {t} page tab(s) are calibrated — …` ·
  `Neither a return slot nor any queued pet icon is set — …` ·
  `No food cells are marked — Calibrate Pet. Nothing says where the pet food lives.` ·
  `No MAX is calibrated — Calibrate Buy/Sell, or mark one on Calibrate Pet.`
- Card: statuses `given up` / `boarding` / `waiting for a reload`; detail `{pet}  ·  → +{growth}  ·  {rate}/min (game line)`;
  schedule line `boarding {rows}  ·  next {row} at {HH:mm}  ·  {n} more queued` or `no rows scheduled`.
- Schedule/reload: `Checking what is already running…` · `Opening the breeder for {n} rows…` / `{row}: opening the boarding window…` ·
  five step lines `{row}: reloading…`, `ending boarding…`, `placing the pet…`, `loading the food…`, `starting boarding…` ·
  `{row} reloaded. Next {HH:mm}.` + ` (the pet finishes first, in {min} min)` ·
  `{row} is waiting for the boarding window to open.` · `{row} is waiting for a pet that can be fed.` ·
  `{row} failed {strikes}/3: {error}`
- Guards/aborts: `The game window isn't in front — {row} keeps its place; nothing was clicked, and it is tried again in 5 min.` ·
  `The game window left the front while reading the rows — trying again in 5 min.` ·
  `The pet did not go in after 3 right-clicks — the slot still looks empty. Nothing was loaded. Check the return slot on the Pet tab.` ·
  `The pet did NOT come back out of the loader after pressing the end button — …` ·
  `Every pet that could be boarded reads +9/100%. Refused {list}. …` ·
  `The return slot holds no pet, and no queued icon matched one in the bag. …` ·
  `Neither a queued pet icon nor a return slot is set — there is nothing to right-click. …` ·
  `No queued pet is in the bag ({why}). …` · `The bag grid isn't calibrated — there is nothing to right-click.` ·
  `Every row has failed repeatedly — stopped.` · `Stopped before it could read the rows.` ·
  `Stopped during {row} — it may not have been fed. Check it in game.` (and the N-row form).
- Food-load mode (`food_load.rs`): drag vs right-click (`is_drag`), `board_can_drag` (a silent board is a
  definite no), and the two `Food load is set to drag but the board …` complaints.

`…` inside a quoted line marks a long sentence truncated for this table; the exact text is in
`pet/{decision,food_load}.rs` and pinned by `tests/pet_decision.rs`. Every un-elided string above is
verbatim.

**Guards.** Foreground before a visit; the veto is positive-only ("unknown must never cancel a reload");
zero means now; three strikes drop the row; a no-pet bag waits 30 min without a strike; the pet slot's
0.85 "not the window" ceiling; food counts need two agreeing crops above the score gate. **Loop not
built:** capture/OCR/board (G1–G3), the resident claim/wait/45 s timeout (OPEN §1 items 4/5), and the
visit loop's composition of these decisions.

## Hold Space — `holdspace`

*Holds the spacebar down on the board so the game picks items up, and always lifts it on the way out.*

**Flow (`tools/hold_space.rs`).** `tick(mirror_running, panel_running, hotkey_down, hotkey_was_down)` →
`HoldSpaceTick{mirror_running, actions, panel_running, clear_quit}`: the launcher's mirror sets the state,
then the hotkey's RISING edge toggles it and the card follows (`panel_running`). A tick can legitimately
carry TWO writes — a launcher start on the same tick as a stop press sends `P` then `U` (v2 behaviour,
pinned). `HoldSpaceAction::wire()` is `P\n` / `U\n`; `cleanup()` is the `finally`'s owed `Release`; the
steps carry v2's log lines and beeps (`[Panel] HOLDING space`, `[Panel] RELEASED space`,
`[GO] holding space`, `[STOP] released`; 523 Hz/100 ms and 1000 Hz/150 ms).

**Player lines (verbatim).** `Could not hold the spacebar: {error}` and
`Could not release the spacebar — tap it once in game ({error})` — the release line is the dangerous one,
which is why it says what to do.

**Guards.** "Engaged" is `ToolPhase::engaged` (the `ToolToggle` re-expression); the held-input RAII guard
releases on every stop path independently; `cleanup()` must be performed even after a cancellation.
**Loop not built:** the `P`/`U` writes (G2), the toggle/start-stop wiring in the shell/arbiter, and the
beeps; it has no card — its reporting is the Spammer card's surface (PROTOCOL §9).

---

## What every tool still shares, and what is intentionally different from v2

- **Every loop needs the same three things:** its reader crate (G1 `seal-win`, G2 `seal-board`, G3
  `seal-ocr`), its own body (OPEN §1), and its card rows (OPEN §2 pins the inner `rows` shape). Nothing
  in `seal-domain` reads a pixel, opens a port or looks at a window — that is the boundary this document
  keeps naming.
- **Deliberate differences, recorded not re-explained:** a second start is refused (`tool_busy`) instead
  of auto-stopping the first (PROTOCOL §4); held inputs release LIFO and two new button lines exist
  (README §divergences); the pet half speaks minutes-from-now and facts-only to the arbiter
  ([CALIBRATION.md](CALIBRATION.md) §2's "never cache" sibling choice: the clock belongs to the loop).
- **One number to remember when reading the code:** a `StopReason` is not a log line — its message is
  the line the player sees, which is why the decision halves own their wording and their tests pin it.
