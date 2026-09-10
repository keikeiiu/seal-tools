# Ideas — a wider backlog

Nothing here is committed work. This is the "what could we improve" list, kept separate from
[TODO.md](TODO.md) (which holds work that is planned or waiting on a decision).

Sizes: **S** ≈ an evening, **M** ≈ a day or two, **L** ≈ a project. **⚑** = decide before building,
because it changes how something else should be done.

---

## 1. Reliability — make a run trustworthy

- **Verify a click landed** (M). After a combine or a register click, check the UI actually changed
  (grade/result box) and retry once. Turns a silent miss into a self-correction instead of a drift.
- **Refuse to run on stale calibration** (S). At tool start compare the live client size — and a
  cheap fingerprint of the captured UI against the calibration screenshot — and stop with
  "recalibrate" rather than clicking points that moved.
- **Harden the empty check** (S). Only trust a verdict while the game window is the foreground
  window; a covering window currently yields garbage (`RGB(26,26,46)` in one real case).
- **Game-window watchdog** (S). If the window closes or minimizes mid-run, stop cleanly.
- **Surface crashes on the card** (S). A tool exception already writes `logs/error.log`; put the last
  error on the card so it is seen, not buried.
- **Auto-screenshot on failure** (S). Capture the window whenever a tool stops with an error —
  post-mortems without reproducing.

## 2. Safety and control

- **Global human override** (M). Grab the mouse and the running tool stops — generalises the tuner
  guard ([PLAN-TUNER-SPRING.md](PLAN-TUNER-SPRING.md)).
- **Run budget** (S). Max cycles / minutes / clicks per run, always bounded.
- **Dry-run mode** (M). Place the cursor and flash where it *would* click, without clicking. The
  fastest way to validate a fresh calibration.
- **Confirm expensive actions** (S). A prompt or a config threshold before combining above a chosen
  grade.
- **A panic stop that works in-game** (M/L). Hotkeys are blocked while the game is focused
  (anti-cheat); a physical button on a spare Arduino pin, or a second input device, would not be.

## 3. Calibration and setup UX

- **"Validate calibration" report** (M). Script every point and region, print a table of what landed.
- **Export / import calibration** (S). `local.yaml` + a version + a machine fingerprint, so a second
  PC is a copy rather than a re-calibration.
- **Drift detection** (M). After a run, diff the live UI against the calibration screenshot and warn
  when the layout moved (a patch, a resolution change, a moved panel).
- **A guided wizard** (M). One pass through Tuner + Gem instead of two tabs.

## 4. Observability

- **Live dashboard** (M). Attempts, grade, remaining, filter verdict, timings — `ToolState` already
  carries most of it.
- **Unify log locations** (S). The launcher writes `v2/logs`, the tools write `<bin>/logs`; pick one.
- **Build stamp** (S). Write the version/commit into each run log so a log says which build made it.
- **Diagnostics tab** (M). Window rects, DPI, port, a sample capture, the empty-check crop and the
  last log lines in one place.

## 5. Throughput

- **Measure this PC's delays** (M). The timing values are config; a step could measure and suggest
  them instead of hand-tuning.
- **Cheaper empty check** (S). It captures the whole box every cycle; a smaller crop or a
  change-detector would cut the per-cycle cost.

## 6. Code health

- **`GemPointer` → `HidPointer`** (S). Already planned; it is the Arduino pointer, not a gem one.
- **Split `WindowFinder`** (S). Window queries, cursor helpers and diagnostics in one class today.
- **Move the static UI to XAML** (L) ⚑. The launcher builds ~900 lines of controls in code. XAML +
  bindings would let the UI cleanup land **once**; doing the cleanup first means doing it twice.
- **More tests** (M). Config round-trips for the new fields, route resolution, the empty metric.
- **One place for log/config paths** (S).

## 7. Release and packaging

- **Refresh `dist/`** (S). The copies are v2.1; `publish.bat` should update them as part of a release.
- **Decide whether the zip ships `local.yaml`** (S, already a TODO decision).
- **Changelog** (S). One file, newest first, so a release note is a copy-paste.

## 8. Game-side mini-features

Already captured in TODO.md: auto-sell, auto-buy, auto-submit missions, anti-AFK nudge.
Additional candidates, same shape (calibrated points + placement + HID click + a capture pass):

- **Auto-pot** — watch the HP/MP bar (colour/OCR) and press a key below a threshold.
- **Buff upkeep** — recast buffs on a timer; the spammer's cooldowns are already the right shape.
- **Auto-repair / auto-storage** — vendor points plus a rule set.
- **Route recorder** — record a sequence of clicks/keys and replay it.

## 9. Multi-account (probably out of scope)

Per-window calibration and a second Arduino/port. The whole design assumes one game window and one
COM port; note it, don't build it unless the need is real.
