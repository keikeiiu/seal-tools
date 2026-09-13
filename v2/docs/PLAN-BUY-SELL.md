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

## Design

### Calibration is the gate

Nothing here may run live until calibration is done and *visibly verified*. The pattern to follow is
the existing calibrator: drag/click points on a screenshot, persist to `local.yaml`, with a **Test**
button per action so the result can be seen before it is trusted.

To capture:

| What | How | Why |
|---|---|---|
| Bag grid, slot (0,0) and slot (7,7) | two clicks | pitch = (br − tl) / 7, per axis |
| Shop list region | a box | bounds the OCR/scroll area |
| Shop list, first row centre | one click | row origin |
| COUNTER: MAX button | one click | sets the quantity |
| COUNTER: confirm button | one click | commits |
| Bag + shop panel edges | boxes | sanity bounds; refuse to click outside |

Then a **"Test grid"** button that overlays all 64 computed centres on a screenshot. This is not
decoration: the whole design rests on the grid being linear, and the only way to know that for *your*
render is to look at 64 dots at once. If the fit is wrong it will be obvious there and nowhere else.

### Buying and selling share one flow

Selling reuses the buying UI — right-clicking an item opens the same COUNTER dialog a shop does. So
there is **one** click sequence, driven twice:

1. Open the dialog: left-click the shop row (buy) or right-click the bag slot (sell).
2. Click **MAX** — the only click in the sequence that has no keyboard equivalent.
3. **Enter.** This is taken by the COUNTER's confirm button.
4. **Buying stops here — there is no confirmation.** Selling has one more step: a modal asking
   "確定要將 … 出售嗎?". **That one is also taken by Enter**, so step 4 is a second `E`.

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

You say how much to sell, not the tool: **a number of slots**, or **a full page** (64). Whatever sits
in those slots is what goes, and the decision of what is in them stays with you. No rule engine, no
OCR of the bag.

Worth being blunt about the rest: this is the only destructive action in the suite. A mis-aimed click
sells the wrong stack and it is gone — every other tool here is recoverable, a bad roll just costs
springs.

Required before it runs live:

- a **hard per-run cap**, enforced in the loop rather than advisory
- a **dry-run mode** that highlights the slots it would click and sells nothing
- the grid overlay above as the calibration gate

### Scrolling

Firmware has `Q n` / `Z n` (wheel up/down, n notches). Two things to know:

- The wheel scrolls **whatever is under the cursor**, so the tool must place the cursor over the list
  before scrolling — a calibration point, not a firmware concern.
- Because a shop's listing is stable, an item's scroll amount is a **fixed constant per item**, not
  something searched for at runtime. So the per-item config is just {scroll notches, row index}, and
  the tool scrolls to the top first to have a known origin.

The one number still needed is **notches per row**, which a Test Scroll button measures in a minute.

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
3. **How many notches equal one row?** Measurable with a Test Scroll button, and it makes the
   per-item scroll amounts calibratable rather than guessed.
4. **A capture of the COUNTER dialog**, so MAX and confirm can be measured.

---

## Phasing

1. ~~Firmware: wheel + full keyboard~~ — landed 2026-09-13, **not compile-checked and not flashed**.
2. **Test Scroll button** — smallest thing that makes the wheel verifiable. Do this before any tool.
3. **Grid calibration + Test grid overlay** — pure calibration, no game actions, self-verifying.
4. **Buy path** — the safe half, end to end, with a per-run cap.
5. **Sell path** — dry-run first, cap enforced, live only after the overlay is trusted.
