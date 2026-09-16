# Auto pet-food replacement — plan

Status: **design for review. Nothing built.** Scope is deliberately narrow: **stage-6 pets, boarding
only.** Everything the design rests on is measured — see [PET-DATA.md](PET-DATA.md) — except the seven
items in Open Questions.

Goal: **keep a boarded pet fed without anyone watching.** The pet runs out of boarding food, the tool
notices, reloads the feeder, and carries on. Nothing else about the pet is automated.

Related: [PLAN-WATCHER.md](PLAN-WATCHER.md) covers the wider "notice things while you are not looking"
design (death as well as pets) and is the parent of this document. This one is only the food half.

---

## 1. Why stage 6

Stage 6 is the right first target because it is the **shortest and cheapest** boarding cycle of any
stage — and therefore the one where a bug costs least:

| | Stage 6 | Stage 7 | Stage 5 |
|---|---|---|---|
| Food | 高级宠物食物 | 高级宠物食物 | 营养满分宠物食物 |
| Burn | 3/min | 4/min | 1/min |
| Two stacks (600) last | **3 h 20 m** | 2 h 30 m | 10 h 00 m |
| Full stage to +9 | 2,184 items | 3,444–8,568 | 4,200 items |
| Time to +9 | **12 h 08 m** | 14–35 h | 2 d 22 h |

A stage-6 pet completes in about **12 hours and 4 feeder reloads** — short enough to watch a whole run
end to end in one session, which is what makes it a sane first target. Stage 5 is the opposite: a
2 d 22 h job where a wrong calibration burns a weekend before anyone notices.

---

## 2. What the tool has to know, and what it has to remember

The game does the feeding. The tool only **reloads the feeder** — it never feeds. So there are exactly
two pieces of state, and neither needs the screen:

| State | Value (stage 6) | Where it comes from |
|---|---|---|
| **Rate** | 3 items/min | Config, from the stage. Measured game constant. |
| **Load** | 600 items (2 stacks × 300) | What the tool placed last time. |
| **Feeder empties at** | last load + 200 min | Arithmetic — `load / rate`. |
| **Stacks left in the bag** | the marked grid cells | **The Sell-style bag grid** — you mark the slots holding pet food. |

**Where the food is comes from a Sell-style grid — but with its own calibration.** The Buy/Sell tab's
8×8 clickable bag grid is the right *interaction*: click slots, they light up, those are the ones that
go. **The grid itself cannot be shared**, because the bag opens in a different place here than it does
beside the shop window. So this feature carries its own two-corner bag calibration; the Sell one is
**not** reused. (An earlier revision of this document claimed otherwise. It was wrong.)

Past that it works the same way: you mark **every slot holding pet food — around ten of them, not two**
— and that selection is the inventory. The tool consumes them highest index first and knows both where
to click and how many are left, with no bag OCR anywhere: the project has none by design (see
[PLAN-BUY-SELL.md](PLAN-BUY-SELL.md)), and counting stacks by reading the bag would be a large new
failure surface for something you can simply point at.

**Highest index first, for the same reason `SellPass` does it.** Consuming from the top is correct
whether or not the bag compacts, whereas ascending silently skips items if it does — and here a shifted
index does not mis-sell an item, it feeds the pet the wrong stack.

### Pages

**The bag is paged, and ten stacks of food need not sit on page 1.** So the food map is a set of
**(page, cell)** pairs rather than plain cell indices, and the tool has to be able to reach the right
page and know it is on it — a "next page" click issued from the wrong page lands on the wrong food.

That is three more calibrated things: page-forward, page-back, and **the page indicator**, so the tool
can confirm where it is rather than counting clicks and hoping. Whether that indicator is readable is
an open question below.

Two further consequences:

- **The marked cells are the whole inventory model.** There is no separate number to seed and no way for
  two sources of truth to drift apart — if reality changes, you re-mark and the tool is correct again.
- **The bag lock is now doubly load-bearing.** A cell index is trustworthy *only* while the contents
  stay put. The character is auto-farming while this runs, so items are being picked up and consumed
  around the food the whole time; without the lock a compacting bag slides the food into different
  cells, and every marked index points at something else.

---

## 3. The two triggers

You asked for two intervals. They are genuinely different mechanisms, and it is worth being clear which
does what, because **they fail in opposite directions**:

### Trigger A — the schedule (predictive, free)

The tool knows the rate and what it loaded, so it knows when the feeder empties: `last load + 200 min`.
Checking costs nothing at all — no capture, no OCR — because it is arithmetic on state the tool already
holds.

**Catches:** normal operation, which is nearly always.

### Trigger B — the icon (reactive, cheap)

A pixel diff on the 感叹号 region of the pet cartoon image, bottom-right. The icon is the *hunger*
signal — the game's own "this pet is not being fed" indicator.

**Catches:** everything the schedule cannot know — the character went offline (boarding stops and the
schedule falls behind), someone fed the pet by hand, boarding stopped early, or the configured rate is
simply wrong.

### Recommendation: A primary, B as backstop — not one path

**Do both, and do not make the icon the primary.** The reason is a timing asymmetry:

- The icon only appears once **hunger has already dropped** to the feedable threshold. By then the pet
  has been unfed for a while — so an icon-only design puts a **hunger gap into every single reload**,
  by construction. It is not a rare failure; it is the normal case.
- The schedule fires *at* the moment the feeder empties, so there is **no gap at all**.

So the schedule is what keeps the pet fed, and the icon is the safety net for when the schedule's
assumptions break. Both are cheap; together they cost one small capture every 30 s plus an arithmetic
comparison.

**If you want only one, take the schedule.** It is free, it has no calibration, and the icon's own
failure mode (a gap every cycle) is the thing the feature exists to prevent. The icon alone would make
the tool *worse* at its job than no tool at all in the sense that it adds clicks while still letting
the pet go hungry.

**Why not icon-only in one line:** the icon tells you the pet is hungry; the schedule tells you before
it is.

### Interval values

| Trigger | Interval | Why |
|---|---|---|
| **A — schedule** | fires once at `last load + 200 min`; re-check every 5 min while the feeder still reads full | Nothing to poll; the only cost is the re-check. |
| **B — icon** | **30 s** | The state *persists* (the icon stays up until the pet is fed), so a slow poll loses nothing — a missed poll costs 30 s, not the event. |

The persistence is what makes B cheap: unlike a death dialog, hunger does not resolve itself while the
character stays online.

---

## 4. The action flow

```
  trigger A or B fires
          │
          ▼
  click 宠物代养  ──►  boarding window opens
          │
          ▼
  read the feeder slots (empty-check crop, one per slot)
          │
    ┌─────┴──────────────────────┐
    │                            │
  slot empty                   slot full
    │                            │
    ▼                            ▼
  # guard: is the pet at +9?   close, reschedule
  # if so, close and stop      (the schedule was early)
    │
    ▼
  2 × [ right-click a marked bag cell → dialog → MAX → Enter → Enter ]
    │
    ▼
  close the window; next-empty = now + 200 min; 2 cells marked consumed
```

**Everything after the first click is the Sell shape** — `ClickAt(right: true)` then the shared
`MaxEnterEnter` — so the two reloads reuse `ShopTool`'s transaction verbatim. **No firmware change is
needed**: the board already has `C` (click), `R` (right-click) and `E` (Enter).

**The guard matters.** A pet at `+9` has finished its stage; boarding auto-stopped because there is
nothing left to gain, and reloading it wastes two stacks. The boarding window is the right place to
check, because it is already open and it is the only place that knows.

---

## 5. Calibration

Same shape as every other calibrator in this app: capture the game, drag a box or click a point, save
to `local.yaml`, and a **Test** button per action that shows what the tool sees without clicking.

Capture must be `CopyFromScreen` — `PrintWindow` returns black for this game (measured).

| # | What | Kind | Used for |
|---|---|---|---|
| 1 | **宠物代养 button** | point | opens the boarding window |
| 2 | **Feeder slot A** | box | empty-check crop; the drop target |
| 3 | **Feeder slot B** | box | the second slot |
| 4 | **Dialog MAX** | point | *if the dialog has one* — see Open Questions |
| 5 | **Boarding window close (X)** | point | backing out without acting |
| 6 | **感叹号 icon** | box | Trigger B's diff region + reference crop |
| 7 | **Boarding status / EXP%** | box | *optional* — the `+9` guard's read |
| 8 | **Bag grid, two corners** | boxes | the **boarding** bag — its own, *not* the Sell one |
| 9 | **Page forward** | point | reaching pages 2…N where more food lives |
| 10 | **Page back** | point | returning to a known page |
| 11 | **Page indicator** | box | knowing *which* page is showing, rather than counting clicks |

**The bag grid is its own calibration.** Same two-corner method as Sell — a box around the whole 8×8 grid,
a second around one slot, a consistency check between them, and the 64-centre overlay to see the grid is
uniform — but against the bag as it sits in *this* flow, which is somewhere else. Sharing Sell's numbers
would point every cell at the wrong place.

**The food slots themselves cost nothing to calibrate.** Once the grid exists, a food slot is a
`(page, cell)` pair you click, not a captured region — which is what keeps eleven points from becoming
eleven *plus* ten.

The empty-check reuses the existing mechanism exactly: a saved crop, the fraction of differing pixels,
and the **6 px inset** — the inset is not optional, because a one-pixel window shift once scored an
*empty* box at 7.7 %.

**Note on 1 vs 8:** the icon is on the pet cartoon image, but clicking that opens the **喂养** window —
a different system, holding the wrong food. So the icon is a *signal to act*, never the control that
gets you there. The boarding window has its own button, and that is point 1. This is the single easiest
mistake to make in this feature.

---

## 6. What already exists

| Need | Already there |
|---|---|
| Open a window, click a point | `HidPointer.To` + `Click` |
| Right-click a bag slot, work the dialog | `ShopTool.SellPass` + `MaxEnterEnter` |
| An 8×8 bag grid you click to mark slots | the Sell screen's slot picker — the *interaction*, not its calibration |
| Compare a region to a reference | the empty check (crop + differing-pixel fraction + 6 px inset) |
| Drag a box on a captured screenshot | the Buy/Sell and Gem calibrators |
| Read a region as text | `OcrEngine.Scan` — only needed for the optional `+9` guard |
| Tell you something | the card status line + `Beep()` |
| Config | `local.yaml` via `ConfigLoader` |

Nothing new is needed in the firmware, and nothing new is needed in the capture layer.

---

## 7. Failure modes, worst first

1. **Reloading a pet that finished its stage (`+9`).** Wastes two stacks per cycle and never stops.
   Guarded by reading the boarding window before acting.
2. **Feeding the wrong item — now the worst case, not the second.** The character is auto-farming
   throughout, so items are appearing and disappearing beside the food the whole time. Two ways it goes
   wrong: a compacting bag slides the food into different cells, or the tool acts on the wrong **page**.
   Either points a click at something that is not pet food, and unlike a mis-aimed sale there is no
   undo. The **lock is a prerequisite**, not a nicety, and pagination must be confirmed against the page
   indicator rather than assumed.
3. **A wrong-page reload specifically.** "Next page" issued from the wrong page lands on the wrong food,
   after which the tool right-clicks a marked cell index that now holds a different item. This is why
   the page indicator is calibrated rather than clicks being counted and hoped for.
4. **The marked cells drifting from reality.** Lower consequence than (2): the tool reloads an empty
   slot, or misses one. Recoverable by re-marking the grid.
5. **The schedule firing while the character was offline.** The feeder still has food, the empty-check
   says so, and the tool reschedules. Self-correcting, and the reason the empty-check exists rather
   than blind placement.
6. **A capture that is blind** because something covers the game — the launcher included. The watcher
   should say so rather than silently stop seeing.

---

## 8. Phasing

1. **Calibration tab only.** The eleven points above, saved and reloadable, each with a Test button.
   Nothing clicks. Self-verifying by looking at it, and it is the gate for everything after.
2. **Empty-check read-out.** Show the feeder's verdict live, still without clicking. This is the
   measurement that tells you whether the crop threshold is right *before* it is trusted.
3. **One manual reload.** A button that performs the two transactions when you press it. Proves the
   flow on the live game with a human watching.
4. **Trigger A** — the schedule, with the stack counter and the `+9` guard.
5. **Trigger B** — the icon diff, as a backstop.
6. **Stage 7 and 5** — config only, once 6 is trusted.

---

## 9. Open questions

These are the things the design rests on that are **not** yet measured. Questions 1 and 2 block phase 3;
3 and 4 only change numbers; **5–7 decide whether the grid and pagination approach survives at all** and
are worth answering before any of it is built.

1. **Does the boarding item dialog have a MAX?** Unknown. If it does not, the flow is not `MaxEnterEnter`
   and the step count changes — one extra calibrated point, or a different sequence.
2. **Does the bag need to be open while the boarding window is up?** Not yet observed. If it does, that
   is a tenth calibration point and one more click per cycle.
3. **What happens when food is placed into a partially-full feeder?** If it tops up, the schedule can be
   sloppy. If it swaps or refuses, the empty-check must be exact. Changes how tight Trigger A has to be.
4. **Does the feeder really hold two stacks, or two *positions*?** The plan assumes two slots that each
   take a stack, which is why there are two feeder boxes to calibrate. If it is one position accepting a
   stack at a time, those collapse to one and the reload becomes one transaction, not two.
5. **Does the bag show a page indicator the tool can read?** If it does — a number, or a row of dots —
   point 11 becomes a cheap read and pagination is safe. If it does not, the tool has to establish a
   known page first (page back repeatedly until the view stops changing, the same "get to a known
   origin" trick the buy list uses) and count from there, which is materially more fragile.
6. **How many pages does the bag have, and is the grid the same size on each?** The plan assumes the
   8×8 lattice repeats per page at the same coordinates. If the last page is partial, or the grid shifts,
   page-aware cell indices stop being interchangeable.
7. **Is the food kept locked?** Everything in §2 assumes it is. If the slots cannot be locked — or the
   farming run unlocks them — the marked cells are not a stable map and the whole grid approach needs
   rethinking.

---

## 10. What this document deliberately does not cover

- **`.G` pets.** They cannot be boarded at all — they go through the 喂养 window, one item at a time,
  with no auto-feed, and their time depends on which food is used. That is a **different feature**, not
  a different row, and it shares only the icon. See [PET-DATA.md](PET-DATA.md) for the numbers.
- **Stage 5.** The long job (2 d 22 h). Same flow, different config, and the one where a bug is
  expensive — so it comes after 6 is trusted.
- **Death, revive, and the rest of the watcher.** [PLAN-WATCHER.md](PLAN-WATCHER.md).
