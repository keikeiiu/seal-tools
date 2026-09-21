# Seal Tools v2 — User Guide

Every card, tab and button in the launcher, what it does, and when to use it.

**繁體中文版：[USER_GUIDE.zh-TW.md](USER_GUIDE.zh-TW.md)**

Installing for the first time? Start with [INSTALL.md](INSTALL.md). For *why* the coordinates work
the way they do, see [COORDINATES.md](COORDINATES.md); for the step-by-step calibration flow,
[CALIBRATION.md](CALIBRATION.md); for which setting lives in which file, [CONFIG.md](CONFIG.md).

> **One tool at a time — except the Pet Feeder.** All tools share the single Arduino COM port, so
> starting one stops the previous one. The **Pet Feeder** is the exception: it is *resident*, so
> starting another tool leaves it running on its schedule, and it waits for the game if a reload comes
> due while that tool is using it.

---

## The window

The launcher opens as just the tool cards. The configuration tabs stay out of the way until you need
them:

![The launcher as it opens: tool cards, a Configuration chevron, and the Hold Space and pin
buttons](images/launcher-cards.png)

*(The two screenshots on this page are from **v2.9.1** — five cards and twelve tabs. The app now opens
with **six** cards and fifteen tabs; the Pet Feeder card and the `Pet`, `Calibrate Pet` and
`Calibrate Tooltip` tabs are newer than the pictures. Everything outside the cards is unchanged.)*

Each card is one tool: its name, its live status, and **Start** / **Stop**. Buy Items is the only card
taller than the rest — its item and count sit **above** Start/Stop, because they have to be set before
the run rather than during it. Below the cards are **▸ Tools** and **▸ Configuration** on the left,
and **Hold Space** and the pin on the right.

**Hide the cards with ▸ Tools** when you need the window for something else — it is for calibration
on a small screen, where the six cards take height that the capture canvas needs more. It opens the
configuration tabs at the same time, because hiding the cards only makes sense when you are using
them, and collapsing Configuration again brings the cards back. The window is never left showing
neither. *(That is the pair: hiding the cards opens the tabs, closing the tabs restores the cards.)*

**▸ Configuration** expands every tab and grows the window to fit. Clicking it again collapses them
and returns the window to the size it had before.

![The same window with Configuration expanded: the cards, both rows of tabs, and the Tuner tab's
Goal card below](images/launcher-expanded.png)

There are **fifteen tabs**, and at the default window width they **wrap onto two rows** — and they do
not wrap in reading order, so `Tuner` can end up on the *second* row with the calibration and config
tabs above it. How they wrap depends on the window width; what is fixed is the order they are
**defined** in, which is also the order they are listed in this document:

`Tuner` · `Gem` · `Spammer` · `Buy` · `Sell` · `Pet` · `Attributes` · `Calibrate Tuner` ·
`Calibrate Gem` · `Buy / Sell` · `Calibrate Pet` · `Calibrate Tooltip` · `Arduino` · `Setup` ·
`Hotkeys`

> The two-row example that used to be here was measured at a window `619` logical px wide (`929`
> physical at 150 % scaling) when there were twelve tabs — before the Pet work added `Pet`,
> `Calibrate Pet` and `Calibrate Tooltip`. The measurement is gone rather than adjusted, because a
> guessed table is worse than none; re-measure if the wrap is ever worth documenting again.

### The window chrome

| Control | What it does |
|---|---|
| **▸ / ▾ Tools** | Hides the tool cards so the tabs — and a capture canvas — get the whole window. Opens Configuration at the same time; collapsing Configuration brings the cards back. Not remembered between launches. |
| **▸ / ▾ Configuration** | Shows or hides the config tabs. The window resizes to suit. |
| **Pin on top** / **Pinned on top** | Floats the launcher above every other window, so the tool status stays readable while you play. The label and colour change with the state. |
| **Hold Space** / **Stop Space** | Presses the spacebar and **leaves it held** — the auto-pickup toggle, so you are not holding the key yourself. Clicking again releases it. The dot beside it reads `● idle` or `● holding`. It is not a tool card, so it does not shrink the window. |
| `●` status dot | Next to Hold Space. Green while the key is held, grey otherwise. |

**Placement is remembered.** Position, size and pinning are written to `config/local.yaml` about 0.7 s
after a move or resize settles — not only on a clean close, so a killed process does not lose your
window position. Next launch reopens where you left it.

### While a tool runs (mini mode)

While a *foreground* tool is running the window shows **just that tool's card** — the one whose Start
you pressed last. The Pet Feeder never takes this over: it runs for days at a time, and collapsing the
window onto it would hide everything else for the length of a schedule. Its card still shows live in the
normal layout, so "● RUNNING" there really does mean it is feeding:

```
┌─────────────────────────────────────────────────────┐
│  Skill Spammer                     [Start] [Stop]   │
│  ● RUNNING · Grade/Cycle …                          │
│                                                     │
│  ▸ Configuration              [Pinned on top]       │
└─────────────────────────────────────────────────────┘
```

*(A drawing, not a capture: the only way to photograph this state is to start a tool, which clicks in
the game.)*

The other cards come back when it stops. With **Pin on top**, that gives a small always-visible status
strip you can park in a screen corner over the game. Hold Space is the exception — it has no card, so
starting it does not shrink the window.

---

## Tool cards

| Control | What it does |
|---|---|
| **Start** | Opens the Arduino port if needed, then runs that tool. If the board cannot be found you get a message box saying why — check the **Arduino** tab. |
| **Stop** | Cancels the tool, waits for its loop to exit, and releases the port. |
| Live status | Refreshes about every 750 ms: `● RUNNING` / `● paused`, then any of `Grade`, `Remaining`, `Attempt`, `Cycle`, `Current`, the matched attribute lines, `Filter:` and a `⚠` warning (for example an unsupported spammer key). |

Starting a tool makes it act immediately — there is no separate "go" step. A running tuner or composer
can be paused with its hotkey (see the **Hotkeys** tab).

**The Pet Feeder's card is different in two ways.** It is *resident*, so starting another tool leaves
it running (see [The window](#while-a-tool-runs-mini-mode) above), and its live status is a **standing
line** rather than a run counter — `boarding Row 1  ·  next Row 2 at 19:40  ·  2 more queued`, built
from the boarding state and the schedule on the **Pet** tab.

### Buy Items has extra controls on its card

Buy Items is the one tool that needs a decision *before* it starts, so its card carries the choice —
you do not have to open Configuration to run it:

| Control | What it does |
|---|---|
| **Item dropdown** | Which buy item to purchase. The same choice as the **Buy** tab's preset picker; the two stay in step, because it is one choice shown in two places. |
| **Count box with `−` / `+`** | How many to buy this run, seeded from the item's **Usual count** and adjustable per run without editing the item. `−` will not go below 1. Free text — anything unparseable falls back to 1 rather than refusing to start. |

---

## Tuner tab

What the Magic Tuner rolls for and how it judges each result. Saved to `config/defaults.yaml`
(portable).

### Goal

| Control | Meaning |
|---|---|
| **Target grade** | The grade to roll toward: `N`, `G`, `DG`, `XG`, `SG`. The run stops the moment this grade is reached — that outranks everything below. |
| **Require grade** | Grade floor for a filter match. `None` means "any grade". Disabled while **Filter enabled** is off. |
| **Max retries** | Safety cap on attempts. The default is deliberately enormous. |

### Spring (發條)

| Control | Meaning |
|---|---|
| **Spring mode** | `manual` — you put the mouse on the 發條 button yourself; the tuner only clicks and presses Enter. `hid` — the tuner places the cursor on the calibrated spring point at run start. |
| **Mouse guard** | `hid` mode only. `off` never looks at the cursor. `stop` halts the run when the mouse drifts off the button — the human-has-taken-over case. `recenter` puts the cursor back and carries on toward the target grade, stopping only after too many runaway drifts. |

### Timing

| Control | Meaning |
|---|---|
| **Click delay (s)** | Wait after the Arduino click before pressing Enter. |
| **OCR delay (s)** | Wait after Enter before reading the screen. Too short and the read catches the previous frame. |

### Filter — rules (the main goal)

| Control | Meaning |
|---|---|
| **Filter enabled** | Turns the attribute filter on or off. Off, only the grade matters and everything below is disabled. |
| **Match mode** | `any` — one rule matching is enough. `all` — every rule must match. `per_attr` — every rule must reach its own **Count**, counting matching attributes, so **Count** only matters here. |
| Rules list | One row per rule: attribute name, count, min, max, and `✕` to remove it. |
| **+ Add Rule** | Appends an empty rule row. |

### Filter — overrides (stop immediately)

The same editor shape, for rules that **override** the ones above: a hit stops the run at once,
whatever the filter and the grade say. **+ Add Override** appends a row.

### Advanced

| Control | Meaning |
|---|---|
| **Save OCR captures** | Writes every OCR frame to `logs/captures/` as it is read. Debug only — useful when a read looks wrong, at the cost of disk writes on every attempt. |
| **Clean up capture images** | Deletes `logs/captures/*.png` and reports how many were removed, beside the button. |

### Save

**Save Tuner Config** writes all of the above to `config/defaults.yaml`. The result appears in an
InfoBar under the button — a failure says so rather than pretending to have saved.

---

## Gem tab

Behaviour of the Gem Composer. Saved to `config/defaults.yaml`.

| Control | Meaning |
|---|---|
| **Start grade** | The grade the composer begins on: `N`, `G`, `DG`. |
| **On empty result** | What to do when the composed-result box is empty — which means a grade ran out of resources. **Stop**, **Advance to next grade**, or **Clear resources, then advance** (right-clicks the three resource slots first, for a stuck gem). |
| **Save empty-check captures** | Writes the sampled result-box crop plus a `diff=…` line every cycle. Debug only, with disk I/O every cycle. See [CALIBRATION.md](CALIBRATION.md) for how the empty check decides. |
| **Save Gem Config** | Writes the above to `config/defaults.yaml`. |

The click points, the result box and the move sets are all in **Calibrate Gem**, not here.

---

## Spammer tab

Key rotation for the Skill Spammer. Saved to **`config/local.yaml`** — presets are personal, so they
stay out of the `defaults.yaml` that `publish.bat` ships with every release.

The Arduino can send any single letter or digit, plus **F1–F12**. Anything it cannot send is skipped
and reported as a `⚠` on the tool card.

### Active

| Control | Meaning |
|---|---|
| **Preset** dropdown | Which named key set the spammer presses. The list ends with **＋ Add new…**, which opens the name prompt. |
| **Rename** | Opens the name prompt to rename the current preset. **Delete** removes it after a confirmation — the last preset cannot be deleted. |
| Name prompt | A row that appears only while creating or renaming: **Create preset:** or **Rename to:**, a name field, then **Create** / **Rename** and **Cancel**. A duplicate or empty name is refused with a message instead of overwriting. |
| Keys summary | Read-only tiles of the current preset: each key in a keycap, its cooldown below, and `⚡` marking a fast tap. |
| **Edit** / **Done** | Reveals the editor below, or puts it away. |
| Status line | Confirms a create, rename or delete, or says why one was refused. |

### Keys

| Control | Meaning |
|---|---|
| **Key** / **Delay (s)** / **Fast** header | One row per key. `✕` removes that row. |
| **Fast** checkbox | A fast tap: the key is held about 10 ms instead of the normal 30–80 ms. Leave it off for a held skill — that is what most games want. |
| **+ Add Key** | Appends an empty row with a 0.2 s delay. |

### Advanced

Tick **Advanced — edit the raw key:seconds list** to edit the same data as text, for setups the rows
cannot express. Ticking fills the text from the rows; unticking rebuilds the rows from your text (a
`*` prefix there still means a fast tap).

### Save

**Save Preset** writes the active preset, every other preset, and which one is active to
`config/local.yaml`. Switching presets is **not** saved until you press it.

---

## Buy tab

Defines the items you re-buy, and the numbers the Buy card's Start uses. Presets live in
`config/local.yaml`.

### Item

| Control | Meaning |
|---|---|
| **Preset** dropdown | Which item to buy. This is the same choice as the dropdown on the Buy card. |
| **Reload** | Re-reads the presets from config and repopulates both pickers. |

### Position

The four fields describe the picked item. **Editing them does nothing until you save them onto a
name.**

| Control | Meaning |
|---|---|
| **Name** | The item's name — how it appears in both dropdowns. |
| **Row** | Which visible row of the shop list the item sits on, as **Row 1 … Row 10** — Row 1 is the top. The stored value is a 0-based index; the label is not, so there is no off-by-one to make. |
| **Scroll notches** | Wheel-downs **from the top of the list**. See the warning below — this only means anything while the list is actually at the top. |
| **Usual count** | How many you normally buy. The Buy card seeds its count from this; the run can change it without editing the item. |
| **Save item** | Writes the four fields onto the name. Creates it if new, overwrites if it exists. |
| **Delete item** | Removes the picked item. |
| **Dry run** | Scrolls `Scroll notches` down from where the list is, and parks the cursor on **Row** — **and clicks nothing**. The only way to set the scroll number, because a scroll position cannot be measured after the fact: look at where the pointer landed and adjust until it sits on the item you want. |

> ⚠ **The list must be at the top before a run, and that is your setup, not the tool's.** A preset's
> scroll means "notches down from the top", so a list left part-way down puts the item that much
> further off — and **nothing detects it**. The tool deliberately does not scroll to the top: that
> would cost the length of the whole list on every run. Glance at the list before starting.

### Status

Reports what a save, delete or dry run just did — including why a dry run refused to start.

---

## Sell tab

Which bag slots to sell. The geometry comes from the calibrated grid and the shared transaction, so
there is nothing else to configure here.

### Slots to sell

| Control | Meaning |
|---|---|
| **Select all** | Selects all 64 slots. |
| **Clear** | Deselects everything. |
| **Dry run** | Walks the cursor through the slots **in the order the real run would sell them**, updating the status line as it goes. **Clicks nothing.** |

### Grid

A clickable 8 × 8 grid standing in for the bag: **slot 1 top-left, slot 64 bottom-right**, mirroring
the bag so that lighting slots up shows exactly what is about to go. Clicking a slot toggles it; the
tooltip names it. To the right of each row is a **row *N*** button that selects or clears that whole
row of eight — one click instead of eight, and it cannot leave a row half-selected by mistake.

> **The selection is never saved.** It is chosen fresh each time, deliberately: a stale selection
> restored from a previous session is exactly how the wrong stack gets sold.

### Safety

| Control | Meaning |
|---|---|
| **Max slots per run** | A hard ceiling on how many slots one run may sell, **enforced in the loop** rather than offered as advice. |
| **Save** | Writes the cap to `config/local.yaml`. |

If you select more slots than the cap, the status line says so and the run refuses — raise the cap on
purpose, or narrow the selection.

### How selling runs

**Highest slot number first.** That stays correct whether or not the bag closes the gap after a sale,
without the tool needing to know which it does — selling ascending would silently skip items if the
bag does compact.

---

## Pet tab

The Pet Feeder's run state: **where the food is and which pets to breed**. It is not calibration —
the machine's geometry is on **Calibrate Pet**, and the two are separate on purpose. The grid is
measured once; where the food is changes with whatever the character has been doing, which is the
rule the Sell screen already states — a selection carried over from last time is a selection nobody
re-checked.

Read [PET-TAB-DESIGN.md](PET-TAB-DESIGN.md) for how the schedule and the reload actually work.

### The bag, right now

| Control | What it does |
|---|---|
| **Page 1 / 2 / 3** | Which bag page the grid below is editing. The tab **opens on the page your marks are actually on** — it used to open on page 1 regardless, which showed an empty grid and read as "my marks are gone". |
| **Mark FOOD cells** | Clicking a bag cell now marks it as holding pet food. One pool for all rows. |
| **Mark the RETURN slot** | Clicking a cell marks **where a boarded pet comes back to** — one cell, kept empty. |
| **Mark a pet to queue** | Clicking a cell marks **the pet you want to breed**. A queued pet sits in a cell that *holds* a pet, so it is not the same thing as the return slot. |
| The 64-cell picker | The same widget the Sell screen uses, over this screen's own selection. **A click toggles** — so re-clicking a marked cell removes it. |
| Cell info line | What the cell under the cursor is currently marked as. |

Re-mark these whenever the bag changes. A stale mark means the tool right-clicks whatever has taken
that slot since. The three mark modes are stacked vertically rather than in a row because three
buttons across do not wrap — on a narrow launcher, the third one simply ran off the edge.

### Setup so far — this tab

This tab's own checklist, and only its own items. It is the counterpart of Calibrate Pet's — that one
is what is true of the *machine*, this one is what is true of *this run*. Saving with a gap is
allowed: the tool says what is missing rather than clicking into empty screen.

### Ready to run

The readiness verdict. **Start** is on the Pet Feeder card, not here.

### Timing

| Control | What it does |
|---|---|
| **Wait after empty (min)** | How many minutes **past** the feeder emptying to reload. Positive on purpose: reloading after it empties guarantees it *is* empty when the stacks go in, and what the game does with a top-up onto a partial stack is unknown. The cost is that many minutes with nothing fed — `1`–`2` covers any drift. A negative value reloads early and **discards food**. |
| **Action wait (ms)** | The pause after *each step* of a reload before the next. Too short and a click does not register, which costs a whole cycle. |
| **Food load** | `Right-click — the game picks the box` (the earliest empty box; with everything in one queue ordered top-down, a stack meant for a lower row can land in an upper row's box when that one has run dry) or `Drag to the row's own box`, which names the row's own box and **needs firmware 2**. The choice is here rather than hard-coded so a board that has not been reflashed can still feed. |

### Rows and boarding state

| Control | What it does |
|---|---|
| **RUN** (per row) | Whether the tool drives that row **at all**. Untick to leave a row alone — still calibrated, still shown, just not fed. That is how you run fewer rows without deleting the others. A row that is off is not validated, read, scheduled or clicked, so it may sit half-set-up, or be one somebody is feeding by hand. |
| **boarding right now** (per row) | Tick each row where a pet is in the loader *now*. The reload has to **end** boarding to get the pet back before it can put it in again, and the start/end control is one button per row — pressing it with the wrong idea of the state does the opposite of what the step needs. |
| **Reload every row when the run starts** | The override for what *looking* cannot see. The slot says a pet is in the loader; it does **not** say how much food is left, so a feeder that ran dry overnight looks exactly like one just filled, and would be left alone to starve for a whole cycle. Nothing is wasted by forcing it: ending boarding returns the leftover food with the pet. |

**You do not have to get the boarding ticks right.** When the tool starts it opens the breeder once and
*looks* at each row's pet slot, so a row already feeding is left alone rather than ended and redone.
The ticks are the fallback for a row whose slot cannot be read, and the tool keeps them up to date
after every reload.

While the run is up, the card carries a standing line built from exactly this state — e.g.
`boarding Row 1  ·  next Row 2 at 19:40  ·  2 more queued`. It is a *moment* rather than a countdown,
so the card recomputes the wait on every UI tick instead of showing a number that was true when it
was written.

### The queue — which pets to breed

| Control | What it does |
|---|---|
| **Name for the next one** | A label for the pet you are about to capture. |
| **Which pet line** | The feeding table's own `species` — the thing that turns `+9 10%` into a number of minutes. Eleven options, not one per pet: a line holds many pets and they share one figure. |
| **Capture the marked pet's icon** | Photographs the pet in the cell you marked with **Mark a pet to queue**. The crop is taken from the marked cell, so there is nothing to aim. |
| **Remove last** | Drops the last queued icon. |
| Thumbnails | The captures themselves, not just a count — they are the same pixels the scan scores against. A wrong crop matches nothing, which is safe but silent, and a run that never finds a pet is hard to tell from an empty queue. |
| Queue list | The queued pets by label. |

**A pet is found by its portrait, not by its position.** A returning pet lands in the **first free bag
slot** and the character farms in between, so the cell it was taken from means nothing by the time it
comes back. The icon is the pet's own portrait, matched across all 64 cells — so the bag can be
rearranged and the queue still works. With no icons captured the tool falls back to the return slot.

When a pet finishes it is **mailed** and leaves the bag, so a queued pet that can be found is by
definition not finished: the tool boards the first match and needs no other test. Any pet of the right
kind is a harmless substitute, because an idle breeder is wasted time.

### Find them

| Control | What it does |
|---|---|
| **Scan the bag for these pets** | Puts the boarding bag up and matches the crops above against all 64 cells of every page. It **opens nothing, clicks nothing and boards nothing** — except the `ITEM` page tabs, because a pet can be on any page and there is no other way to look at one. |

It answers one question: *which page holds how many of which pet*. The scores, the cells and the
runner-ups are in the file it writes, next to the pages it captured — not on this card, where they
would bury the answer.

### Read a pet's panel

| Control | What it does |
|---|---|
| **Test read the pet panel** | Hovers the pet you marked with **Mark a pet to queue** and reports what the OCR makes of the hover panel. Needs the panel calibrated on **Calibrate Tooltip**. |

The pet has to *be* in that cell — one that is in the loader instead leaves the cell empty and the
read finds nothing. The dump of every number is deliberate: it is how what else the panel states gets
found out.

### Find the pets that can still be fed

| Control | What it does |
|---|---|
| **Scan bag for feedable pets** | Hovers **every cell of every bag page** and reports each pet's growth and EXP, so the ones at `+9/100%` — already finished, and an error dialog if boarded — can be told from the ones at `+0` that still want feeding. Read-only. |
| **Scan + rebuild the queue** | The same sweep, then **replaces the queue** with an icon for every pet that can still be fed. The finished ones get no icon, so the run can no longer board one by accident. The old queue is dropped — it holds icons of pets that have been mailed — and nothing is written if no icon could be cropped. |

It takes roughly **1.5 s per cell**, so around **90 s per page**: the window hides while it runs and
comes back at the end of each page with that page's results. **Nothing is clicked in the bag** — a
click on a pet *switches the equipped pet*, so this only ever moves the cursor. A cell that reads as
neither is reported as neither: a failed read is never called finished, because skipping a pet that
needed feeding is the one outcome here that cannot be undone.

The destructive scan is a **separate button** rather than a checkbox on the one beside it, so a scan
you press to look at cannot quietly rewrite the queue.

### Save

Writes the marks, the timings and the boarding state to `config/local.yaml`. It is called **Save**,
not *Save Calibration*, because this tab holds the run's state rather than the machine's — the two
Saves are scoped to their own halves.

---

## Attributes tab

A read-only view of the OCR attribute dictionary (`config/attributes.yaml`): **Name** (what a filter
rule matches), **Category**, and the **OCR variants** that are auto-corrected to that name — the
garbled forms OCR actually produces.

No controls. To add an attribute, edit `attributes.yaml` and restart — the launcher caches it at
startup.

---

## Calibrate Tuner tab

Points the tuner's OCR at the 發條 (Magic Tuning) window.

| Control | What it does |
|---|---|
| **Capture 發條 window** | Grabs the game window in physical pixels. **The launcher hides itself for ~0.3 s** so it cannot cover the game — the blink is expected. The image must show the whole window. |
| Canvas | Drag three boxes in order: the **grade letter**, the **three attribute lines**, the **spring count**. They are colour-coded, and a too-small drag is rejected rather than accepted as a 3-pixel box. Then **click the 發條 button** in the capture to record its point. |
| **Check OCR** | Runs the full read → match → filter pipeline on the current screen. With all three boxes dragged it uses those; otherwise it uses the saved calibration and says so. Prints grade, spring count, the three attribute lines, `Matched:`, and the `Filter:` verdict. It also measures the real attribute line pitch and stores it as `row_height`. |
| **Test Click (發條)** | Moves the cursor to the recorded 發條 point and clicks it, so you can confirm it lands. Only relevant when **Spring mode** is `hid`. |
| **Save Tuner** | Writes the region, sub-bands, measured row height and spring point to `config/local.yaml`, records the display environment, and saves the reference screenshot `config/calib_tuner.png` with your bands drawn on it. |
| Result | The tab's status and output line — capture result, the OCR dump, or the save confirmation. |

---

## Calibrate Gem tab

Points the composer at the gem-combine UI, tests the moves, and stores which move set it uses.

### Capture

| Control | What it does |
|---|---|
| **Capture gem window** | The same physical-pixel grab, launcher hidden for it. |
| Canvas | Click in order: **N**, **G**, **DG**, **Register**, **Combine**, then the **3 resource slots**, then **drag a box around the composed result gem**. |

### Coordinates

The points as typed numbers — client-relative physical pixels, with W/H for the result area. Editing
one directly nudges a point (for example lining N/G/DG up on the same level) without re-capturing.
**Save Coordinates** persists them.

### Tests

Nothing here writes config or calibrates anything on its own.

| Control | What it does |
|---|---|
| **from** / **to** dropdowns | Choose the two points the move tests use. |
| **Place cursor + click** | Moves the cursor to the **to** point **and clicks it**. |
| **Test tuned move** | Clicks **from**, sends that route's raw `D dx dy`, then clicks **to** — confirms a tuned move lands. |
| **Check Result Colour** | Samples the result box now and reports its colour, the distance to the empty reference, and the empty / has-gem verdict. |
| **Sample result gem** | The same sample with more detail (channel spread, dominant tone), for judging the empty-detection threshold by hand. |
| Result gem box crop | A preview of the exact region being sampled for empty detection, so you can see it is over the right thing. |

### Move set

| Control | What it does |
|---|---|
| **Composer move mode** | Which move set the composer uses. `tuned` sends the hand-tuned counts; `arduino` places the cursor on each route's destination point, closed loop, and re-aims on every move so nothing needs tuning. Saved to `defaults.yaml` by **Save Gem Composer**. See [MOVE-SETS.md](MOVE-SETS.md). |

Whichever set is **not** active is dimmed to 45 % and disabled, so the two grids cannot be confused.
The selector has its own card precisely so it stays clickable while either set is active.

### Moves — tuned (hand-tuned counts)

One row per route — N→Register, G→Register, DG→Register, Register→Combine, Combine→Register,
Register→Resource1, Resource1→Resource2, Resource2→Resource3, Resource3→N/G/DG — with raw `dx`/`dy`
boxes and a **Send** button per row that sends that exact move.

| Control | What it does |
|---|---|
| **Send** (per row) | Runs that single route: click the source, send the raw `D dx dy`, click the target. |
| **Save tuned counts** | Writes `gem.movements` to `config/local.yaml`. These are hand-tuned HID counts — **not computed from pixels**, and specific to this Arduino, this pointer speed and this in-game display. |

### Moves — arduino (cursor placed on the point)

The same routes, each showing the **point it goes to** and a **Run** button that clicks the source
point, places the cursor on the destination with the Arduino, and clicks. This is what the composer
does in `arduino` mode, so a route that lands here lands in the composer.

### Full-run test

**Run one full cycle** plays one whole cycle with the arduino moves: N selected → register → combine,
deregister + register, combine again, clear the three resource slots; then the same for G; then DG
combines once and it stops. **This one really clicks in the game — 21 clicks.** Use it to check the
move set survives a real run before switching **Composer move mode**. Needs the gem-combine window
open with the resource slots loaded.

### Advanced

Input tests that compare the two cursor APIs, plus a capture diagnostic. None of these calibrate
anything.

| Control | What it does |
|---|---|
| **Diagnose capture** | Reports the window's frame and client rects and the non-client offset, and saves `logs/captures/diag_capture.png`. Use it to confirm the launcher is not covering the game. |
| **Debug Cursor (logical)** | Moves the cursor to the selected point using `SetCursorPos` on the converted coordinates. Prints the computed target, whether the API accepted it, and where the cursor ended up. **No click.** Diagnostic only — the tools place the cursor with the Arduino, not this call. |
| **Debug Physical** | The same with `SetPhysicalCursorPos`, kept for comparing the two APIs. |

### Save

**Save Gem Composer** writes the positions, resource points, result area, empty-signature and
`empty_distance` to `config/local.yaml`, and saves `calib_gem.png` plus the result-box crop. It then
asks whether the result box is currently **empty**: answer **Yes** to sample the empty-colour
reference used by empty detection, **No** to leave it off rather than capture a gem as "empty".

---

## Buy / Sell tab

Calibration for both halves of the shop tool: the bag grid the Sell side clicks, and the shop list and
buttons the Buy side uses.

> **Capture with the shop open, the bag open and the count dialog showing** — one frame then covers
> every mark on this tab. If something is not in the capture, capture again before drawing on it.

### Bag grid

| Control | What it does |
|---|---|
| **Capture bag window** | Grabs the game window, launcher hidden for the grab. |
| **Draw grid area** | Arms a drag: box the **whole 8 × 8 bag**. |
| **Draw one slot** | Arms a drag: box **one slot**. |
| **Show 64 centres** | Draws every slot centre the grid implies, plus the derived shop rows. |
| Canvas | Where you drag. Magenta dots are bag slot centres, a yellow dot is the focus point, a light-blue box is the shop list region with green dots for its rows, and an orange-red dot is MAX. |

Both boxes are needed: the whole grid gives the pitch, and the single slot is the check that the grid
really is uniform. **If the two disagree by more than a little, the tab says so rather than averaging
them** — a disagreement means a bad drag, not a number to split.

### Shop (buying)

| Control | What it does |
|---|---|
| **Draw list region** | Arms a drag: box **exactly the visible rows of the shop list** — the first row's top to the last row's bottom. The derived rows are only as good as this drag, and there is a real boundary to aim at. |
| **Mark focus point** | Arms a single click. The next click on the capture sets the point. |
| **Mark MAX button** | Arms a single click, for the count dialog's MAX button. |
| **Save Calibration** | Writes the bag grid, the slot box, the list region, the focus point and MAX to `config/local.yaml`. |

**The focus point must be somewhere INERT.** It is not a marker — **both tools left-click it at the
start of every run.** Starting a tool means clicking the launcher, and that takes focus away from the
game, after which the game ignores both a wheel notch and a right-click. One real click gives the game
focus back. So put it on empty panel space or a window title bar — **never over a shop row or a bag
slot**, where a left-click would select or buy something. Buying also scrolls from this point, which
is harmless because the wheel acts anywhere in the focused window.

### Setup so far

A checklist of which marks are set (`ok`) and which are missing (`--`): bag grid area, one bag slot,
shop list region, focus point, MAX button — plus your buy item and sell slot counts. **Saving with a
gap is allowed**: the tool says what is missing when you press Start, rather than clicking into empty
screen.

### Test scroll (wheel)

| Control | What it does |
|---|---|
| **Notches** | How many wheel notches to send. |
| **Scroll up** (**Q**) / **Scroll down** (**Z**) | Sends that many real wheel notches to the game through the Arduino, so you can see how far one notch moves the shop list — the number that sets every buy item's scroll amount. |

It clicks the focus point first, because you have just been clicking the launcher and the wheel needs
the game focused. One command carries at most **400** notches; that is a guard against a malformed
value wedging the board, not a limit you should meet in normal use — a real shop list needs about 60.

---

## Calibrate Pet tab

The machine's half of the Pet Feeder: the boarding window's geometry and the bag it opens. Everything
about the *run* — where the food is, which pets to breed — is on the **Pet** tab instead.

### Capture

| Control | What it does |
|---|---|
| **Capture game** | Screenshots the game window. **Expect two captures**, because the marks live on screens that cannot both be up: **first** open the icon panel (click 目錄) and mark those two points; **then** open the boarding window and press Capture again. |

Marks already placed are **kept** — each one is written as you make it — so re-capturing only swaps the
background. The capture reads the screen, so keep the game visible and the launcher out of the way.

### How it gets to the feeder

| Control | What it does |
|---|---|
| **Mark 目錄** | The button in the bottom-left icon cluster. It opens a panel of eight round icons. |
| **Mark pet feed icon** | The chick holding a bottle, in that panel. |
| **Mark boarding X** | The boarding window's close button. |

Both of the first two are **points** — they never move, so nothing has to be found. **Do not mark the
pet cartoon image on the main screen**: that opens the manual feeding window, a different system
holding a different food.

### Bag pages

| Control | What it does |
|---|---|
| **Mark ITEM1 / ITEM2 / ITEM3** | The bag's three page tabs. |

They are **absolute** tabs — clicking one lands on that page whatever page you were on. That is why
they are three marks rather than a next/previous pair: there is nothing to read back and nothing to
lose count of.

### Breeding rows

| Control | What it does |
|---|---|
| Row strip | Which row everything you drag below is recorded against. **Pick the row first.** |
| **+ Add row** | Adds a row, up to four. |
| **Food slots in this row** | `2` on the free row, `5` on a paid one. |

The window holds **four rows — one free, three behind the paid expansion** — and each has its **own**
start button, pet slot and food boxes. The slot count is the row's *capacity*, not a preference, and
the tool reloads each row on its own clock because of it: getting it wrong reloads a row early or
leaves it dry. Whether the tool **drives** a row is a run decision and lives on the Pet tab, with the
boarding ticks — not here, where the shape of the row is decided.

### Row geometry — nudged by typing

Each row's food strip in pixels, as four editable numbers: **x, y, width, height**. Change one and the
slots and crops beside it update as you type.

This exists because a capture answers *"is the box in the right place?"* and cannot answer *"is this
row a pixel out from its neighbours?"* — which is how a row reads nothing while the rows above and
below it read fine. It has happened: one row's crops started 1–2 px right of another's, and its widest
number ran past the crop's left edge. **Read down the crop column across rows** — an outlier there is
a row whose strip landed differently, and nudging *its* x is the targeted fix. Moving the shared
**Count crop starts at** number instead shifts every row, including the ones already reading.

### Starting boarding

| Control | What it does |
|---|---|
| **Draw start button** | A box around the one reading 開始代養 or 結束代養. The tool clicks its centre once the food is loaded. It is one button that both starts and ends boarding, but **the label is not read** — the schedule already decides when to reload, so there is nothing to ask. |
| **Draw pet slot** | The square the pet lands in. An empty-check crop, and the one that answers *"did the pet actually go in?"* before any food is loaded — a right-click that missed leaves an empty slot and a window that otherwise looks perfectly normal. |
| **Draw food strip** | **ONE** box across all of this row's food slots. The tool divides it by the row's slot count, so a five-slot paid row is one drag rather than five that all have to agree about where the row starts. |

The strip is also the **whole of the count calibration**: the crop each feed count is read from is
computed inside each slot, so there is nothing else to draw. On the capture — **green** the start
button, **blue** the pet slot, **yellow** a food slot as derived from its strip, **yellow-green** the
strip itself. Two boxes on a food slot, and that is all.

**The magenta line** is where the OCR starts reading: everything to its right, at the slot's full
height, is the crop. It is drawn where **Count crop starts at** puts it, so moving that number moves
the line. It must clear the food icon — a line through the icon is a crop the reader finds nothing in.

### Read the counts

| Control | What it does |
|---|---|
| **Test read** | Open the boarding window **with food in the slots**, then press this. It reads every row that is ticked to run, on that row's own slots, and reports what the reader made of each — including the empties, which are the correct answer for a slot with no food. |
| **Count crop starts at** | How far across each slot the read begins, as a fraction of the slot's width. **The one number the read comes down to.** `0.40` is where it was measured, and the band that works is only a few pixels wide. |
| **Min score** | How sure the reader must be. Real counts scored **0.90–1.00**; everything the food icon produced scored at most **0.63**. |

If a number comes back **clipped** — the first digit missing — the line is too far right; if **nothing**
comes back at all, it may be too far left and into the food icon. The result goes into its own pane
under the button rather than the shared hint line, because a result you have to hunt for is a result
nobody checks.

### Bag grid (this flow's own)

| Control | What it does |
|---|---|
| **Draw grid area** | A box around the whole 8 × 8. |
| **Draw one slot** | A second box around one slot, as the uniformity check. |
| **Show 64 centres** | Draws where the tool would right-click. If the dots miss the slots, re-drag the grid area. |

**The bag the boarding window opens is at a different place from the one the shop opens beside**, so
this is its own calibration — the Buy/Sell numbers are **not** reused. Sharing them would aim every
click at the wrong item.

### Setup so far

Filled in as you mark things. Saving with a gap is allowed — the tool says what is missing rather than
clicking into empty screen.

### The empty-slot reference

What the tool compares each row's pet slot against, for the row selected above, shown as a preview.
It should show an **empty** slot of that row: if it shows a pet, an older window layout, or nothing at
all, re-capture it. This is what decides whether the tool thinks a row is boarding, which decides
whether it presses that row's toggle — a live run went wrong reading three empty rows as occupied
against references that were not what anyone thought they were.

### Save

| Control | What it does |
|---|---|
| **Capture empty pet slot** | With the breeder **open and no pet in it**. This is the reference the tool compares against after every placement, so it can tell a pet that went in from a right-click that did nothing. **Without it the check is skipped and a failed placement is invisible** — which is how a 12-hour run lost half its boarding time while reporting success. |
| **Save Calibration** | Writes the geometry to `config/local.yaml`. |

---

## Calibrate Tooltip tab

Measures the **hover panel** — the box the feeder reads a pet's growth and EXP% from before boarding
it. Only the Pet Feeder uses it; the other tools do not need it.

### Measure the hover panel

| Control | What it does |
|---|---|
| **Capture game** | With the bag, or whatever holds the item, open. |
| **Mark hover point** | Click, in the capture, the item whose panel you want. The tool will park the cursor there — so pick one that reliably shows a panel. |
| **Hover and capture** | The tool moves the cursor there with the Arduino, clicks to give the game focus, waits for the panel, and captures again. Then **drag a box around the panel**. |

The offset is worked out from the point the tool placed the cursor on, so **nothing is typed and
nothing is eyeballed**. Size the box for the **largest** panel you care about — a pet's is bigger than
a food item's — because a smaller panel then just leaves background behind it, which the read ignores.
A box per item type would be tighter and would stop the offset being universal, which is the whole
point of it.

### Save and check

| Control | What it does |
|---|---|
| **Hover delay (ms)** | How long the cursor sits on the item before the panel is read. |
| **Save Calibration** | Writes the offset and the delay to `config/local.yaml`. |
| **Test read** | Hovers and reads, then reports what the OCR made of the panel. |

The delay is a field rather than a constant because `700` ms was measured and then immediately found
wanting: one read caught the bag with the cursor still on it and no panel up yet, **which looks
exactly like a wrong offset**. Raise it until **Test read** is reliable, then press Save.

---

## Arduino tab

Connection diagnostics, split into the two questions you actually ask: *is the Arduino there*, and
*does its click reach the game*.

### Connection

| Control | What it does |
|---|---|
| Status light | Green when a serial device matching the configured VID/PID is present, red when not. |
| Port list | Every serial port the OS sees, with `>>` marking the match, plus the expected VID/PID and each device's PnP id. |
| **Refresh** | Re-runs the scan. It is not live. |

### Input test

**Send a test click** opens the port and sends one Arduino left click **at wherever the cursor already
is** — the end-to-end proof that the HID path works. It does **not** move the cursor; use Calibrate
Gem → **Place cursor + click** for that. Its result shows beside the button, so it cannot overwrite the
connection status above.

---

## Setup tab

Records the display environment the calibration was measured in.

| Control | What it does |
|---|---|
| **Detect** | Measures the live game window: monitor physical size, DPI and scale, and the window's frame and client rects. Fills the fields below. |
| **Scale** | Physical pixels per logical pixel (e.g. 1.5). Detected automatically; editable if detection is wrong. |
| **Reference client size** | The game client size in physical pixels that the calibration belongs to. |
| **Save Setup** | Writes the `calibration:` block to `config/local.yaml`. |
| Stored calibration | The saved scale, client size and timestamp — or "none yet". |
| ⚠ warning | Appears when your live client size differs from the stored one. **Recalibrate — do not trust the old coordinates.** |

This is the tab that tells you whether an existing calibration still applies, which is the first thing
to check on a machine you have not calibrated.

---

## Hotkeys tab

Global hotkeys for the running tool.

| Control | Meaning |
|---|---|
| **Start / stop rolling** | Toggles the running tool. Default `F12`. |
| **Quit (immediate)** | Stops the tool at once. Default `F11`. |
| **Advance grade (gem)** | Advances the composer to the next grade. Default `G`. |
| **Pause (graceful stop)** | Finishes the current cycle, then stops. Default `CapsLock`. |
| **Save Hotkeys** | Writes them to `config/defaults.yaml`. An invalid name is refused with a message rather than saved. |

Type a key name: `F1`–`F24`, `Esc`, `CapsLock`, `Space`, `Tab`, `Enter`, or a single letter or digit.

> **Hotkeys only work while the LAUNCHER has focus.** The game's anti-cheat blocks background key
> reads, so while you are in-game these keys do nothing at all. Click the launcher first, then press
> the key — or use the card's **Stop**.

---

## Typical workflows

**First time on a machine**

1. [INSTALL.md](INSTALL.md) — flash the board, unzip, run as administrator.
2. **Setup** → **Detect** → check the scale and client size → **Save Setup**.
3. **Calibrate Tuner** → **Capture 發條 window** → drag the three boxes → **Check OCR** (it must read a
   real grade, the count and three attribute lines) → **Save Tuner**.
4. **Calibrate Gem** → **Capture gem window** → click the points and drag the result box →
   **Save Gem Composer** → **Send** one tuned route, or **Run** one arduino route.
5. **Buy / Sell** → **Capture bag window** → draw the grid area and one slot → **Show 64
   centres** and check the dots sit on the slots → drag the list region → mark the focus point and
   MAX → **Save Calibration**.
6. **Arduino** → **Refresh** → **Send a test click**.

**Verifying an existing calibration, with no capture**

- **Calibrate Tuner** → **Check OCR** straight away — it tests the saved geometry and prints the
  matcher and filter result.
- **Calibrate Gem** → **Place cursor + click** a point, and **Check Result Colour** for empty detection.
- **Buy / Sell** → **Show 64 centres** and look at where the dots land.

**Setting up a new buy item**

1. Open the shop, scroll the list **to the top**, open the bag.
2. **Buy** tab → type a **Name**, pick a **Row**, leave scroll at 0 → **Save item**.
3. **Dry run** → look at where the pointer lands. If it is not on your item, raise **Scroll notches**
   and dry run again until it is, then **Save item**.
4. On the Buy card, set the count and press **Start**.

**A sell run**

1. Open the bag.
2. **Sell** tab → click the slots to sell (or **row *N*** for a whole row).
3. **Dry run** and watch the cursor visit them in order.
4. Set **Max slots per run** to something you are comfortable with.
5. **Start** on the Sell card. It clicks the focus point first, then sells highest slot first.

**Running**

- **Start** on a card. Stop with **Stop**, or `F11` / `CapsLock` after clicking the launcher.

---

## Troubleshooting

| Symptom | Likely cause / fix |
|---|---|
| Start does nothing, message box "Arduino not found" | Wrong port/VID/PID, or the device is unplugged or on a charge-only cable — check the **Arduino** tab. |
| A tool start did nothing at all, and the cursor did move | The game was not focused, or a coordinate is wrong. Those look identical, because the transaction is fire-and-forget — nothing reports back. Check the **Arduino** light, then use **Dry run**. |
| Capture shows the launcher, or a partly black image | The game must be visible; move the launcher off it. **Diagnose capture** reports the rects. |
| Check OCR reads the wrong text | The tuning window moved, or another in-game window covers it. Re-capture and re-drag. |
| The Buy dry run parks on the wrong item | The shop list was not at the top when the run started, or the scroll amount is wrong. Scroll the list to the top first, then adjust **Scroll notches**. |
| Buy clicks a row but nothing happens | The list has scrolled, so the row is not where the preset says. Nothing detects this — check the list is at the top. |
| Sell sells the wrong stack | **Stop immediately.** A stale or wrong selection, or a bad bag grid — run **Show 64 centres** in **Buy / Sell** and confirm the dots sit on the slots. |
| The focus point click does something in-game | It is marked over a live control. Re-mark it somewhere inert — that point is really left-clicked at the start of every run. |
| Composer clicks drift | "Enhance pointer precision" is on, or the Arduino or pointer speed changed. Fix the setting, then re-tune the counts with **Send**. Or switch **Composer move mode** to `arduino`, which re-aims every move. |
| Everything is off after moving to a new monitor or resolution | Expected. Open **Setup**: if the scale or client size differs, recalibrate. |
| Spammer never presses a key | The key is not a single letter/digit or F1–F12 — the card shows a `⚠`. |
| Hotkeys dead while playing | Expected — focus the launcher first. See the Hotkeys tab. |
| Empty detection never advances the grade | The saved empty reference may have been captured with a gem in the box. Re-save with the box empty. See [CALIBRATION.md](CALIBRATION.md). |
