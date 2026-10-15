# v2.13 — a new tool: Quest Hand-in

A tool that hands in quests repeatedly by replaying a flow you author, plus the launcher bug that tool
exposed on its first day and two UI fixes — one of which reaches the Tuner tab.

**Nothing here needs a reflash.** Everything is live the moment you launch it.

**What was verified, and what was not.** `Test one loop` on the new tool **works** — that was reported
from a live game. Everything else below is verified by construction (unit tests, a clean Release build,
and the standard teardown path) rather than by watching it happen, and the two items that are still
unproven say so:
the card **clearing** after a run finishes is new in this build and has not been watched, and the food
scan from v2.12 still has not run against a live bag.

---

## 1. Quest Hand-in — a new tool

It replays a sequence of inputs at the quest NPC, as many times as you ask. **It reads nothing**: no
capture, no OCR, no window lookup.

**That is why it has no calibration and no calibrate tab.** Every other tool spends its fragility budget
on reading the screen — derived regions, aimed crops, stale geometry. This one has none of it. Instead
you **put the mouse where the clicks have to land** during the flow's initial wait, and the run stops if
it moves. It is the tuner's `manual` spring mode, which works the same way.

**The steps**, as small as the board's protocol allows:

| step | sends |
|---|---|
| `click` / `right_click` | `C` / `R` |
| `key <char>` | one printable character — digits included |
| `enter` | `E` |
| `wait` | nothing; passes time |

Each carries **its own delay** (default `0.25 s`, a starting point rather than a measurement), because a
dialogue that needs 1.5 s next to one that needs 0.2 s is exactly where a single shared number breaks.

**A flow is a named preset**, one per quest, kept in `local.yaml` — personal, like a spammer key
rotation, and not in `defaults.yaml`, which is the file a public build ships.

**A step that cannot be sent is refused before anything is clicked** — a `key` step with no character, or
two. At 500 loops, "sends nothing" and "the game behaved differently today" are the same observation.

**The mouse guard stops the run** if the cursor moves more than 8 px from where you left it. It is the
one place this beats an existing mechanism: the tuner's guard needs a calibrated point *and* the game
window measured, which is why it is documented as **failing open** — window gone, "no drift, carry on",
the clicks continue. This one reads the cursor once at start and needs neither, so it **fails closed**:
an unreadable cursor ends the run rather than clicking on at a place nothing is watching.

**One caveat that is the whole design, not a defect.** A blind run cannot tell a good flow from a bad one.
A wrong flow does its loop count in wrong actions and the game accepts every one. `Test one loop` is the
only verdict there is.

## 2. It exposed a launcher bug on its first day

The tool finished its loops and the card still read **"● RUNNING"**. The fault was not in the tool:
`StartToolCoreAsync` had `try`/`catch` and **no `finally`**, so a tool's running flag was cleared *only
when it crashed*.

**It had never shown because every other tool loops until it is stopped.** This is the first tool in the
suite that ends by itself, so it is the first to expose it — and any future self-ending tool would have
hit the same thing: a card stuck on RUNNING, the tool still registered, and the next Start deferring to a
tool that had already finished.

The teardown now mirrors the stop path. **The card returns to `stopped` when a run ends** — not
"finished", deliberately: the card reads the registration the teardown clears, so showing a completion
would mean keeping a finished run registered, which is the same lie in the other direction.

## 3. A run leaves a record — `logs/quest.log`

Found by checking the claim above rather than trusting it: **a tool's console output goes nowhere in the
launcher** (the published WinExe has no console and nothing redirects one), and only the tuner and the pet
write files of their own. So this tool's runs had **no durable record at all** — which is exactly wrong
for a tool whose card cannot distinguish a completed run from an interrupted one.

Every run now appends to `logs/quest.log` beside the launcher, and every exit names itself:

```
DONE — 500 loop(s) of "red1"
cancelled in loop 37
STOPPED by the mouse guard — loop 37 of 500
```

## 4. Two UI fixes — one of them not just this tool

**The `+ Add` button drifted into the middle of the list it adds to.** A new row was appended *after* the
button, so each row pushed the button further up. Reported on the new tool's step editor — and the shared
editor it copied had the same flaw, so the **Tuner tab's rule grid and override grid** have done this for
as long as they have existed. Both now insert rows before the button, which stays at the bottom.

**`Save` sat 10 px below `Test one loop`.** One button in a row kept `MakeButton`'s 10 px top margin — the
one meant for a button standing under a card heading — and the other had it overridden.

---

## Upgrading

Unzip over the old folder, keeping your `config\`. Nothing to reflash. Nothing to recalibrate — the new
tool has no calibration to do, and it touches no geometry the other tools use.

The Quest tab is empty until you make a flow: pick **＋ Add new…**, name it, and add your steps.
