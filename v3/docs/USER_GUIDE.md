# Seal Tools v3.0 — User Guide

English · [繁體中文](USER_GUIDE.zh-TW.md)

Every card, tab and button in the v3 shell: what it does and when to use it. Installing for the first
time is [INSTALL.md](INSTALL.md). For *why* the coordinates work the way they do and for which setting
lives in which file, the specification lives in the **project's repository docs** —
`v3/docs/spec/CALIBRATION.md` and `v3/docs/spec/CONFIG.md`. (This zip ships only this guide, its zh-TW
twin, `INSTALL.md`, `THIRD-PARTY-NOTICES.md` and `docs/pet-data.csv`; it does not carry the spec tree.)

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
4. [The cards: eight tools and what a run is](#4-the-cards-eight-tools-and-what-a-run-is)
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

The window opens on the **Cards** screen. A **left rail** holds the navigation in three groups —
**RUN** (Cards), **TOOLS** (Skill Spammer, Magic Tuner, Gem Composer, Buy, Sell, Quest Hand-in, Pet
Feeder) and **SYSTEM** (Attributes, Arduino, Hotkeys, Setup, Calibrate) — and the **status strip** runs
along the bottom. When the core stops or cannot start, the whole page becomes the **crash view**.

- **Top bar:** `Seal Tools`, the **core status line** (`core ready · rev <n> · <window title>`), the
  **Logs** button, the **always-on-top pin** (`Keep Seal Tools on top`) and the small product version. (The shell↔core
  protocol client identity is a separate engineering value documented in `PROTOCOL.md`; it does not
  follow the product version.) There is **no Hold Space toggle any more**: Hold Space is a card on the
  Cards screen (section 2.3). The leftmost button is the **sidebar toggle** (Ctrl+B).
- **The rail:** one row per screen, grouped by job, with a badge on **Cards** counting the running
  tools. The rail replaced the twelve-tab strip; a running tool never moves the rail. The top bar's
  sidebar button (or Ctrl+B) hides the rail so the content uses the whole width; the rail returns on
  the next launch (the choice is session-only).
- **The status strip:** `core <version> · rev <n>` and the **root** folder the core is using on the
  left, and the `Ctrl+B sidebar · Ctrl+L logs` hints on the right.


**The always-on-top pin.** A filled pin keeps the shell window above the game and other windows;
pressing it applies immediately and saves the portable setting `settings.ui.always_on_top`, so the
next launch starts in the same state. It is a config preference, not part of the Calibrate hide/show
path. If the save is refused the pin returns to the saved value and the core's line is shown; check
the pin when the window will not stay in front.

**The window remembers its placement and size.** After the core handshake the shell puts the window
back where it was last left, from the machine `ui` block in `local.yaml` (`left`, `top`, `width`,
`expanded_height` — this machine's state, not the portable `settings:` half; `expanded_height` is the
height to restore, not the current height). Moving or resizing saves that placement automatically
about 0.6 s after it settles, through the normal `ui.window` command, so the core remains the only
file writer. A saved rectangle that no longer fits any attached monitor is moved into the primary
monitor's work area (a log line says it was clamped), so the window cannot open off-screen.
`ui.pinned` is carried through unchanged and is **not** the app-bar pin; that is the portable
`settings.ui.always_on_top` above.

Three honest limits: there is no clean-shutdown save hook, so a move made in the last fraction of a
second before closing can be lost (move or resize, then pause a moment before closing); a maximized
window reopens at its geometry — the maximized state itself is not remembered; and mixed-DPI
multi-monitor setups make that clamp approximate.

### 2.1 The Logs drawer

**Logs (N)** in the top bar (or **Ctrl+L**) opens the logs drawer along the bottom of the content;
press either again to close it, or press **Esc**. It shows the `log` lines the core publishes (tool messages, refusals
worth surfacing, warnings) with a level badge per line and an **All / Errors** filter. `N` is how many
lines it holds. This is the live view; the durable per-run files are in `<root>/logs/` — see section 7.

### 2.2 The Cards screen

The Cards screen opens with a **Run** header (the running count sits on the right) and answers "what
is running and is it healthy?" first. Running tools appear as **full cards** above — a success accent,
a phase chip, the core's message, the row table and a Stop button. Everything else is a **one-line
idle tile** with an icon, a short "Ready · …" summary of what Start would do, and an outlined **Start**
(there are eight tools, including Hold Space; section 4 explains a run on each). Tiles with per-run or
saved choices (Buy, Quest, Spammer) also show a small **tune** button: it opens those choices inside
the card, so the collapsed tile stays one line. When nothing runs, a banner shows the next step — "No
runs yet" on a fresh start, "Everything is idle" otherwise. A card's phase moves only when the core
publishes `state`: a Start shows an in-flight label, never an optimistic phase.

**Mini mode.** A foreground run collapses the window to a 360×96 bar beside the game: the tool name
and phase, the core's latest message, the next moment and **Stop**; when the run finishes the bar
switches to **Full window** instead of popping back over the game. The bar stays on top, remembers
the screen edge you dragged it to for the next run, and the resident Pet Feeder keeps the full
window (it is shown as a paw badge beside the foreground tool). Press **Full window** to return for
the rest of this run; the next run collapses again. While a tool runs, the app bar's
picture-in-picture button collapses manually.

**Quick dock.** When nothing is running, the app bar's dashboard button (or **Setup → Quick dock →
Open quick dock**) collapses the window into a small always-on-top launcher. It has seven quick starts —
Spammer, Buy, Sell, Quest Hand-in, Gem Composer, Magic Tuner and Hold Space — each tile with its own **Start** button and a
separate **⋮ options** button for that tool's per-run options (the tile row itself is not a start
target), plus a status-only row for the Pet Feeder that opens the full window. Buy picks a
preset/count and Sell marks bag cells — click individual slots, or a row's **row N** button to
fill/clear that whole bag line — both apply to the next run only; Spammer and Quest choices save the
active preset, exactly like the Cards pickers; Hold Space uses the same Start/Stop button and has no
options. A running tile shows its phase and the button becomes **Stop**. Use **Collapse to edge** in the dock header to shrink it to a small tab (rail, pill or island); click the tab to expand it again. Starting a tool turns the same
window into the mini run bar; when the run finishes, the bar's dock button returns to the launcher.
Choose **right rail, left rail, bottom right, bottom left or top centre** in Setup or from the dock's
placement menu, and drag the dock to fine-tune it. The placement is remembered for this session only.

### 2.3 Hold Space

Hold Space is its own card now (it used to be a top-bar toggle) because it is a run like any other:
press **Start** on its card to hold the **spacebar** down so the game picks items up, and **Stop** to
release it. It has no configuration screen. Its live state, phase and refusal line are on its card; a
failed release is still recorded **only in `holdspace.log`**, because the release happens as the run
ends and has no protocol event.

---

## 3. First run: configure, then calibrate

There is no installer. On its first launch the core copies a seed `config/local.yaml` for you if you had
none (from `config/local.yaml.example`). That seed contains **placeholder measurements from an old
setup**, and they will be wrong on your machine. The order is: **Setup → Calibrate → run a tool.**

### 3.1 Setup

Open **Setup**. The tab's **Save** writes the client variant; the game window title and the four
Arduino board-matching fields are read-only, and the actions below set the title. The board port
itself is chosen with radio buttons on the **Arduino** tab.

| Field | What it means |
|---|---|
| **Game window title** | **Read-only.** The window to find; matching is a **substring**, so the client title's distinctive part is enough. It is set with **Find window**, then **Use detected title** — the title identifies the game window, so it is not typed. |
| **Client variant** | `tw`, `us`, or a variant name your config defines (`game.variants`). The variant supplies the window title **and** which attribute dictionary is loaded. Leave it on the right one: a wrong variant is "game window not found" or "the tuner matches nothing". |
| **Arduino VID / PID list / Baud** | **Read-only.** How the board is found when no port is named. The shipped values fit the Pro Micro used here; nobody has to read or type them. A different board means editing `config/machine/` + the portable Arduino block by hand (advanced, unsupported in the UI). |
| **COM port override** | **Read-only display.** Empty means "find the board by its IDs". A **named port that does not exist is a refusal**, not a silent fallback to another board; set it by editing the machine config, not from Setup. |

The **read-only** section shows the variant currently in effect (and the dictionary file the variant
names). The title and the four Arduino board-matching fields are read-only too. The **Actions** ask
the core to:

- **Find window** — find the game window and report the detected title;
- **Measure** — report the measured frame/client rectangles;
- **Probe board** — report the board's firmware reply;
- **Use detected title** — save the title the last Find detected (enabled only after a successful Find
  with a non-empty title; this is the one action that saves, through the normal
  `config.validate`/`config.apply` path).

Their lines are the core's own, verbatim. Press **Save** when the fields are right; a variant switch is
resolved, validated, its dictionary loaded, and only then written, so it cannot half-switch.

### 3.2 The calibration gate

Tools refuse to start until the marks they use are calibrated. The refusal names the **mark** to save
and points at the single **Calibrate** tab: choose that kind there (and a target instance when the kind
is a named one) and save it. That is not a bug: a click at a guessed coordinate can buy or sell the wrong
thing. Section 6 is the walkthrough for every kind.

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

## 4. The cards: eight tools and what a run is

Each card shows the tool's **name**, its **phase** (`idle`, `starting`, `running`, `stopping`), the
core's **message** for the current step, the **rows** the tool publishes (name / status / detail / next
action), and the **last finished line** or **refusal line** — both verbatim, in the core's words. A
refusal is not a crash: it says what to fix ("No buy preset selected.", or which mark is missing and
that it is saved on **Calibrate**).

| Card | What a run does |
|---|---|
| **Skill Spammer** | Presses the active preset's cooldown-governed rotation: the opening sequence, then combos (each step must be atomic), then the singles on their own cooldowns. It has no natural end; it runs until Stop. It stands down when another tool needs the game. A leading `*` in a key means the fast tap. The card's **Rotation** picker chooses which preset that is and saves it as the active preset. |
| **Magic Tuner** | Clicks the 發條 button through the board, OCRs the result panel, and repeats until the target grade and filter match, the retry budget runs out, the same result repeats three times, the cursor guard trips, the panel cannot be read, or you press Stop. |
| **Buy** | One counted pass: focus the game, scroll the shop list for the selected preset, then for each of the count: click the row, click MAX, Enter, Enter. Finished line `Bought {n}x '{item}'.`; stops by itself. |
| **Sell** | One counted pass over the slots marked for that run in the Sell grid, highest slot first, capped by the sell cap. Finished line `Sold {n} slot(s).`; stops by itself. Selling cannot be undone. |
| **Gem Composer** | Selects a grade, moves to Register, combines; an empty result box (after the configured empty streak) advances N → G → DG, and the run ends by itself after the last grade. |
| **Quest Hand-in** | Replays your flow (or sequence) at the quest NPC the configured number of times, checks the cursor guard, and ends with `DONE — {n} loop(s)…`. It reads nothing from the screen. |
| **Pet Feeder** | The resident tool: keeps each ticked breeding row fed by reloading the feeder before the pet runs out. It publishes one row line per breeding row with its schedule, and keeps running (and waiting) until Stop. The next reload waits while another tool has the game. |

**Starting and stopping.** **Start** is enabled when the core is ready and **this** card is idle;
**Stop** is enabled while **this** tool is engaged. Both are disabled while a start/stop is in flight.
The shell does **not** disable Start because another tool is running: press it and the core refuses
with its `tool_busy` line naming the holder ("… is using the game right now — stop it, then try
again."), rather than stopping the first tool for you.

**The resident Pet Feeder.** Starting another tool leaves the feeder armed; a reload that comes due
while that tool runs waits, and the feeder's card says it is waiting. This is why a quit/stop of the
foreground tool never ends a four-row feeding schedule. It also persists its own progress as it works
(`food_slots_used` and each row's `boarding_running`), so a restart resumes with the cells and boarding
state it already reached rather than the pre-run values.

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
Changes are saved with the tab's **Save** button (the Quest card's pickers save immediately; the Buy
and Sell cards' run choices never save — they affect that run only).

### 5.1 Spammer

Presets of key rotations. **Presets** lists them; you can **Add**, **Rename**, **Duplicate** and
**Delete** a preset, and **Active preset (runs)** is the one the card's Start uses. Under a preset:

- **Opening** — **Lead-in (s)** and the ordered **Opening step (s)** entries.
- **Keys — preset (priority order)** — each **Key** name and its **Seconds** cooldown, in priority
  order; unsupported keys are reported by name.
- **Combos** — grouped keys that must go out together, with the gap between their steps.

A leading `*` is the fast tap form. A missing active preset or an empty preset list is refused at
Start with a line naming the tab.

The **Skill Spammer card** has a compact **Rotation** picker, v2's card control: choosing a preset
there makes it the active rotation and saves `spammer.active` through the normal config save, so the
choice persists across runs and restarts (v3 deliberately differs from v2's in-memory card here:
the core is the only writer). Start still sends the plain `{"tool":"spammer"}` — there is no per-run
spammer argument — and a refused save shows the core's line under the card. The tab's **Active preset
(runs)** is the same field, so the tab and the card always agree.

**The last preset cannot be deleted.** The **Delete** action stays enabled, but refuses with “The last
preset cannot be deleted.” (the tab's note line) while deleting it would leave a spammer config with no
rotation to press. Create another preset first; the guard is v2's, and the draft is untouched by the
refused action.

### 5.2 Tuner

The tuner's portable settings; the calibrated geometry block on this tab is **read-only in this wave**
(it is owned by config until the three-box calibration session lands — section 6.4).

- **Grades** — the **fixed grade order**, read-only (a chip row); the **Target grade** and the
  **require_grade** half of the filter are selected from it.
- **Retries and timing** — **Max retries**, **OCR retries**, **Click/enter delay (s)**, **OCR delay
  (s)**, **Save captures** (writes `logs/captures/` PNGs — see section 7). **OCR retries is stored but
  currently unused:** v3.0's reader scans once; the last-retry forced capture is a named not-built item
  in the repository's backlog.
- **Springs** — **Spring mode** (`hid` places the cursor on the calibrated spring point; `manual` leaves
  it where it is), **Mouse guard** (`off`/`stop`/`recenter`) and its **Guard px** and **Recenter max**.
- **Filter** — enabled, match mode (`any`/`all`/`per_attr`), require-grade and the attribute rules
  (**name**, **count**, **min**, **max**), plus override rules. The **name** is selected from the loaded
  dictionary's attribute names (the **Attributes** tab lists them), not typed: a stored name outside the
  dictionary stays selected, and **Add** is disabled with the reason shown while the dictionary is
  loading, failed, or has no names.

**Clean up capture images** sits under the **Save captures** switch. It asks for confirmation (“Delete
the capture images?”), then deletes only the saved OCR capture PNGs under the root's `logs/captures`
and reports “Deleted N capture image(s) from logs/captures.”; the rest of the log is kept. It is
housekeeping with no game or board access, so it stays available while a tool runs.

### 5.3 Gem

The gem composer's grades and behaviour; the calibrated **Grade positions**, **Resource slots** and
**Result gem area** are **read-only this wave** (draw them in **Calibrate**).

- **Grades** — the **fixed grade ladder**, read-only (a chip row); the **Start grade** is selected from
  it.
- **Empty result** — **Empty mode** (`stop`, `advance_grade`, or `advance_grade_clear` — advance after
  clearing the resource slots first), **Empty streak** (how many consecutive empty reads before acting,
  so one bad read cannot advance a grade), **Colored gap min** and **Save empty captures**.
- **Movement** — **Move mode**: `tuned` sends hand-tuned relative moves (turn off *Enhance pointer
  precision*), `arduino` places the cursor on each route's calibrated destination point.

### 5.4 Buy/Sell

The calibrated geometry block here is **read-only this wave** — draw those marks in **Calibrate**.
The sell-slot *selection* is editable, but it needs the calibrated **Bag grid** to place its cells and
it is for the next run only.

- **Buy presets** — item name, then **Scroll notches**, **Row** and **Count per run**. **Row** is a
  **Row 1**…**Row N** picker derived from the calibrated shop list (labels are 1-based; the config stores
  the 0-based row index the geometry uses), so the off-by-one number cannot be typed. A preset migrated
  with a row past the calibrated list shows that row marked “past the bottom of the list” — plus the
  warning naming the visible row count — and it cannot be picked again; choose a lower row. Add/delete
  presets with **Add item** / **Delete item**. The Buy card picks one for a run (not saved).
- **Sell** — **Max slots per run (sell cap)**, the hard ceiling on one sell pass, and the **bag-slot
  selection**: an 8×8 grid of the bag, where each cell toggles one slot. The header above it prints the
  pitch derived from the calibrated **Bag grid**; until that mark is saved the grid is disabled with a
  line pointing at **Calibrate**, because a Sell run cannot place the cells without it. **Select all**
  fills all 64 cells and **Clear** empties them; each bag row also has a **row 1**…**row 8** shortcut —
  pressing it when the whole row is selected clears it, otherwise it fills the row. The count line below
  the grid shows `n of 64 slots selected`, and the status line under it has three states: nothing
  selected, within the cap, or **over the cap** in the error colour (it says the run will refuse).
  **The selection is for the next Sell run and is not saved** — it lives only in the shell's memory (it
  starts empty each launch) and there is no Save button for it: a stored selection could sell the slots
  chosen last time, so v3 keeps v2's per-run behaviour. Press **Start** on the **Sell card** to send the
  current selection; with nothing selected the card shows the core's own "No slots are selected to
  sell." line. The tab's **Save** still saves the presets and the cap. The core counts the distinct,
  valid slots it would sell, never silently truncates, and refuses a start whose selection exceeds the
  cap **before any click**, with its own line “{n} slots selected but the per-run cap is {cap}. Raise
  the cap on purpose, or narrow the selection.” (shown in full). At run time it sells the selected
  slots highest index first and never past the cap.

### 5.5 Quest

Named **flows** replay fixed click/key sequences at the quest NPC; **sequences** run several flows in a
row. The tab has three parts:

- **Active** — **Active flow** and **Active sequence (empty = single flow only)**. These are what the
  Quest card's pickers edit too.
- **Flows** — per flow: the **Initial wait (s)**, **Loops**, and an ordered step list of **Action**
  (`click` / `right_click` / `key` / `enter` / `wait`; `wait` is the no-input pause), **Key value** and
  **Delay (s)**, with move up/move down/remove. Add and delete flows by name. The delete action stays
  enabled but refuses “This is the only flow — create another before deleting it.” in the tab's note
  line while it is the last flow (a quest config with no flow has nothing a run can replay).
- **Sequences** — per sequence: **Initial wait (s)**, **Master loops**, and the flow entries with their
  **Flow** name and **Times**. The pickers offer only existing names; deleting a flow that a sequence
  still names leaves a dangling reference, which the core refuses on Save (the same as v2) — update the
  sequence before deleting that flow.

The run only clicks where **you** put the cursor during the positioning countdown, and it watches the
cursor against **Cursor guard px**; moving the mouse past the guard stops it.

### 5.6 Pet

The feeder's setup — the machine rows, queue, bag food cells and run settings, plus the portable
fallbacks — edited in one place and saved together. **One Save** sends one `config.apply` carrying the
machine `pet` block and/or the portable `settings.pet` halves (whichever have data); a refusal leaves
the edits in place.

The feeder also keeps its own run progress: while it works it saves `food_slots_used` and each row's
`boarding_running`, so a crash or restart does not resume aiming at used cells or the wrong boarding
state. The Pet tab's stored values can therefore update while a run is in progress — that is the
feeder's own progress write, not a player edit.

**Setup so far & Ready to run.** The tab reads the core's preflight on open, on **Refresh
checklists**, after a successful **Save** and after a successful boarding tick: a **Calibration** list,
a **Setup so far** list, then the core's readiness lines verbatim. **Ready to run.** means the core
expects Start to **accept** this saved setup — it mirrors Start's refusals, not end-to-end run
success: a hand-edited malformed value can still be refused by Start, and mid-run or non-gate facts
can still affect a run; **Not ready yet — see the lines below.** means a readiness line fired. ✓ is a
passed check and ✗ a failed one, but the calibration list mixes Start gates with useful facts — a
row's boarding slot or empty-slot reference can be ✗, the food strip is only required in `drag` food-load mode, and page tabs
only for the pages the saved marks use — so a ✗ is not automatically “not ready”: only the readiness
lines and the verdict decide.

ℹ is the shell's marker for an `ok` item the core did not check (its detail begins “not checked by
preflight”; today only board firmware — “not checked by preflight — normal”), not a third core state.
The lists describe the **saved** config: while anything here is edited they show “Based on the saved
config — Save to refresh” and are not live; open, **Refresh checklists**, a successful Save or a
successful tick re-reads them. A feeder's own mid-run `boarding_running` write updates the **tick**
through the config mirror, but it does not re-read the checklists. The preflight is read-only — it
does not open the board or read the game.

- **Breeding rows** — for each row the config already carries: **Row N enabled** (a ticked row is fed;
  an unticked one is ignored) and **Stacks per reload** (the food strip is divided by it). The row's
  marks — start/end button, boarding pet slot, food strip, empty-slot crop and boarding state — are
  read-only here and ride the save verbatim. The row's **Boarding now** tick writes that row's
  `boarding_running` immediately through the core into `local.yaml` (one field, not part of this tab's
  Save) and works while a tool runs; it is disabled while any edit here is unsaved (“Save the row
  edits first”, or “Save the edits first” for portable-only edits). A refused tick shows the core's
  line under the rows, and the tick shows the core's current value, so the feeder's own mid-run
  updates appear there too.
- **Queue** — each queued pet shows its **captured icon** (the identity the run matches), an editable
  **Label** (log/list only) and a **Species** dropdown, plus **Remove**. Edits and captured entries
  are saved by the tab's **Save** — unlike v2's per-edit auto-save — and a captured entry is added to
  this list as a draft immediately (only persistence waits for Save), so it can be Removed before
  saving. The dropdown lists the game's known species (the `species` column of `docs/pet-data.csv`,
  embedded in the shell), so adding a pet is a pick, not typing; a value inherited from an older
  config is kept as an option rather than dropped. The selected species is what turns the panel
  growth into a minute estimate; empty simply has no estimate. When the queue is empty the
  **Return slot** is the fallback the run right-clicks.
- **Bag food cells** — per page (**ITEM1**..**ITEM3**) an 8×8 grid of the boarding bag; toggle the
  cells that hold pet food. The grid is available only after the **Boarding bag grid** mark is
  calibrated; otherwise a line points at **Calibrate** (the boarding bag's own grid, not the shop's)
  and asks for the **Boarding bag slot** the run needs. A marked page also needs its ITEM tab point
  saved there.
- **Return slot** — **Bag page** (1-based, ITEM1 = 1) and **Cell** (0–63, row-major). The pair is
  written only when both values are known, and the config stores the page 0-based.
- **Food loading** — **Food load mode** (`right_click` lets the game pick the box; `drag` names it,
  which needs the row's food strip and a board whose firmware can drag) and **Reload on start** (open
  and close the boarding window once when a run begins).
- **Timing & read thresholds** — **Action wait (ms)**, **Wait after empty (minutes)** and the
  **Count read — left fraction (0–1)** / **Count read — min score (0–1)** thresholds.
- **Feeding (portable — saved in `settings.pet`)** — **Items per minute (fallback rate)**, used only
  when the boarding window's own rate line cannot be read, and **Occupied above (0–1 distance)**, how
  unlike the empty reference a slot must look before it counts as occupied.

**Add a pet by icon.** Put the bag page holding the pet on screen — captures need the game in front,
and the shell flashes hidden briefly for the shot. Choose the page with the **ITEM1**..**ITEM3** chips,
click the cell holding the pet, then press **Capture pet icon** (one capture at a time). The page is
not clicked for you, and the capture uses the screen only — no Arduino is opened. The preview and the
echoed page/cell/crop size do not verify correctness — a wrong page gives a plausible wrong crop — so
check them yourself; the icon is added to the queue list as a draft, the tab's **Save** writes it, and
you can **Remove** it before saving. A refused capture (another tool running, the game not in front, or
the page-tab mark uncalibrated) shows the core's line and changes nothing. The capture controls are
absent only when the bag grid is uncalibrated; with a calibrated grid but an unmarked page tab, or
while a tool runs, they are shown and the core refuses.

**Capture food icon.** Mark a food cell first — the newest mark is the source — put that page on
screen and press **Capture food icon**. The captured icon is the reference **Scan this page for
food** matches against; **Save** writes `pet.food_icon_png`. Neither icon is the boarding A1 writer:
both are ordinary config fields saved by the tab's Save.

**Scan the bag (read-only).** The bag scan reads **one page at a time** — there is no all-page sweep.
Put the page on screen yourself: the shell hides for the shot, the game must be in front, and the
core does **not** click or verify the ITEM tab, so a **wrong page gives a plausible wrong result**.
Pick **ITEM1**..**ITEM3**, then **Scan this page for food** or **Scan this page for pets**. The scan
uses the **saved** setup, not your unsaved draft (scans are not blocked by edits) — if you just changed
the food cells or re-captured the food icon, **Save** first so the scan matches what is stored.

- **Scan this page for food** matches the saved food icon and lists the found cells (1-based) plus the
  core's report lines. Evidence files are best-effort under `logs/reads/` and may be missing or
  partial.
- **Scan this page for pets** shows each queued pet's hit cells, scores and runner-up, claimed-cell
  conflicts, and the deduced “label ×n” lines. A pet with no hit may be absent, or its stored icon may
  not decode — the scan report says which.

Scanning only looks: it changes no config and applies nothing.

**Apply a food scan.** After a food scan, **Apply N cells to the food list** is a separate, confirmed
action. It **replaces** the food-cell list and **resets the used-cell count** (the run's progress), and
it takes effect immediately even while a run is in progress. Apply uses the page named in the
confirmation (the page that was scanned) — switching the **ITEM** chip afterwards does not change what
Apply sends. It is disabled while there are unsaved edits (“Save the edits first”); check the report and
evidence first, because a wrong page saves wrong cells. An empty scan result means “nothing found” (a
success), so Apply stays disabled. **Capture food icon** remains how the matching icon is (re)made;
**Save** writes it.

The calibrated marks/crops in the read-only section come from the **Calibrate** tab or are learned by
the run; **Food cells used** is the run's own reading and stays display-only. A fresh install has no
breeding rows and v3.0 cannot add rows or re-capture the row's reference crops yet — migrate your v2
`config/local.yaml` (which carries them) or hand-edit that block with the repository's
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

The Arduino tab is a connection card (read-only facts plus the port radio list) and a test surface.
It opens on `board.details`, which
only enumerates the machine's serial ports — it never opens the board — so reading it is safe even
while a tool runs.

- **Connection** — the three-state light and verdict for what a Start would do: **Ready — Start will
  open COM…**, **Start will refuse — the named port is missing** (with the core's own refusal line),
  or **Start will refuse — no Arduino found** (with the charge-only-cable advice: plug it in, press
  **Refresh**; the board can light up yet never appear as a port).
- **Ports this machine has** — the core's chosen-first order: **●** is the port a Start would open,
  **○** is another port, and a port that matches the configured ID but was not chosen says “matches
  the expected ID”.
- **What the launcher looks for** — the VID/PID/baud it matches, and whether `arduino.port` names a
  port (a named port wins over the ID).
- **Board firmware** — the cached **protocol level**, not v2's old drag-test OK/Not-OK verdict: “Not
  checked yet — this is normal.” until a tool has opened the board, then “Protocol level N (checked).”
  (or “Protocol level N (queried now).” after an explicit query). **Query firmware** is the only
  action on this tab that opens the port; it is unavailable while a tool is running, with the reason
  shown (a real probe refusal renders the core's line verbatim).
- **Refresh** re-reads the read-only details. **Click**, **Move** (dx, dy) and **Send key** are
  unchanged: each sends one test action through the board. Setup's **Probe board** is still the same
  explicit open-and-ask.

### 5.9 Setup

The window, client variant and board settings, plus **Find window**/**Measure**/**Probe board** and the
saving **Use detected title** action. See section 3.1 — the same tab is the first-run configuration
surface.

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
scaled by the Windows display scale. The contract and the why are in the repository's
`v3/docs/spec/CALIBRATION.md`; this section is the operator's view.

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
- **The pet's reference crops and row entries** — live-screen session features (the close-X crop is
  even learned by the run), not frame marks. The Pet tab adds queue pets by icon capture, but adding
  rows and re-capturing the row reference crops needs the pet capture session (section 11).
- **The sell-slot selection** — not a calibration mark: it is a memory-only bag-cell selection made on
  the **Buy/Sell** tab for the next run (§5.4). It needs the calibrated **Bag grid** to place the
  cells, but has no kind of its own.
- **The environment check (Detect)** — the comparison rule exists in the core, but v3.0's Setup surfaces
  do not show a "your display moved" result yet. If you change monitor, scale, window size or move to a
  new PC, **recalibrate**; the marks are physical offsets and a proportional transform is deliberately
  not applied (the game UI does not scale linearly).

---

## 7. Logs and files

### Your settings and how to move them

Your configuration is split by whether it belongs to you or to this PC:

- `config/setup/` is **yours** (window/variant/board IDs, hotkeys, tuner and gem behaviour,
  spammer rotations, quest flows, buy presets, pet preferences). **Copy this folder** (and
  `config/user.yaml`, if you have notes) to another PC.
- `config/machine/` is **this PC's measurements** (calibration, OCR geometry, the marks,
  tooltip offset, window placement, COM-port override). Never copy it to another machine;
  calibrate there instead.
- `config/artifacts/` holds the small reference crops. It is regenerated by calibrating.
- `config/meta.yaml` is the core's bookkeeping; leave it alone.

An older install has one `config/local.yaml`. The first launch of this version backs it up
and splits it into the folders above automatically; see INSTALL's *Migrating from v2*. To
keep your setup safe, copy `config/setup/` and `config/user.yaml`; there is no need to back
up `config/machine/` because calibrating rebuilds it. More detail: `config/README.md` and
`docs/spec/CONFIG.md`.


### 7.1 The live log panel

The **Logs (N)** button in the top bar (or **Ctrl+L**) shows the lines the core publishes while it runs. It is for
*watching* a run; it is not the durable record.

### 7.2 Per-run files: `<root>/logs/`

`<root>` is the install folder the shell told the core to use (the status strip prints it). The core
appends; it never truncates a log:

| File | What it holds |
|---|---|
| `<tool>.log` | One line per started run, per **changed** status message, and the final reason. Tool ids: `spammer`, `tuner`, `buy`, `sell`, `gem`, `quest`, `pet`. This is the first file to read when "it did something unexpected". |
| `holdspace.log` | Hold Space's complete trail: v2's `HoldDiag` lines (the failed release, the panel lines) **and** the generic per-run lines (`run started`, then `finished` / `stopped` / `failed`) — its diagnostics and its run log share this one file. |
| `error.log` | Internal errors and tool-opening failures. Player-correctable refusals are **not** here — they are in the card, by design, so they do not bury real errors. |
| `ocr_log.jsonl` | One JSON record per completed OCR scan. |
| `ocr_errors.jsonl` | A completed scan that did not produce what its mode asked for. |
| `captures/capture_<stamp>.png` | The tuner's save-captures diagnostic; the record's `capture_path` names it. |
| `captures/empty_check_crop_<HHmmss_fff>.png` | The Gem's **Save empty captures** diagnostic; each one also writes a `gem.log` detail line. |

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

The full file split, loading order and fold are in the repository's `v3/docs/spec/CONFIG.md`.

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
the protocol gains the commands that measurement chooses. It is listed in the repository's backlog
(`v3/docs/backlog/OPEN.md`).

---

## 9. Typical workflows

**Once per machine (≈15 minutes plus calibration):**

1. Install and launch — see [INSTALL.md](INSTALL.md).
2. **Setup**: client variant and board IDs; **Save**; **Find window**, then **Use detected title**; then
   **Probe board**.
3. **Calibrate**: capture, then draw the marks for the tools you will use. Start with the shop group if
   you Buy/Sell, then Gem, then the pet kinds if you feed. Each kind is one save.
4. Run one short tool to confirm the board and the game respond. `logs/<tool>.log` proves what it did.

**Daily use:**

1. Start the game, windowed, and log in.
2. Open Seal Tools; the core connects (status strip shows `core … · rev …`).
3. On the tool's tab set what the run needs (spammer preset, tuner target/filter, gem start grade,
   quest flow). For the Pet Feeder, its tab edits the existing rows (enabled/stacks), removes or adds
   queue pets by icon capture, marks the bag food cells and sets the load mode and timings; adding rows
   or re-capturing the row crops still comes from a migrated v2 `local.yaml` or a hand-edited block.
   Then **Start** on the card. **Stop** when done.
4. For Buy/Sell use the card's run choice (Buy), or mark the slots in the **Sell** grid and press
   **Start** on the **Sell card** — the selection is for that run and is not saved. Remember: Sell is
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
| **game window not found** | Wrong variant on the **Setup** tab, or a title that no longer matches the game (the title is read-only — use **Find window**, then **Use detected title**), or the game is not running/minimised. **Find window** on Setup reports what the core sees. |
| **`game_not_foreground`** | Another window is in front of the game. Bring the game to the front and retry; the Calibrate tab's capture hides the shell for one frame itself. |
| **`ocr_unavailable`** | The OCR pipeline cannot serve this read on this machine. `detail.reason` is the code's own reason for a pipeline that could not be built — one of `models_env_unset`, `models_dir_missing`, `text_fixes_invalid`, `model_load_failed`, `runtime_failed` — and `detail.message` names the file, model or directory that failed (for a missing-models failure it lists **every directory tried**). A failure while scanning an already-built pipeline (`bad_image`, `det_output_shape`) arrives as `internal` with the OCR line: a bad frame or a bug, not a machine that cannot serve OCR. Resolution order for models: `SEAL_OCR_MODELS` (if set it wins or fails loudly), then `<root>/models`, then the folder `seal-core.exe` is in + `models`; a directory counts only with all three `.onnx` files. Restore the shipped `models/` folder (or set `SEAL_OCR_MODELS` for a custom location). |
| **`board_unavailable`** | No board on the configured port, or the port cannot be opened. Check the cable (data, not charge-only), the board's USB driver, the **VID/PID/port** on Setup, and press **Probe board**. Only one program may hold the COM port. |
| The window does not stay above the game | Check the **always-on-top pin** in the app bar: filled means it is on, and it saves `settings.ui.always_on_top` (portable, default off). A refused save leaves the pin at the saved value and shows the core's line. |
| The window comes back in an odd spot | That is the saved placement restored from the machine `ui` block. Move/resize it and pause a moment before closing so the debounced save lands; a rectangle saved on a monitor that is gone is clamped into the primary work area, and the log says so. |
| A card line saying a mark isn't calibrated | Not a bug; the line names the **mark** to save and the single **Calibrate** tab. Draw that kind there (with its target instance if the kind is a named one) and save it. |
| "No buy preset selected." / "No slots are selected to sell." | Add/choose a buy preset on Buy/Sell (the card's picker). For Sell, mark slots in the **Sell** grid on Buy/Sell (it needs the calibrated **Bag grid**, §5.4), then press **Start** on the Sell card; with no slots marked the card shows this line. The selection is not saved. |
| "{n} slots selected but the per-run cap is {cap}…" | The selection is larger than **Max slots per run (sell cap)**. Raise the cap on purpose or narrow the selection; the core refuses the start before any click and prints the full line, including the count and the cap. |
| "The active preset … was not found — pick one in the Spammer tab." | Set **Active preset (runs)** on the Spammer tab. |
| Pet Feeder refuses on start (a mark or a breeding-row line) | The feeder's marks and rows must exist. Calibrate its `pet_*` kinds; the Pet tab edits the rows the config carries (enabled/stacks) and the food cells, but a new install has no rows yet — migrate a v2 `local.yaml` or edit the block with the repository's `v3/docs/spec/CONFIG.md` open (adding rows needs the pet capture session, section 11). |
| Hotkeys do not fire in game | Expected in v3.0 — there is no poller (section 8). Use Stop in the shell. |
| "The window blinks when I press Capture" | Expected: the shell hides itself for one frame so the capture is of the game, then reopens. v2 blinked ~0.3 s; v3 blinks for a frame plus the round-trip. |
| Need the last run's detail | Read `<root>/logs/<tool>.log` (status strip prints the root); `error.log` and `ocr_errors.jsonl` are beside it. |

---

## 11. What is not built in v3.0

Named here so a missing capability is a known boundary, not a mystery. The living list is the
repository's `v3/docs/backlog/OPEN.md`; every item below is "not in 3.0", not "planned for tomorrow"
unless the repository says so.

- **Global hotkeys.** No poller; see section 8. Stop and every shell control work.
- **The tuner three-box calibration session.** Only the spring point is calibratable; `tuner.ocr`
  geometry is hand-edited from config until the session exists.
- **The tooltip (hover-panel) two-capture session** — hand-edited from v2 config.
- **The pet session surface (increment 4+).** The Pet tab edits the setup the machine already carries —
  rows enabled/stacks, queue add/edit by icon capture, bag food cells, per-page bag scans and the
  explicit food-scan apply, return slot, load mode and timings — but adding rows, re-capturing the row
  reference crops/rects, the panel test read and the feedable sweep/queue rebuild still need a live pet
  capture session (a scan-derived queue rebuild comes later); migrated v2 configs carry the current
  data.
- **The environment check (Detect) surface** — recalibrate manually after a display change.
- **A single-instance guard.** The protocol's instance lock is specified but not built: do not run two
  copies of Seal Tools at once.
- **An installer and auto-update.** v3.0 is a zip; updating replaces program files and keeps your
  `config/`.
- **v2-only chrome and docs.** v2's WPF screenshots are not used here; the v2 docs remain the record of
  the v2 build, not a description of this window.

---

## 12. Where the rest is documented

**What the zip carries:** this guide, its zh-TW twin, `INSTALL.md`, `THIRD-PARTY-NOTICES.md` and
`docs/pet-data.csv`. Everything below that starts with `v3/` is a path in the **project repository**,
not a file inside the zip.

- `v3/docs/INSTALL.md` — install, migrate, update, uninstall, troubleshooting (also in the zip beside
  this guide).
- `v3/docs/spec/CONFIG.md` — every config file, which one is yours, and the loading/fold rules.
- `v3/docs/spec/CALIBRATION.md` — the coordinate contract, why DPI/scale work the way they do, and the
  artifact rules. v2's `docs/spec/COORDINATES.md` and `CALIBRATION.md` are the measurement record.
- `v3/docs/spec/PROTOCOL.md` — the shell↔core contract (engineers; useful when reading a refusal code).
- `v3/docs/spec/TOOL-FLOWS.md` — what each tool decides, and the exact player lines it can produce.
- `v3/docs/backlog/OPEN.md` — the live not-built list, including the global-hotkey measurement.
