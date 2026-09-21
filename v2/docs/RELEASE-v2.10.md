# v2.10 Pet Feeder

A new tool, and two fixes that reach further than it does.

The tool keeps a boarded pet fed while you are not watching: it reloads the feeder on a schedule, so
the pet does not run out while the character farms. It is the first thing in this suite that acts on a
timer rather than on your button press.

Everything below was verified on the live game on 2026-09-17 unless it says otherwise.

---

## Pet Feeder — new

**The game feeds the pet; the tool only reloads the feeder.** That reload is not a top-up: stopping the
feed returns the pet to the bag along with the leftover food, so every reload is
*end → place the pet → load two stacks → start*. The tool drives exactly that, then waits out the
feeding cycle and does it again.

Two tabs come with it:

- **Calibrate Pet** — the geometry: the 目錄 button, the pet feed icon, the boarding window's X, the
  bag's page tabs, the start/end button, the boarding pet slot, the two feeder slots, and the boarding
  bag's **own** grid.
- **Pet** — the bag setup a run reads: which cells hold food and which one the pet goes back into,
  marked on an 8×8 grid with a page selector.

The two are separate on purpose. The grid is measured once; where the food is changes with what the
character has been doing, and the rule the Sell screen already states applies — a selection carried
over from last time is a selection nobody re-checked.

**The bag grid here is not the shop's.** The bag the boarding window opens sits somewhere else on
screen, so it carries its own two-corner calibration. Sharing the numbers would aim every click at the
wrong item.

**A schedule, not a sensor.** The tool knows the burn rate (3 items a minute for a stage-6 pet, which
the boarding window states itself as `每1分 擷取3個`) and what it loaded, so it knows when the feeder
empties. It reloads slightly early, because ending early returns the leftover food while leaving the
pet unfed is the one outcome here that cannot be undone.

**What it will not do:** it refuses to click a bag cell it cannot reach rather than clicking blind, and
it gives up after three failed reloads rather than retrying unattended. A pet that reaches +9 100%
stops being fed by the game, and nothing here notices that yet.

### The boarding state has to be told

The 開始代養 / 結束代養 control is **one button**: it ends boarding when running and starts it when
stopped. So a reload cannot press it without knowing which way it goes. There is a tick box on the Pet
tab for that today, and the tool ticks it itself after a successful reload — since a finished reload
always leaves boarding running. Reading the label is the obvious fix and is not built.

---

## Cursor placement — two fixes, and they affect every tool

Both were found by a full-height move to a bag cell, which is longer than anything the shop or the
composer asks for.

**A move is not finished until it has been *seen* to move.** The closed loop treated two polls reading
the same position as "settled", which is equally true of a move that has not started yet. On a long
move it fed corrections forward against a stale position and the queued asks landed together — two
requests totalling 1,081 px in a 1,141 px gap, clamped at the top of the screen. The shop never
exposed it because its corrections are small.

**The step budget was 6, which is enough for a healthy placement and too few for a damped one.** Once
the loop starts halving its step it closes about half the remaining error each time, so a correction
starting far out needs ~10 steps and was giving up 19 px short. Now 16 — a budget only spent when
something has already gone wrong.

Also: every placement now **re-aims immediately before clicking**. The wait between aiming and pressing
exists so the game is ready, but anything moving the cursor during it moves what the click lands on,
and something does — one trace shows 550 px of travel in answer to a 61 px request. That cause is
still unidentified.

---

## Calibrate Tooltip — new, and nothing uses it yet

A calibrator for the game's **hover panel**, built as a general capability rather than a pet feature.
The panel is anchored to the pointer, so its *offset from the pointer* is the same everywhere in the
game even though its size varies by item — one calibration therefore serves every hover-read this
suite ever does.

It is here because it was designed while working on the pet tool, not because anything reads a tooltip
yet. Mark the point to hover, and the tool parks the cursor there with the Arduino, clicks to focus,
waits for the panel and takes the picture itself — so the offset is measured against a coordinate the
tool chose rather than one read off the screen at an uncertain moment.

---

## Other changes

- **One bag picker, two selections.** The Pet screen and the Sell screen now share the 8×8 widget. The
  per-row shortcut column is Sell's own and is opt-in: "all eight slots of row 3" is a meaningful sale
  and is not a meaningful thing to mark as food.
- **The Sell screen's grid is unchanged behaviourally** — same toggles, same painting — but it is now
  the same `Border` cells the Pet grid uses, so the two look and behave identically.
- **The pet tool keeps a log**, `logs/pet.log` under the install directory, and a failed cursor
  placement writes `logs/cursor_trace.txt`. The card's status line is overwritten by the next step and
  gone when the tool stops, so without these a failure while nobody was watching left nothing to read.
  Both exist because that happened.

## Documentation

- **[PET-DATA.md](PET-DATA.md)** — all 327 normal pets with their boarding food, per-feed counts and
  the time to `+9`. The per-level 喂养值 cost was measured on the live site, not inferred: it rises by a
  tenth of the base each level, so a full stage is `base × 12.6`.
- **[PLAN-PET-AUTOFEED.md](PLAN-PET-AUTOFEED.md)** and **[PLAN-HOVER-INFO.md](archive/PLAN-HOVER-INFO.md)** —
  the designs, including what the live runs corrected.
- **[../game-knowledge/](../../game-knowledge/)** — a new folder for facts about the *game* rather than
  the tool: mechanics, NPCs, quests. Nothing there ships and nothing is built against it.

## Known gaps

Stated plainly, because the tool currently reports success it cannot stand behind:

- **Nothing verifies the pet went in, or that the food landed.** Both slots are calibrated and neither
  is read, so a reload that silently half-fails says `reload complete`.
- **The boarding state is a tick box**, not a reading — see above.
- **The delays are mostly guesses.** The measured ones are the 2.5s after ending and the 0.9s after a
  page tab; the rest are inherited from the shop tool's values.
