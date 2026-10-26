# Seal Tools v3.0 — User Guide

English · [繁體中文](USER_GUIDE.zh-TW.md)

Every card, tab and button in the v3 shell: what it does and when to use it. Installing for the first
time is [INSTALL.md](INSTALL.md); for *why* the coordinates work the way they do, see
`v3/docs/spec/CALIBRATION.md`; for which setting lives in which file, `v3/docs/spec/CONFIG.md`.

**Nothing in v3.0 needs a reflash.** The Arduino sketch is unchanged since v2.15 (firmware level 2):
v3.0 publishes the same `seal_mouse.ino` in `SealTools-v3.0-firmware.zip`, so a board that already
runs the v2 firmware keeps working. The firmware zip is there for a new board, not for an update.

> **One tool at a time — except the Pet Feeder.** All tools share the single Arduino, so starting one
> is refused while another runs (it is not auto-stopped). The **Pet Feeder** is the exception: it is
> *resident*, so starting another tool leaves it armed on its schedule, and it waits for the game when a
> reload comes due while that tool is using it.

---

## Contents

1. [What v3 is](#1-what-v3-is)
2. [The window](#2-the-window)
3. [First run: configure, then calibrate](#3-first-run-configure-then-calibrate)
4. [The cards: seven tools and what a run is](#4-the-cards-seven-tools-and-what-a-run-is)
5. [The tool tabs](#5-the-tool-tabs)
6. [Calibration: the 18 marks](#6-calibration-the-18-marks)
7. [Logs and files](#7-logs-and-files)
8. [Hotkeys in v3.0](#8-hotkeys-in-v30)
9. [Typical workflows](#9-typical-workflows)
10. [Troubleshooting](#10-troubleshooting)
11. [What is not built in v3.0](#11-what-is-not-built-in-v30)
12. [Where the rest is documented](#12-where-the-rest-is-documented)

---

## 1. What v3 is

Seal Tools drives the Seal Online client through an **Arduino Pro Micro**: the board presents as a
real USB mouse and keyboard, so the game accepts its clicks and keypresses where it blocks synthetic
input from software. OCR reads what the game shows (the tuner's grade and attributes, the gem result
box, the pet panel and time lines), and the tools decide from that.

v3 is a **new frontend over the same tool behaviour** as v2:

- `SealTools.exe` is the window you use. It starts `seal-core.exe`, a background process in the same
  folder, and talks to it over a local protocol. You never start the core by hand.
- The tools, their refusal sentences and their written files are v2's behaviour; the **window** is new
  (a tabbed shell instead of v2's card window with five calibration tabs).
- The firmware, `config/local.yaml`'s format and the measured coordinates are unchanged. A v2 install
  becomes a v3 install by copying its `config/local.yaml` and calibration images — see
  [Migrating from v2](INSTALL.md#migrating-from-v2).

**v3.0 has no global hotkey poller.** Hotkey *names* can be edited and saved (section 8), but nothing
listens for F12/F11-style keys in game yet. **Stop and every button in the shell always work.** This
is a deliberate deferral: whether a background process can see keys while the game is focused has to be
measured first, because the anti-cheat may block it.

---

## 2. The window

The window opens showing **Cards** at the top and a row of tabs. Along the bottom is the **status
strip**. When the core stops or cannot start, the whole page becomes the **crash view**.

- **Title bar / app bar:** `Seal Tools v3`, the **Hold Space** toggle on the right, and the small
  product version `v3.0.0`. (The shell↔core protocol client identity is a separate engineering value
  documented in `PROTOCOL.md`; it does not follow the product version.)
- **The tab strip:** `Cards`, `Spammer`, `Tuner`, `Gem`, `Buy/Sell`, `Quest`, `Pet`, `Attributes`,
  `Arduino`, `Hotkeys`, `Setup`, `Calibrate` — the Cards page plus eleven tabs.
- **The status strip:** `core <version> · rev <n>`, the **root** folder the core is using, and the
  **Logs** button.
- **The crash view:** shown instead of the page when the core is not running. It prints the failure,
  the root, and the core's **stderr tail**, with a **Restart core** button (except for a protocol
  version mismatch, which needs a matched pair of binaries, not a restart).

### 2.1 The Logs panel

**Logs (N)** in the status strip opens the live log panel and reads **Hide logs** while it is open.
The panel shows the `log` lines the core publishes (tool messages, refusals worth surfacing, warnings);
`N` is how many lines it holds. This is the live view. The durable per-run files are in
`<root>/logs/` — see section 7.

### 2.2 The Cards page

The Cards page is a grid of seven cards — Skill Spammer, Magic Tuner, Buy, Sell, Gem Composer, Quest
Hand-in, Pet Feeder — plus the Hold Space toggle in the app bar (Hold Space is the eighth tool but has
no card by design; its state and failure lines are reported on the Skill Spammer card). Section 4
explains a run on each card.

### 2.3 Hold Space

The toggle in the app bar holds the **spacebar** down so the game picks items up, and lifts it when you
release the toggle. It is not a card because it has no run to schedule: press **Hold Space**, play, press
**Release Space**. Its lines (including a failed release, which is the one that matters) appear on the
**Skill Spammer** card and in `holdspace.log`. If a release ever fails, the card says so and tells you
to tap the spacebar once in game.

---

## 3. First run: configure, then calibrate

The installer has already copied a seed `config/local.yaml` for you if you had none. That seed contains
**placeholder measurements from an old setup**, and they will be wrong on your machine. The order is:
**Setup → Calibrate → run a tool.**

### 3.1 Setup

Open **Setup**. It saves four things in one action:

| Field | What it means |
|---|---|
| **Game window title** | The window to find. Matching is a **substring**, so the client title's distinctive part is enough. |
| **Client variant** | `tw`, `us`, or a variant name your config defines (`game.variants`). The variant supplies the window title **and** which attribute dictionary is loaded. Leave it on the right one: a wrong variant is "game window not found" or "the tuner matches nothing". |
| **Arduino VID / PID list / Baud** | How the board is found when no port is named. The shipped values fit the Pro Micro used here; change them only for a different board. |
| **COM port override** | Optional. Leave empty to find the board by its IDs. A **named port that does not exist is a refusal**, not a silent fallback to another board. |

The **read-only** section shows the title and variant currently in effect (and the dictionary file the
variant names). The three **Actions** — **Find window**, **Measure**, **Probe board** — ask the core to
check the title, report the measured frame/client rectangles, or report the board's firmware reply.
Their lines are the core's own, verbatim. Press **Save** when the fields are right; a variant switch is
resolved, validated, its dictionary loaded, and only then written, so it cannot half-switch.

### 3.2 The calibration gate

Tools refuse to start until the marks they use are calibrated, and the refusal names the tab to open
("The bag grid isn't calibrated — Calibrate Buy/Sell."). That is not a bug: a click at a guessed
coordinate can buy or sell the wrong thing. The **Calibrate** tab (section 6) is the one place marks are
drawn for every tool.

Before calibrating anything that moves the cursor with hand-tuned counts (the Gem Composer's `tuned`
move mode), turn **off** Windows' *Enhance pointer precision*:

`Settings → Bluetooth & devices → Mouse → Additional mouse options → Pointer Options` → uncheck
**Enhance pointer precision**. It changes how far a relative move travels, so the game's cursor lands
somewhere else as soon as it is on.

### 3.3 Hotkeys (names only in v3.0)

The **Hotkeys** tab edits the four names — Start, Quit, Advance grade, Pause. The core translates the
name you type to the virtual-key code the config stores and names the code back to you; a typo gets a
line under the box and the save is refused. **The codes are stored but not polled in v3.0** — see
section 8.

---

## 4. The cards: seven tools and what a run is

Each card shows the tool's **name**, its **phase** (`idle`, `starting`, `running`, `stopping`), the
core's **message** for the current step, the **rows** the tool publishes (name / status / detail / next
action), and the **last finished line** or **refusal line** — both verbatim, in the core's words. A
refusal is not a crash: it says what to fix ("No buy preset selected.", "The MAX button isn't
calibrated — Calibrate Buy/Sell.").

| Card | What a run does |
|---|---|
| **Skill Spammer** | Presses the active preset's cooldown-governed rotation: the opening sequence, then combos (each step must be atomic), then the singles on their own cooldowns. It has no natural end; it runs until Stop. It stands down when another tool needs the game. A leading `*` in a key means the fast tap. |
| **Magic Tuner** | Clicks the 發條 button through the board, OCRs the result panel, and repeats until the target grade and filter match, the retry budget runs out, the same result repeats three times, the cursor guard trips, the panel cannot be read, or you press Stop. |
| **Buy** | One counted pass: focus the game, scroll the shop list for the selected preset, then for each of the count: click the row, click MAX, Enter, Enter. Finished line `Bought {n}x '{item}'.`; stops by itself. |
| **Sell** | One counted pass over the prepared slot selection, highest slot first, capped by the sell cap. Finished line `Sold {n} slot(s).`; stops by itself. Selling cannot be undone. |
| **Gem Composer** | Selects a grade, moves to Register, combines; an empty result box (after the configured empty streak) advances N → G → DG, and the run ends by itself after the last grade. |
| **Quest Hand-in** | Replays your flow (or sequence) at the quest NPC the configured number of times, checks the cursor guard, and ends with `DONE — {n} loop(s)…`. It reads nothing from the screen. |
| **Pet Feeder** | The resident tool: keeps each ticked breeding row fed by reloading the feeder before the pet runs out. It publishes one row line per breeding row with its schedule, and keeps running (and waiting) until Stop. The next reload waits while another tool has the game. |

**Starting and stopping.** **Start** is enabled when the core is ready, the card is idle, and no other
tool is engaged; **Stop** is enabled while this tool is engaged. Both are disabled while a start/stop
is in flight. Starting a second tool is **refused** with the holder's name ("… is using the game right
now — stop it, then try again."), not by stopping the first one for you.

**The resident Pet Feeder.** Starting another tool leaves the feeder armed; a reload that comes due
while that tool runs waits, and the feeder's card says it is waiting. This is why a quit/stop of the
foreground tool never ends a four-row feeding schedule.

**The Buy card's run choice (v3.0).** The Buy card carries **Buy item** (one of `buy_sell.presets`) and
a **count** stepper. They choose **this run only** — nothing is saved to the config, so the next run
starts from the preset's configured count again. With no preset configured the card's Start sends the
plain start and the core's "No buy preset selected." appears under the card; add a preset on the
Buy/Sell tab.

**The Quest card's pickers.** **Target flow** and **Sequence** are different: they *are* config fields,
so changing one saves it (`quest.active` / `quest.active_sequence`) through the core and the core's
validation applies (a sequence naming a missing flow is refused). **Single flow only** is the empty
sequence choice.

**Live countdown.** A card with a scheduled next action shows the core's published time, counted down
once a second; it is a display of the core's value, not a second schedule.

---

## 5. The tool tabs

The tabs edit settings; the **core** merges and writes them, and every refusal line is shown as-is.
Changes are saved with the tab's **Save** button (some cards' pickers save immediately — the Buy card's
run choice is the exception and never saves).

### 5.1 Spammer

Presets of key rotations. **Presets** lists them; you can **Add**, **Rename**, **Duplicate** and
**Delete** a preset, and **Active preset (runs)** is the one the card's Start uses. Under a preset:

- **Opening** — **Lead-in (s)** and the ordered **Opening step (s)** entries.
- **Keys — preset (priority order)** — each **Key** name and its **Seconds** cooldown, in priority
  order; unsupported keys are reported by name.
- **Combos** — grouped keys that must go out together, with the gap between their steps.

A leading `*` is the fast tap form. A missing active preset or an empty preset list is refused at
Start with a line naming the tab.

### 5.2 Tuner

The tuner's portable settings; the calibrated geometry block on this tab is **read-only in this wave**
(it is owned by config until the three-box calibration session lands — section 6.4).

- **Grades** — the grade list, the **Target grade**, and the **require_grade** half of the filter.
- **Retries and timing** — **Max retries**, **OCR retries**, **Click/enter delay (s)**, **OCR delay
  (s)**, **Save captures** (writes `logs/captures/` PNGs — see section 7).
- **Springs** — **Spring mode** (`hid` places the cursor on the calibrated spring point; `manual` leaves
  it where it is), **Mouse guard** (`off`/`stop`/`recenter`) and its **Guard px** and **Recenter max**.
- **Filter** — enabled, match mode (`any`/`all`/`per_attr`), require-grade and the attribute rules
  (name, count, min, max), plus override rules. The names must match the loaded dictionary — check the
  **Attributes** tab.

### 5.3 Gem

The gem composer's grades and behaviour; the calibrated **Grade positions**, **Resource slots** and
**Result gem area** are **read-only this wave** (draw them in **Calibrate**).

- **Grades** — the list and the **Start grade** to begin at.
- **Empty result** — **Empty mode** (`stop`, `advance_grade`, or `advance_grade_clear` — advance after
  clearing the resource slots first), **Empty streak** (how many consecutive empty reads before acting,
  so one bad read cannot advance a grade), **Colored gap min** and **Save empty captures**.
- **Movement** — **Move mode**: `tuned` sends hand-tuned relative moves (turn off *Enhance pointer
  precision*), `arduino` places the cursor on each route's calibrated destination point.

### 5.4 Buy/Sell

The calibrated geometry block here is **read-only this wave** — draw those marks in **Calibrate**.

- **Buy presets** — item name, then **Scroll notches**, **Row** and **Count per run**. Add/delete
  presets with **Add item** / **Delete item**. The Buy card picks one for a run (not saved).
- **Sell** — **Max slots per run (sell cap)**, the hard ceiling on one sell pass. **Note:** the editor
  for *which* bag slots to sell is one of the not-built surfaces in v3.0 (section 11); without a
  selection the Sell card refuses with "No slots are selected to sell.", and the core never runs past
  the cap.

### 5.5 Quest

Named **flows** replay fixed click/key sequences at the quest NPC; **sequences** run several flows in a
row. The tab has three parts:

- **Active** — **Active flow** and **Active sequence (empty = single flow only)**. These are what the
  Quest card's pickers edit too.
- **Flows** — per flow: the **Initial wait (s)**, **Loops**, and an ordered step list of **Action**
  (`click` / `right_click` / `key` / `enter` / `wait`; `wait` is the no-input pause), **Key value** and
  **Delay (s)**, with move up/move down/remove. Add and delete flows by name.
- **Sequences** — per sequence: **Initial wait (s)**, **Master loops**, and the flow entries with their
  **Flow** name and **Times**. The core refuses a save whose sequence names a flow that does not exist.

The run only clicks where **you** put the cursor during the positioning countdown, and it watches the
cursor against **Cursor guard px**; moving the mouse past the guard stops it.

### 5.6 Pet

The feeder's **portable** settings — the machine rows, crops and marks stay **read-only** until the
pet setup/calibration surfaces land (sections 6 and 11).

- **Feeding** — **Items per minute (fallback rate)** (used only when the boarding window's own rate
  line cannot be read) and **Occupied above (0–1 distance)** (how unlike the empty reference a slot
  must look before it counts as occupied).
- **Machine rows — read-only this wave:** the configured breeding rows (enabled/disabled, stacks, feeder
  strip), and the **Queue** count.
- **Calibration crops — read-only this wave:** whether the boarding-X crop and food-icon crop exist,
  the boarding bag grid, the food slots, the return slot and the food-load mode.

A fresh install has no breeding rows, and v3.0 has no row editor yet: set the feeder up by migrating
your v2 `config/local.yaml` (which carries them) or by hand-editing that block with
`v3/docs/spec/CONFIG.md` open; otherwise the feeder refuses with its setup line. The **Calibrate** tab
still draws the feeder's marks (`pet_*` kinds); the close-X reference crop is learned by the run
itself.

### 5.7 Attributes

Reads the dictionary the core loaded for the active client — **read-only**. It shows the **Client
variant** and **Dictionary file**, a **Search names and OCR forms** box, the shown/total count, and the
groups of attribute names and their OCR forms. The tuner's filter rules and the pet panel's reads match
the names and forms shown here; if a filter rule does not match, look here first. **Refresh** re-reads
the dictionary from the core.

### 5.8 Arduino

A board test surface, not a setting: **Refresh probe** asks the core to open the board and report the
firmware reply; **Click**, **Move** (dx, dy) and **Send key** send one test action each. Use it to
separate "the board is wrong" from "the app is misconfigured" before blaming a tool. The same probe is
on the **Setup** tab's actions (**Probe board**).

### 5.9 Setup

The window, client variant and board settings, plus the three read-only actions. See section 3.1 — the
same tab is the first-run configuration surface.

### 5.10 Hotkeys

Four key names — **Start**, **Quit**, **Advance grade**, **Pause**. Type a name (`F1–F24`, `Esc`,
`CapsLock`, `Space`, `Tab`, `Enter`, or one key); the core translates it, names the stored code back,
and refuses an unparseable box with a line under it. **In v3.0 the stored codes are not polled** — see
section 8.

### 5.11 Calibrate

The one calibration surface for every tool. Section 6 is the full walkthrough: capture the game
window, choose a **kind**, optionally a **target instance**, draw the mark, **Save mark**, and read the
core's stored mark / derived values / artifact below. The tab's panel caption repeats the capture
behaviour: the shell hides itself for the grab, waits one frame, and reopens.

---

## 6. Calibration: the 18 marks

A calibration mark is stored in **physical pixels relative to the game window's CLIENT area** (origin
= the client's top-left, no title bar), so moving the window does not invalidate it. It is **not**
scaled by the Windows display scale. The contract and the why are `v3/docs/spec/CALIBRATION.md`; this
section is the operator's view.

### 6.1 Saving a mark

1. Put the game window in front (the game must be the foreground window; the shell hides itself for the
   grab, so the capture is of the game, not of Seal Tools).
2. **Capture** — the window blinks while Seal Tools hides itself for one frame, captures the full
   client, and reopens. The frame and the measured display summary appear.
3. Choose a **Kind** from the grouped list. For a *named-instance* kind choose a **Target instance**
   too (the list stays empty until you do; Save is disabled until a target is chosen).
4. Draw on the picture (a click sets a point; a drag sets a rectangle) or type the **x / y / (w / h)**
   fields — the fields are authoritative and the picture mirrors them.
5. For **Result gem area** answer the **empty-result question** first: is the result box empty right
   now? **Save is disabled until you answer.** `empty: true` saves the crop *and* samples the colour
   signature; `empty: false` samples the signature only (a crop of a busy box would poison the strong
   empty test).
6. **Save mark**. The result line shows the kind, the revision, and the mark; a `bag_grid` also shows
   its derived pitch, and the result-gem crop appears under **Artifact**. **Refresh stored** re-reads
   the stored marks.
7. The **Stored mark** section shows the mark for the selected kind/target (and the target the core
   echoed), or "Not calibrated yet".

A refusal is the core's sentence, not a silent failure: the bag slot must agree with the bag grid's
pitch ("two numbers that cannot both be right are a bad drag"), a target the config does not carry is
refused, and an unsaved mark writes nothing.

### 6.2 The kinds, grouped

The picker groups them so 18 entries stay readable. **One tool each:**

| Group | Kind | Shape | Used by |
|---|---|---|---|
| Shop | **Bag grid** | rect (the whole 8×8 bag) | Buy/Sell — derives all 64 slot centres; its pitch is checked against the bag slot |
| Shop | **Bag slot** | rect (one slot) | Buy/Sell — the independent pitch check |
| Shop | **Shop list region** | rect (the visible rows) | Buy/Sell — row tops/bottoms derive from it |
| Shop | **Focus / scroll point** | point | Buy/Sell — the inert click that focuses the game and aims the wheel; must not sit on a row or slot |
| Shop | **MAX button** | point | Buy/Sell and the pet count dialog |
| Gem | **Result gem area** | rect + empty verdict | Gem Composer — the crop/signature the empty test compares |
| Gem | **Grade point** | point | Gem Composer — named instances N, G, DG, Register, Combine |
| Gem | **Resource slot point** | point | Gem Composer — named instances "1".."3" |
| Pet — points | **目錄 (menu) point** | point | Pet Feeder |
| Pet — points | **Pet feed icon point** | point | Pet Feeder |
| Pet — points | **Boarding X point** | point | Pet Feeder |
| Pet — points | **Bag page tab point** | point | Pet Feeder — targets ITEM1..ITEM3 |
| Pet — bag | **Boarding bag grid** | rect | Pet Feeder — its OWN grid: the boarding bag sits elsewhere on screen |
| Pet — bag | **Boarding bag slot** | rect | Pet Feeder — one slot of that grid |
| Pet — rows | **Row start/end button** | rect | Pet Feeder — targets "1".."4" |
| Pet — rows | **Row boarding pet slot** | rect | Pet Feeder — targets "1".."4" |
| Pet — rows | **Row food strip** | rect | Pet Feeder — targets "1".."4" (divided by the row's stack count) |
| Tuner | **Spring point** | point | Tuner — only read in `spring_mode: hid` |

**Named instances.** A kind that marks a repeated thing takes a **target** — the player-facing instance
name, not an array index: `pet_page_tab` takes `ITEM1`…`ITEM3`; the three pet-row kinds take row `"1"`…
`"4"`; `gem_grade` takes `N`, `G`, `DG`, `Register`, `Combine`; `gem_resource` takes `"1"`…`"3"`. A
target the config does not carry is refused (add the row/grade first), and saving one row's mark leaves
that row's stacks, crops and other marks untouched. Switching kind resets the target, so a mark can
never be saved against the wrong instance by accident.

### 6.3 The gem empty question

**Result gem area** is the only kind that asks a question. Answer it from the live game before saving:

- **The result box is EMPTY right now** → save the crop and sample `empty_signature`. This is the
  strong test the composer uses to decide a grade is done.
- **A gem is in the box** → sample the colour signature only, no crop. The composer still has the
  colour fallback; it just does not have the strong crop test.

One answer is not "which is better": it is "what is in the box at the moment you captured". You can
re-save the same kind later with the box empty to add the crop.

### 6.4 What is not calibratable yet

These are **not missing marks you can hand-wave**: each needs a different kind of session, and v3.0
deliberately offers no fake one.

- **The tuner's three-box geometry** — v2 dragged a grade area, an attribute band and a remaining band
  and the core *derived* the OCR region, the bands and the row height from them. v3.0 has only the
  **Spring point** kind; there is no `tuner_region`, and the tuner OCR geometry is still configured in
  `local.yaml` (`tuner.ocr`) until that session exists. The Calibrate tab says so in its tuner note.
- **The tooltip panel** — a hover offset measured by parking, hovering and capturing cannot be
  expressed as one frame mark; hand-edited from v2 config.
- **The pet's reference crops and food cells** — live-screen session features (the close-X crop is even
  learned by the run), not frame marks.
- **The sell-slot selection** — a bag-cell editor, not a calibration mark.
- **The environment check (Detect)** — the comparison rule exists in the core, but v3.0's Setup surfaces
  do not show a "your display moved" result yet. If you change monitor, scale, window size or move to a
  new PC, **recalibrate**; the marks are physical offsets and a proportional transform is deliberately
  not applied (the game UI does not scale linearly).

---

## 7. Logs and files

### 7.1 The live log panel

The **Logs (N)** button in the status strip shows the lines the core publishes while it runs. It is for
*watching* a run; it is not the durable record.

### 7.2 Per-run files: `<root>/logs/`

`<root>` is the install folder the shell told the core to use (the status strip prints it). The core
appends; it never truncates a log:

| File | What it holds |
|---|---|
| `<tool>.log` | One line per started run, per **changed** status message, and the final reason. Tool ids: `spammer`, `tuner`, `buy`, `sell`, `gem`, `quest`, `pet`. This is the first file to read when "it did something unexpected". |
| `holdspace.log` | Hold Space's own diagnostic trail (its shape is v2's `HoldDiag`; a failed release lands here). |
| `error.log` | Internal errors and tool-opening failures. Player-correctable refusals are **not** here — they are in the card, by design, so they do not bury real errors. |
| `ocr_log.jsonl` | One JSON record per completed OCR scan. |
| `ocr_errors.jsonl` | A completed scan that did not produce what its mode asked for. |
| `captures/capture_<stamp>.png` | Only while **Save captures** is on (Tuner/Gem). Each record names its capture in `capture_path`. |

A failed log write is swallowed by design: it never fails a command or a run. `shutdown` flushes the
files.

### 7.3 `config/` and `docs/`

- `config/defaults.yaml.example` — the shipped template (portable settings); read as the base.
- `config/local.yaml` — **your machine's state**: calibration, presets, the portable half's saved
  values. The core is its only writer; never share it; keep it when you update.
- `config/attributes.yaml` and `config/attributes.<variant>.yaml` — the OCR dictionaries. The TW one is
  release-maintained and hand-editable; the US one is seeded from `.example` and grown in place.
- `config/calib_gem_result.png` — the result-gem crop artifact (when saved with an empty box).
- `docs/pet-data.csv` — the feeding table the feeder reads from `<root>/docs/pet-data.csv`; without it
  the feeding estimate is unavailable and the schedule falls back to the configured cycle.

The full file split, loading order and fold are `v3/docs/spec/CONFIG.md`.

---

## 8. Hotkeys in v3.0

**There is no global hotkey poller in v3.0.** The four hotkeys can be named, translated and saved on
the Hotkeys tab, and the codes are stored in `local.yaml`, but **nothing watches for those keys while
you play**, so pressing F12/F11/"the start key" in game does nothing yet.

What this means in practice:

- **Stop a run with the card's Stop button** (or the shell's own controls) — they always work.
- The Pet Feeder is resident; it can be left running and stopped from its card or by closing Seal Tools.
- Do not rely on a "quit/pause key" in v3.0; those behaviours return only with the poller.

This is a deliberate deferral, not a regression hidden in the docs: the plan requires a measurement of
whether a background process can see keys while the game is focused (the anti-cheat may block it), and
the protocol gains the commands that measurement chooses. It is listed in `v3/docs/backlog/OPEN.md`.

---

## 9. Typical workflows

**Once per machine (≈15 minutes plus calibration):**

1. Install and launch — see [INSTALL.md](INSTALL.md).
2. **Setup**: game window title, client variant, board IDs; **Save**; then **Probe board**.
3. **Calibrate**: capture, then draw the marks for the tools you will use. Start with the shop group if
   you Buy/Sell, then Gem, then the pet kinds if you feed. Each kind is one save.
4. Run one short tool to confirm the board and the game respond. `logs/<tool>.log` proves what it did.

**Daily use:**

1. Start the game, windowed, and log in.
2. Open Seal Tools; the core connects (status strip shows `core … · rev …`).
3. On the tool's tab set what the run needs (spammer preset, tuner target/filter, gem start grade,
   quest flow, pet rows), then **Start** on the card. **Stop** when done.
4. For Buy/Sell use the card's run choice (Buy) or the prepared selection (Sell) and remember: Sell is
   irreversible and capped.
5. Close Seal Tools when finished — closing the UI stops the tools and the core. One copy at a time
   (there is no single-instance guard in v3.0).

**After a display change** (new monitor, Windows scale, game window size, new PC): recalibrate. Stored
marks are physical client-relative offsets; they do not travel.

---

## 10. Troubleshooting

| Symptom | What it means / what to do |
|---|---|
| Whole window says **"The core is not running"** with **Stderr tail** and **Restart core** | The core crashed or its boot was refused. The tail is the reason (a boot refusal prints to stderr, not as a protocol message). **Restart core** starts a fresh process and handshake. A **protocol mismatch** (`bad_version`) hides Restart on purpose: replace both `SealTools.exe` and `seal-core.exe` with a matched pair; do not mix versions. |
| `bad_version` | You mixed binaries. Use one zip's pair. |
| **game window not found** | Wrong window title or variant (`Settings → Game window title` / `Client variant`), or the game is not running/minimised. **Find window** on Setup reports what the core sees. |
| **`game_not_foreground`** | Another window is in front of the game. Bring the game to the front and retry; the Calibrate tab's capture hides the shell for one frame itself. |
| **`ocr_unavailable`** | The OCR models were not found or would not load. `detail.reason` says `models_env_unset`, `models_dir_missing` or `model_load_failed:<file>`, and `detail.message` names **every directory tried**. Resolution order: `SEAL_OCR_MODELS` (if set it wins or fails loudly), then `<root>/models`, then the folder `seal-core.exe` is in + `models`; a directory counts only with all three `.onnx` files. Restore the shipped `models/` folder (or set `SEAL_OCR_MODELS` for a custom location). |
| **`board_unavailable`** | No board on the configured port, or the port cannot be opened. Check the cable (data, not charge-only), the board's USB driver, the **VID/PID/port** on Setup, and press **Probe board**. Only one program may hold the COM port. |
| A card line like "The bag grid isn't calibrated — Calibrate Buy/Sell." | Not a bug; the mark that tool needs is missing. Draw it in **Calibrate** (the line names the tool's group). |
| "No buy preset selected." / "No slots are selected to sell." | Add/choose a buy preset on Buy/Sell (the card's picker). Sell needs its slot selection, which v3.0 has no editor for (section 11): a migrated selection can be present, otherwise the tool cannot run. |
| "The active preset … was not found — pick one in the Spammer tab." | Set **Active preset (runs)** on the Spammer tab. |
| Pet Feeder refuses on start (a `… isn't calibrated` or `No breeding row is set up` line) | The feeder's marks and rows must exist. Calibrate its `pet_*` kinds; a new install has no rows yet (section 11) — migrate a v2 `local.yaml` or edit the block per CONFIG.md. |
| Hotkeys do not fire in game | Expected in v3.0 — there is no poller (section 8). Use Stop in the shell. |
| "The window blinks when I press Capture" | Expected: the shell hides itself for one frame so the capture is of the game, then reopens. v2 blinked ~0.3 s; v3 blinks for a frame plus the round-trip. |
| Need the last run's detail | Read `<root>/logs/<tool>.log` (status strip prints the root); `error.log` and `ocr_errors.jsonl` are beside it. |

---

## 11. What is not built in v3.0

Named here so a missing capability is a known boundary, not a mystery. The living list is
`v3/docs/backlog/OPEN.md`; every item below is "not in 3.0", not "planned for tomorrow" unless the
repository says so.

- **Global hotkeys.** No poller; see section 8. Stop and every shell control work.
- **Mini mode / pin-on-top / hide-the-cards.** v2's card-window chrome is gone; v3's window is the tab
  shell, and a compact mode is a later layout decision.
- **The tuner three-box calibration session.** Only the spring point is calibratable; `tuner.ocr`
  geometry is hand-edited from config until the session exists.
- **The tooltip (hover-panel) two-capture session** — hand-edited from v2 config.
- **The pet reference-crop / food-cell editor and the pet row/queue setup UI.** The Pet tab shows
  machine rows read-only in v3.0; migrated v2 configs carry them, fresh installs need the editor (or
  hand-editing) before the feeder can board.
- **The sell-slot selection editor.** The cap is editable; selecting which bag slots to sell is not yet.
- **The environment check (Detect) surface** — recalibrate manually after a display change.
- **A single-instance guard.** The protocol's instance lock is specified but not built: do not run two
  copies of Seal Tools at once.
- **An installer and auto-update.** v3.0 is a zip; updating replaces program files and keeps your
  `config/`.
- **v2-only chrome and docs.** v2's WPF screenshots are not used here; the v2 docs remain the record of
  the v2 build, not a description of this window.

---

## 12. Where the rest is documented

- `v3/docs/INSTALL.md` — install, migrate, update, uninstall, troubleshooting (same folder as this file).
- `v3/docs/spec/CONFIG.md` — every config file, which one is yours, and the loading/fold rules.
- `v3/docs/spec/CALIBRATION.md` — the coordinate contract, why DPI/scale work the way they do, and the
  artifact rules. v2's `docs/spec/COORDINATES.md` and `CALIBRATION.md` are the measurement record.
- `v3/docs/spec/PROTOCOL.md` — the shell↔core contract (engineers; useful when reading a refusal code).
- `v3/docs/spec/TOOL-FLOWS.md` — what each tool decides, and the exact player lines it can produce.
- `v3/docs/backlog/OPEN.md` — the live not-built list, including the global-hotkey measurement.
