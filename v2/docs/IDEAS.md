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
- ~~**Harden the empty check**~~ — **done** [2026-09-10]: the check only judges while the game is the
  foreground window, and logs a refusal otherwise. See [CALIBRATION.md](CALIBRATION.md).
- **Game-window watchdog** (S). If the window closes or minimizes mid-run, stop cleanly.
- **Passive watcher — death and pet notifications** (L) ⚑. Poll the game while you are not looking and
  either act or tell you: **death** by OCR on a calibrated region, **pet needs attention** by a pixel
  diff on a menu-bar icon. Designed in [PLAN-WATCHER.md](PLAN-WATCHER.md) — it is a launcher-level
  service rather than a tool card, because it must poll *without* holding the Arduino and only take the
  port in order to act. Notify-only is the recommended first phase.
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

- ~~**`GemPointer` → `HidPointer`** (S)~~ — **done** (2026-09-11), the tuner now reuses it for spring placement.
- **Split `WindowFinder`** (S). Window queries, cursor helpers and diagnostics in one class today.
- **Split the calibrator out of `MainWindow`** (M). `MainWindow.xaml.cs` is ~3 550 lines, and the
  calibrator's state is already grouped by comment — "Tuner calibrator" and "Gem calibrator" fields
  at `MainWindow.xaml.cs:106-142`, then the preview/drag/save methods further down. A `partial class
  MainWindow.Calibration.cs` would be a pure file move (the compiler verifies it completely) and
  takes roughly 40% out of the file. **Assessed 2026-09-13 and deliberately deferred**: it is
  organisational only, the diff is ~1 400 moved lines that are hard to review, and it cannot be
  verified behaviourally without a live game — so it wants doing on a day with time to run a full
  calibrate, and ideally together with the XAML move below so the churn happens once.
- **Move the static UI to XAML** (L) ⚑. The launcher builds ~900 lines of controls in code. XAML +
  bindings would let the UI cleanup land **once**; doing the cleanup first means doing it twice.
- **More tests** (M). Config round-trips for the new fields, route resolution, the empty metric.
- **One place for log/config paths** (S).

## 7. Release and packaging

- **Refresh `dist/`** (S). ~~The copies are v2.1~~ — `publish.bat` now writes `dist\SealTools-v2.3.zip`
  and `dist\SealTools-v2.3-local.zip` (2026-09-11). The old v2.0/v2.1 folders and zips are still there;
  delete them once the v2.3 release is confirmed.
- **Decide whether the zip ships `local.yaml`** (S) — **decided**: two publish modes, `public` (templates
  only) and `local` (full calibration). See [publish.bat](../publish.bat) and [README.md](../README.md).
- **Changelog** (S). One file, newest first, so a release note is a copy-paste.

## 7b. Spammer as a key pad (nice-to-have, designed not built)

Idea from review: replace the key/delay rows with a small **key layout** — click a key to include it
in the preset, and have it light up (or flash) while the spammer is pressing it.

**It is cheap, because two things already hold:**

- The spammer already publishes the key it is firing — `SkillSpammer.cs:97` sets
  `State.Current = k`, which the tool card prints as `Current:`. The highlight needs a faster poll
  than the card's 750 ms, nothing else.
- The Arduino now presses any printable character plus `F1–F12`, so the pad can show a real keyboard
  layout — letters and digits as themselves, with the function keys separate. Unsupported keys stop
  being typeable rather than being rejected at runtime. (Before 2026-09-13 this was a fixed 20 keys,
  digits `0–9` and `F1–F10`; the pad is easier to design now that it is a keyboard.)

**Sketch**

```
Keys — click to include

  1  2  3  4  5        digits, lit when in the preset
  6  7  8  9  0
  F1 F2 F3 F4 F5       function keys
  F6 F7 F8 F9 F10

  click        add / remove the key
  right-click  toggle the fast tap (the '*' in the raw list)
  caption      its cooldown, e.g. 0.2s
```

**Open questions before building**

- **Where do the cooldowns get edited?** A tiny field under each key is cramped; the alternatives are
  a "select a key, edit its delay beside the pad" panel, or keeping a compact list under the pad for
  numbers only. This is the one thing the current row UI does *better* than a pad.
- **Flash or steady highlight?** `State.Current` holds the last key fired until the next one, so a
  steady moving highlight is what the data honestly supports; a flash would be a timer illusion on
  top. At 0.2 s cooldowns a flash also risks looking flickery.
- **The `*` flag** needs a place — right-click is undiscoverable on its own, so it probably wants a
  visible marker on the key plus a mention in the hint.
- Keep the **raw `key:seconds` editor** (Advanced) either way: it is the only way to set up a preset
  in bulk, and the only escape hatch if the pad ever can't express something.

**Effort:** moderate — ~20 toggle buttons, the delay control from the question above, a faster status
poll, and the highlight style. No new plumbing, no firmware change.

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
