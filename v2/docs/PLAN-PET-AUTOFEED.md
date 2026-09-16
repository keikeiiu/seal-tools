# Auto pet-food replacement — plan

Status: **design for review. Nothing built.** Scope is deliberately narrow: **stage-6 pets, boarding
only.** Everything the design rests on is measured or confirmed — see [PET-DATA.md](PET-DATA.md) —
except the five items in Open Questions.

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

**The farming is the game's own, so the board is free.** The premise is that this runs while the
character auto-farms. That feature belongs to the game and runs by itself — it is not one of our tools —
so it holds no Arduino port, and the one-tool-at-a-time rule in [PLAN-WATCHER.md](PLAN-WATCHER.md) never
bites. Had the farming been ours, the pet reload would be impossible while it ran and this feature would
need a different shape entirely.

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

### What the live boarding window shows

Captured 2026-09-17. The window is titled **PET BREED** and the player has confirmed the chick icon opens
it — so it is the 代養 window, not 餵養.

| Shown | Reading | Why it matters |
|---|---|---|
| `每1分 讀取3個` | 3 per minute | **The game states the burn rate**, and it matches the stage-6 figure derived from the news page — so the model is confirmed from the game itself rather than inferred |
| `79.04%` | the pet's EXP | the `+9` guard's read |
| `到+9為止預計所需時間: 約 15分` | ETA to `+9` | the advance feature's headline number, **displayed by the game** rather than computed by us — if it reads stably, that feature is an OCR away |
| `結束代養` / `開始代養` | **one toggle button** | the same button starts and ends boarding (player, 2026-09-17), so its **label is a free state read** — `結束代養` means running, `開始代養` means stopped |

**That toggle is the most definitive state read available.** Boarding auto-stops on `+9`, on logout, and
probably when the food runs out; in every one of those cases the button flips back to `開始代養`. So one
small crop compared against a saved label answers *"is this pet currently being fed"* directly.

**But it is not passive, and that decides where it belongs.** The button is only visible while the
boarding window is open, so it cannot be the polling trigger without opening a UI window every 30
seconds while you play. The 目錄 icon and the hunger % are both visible without opening anything.

So the split is: **poll passively, then confirm on the button.** The cheap check decides *whether* to
open the window; the button label decides *what to do* once it is open. Same "a nearly-free check wakes
an expensive one" pairing the watcher uses for death — and it means the trigger question (icon versus
hunger %) is only about which passive signal is more reliable, not about the button at all.

It also does **not** say *why* boarding stopped, which is the `+9` guard's job.

**And it raises a question the flow depends on: must boarding be stopped before the feeder can be
restocked?** If the food slot is only editable while stopped, every reload becomes
*end → restock → start*, and the tool has to put the state back exactly as it found it rather than just
closing the window.

### One thing that capture contradicts

The food area reads as a **single** slot showing `150 300` — current over cap — with three **locked** rows
below, marked `需要擴張欄位` and `+ 15 Days` / `+ 30 Days`.

If that reading holds, two figures recorded earlier are wrong:

- **"the feeder holds two slots."** It looks like one. The locked rows are more likely *additional pet*
  slots — the 代養欄位擴張券 the news page describes — than extra food slots, and those are different
  things.
- **A 600-item load.** A 300 cap makes a reload **300 items, not 600**, so the feeder empties in
  **100 minutes, not 200**, and a full stage-6 pet takes **8 reloads, not 4**.

Both are left as they stand until confirmed. (Rewriting a schedule on an inference is how the earlier
"the Sell grid can be reused" mistake happened, and that one was only caught because the player knew the
bag moves.)

### Ending boarding drops the pet too

Player, 2026-09-17 — and visible in the capture where the button reads `開始代養` with **both** the pet slot
and the food slot empty: ending boarding returns the pet as well as the leftover food.

So a reload that has to end boarding is **not a top-up**. It is:

```
end  →  re-place the pet  →  place the food  →  start
```

That is much more than two right-clicks, and it pulls the **pet item itself** into the bag map.

**And the pet lands in the first free bag slot** (player, 2026-09-17) — so its position is a function of
whatever else the bag holds at that moment, which during a farming run is changing constantly. That is
the worst property a cell-indexed map can have: the pet is not *at* a cell, it is wherever the bag
happened to be empty. Reserving a slot only fixes it if the bag's fill state is controlled, and it is
not.

So there are two ways to handle the returned pet and both are bad:

- **Find it** — which needs bag OCR, and this project has none by design
  ([PLAN-BUY-SELL.md](PLAN-BUY-SELL.md)).
- **Reserve a slot** — which only works while nothing else changes the bag, and the whole premise is
  that the character is farming.

**Which leaves not ending boarding at all as the only robust answer.** Everything above collapses into
the single question below.

**Which makes one question decide the whole shape of the feature: can the feeder be topped up while
boarding is running?**

- **If yes**, the tool never ends anything, the pet is never dropped, and none of the above exists. The
  reload stays the simple two-transaction flow, and the four-step cycle is a manual-recovery path the
  tool only needs to *detect*, not perform.
- **If no**, every reload is the four-step cycle, the grid grows a second selection, and there is a
  second way to mis-click — one that leaves the pet sitting unboarded in a bag slot rather than merely
  wasting food.

This is now the most important unknown in the document, and it is a single observation in-game.

**The bag is paged, and ten stacks of food need not sit on page 1.** So the food map is a set of
**(page, cell)** pairs rather than plain cell indices, and the tool has to reach the right page.

**Navigate by absolute tab, not by "next".** The page control is calibrated as **three points, one per
page**, and the tool clicks the page it wants directly. That is what makes pagination safe: clicking the
"page 2" tab lands on page 2 *whatever page you were on*, so there is no relative navigation to get out
of step, and **nothing needs to be read** — the tool knows which page it is on because it chose. A
"next page" scheme would need the indicator read back to confirm each step; absolute tabs need nothing.

(The player's call, and it is the better one — it removes the indicator question rather than answering
it.)

**The bag comes up on its own.** Opening the boarding window auto-opens the bag, at a fixed position —
just not the same one the buy/sell flow uses. So there is no bag-open click to calibrate and none to
spend each cycle; the grid merely has to be measured where *this* flow puts it.

**The lock is the player's tool, not ours.** The bag lets you lock individual items in place, and the
ten food stacks are expected to be locked. That is what makes a marked cell a stable address across a
farming session, and it is why the plan treats "food is locked" as a setup fact rather than something
to verify.

**What you own, and what the tool owns.** You buy the right food and mark where it lives; the tool keeps
the feeder loaded so you do not have to attend the PC around the clock. That split is deliberate and it
is the load-bearing assumption of the whole feature: **the tool never verifies the item is the right
one.** There is no bag OCR (above), so a marked cell is *trusted* to hold pet food. If the wrong item
gets marked, the tool will cheerfully feed it — which is exactly why marking is a setup step you own
rather than something the tool infers.

**Ten marked stacks is a full stage-6 pet with one reload to spare**, which is what makes "ten" the right
default rather than an arbitrary number: 2,184 items is 8 stacks, the feeder takes 2 per reload, so ten
cells is five reloads against the four a complete pet needs.

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

A pixel diff on the **目錄 button** in the bottom-left icon cluster, which gains a **red highlight** when
the pet wants attention — confirmed by the player against the 2026-09-17 capture, where it is lit. (The
pet's own cartoon image also shows a prompt, but the player's observation is that this is the reliable
one, and it is the signal the original watcher design was built around.)

The tool needs the button's box in both states: **lit** for "needs attention", **plain** as the
reference. The plain one is still uncaptured, and it is the one that matters — a diff needs the *quiet*
state to compare against.

**Catches:** everything the schedule cannot know — the character went offline (boarding stops and the
schedule falls behind), someone fed the pet by hand, boarding stopped early, or the configured rate is
simply wrong.

**A better trigger may exist, and it is visible in the 2026-09-17 capture.** The game prints the pet's
hunger as a **readable percentage** at the bottom right — `肚子餓(34%)`, sitting directly above
`EXP(102.86%)` — which matches the news page's "look at the bottom-right for the pet's hunger". If that
text is stable enough to read, an OCR poll replaces the pixel diff outright:

- a number is **unambiguous** where a diff is binary, so it also gives *how* hungry, not just that it is;
- it **retires the "does the icon blink" question** — there is no reference crop to flap;
- the **same region carries the EXP%**, which is exactly what the `+9` guard wants, so one read could
  serve both.

The cost is an OCR per poll rather than a near-free diff — which matters far less here than it does for
death, because this state *persists*. **Worth settling before the trigger is built**, because it could
remove the icon calibration and the guard's separate region in one go.

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
  click 目錄  ──►  secondary icon panel opens
          │
          ▼
  click the pet feed icon  ──►  boarding window opens, bag auto-opens with it
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
  go to the page holding the first marked cell, confirm on the indicator
    │
    ▼
  2 × [ right-click the marked cell → count dialog → MAX → Enter ]
    │        one Enter — there is no second confirmation here
    ▼
  close the window; next-empty = now + 200 min; 2 cells marked consumed
```

**The transaction is the Sell shape minus its last step.** Right-click the bag cell, click MAX in the
count dialog, press Enter — then stop. `MaxEnterEnter` sends a *second* Enter for the sell confirmation,
and **this dialog has no confirmation behind it**, so the reload needs a sibling rather than a straight
reuse: same two clicks, one Enter instead of two. The count dialog's MAX still has to be calibrated,
because it is a different dialog in a different place from the sell one.

**Dragging is the other way the game allows loading the feeder, and it is not available to us.** The
firmware has no press/release pair — `C` is a press and release in a single command — so a drag would
mean new commands and a reflash of every board. The right-click path needs none of that.

**No firmware change is needed at all**: the board already has `C` (click), `R` (right-click) and `E`
(Enter).

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
| 1 | **目錄 button** | point | opens the secondary icon panel |
| 2 | **Pet feed icon** — the chick holding a bottle | point | opens the boarding window |
| 3 | **Feeder slot A** | box | empty-check crop; the drop target |
| 4 | **Feeder slot B** | box | the second slot |
| 5 | **Count dialog MAX** | point | *if it has one* — a different dialog, in a different place, from the sell one |
| 6 | **Boarding toggle label** | box | `結束代養` vs `開始代養` — a direct read of whether boarding is running |
| 7 | **Boarding window close (X)** | point | backing out without acting |
| 8 | **Hunger % region** | box | `肚子餓(nn%)` — Trigger B, or the whole trigger (see below) |
| 9 | **Bag grid, two corners** | boxes | the **boarding** bag — its own, *not* the Sell one |
| 10 | **`ITEM1` tab** | point | go to page 1 directly |
| 11 | **`ITEM2` tab** | point | page 2 |
| 12 | **`ITEM3` tab** | point | page 3 |

**Getting to the feeder is two clicks, not one.** `目錄` opens a secondary panel of eight round icons,
and the pet feed icon is one of them — so the flow needs the menu button *and* the icon, in that order.
The icon is at a fixed position in a static grid, which means **a point, not a reference image**: nothing
has to be *found*.

The panel carries **▲▼ scroll indicators**, so it may hold more icons than the eight visible. If the pet
icon needs scrolling to reach, that is the same problem as the bag pages and wants the same answer —
navigate to a known position, never scroll-and-hope.

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
| Right-click a bag slot, work the count dialog | `ShopTool`'s right-click + MAX, **minus** the second Enter — this dialog has no confirmation |
| An 8×8 bag grid you click to mark slots | the Sell screen's slot picker — the *interaction*, not its calibration |
| Compare a region to a reference | the empty check (crop + differing-pixel fraction + 6 px inset) |
| Drag a box on a captured screenshot | the Buy/Sell and Gem calibrators |
| Read a region as text | `OcrEngine.Scan` — only needed for the optional `+9` guard |
| Tell you something | the card status line + `Beep()` |
| Config | `local.yaml` via `ConfigLoader` |

Nothing new is needed in the firmware, and nothing new is needed in the capture layer.

---

## 7. Failure modes, worst first

1. **The pet dying.** The news page is explicit that below 100 % EXP, hunger reaching zero *twice* kills
   the pet. So a long unattended gap is not merely lost growth — it is the one outcome here that cannot
   be undone, and it is why the schedule wants a wide margin rather than a tight one. This is the only
   failure in the list whose cost is permanent.
2. **Leaving the pet dropped.** Ending boarding returns the pet to the bag, so a reload that *has* to end
   — or a tool that dies partway through one — can leave the pet sitting unboarded, which is failure (1)
   on a timer. **If topping up while running is possible this failure does not exist at all**, which is
   the strongest argument yet for the simple flow.
3. **Reloading a pet that finished its stage (`+9`).** Wastes the food per cycle and never stops.
   Guarded by reading the boarding window before acting.
4. **Feeding the wrong item.** The character is auto-farming throughout, so items appear and disappear
   beside the food the whole time; a compacting bag slides the food into different cells and a marked
   index now points at something else. Unlike a mis-aimed sale there is no undo. **The lock is a
   prerequisite**, not a nicety. (Absolute `ITEM1`–`ITEM3` tabs removed the wrong-page version of this,
   since clicking a tab cannot overshoot.)
5. **The marked cells drifting from reality.** Lower consequence than (4): the tool reloads an empty
   slot, or misses one. Recoverable by re-marking the grid.
6. **The schedule firing while the character was offline.** The feeder still has food, the empty-check
   says so, and the tool reschedules. Self-correcting, and the reason the empty-check exists rather
   than blind placement.
7. **A capture that is blind** because something covers the game — the launcher included. The watcher
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

These are the things the design rests on that are **not** yet confirmed. Only the first two block
building the calibration tab; the rest change numbers or tolerances.

Answered by the player on 2026-09-17, and now folded into the design above: the farming is the game's
own; the bag auto-opens with the boarding window; the bag has **three** pages; the feeder has two slots;
the food is locked via the bag's own lock; the count dialog takes **one** Enter with no confirmation
after it; and all ten stacks are a single item type, which is your setup responsibility rather than
something the tool checks.

1. **Can the feeder be topped up while boarding is running?** The decisive one — see above. It is also
   the same question as "what happens when food goes into a partially-full feeder": if adding food
   works while running, the tool never needs to end anything and the pet is never dropped; if it does
   not, every reload becomes *end → re-place pet → place food → start*. **One observation answers both
   halves**, and it decides whether this feature has one grid selection or two.
2. **Does the count dialog have a MAX?** Assumed yes, since the sell dialog does and the sequence is
   otherwise identical. If it does not, the quantity is typed and the flow changes shape — the one
   remaining unknown that would alter the *transaction* rather than its numbers.
3. **Is the 8×8 lattice identical on all three pages?** Assumed, with the last possibly partial. If the
   grid shifts per page, `(page, cell)` indices stop being interchangeable. (The page-*indicator*
   question is closed — absolute `ITEM1`/`ITEM2`/`ITEM3` tabs removed the need to read it at all.)
4. **Does the 感叹号 menu icon blink?** If it animates, a single reference crop will flap and the diff
   needs two crops or a wider tolerance. Two captures — plain and red — answer it and supply the
   reference at the same time, so this one is free.

---

## 10. What this document deliberately does not cover

- **`.G` pets.** They cannot be boarded at all — they go through the 喂养 window, one item at a time,
  with no auto-feed, and their time depends on which food is used. That is a **different feature**, not
  a different row, and it shares only the icon. See [PET-DATA.md](PET-DATA.md) for the numbers.
- **Stage 5.** The long job (2 d 22 h). Same flow, different config, and the one where a bug is
  expensive — so it comes after 6 is trusted.
- **Death, revive, and the rest of the watcher.** [PLAN-WATCHER.md](PLAN-WATCHER.md).
