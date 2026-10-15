# v2.14 — the spammer grows up, and the pet feeder stops waiting

The skill spammer gets a rotation model worth the name — an order, combos that must be cast together, a
lead-in, and a picker on its card. The pet feeder stops being blocked by a spammer that never ends. Hold
Space gets a bug fixed that took a log to find. And the US client's frame, plus quest sequences, arrive
from the previous session.

**Nothing here needs a reflash.** Everything is live the moment you launch it.

**What was verified, and what was not.** This release is unusually honest on this point, because most of
it was built in one long session and only part of it has been run.

| | |
|---|---|
| **The Hold Space cold-start bug** | **Found from a log, and the fix is in.** Nobody has yet confirmed the fix on a live reproduction. |
| **The pet food re-marking** | The player has run the tab's scan routinely. The *automatic* re-marking during a run is new and has not been watched. |
| **The spammer's combos, lead-in, Start-cast, and yielding** | All four live inside `Run`, where no test reaches them. **None has been run.** |
| **Quest sequences** | `Test one loop` on a single flow was reported working before this release. A sequence has not been run. |
| **The US client** | The frame is built and tested; the English dictionary cannot be written from here. See §5. |

---

## 1. The spammer: a rotation, not a list of keys

Until now a preset was `key → seconds` and every key that came due was pressed. That is still the
default, and a preset written before this release behaves **exactly as it did**.

**§1.1 An order.** Keys named in a preset's order outrank every key that is not, and are tried first —
so a filler key runs only in the gaps. A key that is due but outranked keeps its cooldown and goes on a
later tick, so it is *delayed* rather than skipped. An empty order means no rule at all, which is what
keeps this additive.

**§1.2 Combos.** A combo is a group of keys that must be cast **in order, with a pause between them** —
the shape a game's combo effect needs, where the second skill only lands while the first is still
animating. Two properties make it a unit rather than a suggestion:

- **A combo is ready only when every key in it is off cooldown**, so one is never started that cannot be
  finished, and **a key named in a combo is never pressed on its own.** Half a combo is worse than none:
  it spends the first skill and leaves the effect un-triggered, with nothing to show for it.
- **A running combo owns the tick.** No filler, no other combo, until its last step has gone out — that
  silence is the point, since the previous cast's animation would swallow anything sent inside it.

**§1.3 The lead-in and the opening step.** Two per-preset numbers, both optional, and **some rotations
want them and some don't**:

- **Lead-in** — seconds to wait after Start before anything is pressed, which is the window for putting
  the game in front. **The card counts it down**, so a run that is waiting does not look like one that
  failed to start.
- **Opening step** — the gap between those first presses. Start makes every key due at once so a run
  opens with a cast rather than idling out each key's first cooldown; with animations that whole opening
  then fires into itself and the game swallows all but the first.

**§1.4 Start casts.** Related, and it is why the step exists: pressing Start makes the rotation *go*,
instead of waiting out each key's first cooldown. Without it a 20-minute buff would never go up at all on
a short session.

**§1.5 A rotation picker on the card**, above Start/Stop — the same control as Buy's, so a rotation can
be changed without opening Configuration. It sets the same value the tab does, and the two follow each
other rather than being two settings that look alike.

---

## 2. The pet feeder stops waiting for a spammer that never ends

**This was working as designed, and the design was wrong for one tool.** A foreground tool takes the game
for its whole run and the feeder waits — which is fine for the tuner, the composer, buy and sell, all of
which *finish*. The spammer doesn't. So the feeder waited, and would have waited until it ran dry, behind
a tool that would have been happy to stand down for half a minute.

Now **the spammer hands the game over when the feeder needs it, and takes it back when the feeder is
done.** The pause ends when the game comes free, not on a timer, so a 40-second reload is waited out and
a 10-second one is not waited 30. The Spammer card says `paused — pet has the game` while it lasts, and
the rotation re-opens through its **step** on the way back — but deliberately *not* its lead-in, which
exists for focusing the game after Start and has nothing to do on a resume.

**The quest hand-in still holds the game the same way** and can run just as long, so the feeder will
still wait it out. It is the same fix and it is deliberately not in this release: one tool at a time is
how a concurrency change gets verified.

---

## 3. The food scan keeps itself current

The feeder's food cells used to be marked by hand, and went stale as stacks were consumed. Now the scan
**rides along with the pet-position scan** the tool already does — the same pages walked, the same cursor
parked, and **the same capture** — so the food cells are re-marked every time the run looks for a pet.
No second pass over the bag, and no more marking by hand.

Two properties worth knowing:

- **It writes only when it finds something.** An empty result is ambiguous — no food left, or a page that
  never came up — and the two cannot be told apart from inside the tool. Overwriting on an ambiguous
  empty scan would destroy your marked cells; a stale list costs nothing by comparison.
- **It only runs when a pet scan runs**, which means at least one queued pet and a walk that completes.

---

## 4. Hold Space: the toggle that would not switch off

Reported as *"the toggle froze and would not switch off"*, and it took an instrumented build to find,
because three plausible causes were all wrong.

The toggle decided stop-vs-start from `CurrentId` — but `CurrentId` is only set when a start **completes**.
So through the whole ~2 second cold start the button answered "not running", every press re-entered Start,
and the second press was **swallowed while returning success** — no message, no change, and the spacebar
stayed held. Hold Space has no card, and its hotkeys are blocked while the game is focused, so the one
control that releases the key was the one that was dead.

**A press during a start now cancels it**, and the button reads **"Starting…"** during the wait rather
than sitting on "Hold Space" — the missing feedback is what made a busy press look like a dead one.

---

## 5. The US client — the frame is in, the dictionary is not

`game.variant: tw | us` selects the window title **and** the attribute dictionary, resolved at load, so
every tool keeps reading the one `window.title` it always read. The window side is a config line.

**What is not here, and cannot be written from this machine:** the English dictionary itself — the
attribute names, their OCR variants, and the fixes table. Its value is the variants table, built from
real misreads over many sessions, and English OCR fails completely differently from Chinese. That is a
process, not a file: run it, collect the misreads, fill the table.

**Also not here:** a Setup-tab switch, so changing clients is still a file edit.

**One measurement worth having,** if you try it: the US client runs at `1926×1112` against the TW
calibration's `1926×1232` — **same width, 120 px shorter.** Resize the US window to match before judging
whether the calibration transfers.

---

## 6. Quest sequences

Several flows in one run, for the quests that share an NPC. A sequence is a list of `flow × times`, and
the editor lives on the Quest tab with a target picker and a loop counter on the card.

**The design decision that is really a consequence:** every flow in a sequence clicks the *same* place,
and the positioning wait belongs to the sequence once. That falls out of the manual-placement design —
the tool never moves the cursor, so a flow inside a sequence cannot have a position of its own. It is
also what makes the feature sensible: one NPC, several quests, one place to stand.

**There is no abort rule**, deliberately. Asked how the tool would know a flow had failed, the honest
answer is that it cannot — it reads nothing. A rule that can never fire is one a later reader would
trust.
