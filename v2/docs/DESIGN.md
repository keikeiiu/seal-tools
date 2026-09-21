# Seal Tools v2 — technical design

**How it works, and why it works that way.** This is the document a plan's reasoning graduates into.
[PROGRESS.md](PROGRESS.md) is the dated history and [TODO.md](TODO.md) is what is open; this is the
current truth about the shape of the thing.

Topic detail lives in its own file and is linked rather than repeated — one document per question, so
there is one place to correct when something turns out to be wrong.

---

## 1. The shape

```
SealTools.Launcher      WPF UI. Cards per tool, configuration tabs behind them. Owns the config,
                        the service that starts and stops tools, and every calibrator.
SealTools.Core          Everything shared: the OCR engine, the HID/Arduino link, the config model,
                        the geometry, the icon matcher, the attribute matcher.
SealTools.Tests         xUnit. 172 tests, all pure — no game, no board, no screen.
SealTools.Pet           The resident feeder: schedule, reload, placement, food.
SealTools.Tuner         Seal-breaking: OCR the attributes, decide, click.
SealTools.Shop          Buy/sell.
SealTools.GemComposer   Gem composition: move sets over the Arduino.
SealTools.Spammer       Hold-space and key spam.
```

**One game, one tool at a time.** `LauncherService` owns a single "current tool" record; a second tool
starting defers to the first rather than fighting it for the mouse. The pet feeder is the exception that
shaped the rule — it is *resident*, so its card keeps a standing line while another tool uses the game,
and the rules for that are in §4.

**Directories.** The launcher finds the repository root by walking up for `config/defaults.yaml`, so the
`.exe` can be run from anywhere under it. Calibration images sit in `config/`; evidence — captures and
scan reports — lands in `logs/reads/` beside the binary.

---

## 2. The two engines everything rests on

### 2.1 The mouse and keyboard — the Arduino

The cursor is moved by a **raw HID mouse on an Arduino**, not by `SetCursorPos`. The OS intermittently
refuses the process its own cursor placement, and a game that reads raw input sees `SetCursorPos` moves
as teleports — see [CURSOR-INVESTIGATION.md](CURSOR-INVESTIGATION.md).

- **`D dx dy` counts are RAW HID COUNTS, per machine, and are never scaled.** They are valid until the
  board, the pointer speed or the in-game display changes, and then they are re-measured by hand.
- Placement is a **closed loop**: move, read the cursor, correct, until it is within tolerance or the
  step budget runs out. A failure is reported with both the goal and where the cursor actually stopped.
- **`SleepCheck` takes SECONDS.** The launcher's `Task.Delay` takes milliseconds. Passing one to the
  other sleeps for eleven minutes and looks exactly like a hang — it has happened, once, live.
- The protocol is one command per line: `C`/`R` click, `D` move, `E` Enter, `K`/`F` keys, `Q`/`Z` wheel,
  `L`/`l` hold and release the left button (the two halves of a drag), `V` report the firmware level.
  **`V` is the only thing the sketch ever writes back**, which is what lets the launcher tell a level-1
  board (silent) from a level-2 one (answers) without guessing.
- **A held key must not survive the host.** If the launcher dies while the spacebar or the left button is
  down, the board releases both when the serial port closes.

### 2.2 Reading the screen — the OCR engine

Capture is **`CopyFromScreen`**, never `PrintWindow` — the game returns a black frame to the latter.
This means a capture is of **the SCREEN**, so whatever is in front is what gets read; guards for that
exist in three places and are load-bearing (see §5).

Text comes back through detections whose boxes are grouped into lines and cleaned by the `text_fixes`
table in `attributes.yaml`. What has been measured about it, and must not be re-learned:

- **A crop cut to the digits' height finds NOTHING** while holding a perfectly legible number. Measure
  at the region's FULL height. The mechanism is not known; the rule is measured on three crops.
- **A cramped crop does not fail blankly — it lies.** A clipped `138` came back as `3` at 0.99, the same
  confidence as a correct read. This is why a count is only accepted above a score gate, and why the
  feeder-count band is a few pixels wide.
- The engine upscales before recognition (×3 by default); the saved debug image is at the upscaled size,
  because that is what recognition actually ran on.

---

## 3. Configuration

Three layers, and the split is the whole point — [CONFIG.md](CONFIG.md) owns the detail.

```
config/defaults.yaml   portable, committed. Anything the same on every machine.
config/local.yaml      per machine. Calibration, geometry, presets, run state, everything measured.
config/attributes.yaml the OCR dictionary and the text fixes.
```

**Two traps, both live, both recorded here because both have bitten repeatedly:**

- **A hand-written projection drops a field on LOAD, silently.** `ConfigLoader` maps between the config
  model and the on-disk shape by hand, and a field missing from one side loads as its default and is
  written back blank on the next save. It has happened three times across two sessions. Any new field
  goes in **both** directions, and both directions are pinned by tests.
- **Removing a config field deletes the player's stored value** on the next save. Deprecate by ignoring,
  not by deleting.

---

## 4. The tool lifecycle

```
LauncherService.StartToolAsync(id)      one tool at a time; a second defers
   per-tool record                      what is running, its state, its schedule line
StopTool(id)                            slot-aware: stopping one tool does not disturb another's port
ClaimGame / ReleaseGame                 a tool takes the game for a bounded operation, or waits
```

- **The port is a shared resource with a gate.** Two tools cannot both hold the Arduino; the gate is what
  stops one starting while another is mid-move.
- **A resident tool is one that owns a schedule rather than an operation.** The pet feeder runs for days,
  so its card shows a standing line — what is boarding and when the next action is — and it yields the
  game whenever another tool claims it, resuming its wait afterwards. See
  [archive/PLAN-RESIDENT-PET.md](archive/PLAN-RESIDENT-PET.md) for the reasoning.
- **The pet feeder ignores the quit hotkey** by construction: a press meant for a foreground run must not
  end a schedule that is feeding four pets.

---

## 5. The invariants

The things that would look like improvements to reverse. Part D of [REVIEW.md](REVIEW.md) is the
original list and remains authoritative; this is the short version plus what the last two sessions
added.

**Reading**
- Full height for any digit crop, always — §2.2.
- Never accept a count without its score gate; junk scores 0.1–0.6 against a real read's 0.9–1.0.
- **"Nothing read" and "the thing is empty" are different answers.** An empty feeder slot has no digits,
  so the OCR cannot tell them apart — the pixels must, and a bare cell is 0.0 % warm against 30 %+ for
  one with food in it.
- **A capture reads whatever is in front.** Guard the foreground, park the cursor off the target, and
  refuse to judge a capture that is not the game.

**Acting**
- **Click, then LOOK.** Every risky action checks its own effect — the pet slot after a placement, the
  bag cell after a drag, the toggle after a press. The one that did not, asserted success and lost food
  silently for a while.
- **Unknown boards; unknown does not skip.** A guard may only ever *remove* a boarding. Leaving a pet
  unfed is the one outcome here that cannot be undone, so an unreadable panel means "board it as before"
  and never "skip it".
- **A MOVE is not a CLICK.** A click on a pet in the bag switches the equipped pet, so anything that
  measures a pet — a panel read, a scan — moves and never clicks.

**Scheduling**
- Look first, do not act first: read the state and decide from it.
- **Whichever runs out first** — the food in the tray, or the pet itself.
- **Zero means now.** The "past empty" margin exists for a tray that still has food; one already at zero
  is past it, and waiting five minutes there is sleep for its own sake.

**Evidence**
- Every read saves its crop, every scan saves its report, every failure saves what the reader was given.
  A blank result has two causes that need opposite fixes and only the image separates them — three
  separate wrong theories in one day were each settled by looking at a crop.

---

## 6. The pet feeder

The live feature, and the one with a document of its own:
**[PET-TAB-DESIGN.md](PET-TAB-DESIGN.md)** — the tab, the look-versus-reload split, the schedule and
where its number comes from, the placement and its guard.

Its data comes from **[PET-DATA.md](PET-DATA.md)** — the scraped per-pet feeding values, and the
measured per-level cost that decides how long a pet still needs.

---

## 7. Where the rest lives

| question | document |
|---|---|
| coordinates, DPI, logical vs physical pixels | [COORDINATES.md](COORDINATES.md) |
| why the cursor is not placed with `SetCursorPos` | [CURSOR-INVESTIGATION.md](CURSOR-INVESTIGATION.md) |
| the config split | [CONFIG.md](CONFIG.md) |
| calibrating a machine | [CALIBRATION.md](CALIBRATION.md) |
| the composer's hand-tuned move sets | [MOVE-SETS.md](MOVE-SETS.md) |
| the guardrails, in full | [REVIEW.md](REVIEW.md) |
| the launcher UI, measured (for a next UI pass) | [ANALYSIS-UI.md](ANALYSIS-UI.md) |
| how to use it | [USER_GUIDE.md](USER_GUIDE.md) |
| finished plans, with their reasoning | [archive/](archive/README.md) |
