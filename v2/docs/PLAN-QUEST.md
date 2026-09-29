# Plan — the quest hand-in tool

Written 2026-09-26, from the player's description and four decisions taken before any code.

**BUILT 2026-09-26** — config, tool, project, card, tab, step editor, preset picker and Test one loop.
174 tests pass, Release build 0 warnings / 0 errors. **Never run against the live game**, which for this
tool means the flow itself is unproven: it reads nothing, so only watching a test loop can say whether a
sequence does what it was meant to.

A new tool that **hands in quests repeatedly** by replaying an input sequence at the quest NPC. It is
**blind**: it never looks at the screen, sends the sequence the player authored, N times, and stops.

---

## 1. What it is, and what that buys

The four tools that read the screen all paid for it — derived regions, aimed crops, stale geometry, and a
row that silently read nothing for four visits. **This one reads nothing**, which is why it is the
smallest tool in the suite and why it has no calibration at all:

- **no capture, no OCR, no window lookup, no mark to drag;**
- **no "Calibrate Quest" tab**, because there is nothing to place — see §3.

Its whole fragility budget is spent on one thing instead: **a wrong sequence does 500 wrong things**, and
the game will accept every one of them. That is what §4 and the loop count exist to bound.

## 2. The step vocabulary

As small as the board's protocol allows. Every command below is one the firmware already implements —
checked, not assumed: `C`/`R` click, `E` Enter, and `K <char>` for **any printable ASCII**, so digits
work as well as letters.

| step | sends | note |
|---|---|---|
| `click` | `C` | left |
| `right click` | `R` | |
| `key <char>` | `K 1`, `K q` | one printable character |
| `enter` | `E` | |
| `wait` | — | a pause with no input |

**Every step carries its own delay**, defaulted to **0.25 s**. The value is a placeholder, not a
measurement — the shape is per-step because a dialogue step that needs 1.5 s sitting next to one that
needs 0.2 s is exactly where a single global number breaks, and the row already exists to hold it.

## 3. The flow, and why there is nothing to calibrate

```
initial wait 10s          ← the player places the mouse here
  × 500 (the loop count)
     left click → 1 → enter → enter → left click → 2 → 3 → enter enter enter
```

**The player puts the mouse where it needs to be, and the tool clicks wherever it is.** That is the
tuner's `manual` spring mode, which works exactly this way — `if (SpringMode == "hid")` gates the
automatic placement, so in manual the tool never moves the cursor.

With no point to place, **there is no calibration and no calibrate tab**. A `hid` mode — the tool placing
the cursor onto a calibrated point before each click — was considered and is deliberately **not** in v1:
it would add back the entire machinery this design gets to skip, and the guard in §4 covers the risk it
would have covered.

## 4. The guard: stop when the mouse moves

The player's words: *"if there is big swing on mouse movement, we stop it."* That is the tuner's mouse
guard, and it exists for precisely this — the human has taken over.

**Here it can be better than the tuner's, and the reason is the missing window.** That guard compares the
cursor against a calibrated point and needs the game window measured, which is why it is documented as
**failing open**: window gone → `return false` → "no drift, carry on" → the loop keeps clicking wherever
the cursor sits. This tool needs neither a window nor a point — it reads the cursor **once, when the run
starts**, and compares against that.

| | tuner's guard | this one |
|---|---|---|
| needs the game window | yes | **no** |
| needs a calibrated point | yes | **no** |
| when the cursor can't be read | carries on | **stops** |

So it **fails closed**: an unreadable cursor ends the run rather than letting it click 500 times blind.
Threshold **8 px**, the same number `tuner.guard_px` ships, for no better reason than that it is the
number already in the player's head.

**Stops, in full:** the loop count completing · the drift guard · the launcher's **Stop** button.

> The quit hotkey will **not** stop this while you are in-game — the anti-cheat blocks background key
> reads, which is already recorded in the notes. The Stop button is the real control, here as elsewhere.

## 5. Config: named presets, in `local.yaml`

The flow is different per quest, so a flow is a **named preset** — the same shape as spammer key rotations
and buy presets.

```yaml
quest:
  active: <name>
  presets:
    <name>:
      initial_wait_s: 10
      loops: 500
      steps:
        - { action: click, delay_s: 0.25 }
        - { action: key, value: "1", delay_s: 0.25 }
        - { action: enter, delay_s: 0.25 }
```

**In `local.yaml`, not `defaults.yaml`** — presets are personal, and `defaults.yaml` is the file
`publish.bat public` copies into the release zip. This is the same reasoning, and the same bug it avoids,
as the spammer's presets: a preset written there would ship with the next public build.

## 6. The UI

**A card** — `Quest Hand-in`, Start / Stop, live status as `loop 37/500`.

**A tab** holding:

- the **preset picker** with New / Rename / Delete, exactly as the Spammer tab's works (including the
  refusal to delete the last one);
- **Initial wait** and **Loops**;
- the **step list** — one row per step: action, value, delay, and a ✕; `+ Add` at the bottom. The shape
  [`BuildRulesEditor`](../../SealTools.Launcher/MainWindow.xaml.cs) already established, which is a
  row-per-item editor with a ✕ that removes from both the panel and the backing list;
- **Test one loop** — one iteration and stop, so the sequence can be watched before committing to 500.
  The composer's "Run one full cycle" and the shop's dry run, in a third place;
- **Save**.

## 7. Under the hood

- a new **`SealTools.Quest`** project, one per tool as the others are, with `QuestTool : ToolBase`;
- **`QuestConfig`** + `QuestPreset` + `QuestStep`, and the local↔config projection in **both
  directions** plus the guard test's fixture — the drill, and the trap that has silently dropped a field
  three times;
- registration: the launcher's `Tools` list, `RunTool`, and the tab.

## 8. Limits, stated up front

- **Single printable ASCII keys only.** The firmware's `K` handler presses one character: no arrows, no
  shift-modified keys, no combinations. F1–F12 exist as their own command.
- **Blind.** Nothing is verified: if a click misses or a dialogue is slower than its delay, the run keeps
  going and the count is the only thing that ends it.
- **The quiet failure is the likely one.** Not a crash — a sequence that is *slightly* wrong, run 500
  times. Test one loop, then run a short count, then the real one.

## 9. Not in v1

- **Stop on a condition** — reading the quest list to end early. A real feature, and a big one: it needs
  capture, OCR and a detection rule, and it inherits every fragility the reading tools have.
- **A `hid` click point** — see §3.

## 11. Sequences — several flows in one run (built 2026-09-30)

Two quests at the same NPC, handed in in a rhythm: **A three times, then B twice**, and the whole thing
repeated by a master loop.

```yaml
quest:
  active_sequence: ""        # empty = run one flow, exactly as before
  sequences:
    nightly:
      initial_wait_s: 10     # ONE positioning window for the whole sequence
      loops: 5               # the master loop
      entries:
        - { preset: a, times: 3 }
        - { preset: b, times: 2 }
```

Five master loops × (3×A + 2×B) = 25 flow runs from **one** mouse placement. Purely additive: a config
with no `sequences` runs one flow, exactly as v2.13 did.

### The constraint that shapes it

**Every flow in a sequence must click the SAME place**, and the initial wait belongs to the **sequence,
once**. That is not a shortcut — it follows from the design: the tool never moves the cursor, so a flow
inside a sequence cannot have a position of its own, and a flow's own `initial_wait_s` means *"when I am
run alone"*. It is also what makes the feature coherent: the natural use is **several quests at one
NPC** — one position, several dialogues.

If flows ever need different positions, that is the deferred `hid` click point (§9), and it brings
calibration back with it.

### There is still NO failure detection, and that is deliberate

`Run()` here reads nothing, so it cannot tell a flow that worked from one that did not. What it can see,
and nothing else, is:

| | when |
|---|---|
| a step that cannot be sent | before the run starts — a config error |
| a serial write throwing | mid-run — the board is gone, and every later step is pointless |
| the mouse guard | mid-run — the cursor moved |

The first two are **transport**, not "the quest did not hand in", and a throw already aborts through the
launcher's crash path. **No rule pretends to detect a bad flow**, because a rule that can never fire is
one a later reader would trust. Real detection — *is the quest still in the list?* — needs the tool to
read the screen, which is what this design gave up; it is `Stop on a condition` (§9) and it is a feature
of its own.

### The rest

- **`Test one entry`** runs one named flow once, so a new flow can be checked without committing to five
  master loops. `Test one loop` stays for the flow being edited.
- **The card says where it is**: `sequence "nightly" · 5 × (a×3 + b×2)` on the standing line, the master
  loop as `Cycle`, and `a 3/3 · key 1 (0.25s)` as `Current`. On a blind run that line is the only
  account of what it is doing.
- **The sequence picker on the tab carries `(none — run the single flow above)` as its first item**, so
  one control says which of the two runs and there is no second switch to keep in step.

## 10. Order of work

1. Config: the model, both projection directions, the guard-test fixture.
2. The tool loop: initial wait → guard → per-step send → loop count.
3. The project and its registration (card + tab).
4. The step editor and the preset picker.
5. **Test one loop.**
6. Docs: `USER_GUIDE.md` and its zh-TW mirror, and a release note when it ships.
