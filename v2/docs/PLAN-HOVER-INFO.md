# Reading the hover tooltip — plan

Status: **calibrated and reading, verified live 2026-09-18.** The Calibrate Tooltip tab measures the
offset, hovers with the Arduino, and reads the panel; a Test read reports what the OCR saw, saved beside
the image. What is NOT built is the parser that turns a read into something a tool acts on, and any
consumer of it — so today this is a capability with no users.

A general capability, not a pet-feature detail, which is why it has its own document rather than a
section in [PLAN-PET-AUTOFEED.md](PLAN-PET-AUTOFEED.md).

Goal: **read what the game shows when you hover an item.** The pet tool wants to know a pet's stage,
growth and EXP — to decide what to board and to know when it is finished; the same read gives the food's
remaining count, which is the difference between predicting when the feeder runs dry and knowing. See
the measurement below for what a read actually yields.

---

## The insight that makes it one calibration

**The tooltip is anchored to the cursor.** Its *size* varies by what is under it — a pet's panel is
bigger than a food item's — but its *position relative to the pointer* is the same everywhere in the
game (player, 2026-09-17).

So the thing worth calibrating is not "where the pet tooltip is" but **the tooltip's offset from the
cursor**, which is a property of the game and the machine's scale, not of the item. One calibration then
serves every hover-read this tool ever does.

## Calibrating it

The awkward part: at the moment you drag the box, the mouse has moved to drag, so the cursor position
cannot be read off the screen. The fix is to have the **tool place the cursor first**.

```
1. Press "Place cursor and capture"
     → the tool moves the cursor to a point it chooses, with the Arduino
     → waits out the hover delay
     → captures with the launcher hidden
2. Drag a box around the tooltip in the capture
3. The tool computes both values itself:
     offset = box top-left − the point it placed the cursor at
     size   = the box
```

Because the tool knows where it put the cursor, the offset is exact. Nothing is typed in, and nothing is
eyeballed — which matters, because this is the same class of number as every other calibrated
coordinate here and the same class of mistake applies.

### Machine-specific, so it lives in local.yaml

The offset and size are in **physical pixels at this machine's scale**, so a box calibrated on one PC is
meaningless on another. That puts them in `local.yaml` beside the other measured geometry, never in the
portable `defaults.yaml` — the same rule the bag grids and the OCR band already follow, and the reason
`CalibrationInfo` records the DPI scale alongside them.

## Three things that will bite

1. **The hover delay.** The tooltip appears only after a pause over the item, so the capture must happen
   *after* it and not immediately. This wants a generous, configurable wait for the same reason the
   click delays do — a machine that is slower than the one it was tuned on produces an empty read that
   looks exactly like "no tooltip here".

2. **One box, sized for the largest tooltip.** A pet's panel is bigger than a food item's, so calibrate
   against the biggest and let OCR see some background behind the smaller ones. The offset stays right;
   only the box is oversized. A tight box per item type would be more precise and is more to keep in
   sync — and it would stop being universal, which is the whole point.

3. **Edge flip.** Tooltips near a screen edge often flip to the other side of the cursor. If this game
   does, the offset only holds mid-screen and a read near the bag's right edge would OCR empty
   background and report nothing. Worth one deliberate check: hover an item at the far right of the bag
   and see which side the panel appears on.

## What it unlocks

| Read | Enables |
|---|---|
| A pet's **stage, growth and EXP** | "where is this pet" and "is it done" — the automatic direction in [PLAN-PET-AUTOFEED.md](PLAN-PET-AUTOFEED.md) §11, and the guard that stops the feeder reloading a finished pet |
| A food stack's count | knowing when the feeder runs dry instead of predicting it, which is the single biggest accuracy upgrade available to that tool |
| Anything else's tooltip | whatever wants it next, for the cost of a parser rather than a calibration |

### Measured on the live game: read the NUMBERS, never the text

Verified 2026-09-18, reading a pet panel on the live game. **Every digit came back exact; the Chinese
did not.**

```
on screen   (6階) 真蔚藍鳳凰 +7 [52.18%]
read        （6）真蔚蓝凤凰+7[52.18%]

on screen   所有職業皆可使用    讀作   所有瞬業皆可使用
on screen   販賣價格            讀作   贩直價格
```

**And the cause is already documented in this repo — in the config file.** `attributes.yaml` carries a
`text_fixes.simplified_traditional` list whose own comment reads *"RapidOCR defaults to simplified
Chinese; convert to traditional"*:

```yaml
simplified_traditional:
    - {from: "减少", to: "減少"}
    - {from: "级",   to: "級"}
```

So the recogniser outputs Simplified against a Traditional game, and the project has carried a
conversion table for it since v1. `TextCleaner` applies it, and `ReadLines` runs its output through
`TextCleaner` — **the reads above already had the table applied**. 藍 → 蓝 and 鳳 → 凤 survived it only
because those pairs are not in the list yet.

**So the name mangling is a config gap, not a code problem.** Add the missing pairs to
`simplified_traditional`; the recognition errors that are not script conversions (階 → 踏, 職 → 瞬,
賣 → 直) go in `substring`, the same table's other half.

*(Two earlier revisions of this section got it wrong in opposite directions — first asserting the
Simplified model outright, then withdrawing it because `高級寵物食物` arrived in Traditional and that did
not fit. The mechanism was right; the expectation of uniformity was wrong. Most strings arrive
Simplified and get converted; occasionally one arrives Traditional already. The config file said so the
whole time.)*

**And the log is how the table gets filled.** Every read keeps its raw text, so a run over many pets
accumulates exactly the evidence of which characters misread — and each fix is one line of YAML. A table
grown from measurement rather than guessed at.

What that means in practice: **a Traditional string that reads correctly once is not a string that reads
correctly reliably**, and there is no way to tell which kind you have without testing that exact string
repeatedly. Digits and punctuation have shown no such variance.

**Which kills the original plan for this read.** The idea was to resolve a pet's stage from its NAME
via the scraped table in [PET-DATA.md](PET-DATA.md) — but that table is Simplified (it comes from the
Simplified data site, see [[seal-game-data-source]]) and the game is not, so even a *perfect* read would
not match. Two independent reasons the lookup could not work.

**And it turns out not to be needed.** The panel states the stage outright as `(N階)`, the growth as
`+N` and the EXP as `[..%]` — three numbers, all of which read cleanly. So the design is to parse the
digits and ignore every surrounding character, which sidesteps the script mismatch and the recogniser's
weakness in the same move. The 327-entry table is not needed for this at all.

## Where it lives in the UI

**Not on a tool's calibrate tab** — it is universal, so putting it under one feature would hide it from
every other. Two options, and the choice is about how often it gets re-tuned:

- **Its own tab.** Clearest. Justified if it turns out to need re-tuning often, or if several features
  end up reading tooltips.
- **A section on Setup.** Setup already holds cross-cutting facts about *this machine* rather than about
  one tool, which is exactly what this is. Cheaper, and avoids a tab that is opened once and never again.

## Open questions

1. **Which side does the panel appear on near an edge?** Decides whether the offset is universal or only
   universal mid-screen — see (3) above. One deliberate hover answers it.
2. **How long is the hover delay?** Needs measuring, not guessing, since too short a wait reads an empty
   region.
3. **Does the panel move while it is up?** If it animates in, the capture has to wait for it to settle,
   not merely for it to appear.
