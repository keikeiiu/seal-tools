# Buy / Sell tool — plan

Status: **design only, nothing built.** The firmware prerequisites landed on 2026-09-13 (see
[PROGRESS.md](PROGRESS.md)); everything below is pending measurements from the live game.

Goal: bulk-buy the items you re-buy constantly, and bulk-sell what you don't keep, without repeating
the clicks by hand.

---

## What the captures established

From `螢幕擷取畫面 2026-09-13 2137xx.png` (three frames, shop open).

**Both windows are fixed and appear together.** Reaching the shop opens the SHOP window and the ITEM
bag together, and they sit at the same place every time. That removes the hardest problem a tool like
this usually has — there is no need to find or anchor either window per run.

**The bag grid is 8 × 8 = 64 slots, uniform.** A lattice fit over the slot borders gives a pitch of
**51.0 px horizontally and 50.9 px vertically**, with no drift across the eight columns or rows. That
uniformity is what makes a two-corner calibration valid instead of needing all 64 points.

**The shop list is two lines per entry** — item name above, gold price below — with a pitch of about
**47 px**, ten entries visible at once, and a scrollbar down the right edge. The list is long enough
to scroll, so an item's position is not fixed by name alone.

**The capture frames are not a consistent coordinate space.** The three frames are 2864×1832,
2866×1791 and 2868×1789 — the first includes the Windows title bar, the others do not. The configured
client is 2865×1789, so the client-only frames are 1:1 client pixels, but the numbers above come from
screenshots and are **approximate**. Every value the tool relies on must come from calibration on the
live game, not from these.

**The quantity dialog was not in any of the three frames.** It needs a fresh capture — see below.

---

## The tool's shape

Three tabs, because the jobs are genuinely different:

| Tab | What it does |
|---|---|
| **Buy** | Pick a preset — Springs, Pet Food, … — set how many, start. |
| **Sell** | Click the slots to sell on an 8×8 grid, then Dry run or Sell. |
| **Calibrate** | The bag grid, the shop row, MAX, and each buy item's row and scroll amount. |

**First release: springs and pet food only.** Those two are what actually gets re-bought, and between
them they cover the cases that matter — a plain item, and one expensive enough to raise the second
confirmation. But the machinery is generic from the start: the preset list, the row picker and the
scroll field know nothing about springs. A third item is a preset you add, not a code change; only the
two seeds are specific.

## Design

### Calibration is the gate

Nothing here may run live until calibration is done and *visibly verified*. The pattern to follow is
the existing calibrator: drag/click points on a screenshot, persist to `local.yaml`, with a **Test**
button per action so the result can be seen before it is trusted.

Its own tab, with **two screens** — buying and selling need different things measured, and neither
needs the other's.

### Sell screen — the bag grid

Drag a box around the **whole 8×8 grid**, then a second box around **one slot**. Drag-select rather
than clicking points: each is four coordinates, and a bad drag is corrected by dragging again instead
of starting over.

Both boxes, because they check each other:

- the whole-grid box gives the pitch, `gridWidth / 8` — the value that actually matters, since one
  slot's width is only as good as that one drag
- the single-slot box gives the real size and origin of a slot, so the tool can *verify* the grid is
  uniform rather than assume it

If the two disagree by more than a pixel or two, the calibration is wrong and says so rather than
quietly using the average. Then a **grid overlay** draws all 64 computed centres on the capture —
the design rests on the grid being uniform, and that is the only way to see it is, on your render.

Plus **MAX** (one click), shared with buying.

### Buy screen — the shop list

One capture with **the shop open and the dialog open**, so the list and the dialog are measured from
the same frame: the list region (drag a box), the first row (one click), and MAX. Where each item
sits is per-item — see below.

### Why calibrate instead of using the captures

The frames I was given are ad-hoc screen captures and are not one coordinate space: 2864×1832,
2866×1791 and 2868×1789, the first including the Windows title bar. They were good enough to learn
the *shape* of the problem — 8×8, uniform, two-line shop rows — and nowhere near good enough to
hardcode a coordinate. Everything the tool uses is measured in-app, in the client space the tools
actually click in.

### Buying and selling share one flow

Selling reuses the buying UI — right-clicking an item opens the same COUNTER dialog a shop does. So
there is **one** click sequence, driven twice:

1. Open the dialog: **right-click the item** — a shop row to buy, a bag slot to sell. (It is a
   right-click for both. A left click on a shop row does nothing, which is what made the first live
   buy silently no-op — it looked like a positioning failure and was a button failure.)
2. Click **MAX** — the only click in the sequence with no keyboard equivalent.
3. **Enter** — the COUNTER's confirm button.
4. **Enter again** — a second confirmation.

Selling always shows the second one ("確定要將 … 出售嗎?"). Buying shows it when the item is
expensive, and **both items wanted here qualify** — springs and pet food. So the two flows are the
same shape after all: *click, click, Enter, Enter*.

Open question kept in mind: if a cheaper item does **not** confirm, that second Enter lands with no
dialog up. Expected to be harmless, but it wants one deliberate test before the buy path is trusted
with anything that matters.

The whole sequence therefore needs exactly **four** calibrated things — the bag grid's two corners,
the shop row, and MAX — plus firmware commands that already exist (`C`, `R`, `E`, `Q`/`Z`). Neither
the COUNTER's confirm nor the sell dialog's O needs a point, because Enter reaches both. That is fewer
moving parts than this design started with, and every one removed is a point that can't drift.

Measured from the capture anyway, as a fallback if Enter ever fails to take: the sell dialog is
centred in the game window (its O glyph at x≈1430 against an image centre of 1432.5), with X about
77 px to its right, around y≈958 in a 1788-tall client. Clicking O instead of pressing Enter is a
fallback that needs one calibrated point, not a redesign.

**Quantity is always MAX.** The game caps a stack at 300 (it occasionally shows 301, which is a game
bug). MAX takes whatever is there, so the tool never needs to know or type a number — which is also
why no backspace went into the firmware. A fixed sequence of clicks needs no text-field semantics
from the game at all.

Buying is the safe half: worst case is spending gold.

### Selling — which slots

You say which slots, not the tool. The Sell screen shows a **clickable 8×8 grid** standing in for the
bag — you click the slots to sell, they light up, and those are what go. A full page is one click on
"select all" rather than a number you type.

An explicit selection rather than a count is the right call here specifically because selling is
destructive: "the first 40 slots" is a guess about what is in them, while lighting up 40 boxes shows
you exactly what you are about to lose. No rule engine, no OCR of the bag — the decision stays yours
and stays visible.

Worth being blunt about the rest: this is the only destructive action in the suite. A mis-aimed click
sells the wrong stack and it is gone — every other tool here is recoverable, a bad roll just costs
springs.

Required before it runs live:

- a **hard per-run cap**, enforced in the loop rather than advisory
- a **dry-run mode** that highlights the slots it would click and sells nothing
- the grid overlay above as the calibration gate

### The list starts at the top — that is the user's setup, not the tool's

A preset's scroll amount means "notches down from the top", so the list has to *be* at the top when a
run starts. The tool does not put it there. **Scrolling it there is yours to do before starting.**

That is a deliberate reversal. An earlier design sent "scroll to the top" as one command at the
firmware's maximum, which had two bad consequences: it made the firmware's per-command ceiling
*load-bearing* — the cap was the reach, so a list longer than the cap left every preset measured from
the wrong origin — and it cost ten seconds of the board being unable to read serial, on every run.

The ceiling is now only a guard against a malformed value wedging the board, which is all it was ever
meant to be. It sits in `ShopGeometry.MaxScrollNotches`, shared by the tool and the calibrator; the
firmware's `WHEEL_MAX_NOTCHES` is the one duplicate that cannot be merged, being a different language.

**The cost of the reversal:** if the list is not at the top when a run starts, every click lands
further off the further the item sits from where the list actually was. Nothing detects that, so it is
worth glancing at the list before starting.

### Scrolling — a per-item amount, set in dry run

Each buy item stores its own **scroll amount**: how many wheel notches to apply after scrolling to the
top. It is a plain number on the item preset, and you set it by trial in **dry run** — pick the item,
run the dry run, and the tool scrolls to the top, applies the stored amount, and moves the cursor onto
the row it would click. **It does not click.** You look at where the pointer landed and adjust the
number until it sits on the item you want, then save.

That keeps it one number you own, rather than a measurement the tool tries to infer — and the dry run
is what makes the number trustworthy, because it shows the result on the live game without spending a
click. A no-scroll item is simply one whose amount is 0, so there is no separate "needs scrolling"
flag.

**Scroll to the top first, on every run.** Send the wheel-up maximum and let it clamp, which gives a
known origin so the stored amount always means the same thing. Without that, the amount would depend
on wherever the list happened to be left.

The wheel scrolls **whatever is under the cursor**, so placing the cursor over the list is one more
calibration point.

Nothing here needs a new control: the number is a field on the preset, and the dry run is a button
next to it.

### Buying items are presets

You re-buy the same few things (springs, potions, pet food), so each becomes a **named preset**, the
same shape as the spammer's — a named list you pick from, with add / rename / delete. A preset holds:

| Field | From |
|---|---|
| name | you type it |
| shop row index | clicked in the Buy capture |
| scroll notches | the nudge count |
| buy count | how many to buy this run |

Adding a new item is: open the shop, capture, nudge to the item, click its row, name it, save. After
that it is one click to buy.

Where they live is already decided by earlier work: **presets are personal, so `local.yaml`**, not the
`defaults.yaml` that ships — the same rule the spammer presets now follow, and `ConfigLoader` already
has the plumbing.

---

## Settled

- **The shop listing is stable.** Shops differ from each other, but a given shop keeps the same
  entries in the same order. Only a few items matter — springs (for magic tuning), potions, pet food
  — so an item sits at a **fixed row**, and the scroll needed to reach it is a **fixed number of
  notches**. No scroll-and-find, no OCR of the list. This was the biggest fork and it went the easy
  way.
- **Selling uses the buying UI.** Right-click opens the same COUNTER dialog, so one click sequence
  serves both.
- **Quantity is always MAX.** No typing, no per-item quantity logic.
- **What to sell is your call**, expressed as a slot count or a full page.

## Open questions

1. **Does the bag compact after a stack is sold?** If selling slot 0 shifts everything left, the
   whole job is clicking slot 0 N times — much simpler and much safer than stepping through slots.
   If slots are vacated in place, the tool walks 0..N-1. This changes the loop, so it wants one
   deliberate test.
2. **The sell confirmation is answered** — a modal, taken by Enter, and buying has none. What is not
   yet known is whether it appears for *every* sale or only above some value. If it is conditional,
   sending Enter when no dialog is up must be harmless — otherwise the per-item config needs a
   "confirms" flag. One deliberate sale of a cheap stack answers it.
3. **Roughly how many notches equal one row?** Now only a starting guess — the per-item amount is
   tuned in dry run regardless. Knowing it means the first attempt lands close instead of far off,
   which saves a few dry runs. A single Test Scroll answers it.
4. **A capture of the shop open with the dialog open**, in the same frame — that is what the Buy
   screen calibrates against, and the earlier frames had only one or the other.

---

## Phasing

1. ~~Firmware: wheel + full keyboard~~ — landed 2026-09-13, **not compile-checked and not flashed**.
2. **A "Test Scroll" button on its own** — enter a number of notches, send it, watch the list move.
   The smallest thing that makes the reflashed firmware *verifiable*, and the dry run's first half, so
   nothing built here is thrown away.
3. **Sell screen calibration** — grid drag-select, the two-box consistency check, and the 64-centre
   overlay. Pure calibration: no game actions, and self-verifying by looking at it.
4. **Sell path** — dry-run first, then live with the cap enforced. Selling is the simpler half
   because the only config it needs is the grid, which step 3 has just produced.
5. **Buy screen calibration** — list region, first row, MAX, and the per-item row picker.
6. **Buy presets and path** — named items, scroll notches carried over from the nudge, buy count per
   item, per-run cap.
