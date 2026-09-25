# Plan — the quest hand-in tool

Written 2026-09-26, from the player's description and four decisions taken before any code. **Not built.**

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

## 10. Order of work

1. Config: the model, both projection directions, the guard-test fixture.
2. The tool loop: initial wait → guard → per-step send → loop count.
3. The project and its registration (card + tab).
4. The step editor and the preset picker.
5. **Test one loop.**
6. Docs: `USER_GUIDE.md` and its zh-TW mirror, and a release note when it ships.
