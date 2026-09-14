# Watch — passive monitoring and unattended actions

Status: **design only, not built.** Two triggers are agreed and the architecture falls out of a
constraint that already exists in the codebase; the open questions at the end are the ones that decide
how much of it is safe to build.

Goal: notice two things about the game while you are not looking, and either act or tell you.

| Trigger | Detected by | Because |
|---|---|---|
| **Character died** | **OCR** on a calibrated region | You said there is text to read, so the read is the direct signal |
| **Pet needs attention** | **Pixel diff** on a menu-bar icon | The game puts a notification on that icon, and a diff is the cheapest way to see it appear |

---

## The constraint that decides the architecture

**One Arduino port, one tool at a time.** `LauncherService` holds a single `SerialPort`, and starting a
second tool is refused. So:

- **The watcher is not a tool card.** A tool holds the port for its whole run; the watcher must *poll
  without the port* and only take it in order to act. It is therefore a launcher-level service with a
  toggle in the chrome row beside Hold Space — not a Start/Stop card. That also keeps it out of the
  one-tool-at-a-time rule, so it can keep watching while the composer runs.
- **It acts only when no tool is running.** If a tool holds the port, the watcher notifies and defers
  rather than queueing a click.

> Acting *alongside* a running tool was considered and set aside. Two things driving the board means
> interleaving commands on one serial line and deciding which wins — a much larger change, with
> failure modes that are hard to reason about from a log.

## Why this is mostly assembly, not new machinery

| Need | Already exists |
|---|---|
| Read a region as text | `OcrEngine.Scan(OcrGeometry)` — the calibrator's **Check OCR** already calls it with arbitrary geometry, not just the tuner's |
| Compare a region against a reference | The empty check: a saved crop, the fraction of differing pixels, and a **6px inset** because a one-pixel window shift once made an *empty* box score 7.7 % |
| Drag a region to calibrate it | The Buy / Sell and Gem calibrators' canvas drag, and their `SizeChanged` overlay redraw |
| Click a calibrated point | `HidPointer.To` + `Click` — the shared closed-loop placement both other tools use |
| Tell you | The card status text plus `Beep()`. No new dependency, no tray icon, no toast |
| Take and release the port | `ArduinoPortAsync` / `Dispose` on `LauncherService` |

The calibration for a trigger is the same shape as everything else in this app: capture, drag a box,
save a reference crop, mark the action point, and a **Test** button that shows the verdict without
clicking.

## Risks, in the order they matter

1. **A false positive here is unrecoverable and unattended.** This is worse than the Sell tool: Sell
   runs once, while you are watching it. A watcher misreads a busy screen and clicks while you are in
   another room. Detection quality is the whole feature; everything else is plumbing.
2. **A revive usually costs something** — a scroll, a percentage of experience. Auto-reviving spends a
   resource on a decision you would normally make yourself.
3. **The capture reads the screen**, so the watch is blind whenever anything covers the game — the
   launcher included. A watcher that silently stops seeing is worse than one that says so.
4. **OCR cadence.** The engine is cached but a read is not free. Death wants a short interval and OCR is
   the wrong thing to run every second — which argues for the diff-then-confirm pairing below.
5. `OcrEngine` lives in `SealTools.Tuner` and its `ScanResult` is tuner-shaped (`Grade`, `Attributes`,
   `Remaining`). A watcher wants the raw text lines of a region. That is a small addition, and probably
   a move of the engine to `Core` so the launcher is not reaching into the tuner.

### The pairing worth considering for death

Diff first, OCR to confirm. The diff runs every few seconds and is nearly free; only when a region
*changes* does the OCR wake up and read it. That keeps the cheap check at a high frequency and the
expensive one rare — and it is what stops a single bad read from becoming a click. (This is the "both"
option; it is more moving parts than either alone.)

## Open questions

1. **What does the pet action actually do?** Click the menu-bar icon and then a button in the window it
   opens? **If the tool has to open that UI itself, this is a scripted sequence rather than a click** —
   a different and larger feature.
2. **How long does the death state last?** If the game revives you automatically after some seconds,
   the window to act is short and the poll interval decides whether it is catchable at all.
3. **What is the death text**, and how stable is the region it appears in? A phrase to match is easy; a
   region that is also quiet when nothing is happening is the part that needs measuring.
4. **Does the pet trigger act, or only notify?** "Needs to upgrade and such" may be a decision you want
   to make rather than one to delegate.
5. **What happens on repeated failure** — retry forever, or give up and say so? Unattended retrying is
   how a misread becomes a loop of clicks.

## Phasing

1. **Notify-only watcher**, both detectors, no port and no clicks. This is worth doing first on its own
   merits: it is the only way to learn the false-positive rate, and a false positive costs a beep
   rather than a mis-click. It also settles questions 2, 3 and 4 by observation instead of guesswork.
2. **Acting**, guarded — only when no tool runs, with a per-trigger cooldown and a give-up after N
   attempts.
3. **The pet action sequence**, once question 1 has an answer.
