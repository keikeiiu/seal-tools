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

### Buying

The safe half. Worst case is spending gold.

1. Click the list entry (position comes from calibration; see the open question about list order).
2. The COUNTER opens — click **MAX** for a stack, or the keypad for a specific number.
3. Click confirm.
4. Repeat `n` times, where `n` is set per item.

The COUNTER's keypad is clickable, so quantity is set by clicking buttons, not by typing into a
field. **That is why no backspace was added to the firmware** — a fixed sequence of clicks needs no
text-field semantics from the game at all.

### Selling

The destructive half, and worth being blunt about: a mis-aimed click sells the wrong item and it is
gone. Every other tool here is recoverable — a bad roll costs springs. This one does not.

1. Right-click the slot (the firmware's `R`; no new command needed).
2. If the game confirms, click confirm.
3. Repeat across the slots you selected.

Required before it runs live:

- a **hard per-run cap**, set by you, enforced in the loop — not advisory
- a **dry-run mode** that highlights the slots it would click and sells nothing
- the grid overlay above as the calibration gate

Selecting *which* slots is the part still undesigned. Options: an explicit list of slot indices; a
whole-page sweep; or an attribute rule reusing `AttrMatcher` (already flagged in
[IDEAS.md](IDEAS.md) as the natural home). The rule-based one is the most useful and the most work.

### Scrolling

Firmware now has `Q n` / `Z n` (wheel up/down, n notches). Two things to know:

- The wheel scrolls **whatever is under the cursor**, so the tool must place the cursor over the list
  before scrolling — that is a calibration point, not a firmware concern.
- How many notches move one row is unknown and must be measured. A **Test Scroll** button in the
  calibrator (point at the list, scroll n, see where it lands) is the cheap way to find it.

---

## Open questions

1. **Does the shop list order or contents change between visits?** This is the biggest fork. If the
   list is stable, an item sits at a fixed row and we click it directly. If stock changes what is
   listed, we need scroll-and-find by OCR, which is markedly slower and needs the list readable.
2. **How many notches equal one row?** Measurable with a Test Scroll button.
3. **Does right-clicking an item sell immediately, or open a confirmation?** If it confirms, that is
   another point to calibrate.
4. **Do stacked items show a count on the icon?** Decides whether the tool can detect "this needs a
   quantity" itself, or you flag it per item.
5. **A capture of the quantity dialog**, so its buttons can be measured.

---

## Phasing

1. ~~Firmware: wheel + full keyboard~~ — landed 2026-09-13, **not compile-checked and not flashed**.
2. **Test Scroll button** — smallest thing that makes the wheel verifiable. Do this before any tool.
3. **Grid calibration + Test grid overlay** — pure calibration, no game actions, self-verifying.
4. **Buy path** — the safe half, end to end, with a per-run cap.
5. **Sell path** — dry-run first, cap enforced, live only after the overlay is trusted.
