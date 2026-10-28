# v3 — open work

What is genuinely not done in v3, and where it lands. Like v2's backlog, this is decided or named
work — not ideas. **Nothing gets an entry just because it is hard; when a thing ships it leaves this
file** and its reasoning graduates into `spec/` (dated history lives in `records/PROGRESS.md`). The
cut-over steps themselves are [plans/CUTOVER.md](../plans/CUTOVER.md).

## 1. Needs a player measurement or the game

- **Live verification per stage.** The repo's rule: a tool is done when it has run in the game, not
  when it compiles. No card has yet been watched end-to-end against the game in v3, and no calibrator
  pass has been taken from the shell UI. This needs the player and the game (PLAN §8).
- **Global hotkeys — do not build until measured** (PLAN §6.5). The core never owns the foreground
  window, so whether a core-side `GetAsyncKeyState` poller works in game is an open measurement; the
  decision decides between core-side polling and shell-side commands. **v3.0 ships without a global
  poller and says so** — Stop and the shell's buttons always work. This is why there is no pause
  semantics either (v2's `PauseRequested`/`IgnoresQuitHotkey` rules become relevant only with the
  poller).
- **OCR parity recordings** (PLAN §13.10): the archived corpus covers the tuner's full-region path
  only; the feeder-count and pet-panel reads need raw crops recorded from a live session, per read
  path.

## 2. Named code follow-ups (built around, not hidden)

- **Spammer runner-side stand-down.** `GamePort::should_stand_down` exists and the supervisor cancels
  a PREEMPTED spammer through the arbiter (v2's [YIELD] behaviour); the runner loop has no yield hook,
  so a running spammer is stopped rather than asked to step aside in place. The deferred start's 45 s
  timeout is a `log` warning because the caller was already answered `Deferred` (this protocol has no
  channel to update it).
- **Single-instance cross-process lock.** The save gate is in-process; two cores on one root can
  interleave (PROTOCOL §1's lock is specified, not built).
- **OCR no-retry gap.** v3's pipeline scans ONCE; `tuner.ocr_retries` exists in config but is unused,
  so v2's last-retry forced capture and its `refused:` diagnostics branch are not implemented
  (`ocr_errors.jsonl` records a completed-but-unconfirmed scan instead). The grade colour fallback and
  the `remaining`-band read (v2's) are also unported: `remaining` is null and the grade comes from the
  line only.
- **Pet feeder reader follow-ups.** v2's feeder-count 3x/9x/24x upscale ladder is not ported
  (`scan_rgba` has no upscale parameter; one pass per slot plus the v2 warm-colour verdict). There is
  no close-button "window already open" probe and no food-cell re-mark from the bag scan
  (`cells_holding` is ported; the configured food list is used until exhausted). `ToolStatus` has no
  schedule field, so the schedule line travels in the rows' `next_action_in_seconds`.
- **Tuner wording.** The guard's recentre path surfaces a failed placement as the effects' own line
  rather than v2's composed "cursor wouldn't re-centre — <detail>" (the runner performs effects; the
  tool cannot see their failure).
- **Tuner grade-colour fallback (v2 `DetectGradeColorScores`) is not ported.** v2 used
  `tuner.ocr.grade_area` for two jobs: selecting the grade TEXT line (now wired in v3, region-relative)
  and deciding the grade by COLOUR when the text was unclear. v3 does the first and not the second; the
  fallback belongs with the calibration-feature plan's tuner increment. The colour palette
  (`tuner.grade_colors`) is already config and readable.
- **Clock trait duplication.** `seal-tools::clock::Clock` and `seal-board::clock::Clock` are the same
  one-method seconds-only trait (seal-board does not depend on seal-tools); move it to `seal-domain`
  when convenient. The seconds-only unit is deliberate (the eleven-minute-sleep incident).

## 3. Session-shaped features (named, not marks)

- **The tuner's three-box calibration session** — v2 dragged grade area + attribute band + remaining
  band and DERIVED `region`/bands/`row_height`. The one-rect kind was withdrawn rather than faked; a
  session-shaped calibration is the only way to keep the derivation.
- **The tooltip's two-capture hover session** — measure the offset by parking the cursor, hovering and
  capturing, then boxing the panel. A single frame mark cannot express it.
- **The pet's capture session** — reference crops, new row/queue entries and food-cell re-marking from
  the live bag (the close-X crop is learned by the run); the Pet tab edits the setup the config already
  carries, not the captures.
- **The environment-check (Detect) surface** — `CalibrationCheck` is ported and tested in the domain
  and `calibration.save`'s optional `display` argument records the environment, but there is no shell
  canvas and no protocol path that SHOWS the comparison; today the player recalibrates manually after
  an environment change.
- **Mini mode / shell layout questions** — the shell has a stub; the ⚑ questions in
  [PLAN-V3-SHELL.md](../plans/PLAN-V3-SHELL.md) still decide the layout.

## 5. Parity gaps from the v2-to-v3 audit (2026-10-28)

The full matrix and evidence live in [records/PARITY-AUDIT-2026-10-28.md](../records/PARITY-AUDIT-2026-10-28.md).
The gaps still to restore, in the audit's priority order:

**Restored since the audit** (verified in the committed shell/core): the Sell grid's **row 1..8**
shortcuts, **Select all**/**Clear** and three-state cap status line; the **over-cap refusal** (A6 —
the deduped, valid selection is compared with the cap and the start is refused before any click); the
**always-on-top pin** with its portable `settings.ui.always_on_top`; the **remembered window
placement/size** (restored after `hello` from the machine `ui` block, with debounced `ui.window`
saves and an off-screen rectangle clamped into the primary work area); the **Spammer card Rotation
picker**; the **last-preset / last-flow deletion guards**; the **Buy row picker** (1-based labels,
0-based stored, legacy out-of-range marked); the **Arduino connection diagnostics** (read-only
`board.details` + the rebuilt Connection/ports/firmware card); the Tuner's **Clean up capture images**
maintenance row (confirmed, deletes only `logs/captures` PNGs, works mid-run); and the **Pet Feeder's
run-state persistence** (`food_slots_used` and per-row `boarding_running` are written mid-run, so a
restart no longer resumes on stale state). The pin is the portable `settings.ui.always_on_top`;
placement is independent machine state in `ui` and is not the pin.

- **Buy/Sell dry runs** (click nothing; verify targeting) and **Quest Test one loop** - v2's two
  pre-run safety tests; absent in v3 (need run-argument commands).
- **Pet tab session surface** - capture/scan food cells, queue pets by icon capture, bank scans, test
  read, and the Setup-so-far / Ready-to-run checklists (v2's Pet-tab maintenance workflow).
- **Tuner card live detail** (grade/remaining/attempt/attributes - needs a ToolStatus/protocol
  extension); **Spammer advanced raw-key editor**.
- **Calibration plan additions**: the Pet tab's Setup-so-far checklist; loading the saved overlay
  PNG when a calibrate tab reopens; one decided home for the Gem Move mode.

## 4. Release inputs

- **DirectML redistribution is RESOLVED and recorded**: Microsoft's license terms for DirectML,
  clause 1(a), permit distributing the unmodified DLL inside Windows applications; the payload ships
  `docs/THIRD-PARTY-NOTICES.md` and `-Release` still requires the explicit acknowledgement flag.
- **Capture/hide lifecycle is CLOSED**: the runner's `seal/host` channel and the Calibrate
  hide → one frame → capture → show path landed (`CUTOVER.md` decision 8), so calibrating from the UI
  works.
- **Sell-slot selection is CLOSED (v2 parity restored)**: the per-run selection is implemented —
  the Sell tab's 8×8 grid is memory-only, the **Sell card's Start** sends it as `tool.start`'s `slots`
  (PROTOCOL §12.6), and `buy_sell.sell_slots` is never read by a run (migration-compatible field only).
  No open sell-slot work remains.
- **What remains is not a release INPUT**: the player's live acceptance pass (the repo rule: a tool is
  done when it has run) and the publishing steps (merge/tag/GitHub release/root README) listed in
  [CUTOVER.md](../plans/CUTOVER.md)'s publishing checklist.
