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

## How it watches while another tool is running

**Watching needs no Arduino at all.** Detection is pure screen reading — a capture, then OCR or a
pixel diff. The port is only needed to *click*. So the watcher polls continuously no matter what else
is running; only the **action** is gated on a tool not running.

That makes the gate far less limiting than it first looks, because **stopping a tool does not need the
port either** — `StopTool()` cancels a token, it writes nothing to the board. So on a death while the
composer is running, the watcher does not have to queue and wait for the tool to end:

1. **stop the running tool** — no port needed; this is also the right thing on its own, since a
   composer clicking through a death dialog is doing damage;
2. the port is now free;
3. **act** — revive, then optionally leave things stopped, or restart.

That removes most of why acting *alongside* a tool looked hard. What it does not remove is two writers
on one serial line at the same instant, so the rule stays: the watcher takes the port only when nothing
else holds it.

Two things this depends on:

- **A running tool can pollute a watched region.** The composer drives the gem UI, so a watched box
  that overlaps anything it animates would see the tool's own clicks as a state change. The death
  region in particular must be somewhere the tools never touch.
- **The poll must not run on the UI thread.** The status timer is a `DispatcherTimer`, and an OCR read
  is long enough that doing it there would visibly stutter the window. The watcher needs its own
  background loop. (The calibrator's **Check OCR** does block the UI thread today — fine for a button
  press, not for a poll.)

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
2. ~~**A revive costs something.**~~ **Downgraded** — the user's revive cost is close to nothing, so
   spending a resource is not the objection. What is left is the part that is not a resource: acting
   on a *wrong* read, or repeatedly, which is risk 1 wearing a different hat.
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

**Agreed (2026-09-15): the first phase is notify-only, because the checking is the hard part.** The
detectors get wired to a beep and a status line and nothing else, so they can be watched getting it
wrong for a day and the false-positive rate measured — rather than trusted.

1. **Notify-only watcher**, both detectors, no port and no clicks. A false positive costs a beep. It
   also settles questions 2, 3 and 4 by observation instead of guesswork, which is the point: none of
   them can be answered from a desk.
2. **Acting**, guarded — only when no tool runs, or after stopping the one that does, with a
   per-trigger cooldown and a give-up after N attempts.
3. **The pet action sequence**, once question 1 has an answer.
