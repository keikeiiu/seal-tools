# Auto pet-food replacement — plan

Status: **the reload works, both ways, verified live 2026-09-17.** Both entry states run end to end —
boarding already stopped with the pet in the bag, and the true reload with the pet in the loader, which
is the one that needs ending first.

What that leaves unproven is everything around the reload rather than the reload itself: nothing verifies
the pet actually went in or the food actually landed, and the boarding state is still asserted by hand
(see §4). Scope is deliberately narrow: **stage-6 pets, boarding only, one row.**

Everything the design rests on is measured or confirmed — see [PET-DATA.md](PET-DATA.md) — except the
open questions at the end.

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

### What the EXP% actually is, and why it matters

**`[52.18%]` is 喂养值 eaten over the 喂养值 needed for the WHOLE STAGE** — not for the current level
(player, 2026-09-18, answering the question this section first raised).

**And the stage reading is the one that fits the finished state**, which is what makes it credible
rather than merely asserted: the game stops boarding at `+9` with exactly 100%, and that is the point at
which the pet can evolve. A stage total of 100% is precisely the evolve threshold. Read per level,
`+9` at 100% would mean only "this level is done", with no next level to level into — and nothing would
explain why the breeder stops there.

The news page's worked example (10 fed against a 30-point level showing 33%) still describes the
mechanic correctly; on this reading `30` was simply that pet's *whole-stage* figure for the level it
happened to be describing, and the page was illustrating the division rather than the scope.

**What follows is simpler than the per-level reading would have been:**

```
remaining 喂养值 = base × 12.6 × (1 − pct/100)
```

with `base` from [PET-DATA.md](PET-DATA.md) and the percentage straight off the panel — one subtraction
over the whole run to `+9`, with no level-by-level accumulation and no need to follow the pet's growth
indicator at all.

That is the piece that turns "how much feeding is left" from a guess into arithmetic, because the other
two terms are both available:

| Term | From |
|---|---|
| The level's total cost | [PET-DATA.md](PET-DATA.md) — `base × (1 + n/10)` for level `+n` |
| Which level it is on | the panel's `+N` |
| How far through it | the panel's `[..%]` |

so **this level's remaining 喂养值 is `cost × (1 − pct/100)`**, and the rest of the run to `+9` is the
sum of the levels after it. Divide by the food's 喂养值 for items, or by the rate for time.

**The catch is the `base`.** The per-level cost is `base × (1 + n/10)`, and `base` is a property of the
pet's *species line*, not its stage — a 神兽 costs double a standard line at the same stage. The panel
gives the stage but not the line, and the line lives in the name, which is the one field the OCR cannot
be trusted with. Two ways out, neither built: read the boarding window's own
`到N為止預計所需時間` and multiply by the rate to get the current level's cost directly, or pick the
species once when the tool is set up and treat it as configuration.

### The load size — resolved

The food area reads `150 300`, which I took to be a 300-item cap on the whole load. Combined with the
player's answer — **every two stacks, the pet must be reloaded** — it means:

- **the load is two stacks, 600 items** — exactly what the schedule was first built on;
- **`300` is the per-stack cap**, not the size of a load;
- so a load lasts **200 minutes**, and a stage-6 pet takes **four loads** to `+9`.

The "one slot, 100-minute period, eight reloads" reading was wrong and is withdrawn. (It is also the
second time an inference from a screenshot has been wrong — the Sell-grid one being the first — which is
why the schedule keeps getting marked as provisional until the player confirms it.)

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

**Answered, and the answer is the hard one: ending is mandatory.** The player's rule is that **every two
stacks, the pet must be reloaded** — so topping up while running is not possible, and the
end → re-place → place → start cycle is not an edge case. It *is* the reload path, run **four times** to
take a stage-6 pet to `+9`.

That is a substantially bigger feature than the one this document opened with, and it has a new central
problem.

### Finding the pet — the new central problem

The pet lands in the first free bag slot, and during a farming run the bag is not static — so a marked
cell cannot hold it, and neither can any fixed point. It has to be *found*.

**The player confirms the pet item carries a distinctive pet icon**, which is what makes matching viable.
It is also the mechanism that generalises: **a second pet is another crop, not a redesign** — which is why
this is the right shape for the multi-pet stage that comes later.

The primitive already exists here — a saved crop compared against a region by differing-pixel fraction,
the empty-check's mechanism with its 6 px inset. One pet-icon crop against the 64 calibrated cells is 64
cheap comparisons, **no OCR anywhere**, and it finds the pet **wherever it landed** rather than requiring
it to be somewhere.

#### Why not reserve a slot for it instead

The player's suggestion — pre-calibrate a slot the pet always lands in — is tempting because it turns a
search into a fixed point, and a fixed point needs no crop and no matching. It does not survive the
farming premise:

- "First free slot" is a property of the bag **at the instant boarding ends**, and the character is
  picking loot up throughout. **Loot takes the first free slot too** — so the reserved slot is precisely
  the one loot lands in.
- The lock does not rescue it: locks apply to **items**, and the slot has to be **empty** for the pet to
  land in it.

So a reserved slot can be *expected* but never *guaranteed*. Worth keeping as an optimisation — if the
pet is usually there, that is one comparison instead of 64 — but the full scan has to sit underneath it,
and the scan is cheap enough that the optimisation buys little.

**What remains is visual ambiguity**: if two items look close enough to confuse, the tool right-clicks the
wrong one. That is the failure most worth a deliberate test before any of this is trusted, and the pet's
icon being distinct is encouraging but not the same as *no other item looking similar*.

The same matching could find the **food** too, replacing the marked-cells design outright — but locked
food does not move, so marking stays the simpler answer, and matching should earn its place on the pet
first.

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

**The hunger readout is NOT this pet's** (player, 2026-09-17). `肚子餓(nn%)` at the bottom right belongs
to the **carried** pet — the one equipped on the character — not to the pet sitting in the boarding
feeder. That kills the idea of polling it as the trigger for this feature, and it is a correction to the
first draft of this section, which read the 34 % → 77 % change in the 2026-09-17 captures as boarding
feeding the pet.

Which leaves **the schedule as the only passive trigger**, and it needs nothing read at all — which is
what §3 already argued for on cost grounds. The loop closes in the window instead:

```
schedule fires  →  open the window  →  toggle reads 開始代養?
                                        (boarding is stopped, so it needs a reload)
                                        →  reload  →  start
```

That is still sufficient: the schedule decides *when to look*, and the toggle decides *what to do*. The
only thing lost is the ability to notice an early stop — and the toggle read catches that anyway, one
poll later. What the schedule genuinely cannot do is tell you the pet needs food *before* the boarding
window is opened; it does not need to, because opening it is cheap and causes nothing.

**Open:** whether the carried pet and the boarded pet can be different at all, and what the `目錄` red
means if they can. If the icon is also the carried pet's, then nothing on the main screen says anything
about the boarded one, and the schedule is not merely the cheapest trigger but the *only* one.

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
  click the pet feed icon  ──►  boarding window + bag open
          │
          ▼
  confirm the toggle reads 開始代養  (i.e. boarding is stopped)
          │
    ┌─────┴──────────────────────┐
    │                            │
  stopped                     running
    │                            │
    ▼                            ▼
  # guard: EXP% at +9?         close, reschedule
  # if so, close and stop      (the schedule fired early)
    │
    ▼
  find the pet — template-match its icon across the 64 bag cells
    │
    ▼
  right-click the pet → it drops into the boarding slot
    │
    ▼
  go to the page holding the first marked food cell
    │
    ▼
  2 × [ right-click the marked cell → count dialog → MAX → Enter ]
    │        one Enter — there is no second confirmation here
    ▼
  click 開始代養;  close;  next-empty = now + 200 min
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

**The guard matters, and "finished" is narrower than it sounds.** A pet is finished at **`+9` AND 100% EXP or more** — not `+9` alone (player, 2026-09-18), and
100% exactly counts. At `+9` under 100% it is still growing and still wants
feeding; the game stops boarding only once it is both, which is the state that lets it evolve.

**And at `+9` the boarding stops at exactly 100%, not past it** (player, 2026-09-18). Below `+9` hitting
100% simply levels the pet up and it carries on; at `+9` there is no next level, so the breeder feeds it
to the evolve threshold and stops. `100.00%` is therefore the normal finished reading rather than an
edge case — which is why the comparison is inclusive.

So the check is on **growth and EXP together** — `+9` and `EXP >= 100` — and reading the growth alone
would stop feeding a pet that has not finished, the opposite of the failure this guard exists to
prevent. Both are on the pet's panel as `+N` and `[..%]`, and both read cleanly
(see [PLAN-HOVER-INFO.md](PLAN-HOVER-INFO.md)).

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
| 5 | **Boarding toggle label** | box | `結束代養` vs `開始代養` — a direct read of whether boarding is running |
| 6 | **Boarding window close (X)** | point | backing out without acting |
| 7 | **Hunger % region** | box | `肚子餓(nn%)` — Trigger B, or the whole trigger |
| 8 | **Bag grid, two corners** | boxes | the **boarding** bag — its own, *not* the Sell one |
| 9 | **Pet item icon** | crop | **template-matched across the 64 cells** to find the dropped pet |
| 10 | **`ITEM1` tab** | point | go to page 1 directly |
| 11 | **`ITEM2` tab** | point | page 2 |
| 12 | **`ITEM3` tab** | point | page 3 |

**There is deliberately no MAX point.** The boarding count dialog is **the same dialog the buy/sell
tools use** (player, 2026-09-17), so `BuySellConfig.MaxButton` is reused verbatim. A second mark for one
button would be a second thing to drift, and a wrong MAX here means feeding the wrong quantity.

**The sequence is still not the same, though.** This dialog takes MAX and then **one** Enter — there is
no confirmation behind it — so `MaxEnterEnter`'s second Enter must not be sent. Same clicks, one fewer
keystroke, and getting that wrong would fire an Enter into whatever the game shows next.

**Point 9 is the odd one out** — the only crop used to *find* something rather than to detect a change or
check emptiness. The pet's bag icon is the **pet's own portrait** (the player's example: a 咕咕寶寶), which
is what makes matching possible at all — and also what means **a second pet is a second crop, not a
redesign**, so the multi-pet stage is a config addition. It is still the piece of this design I am least
confident in: a poor match clicks an arbitrary bag item, which is the worst failure the feature has.

**Getting to the feeder is two clicks, not one.** `目錄` opens a panel of eight round icons, and the pet
feed icon is one of them — so the flow needs the menu button *and* the icon, in that order. Both are at
fixed positions, which means **points, not reference images**: nothing has to be *found*.

The panel carries **▲▼ scroll indicators**, so it may hold more icons than the eight visible. If the pet
icon ever needs scrolling to reach, that is the bag-page problem again and wants the same answer —
navigate to a known position, never scroll-and-hope.

**The bag grid is its own calibration.** Same two-corner method as Sell — a box around the whole 8×8 grid,
a second around one slot, a consistency check between them, and the 64-centre overlay to see the grid is
uniform — but against the bag as it sits in *this* flow, which is somewhere else. Sharing Sell's numbers
would point every cell at the wrong place.

**The food stacks themselves cost nothing to calibrate.** Once the grid exists, a food cell is a
`(page, cell)` pair you click, not a captured region — which is what keeps the point count from growing
with the number of stacks.

The empty-check reuses the existing mechanism exactly: a saved crop, the fraction of differing pixels,
and the **6 px inset** — the inset is not optional, because a one-pixel window shift once scored an
*empty* box at 7.7 %.

**One thing not to confuse:** the trigger is the `目錄` button going red, and that is a *signal to act* —
never the control that gets you there. Clicking the **pet cartoon image** on the main screen opens the
**喂养** window, a different system holding a different food. The boarding window is `目錄` → the pet feed
icon, points 1 and 2. That is the single easiest mistake in this feature.

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
3. **Reloading a pet that has finished.** `+9` with 100% EXP or more is the state that lets a pet evolve,
   and the game stops boarding there; restocking it wastes the food every cycle. Guarded by reading the
   growth and the EXP **together** — see above for why `+9` alone is the wrong test. The mirror failure
   is treating `+9` as finished and abandoning a pet that still wants feeding.
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

1. ~~**Can the feeder be topped up while boarding is running?**~~ **Answered: no.** Every two stacks the
   pet must be reloaded, so *end → re-place pet → place food → start* is the reload path, four times per
   stage-6 pet. Which promotes the next question from an implementation detail to the risky part.
2. **Could another item be mistaken for the pet?** The pet's bag icon is **the pet's own portrait** — a
   咕咕寶寶 for the player's current pet (2026-09-17) — so matching is viable, and a second pet is a
   second crop rather than a redesign. What is still unknown is what else ends up in the bag, since
   farming fills it with loot. One near-miss means right-clicking the wrong item, which is the failure
   most worth a deliberate test: fill the bag with a typical farming load, run the match across all 64
   cells, and **read the scores** rather than only the winner. **Still the highest-risk unknown.**
3. ~~**Does the count dialog have a MAX?**~~ **Answered: yes, and it is the same dialog the buy/sell
   tools use** — so its point is reused rather than re-marked, and no MAX is calibrated on the pet tab.
   The part to keep in view is the sequence, which is *not* the same: MAX then **one** Enter, with no
   confirmation behind it.
4. **Is the 8×8 lattice identical on all three pages?** Assumed, with the last possibly partial. If the
   grid shifts per page, `(page, cell)` indices stop being interchangeable. (The page-*indicator*
   question is closed — absolute `ITEM1`/`ITEM2`/`ITEM3` tabs removed the need to read it at all.)
5. **Does the 感叹号 menu icon blink?** If it animates, a single reference crop will flap and the diff
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

---

## 11. The multi-pet direction

**Not built, and deliberately not structurally prepared for yet.** Recorded because it changes what
counts as a good decision *now*, and because the restructure is cheap while it is imagined and
expensive once the code assumes one pet everywhere.

### What the window already supports

`PET BREED` shows **four rows**: one active, three locked behind `需要擴張欄位` and the paid
`+ 15 Days` / `+ 30 Days` service. So four pets can be boarded at once once the service is paid for.
The rows are already there; this is not a redesign of the window.

### Why it is not just "the same thing four times"

| | Today (one row) | Four rows |
|---|---|---|
| Start button | one, marked | **one per row** (believed — see below) |
| Food location | one set of cells | **per row**, and possibly on different bag pages |
| Food type | whatever you locked | **can differ**, because the pets can be at different stages |
| Schedule | one clock | **per row** — a stage-6 pet burns 3/min and a stage-7 4/min, so one timer is wrong for one of them |

The food point is the one that catches people out: [PET-DATA.md](PET-DATA.md) gives 一般宠物食物 for
stages 1–3, 营养满分宠物食物 for 4–5 and 高级宠物食物 for 6–7, so two pets on different stages are fed
from two different items, in two different places.

### Two directions, and which to take

**B — custom setup (recommended first).** You mark each row: which cell the pet goes back into, which
cells hold that row's food. The tool reloads on schedule. This is what is built today, extended from
one row to N. It is honest about who decides, and it needs no new reading.

**A — automatic (later, if it earns its place).** The tool finds whatever pet is not yet `+9` 100 %,
puts it in, and feeds it the right food for its stage. Feasible because **hovering a pet's bag icon
shows its name and growth** (player, 2026-09-17) — and the name resolves to a stage through the table in
[PET-DATA.md](PET-DATA.md), which is already scraped. So A reads something that exists rather than
inventing a signal.

What A costs, stated plainly: a hover-and-wait per pet, an OCR of the tooltip, and the cursor moving
around the bag. That is affordable **once, as a setup scan** — "which pet should I board?" — and not
affordable per reload. So A is a chooser bolted onto B, not a replacement for it.

**Recommendation: build B, and treat A as a natural extension of it.** A's hard part — identifying a
pet and its stage — is exactly the part that would be wasted if B's per-row setup is never generalised.

### UI: a row selector, not four tabs

Four separate tabs would put each pet's marks out of sight of the others, and the likeliest mistake is
**which row you are editing** — the same mistake the bag picker already guards against by painting the
active page. One screen with `Row 1 … Row 4` where the page selector sits makes that visible, and makes
adding a row a click rather than a new tab.

### What to do before building any of it

**Buy one slot and capture the second row.** Two of the three unknowns are answerable only from a
screenshot of an unlocked row — whether the start button is per row, and whether the food slots are
per row or shared — and a restructure built on a guess is a restructure done twice.

---

## 12. What the live runs taught, that the design could not

Recorded because these are measurements now, not guesses, and several of them were wrong in the design.

**The reload takes ~15 seconds.** Comfortably inside the 185-minute cycle, so no delay below is
expensive and every one of them can afford to be generous.

**Ending boarding has to be a step, and the design did not have it.** The reload was drawn as
place → food → start, which only works when the pet is already in the bag. A true reload starts with it
in the loader, where its bag cell is empty — so the right-click meant to place it hits whatever is in
that slot instead. Fixed by pressing the 開始代養 / 結束代養 button first (player confirmed that stopping
the feed is how the pet comes back, and that all the food comes out with it, unfinished stacks too).

**That button is a TOGGLE, so the tool has to know the state.** Pressing it while boarding runs ends it;
while stopped, starts it. There is no reading-free way to be sure, so today it is a checkbox on the Pet
tab that the tool ticks for itself after a successful reload. Replacing it with a crop comparison of the
label is the obvious next step and needs no new machinery.

**The out-of-food message.** When the pet has used up ALL its food, opening the breeder raises a message
that has to be dismissed before 結束代養 is usable. An Enter after the window opens clears it and is a
no-op otherwise, so it is sent every time.

**A cursor move is not finished until it has been SEEN to move.** The placement loop treated two polls
agreeing as "settled", which is equally true of a move that has not started. On a full-height move the
loop fed corrections forward against a stale position and the queued asks landed together, clamping at
the top of the screen. The shop never exposed it because its corrections are small.

**Something moves the cursor that is not the tool** — one trace shows 550 px of travel in response to a
61 px ask. Unidentified. A re-aim immediately before each click covers it, but the cause is worth
knowing, and it is the most likely explanation for a click that lands on the right cell and does nothing.

**The delays, as they now stand.** All guesses except the two marked:

| Delay | Value | Note |
|---|---|---|
| after the start/end press | 2.5 s | the pet returning to the bag |
| after the start press | 2.0 s | the start taking, before the cleanup closes the window |
| after 目錄, and after a page tab | 0.9 s | panel opening; bag re-rendering |
| after Enter / MAX | 0.5 s | dialog opening or closing |
| between aiming and clicking | 0.35 s | plus a re-aim, because the cursor can drift in it |

---

## 13. The pet queue — design, not built

Goal: **board the next pet by itself.** You keep a line of pets waiting; when the boarded one
finishes, the tool puts the next one in.

### The two ways boarding ends, and why they differ

| Ending | The pet | The leftover food |
|---|---|---|
| **Manual** — pressing 結束代養 | returns to the bag | returns to the bag |
| **Auto** — the pet reaches `+9` at 100% | **mailed to the player** | **mailed to the player** |

That second row is what makes this feature small. The finished pet **leaves on its own** — there is
nothing to remove, and nothing to put back — so the only thing the tool does differently when a pet
finishes is take the *next* one out of the bag instead of the same one.

(An earlier revision of this document recorded only the manual case and generalised it to both. The
mailbox is the difference, and it is the one that matters here.)

### How the next pet is found — and why the bag can keep moving

Marked cells assume the bag does not change, and the premise is that the character **farms while this
runs** — loot fills the bag and a static map stops being true. So the queue is not a list of cells.

**It is a list of ICONS: the one to three pets you actually breed.** The player's insight (2026-09-18):
there are hundreds of pets and no need to know them, because the tool only ever looks for the handful
you are working on. Everything needed for that already exists:

- `IconMatch` — a saved crop compared across the 64 cells by differing-pixel fraction, no OCR. Written
  for this, then set aside while the simpler flow was proven.
- The stage/EXP read — a matched cell is hovered, its panel read, and the pet is boarded only if it is
  **not** already `+9` at 100%.

```
scan the bag for a cell matching one of the known pet icons
  no match          → nothing to board; stop and say so
  match             → hover it, read the panel
      +9 and 100%   → finished; look for another
      otherwise     → board it
```

**First match wins, then the panel confirms it** (player, 2026-09-18): the icon narrows the field and
the tooltip read is the double-check on stage and growth. So a poor icon match is caught downstream
rather than acted on.

**And no match means something specific**: every pet in the bag is finished, so there is nothing to
board — not "the search failed". That is a reportable end state rather than an error.

**The status check is what makes one-to-three icons safe.** Without it, a finished pet left in the bag
would be boarded over and over; with it, "is this one done?" is answered per candidate rather than
assumed from position. And it is the same read the reload already wants for its own guard, so nothing
new is needed to ask.

The cost, stated honestly: a hover and an OCR per candidate cell, so the scan is seconds rather than
milliseconds. Acceptable because it runs once per reload — every few hours — rather than per cycle.

### Two different hard problems, only one of which needs the scan

**Pets waiting in the bag can be LOCKED** (player, 2026-09-18), and a locked pet keeps its cell no
matter what the character picks up. So the queue in its resting state is not a search problem at all —
it is a set of known positions, and the lock is what makes that true. Nothing here changes for them.

**The pet that comes OFF the breeder is the hard one.** When a pet's two stacks run out it is returned to
the bag, and *that* landing place is the variable one: the character has been farming the whole time, so
the free slots are not the free slots that existed when boarding started. A locked pet cannot be that
one — it was not in the bag to be locked.

So the icon scan earns its place on **one pet per reload** rather than on the queue. Everything else
keeps working positionally, and the search exists for the case that genuinely cannot be one.

*(Worth noting what this does to the earlier worry: the reason a static map was thought unworkable was
loot moving things, and the lock answers that for anything the player parks. It is specifically the
offloaded pet that has no home position.)*

### Where the queue configuration lives

**On the Pet tab, not Calibrate** (player, 2026-09-18). The crops are the player's own working set —
which pets are being bred right now — and that changes between runs in a way the bag grid never does.
Putting them under Calibrate would hide a per-run decision behind a once-a-machine screen, and the Pet
tab is already where the marks that describe the bag *as it is now* live. The saved crops sit beside
them for the same reason: so the configuration is visible and checkable before a run, not inferred.

*(What this replaces: a queue of marked bag cells, which was the first shape and assumed a still bag.
Kept in the history because the reasoning that killed it — loot moving things — is the same reasoning
that protects the food cells, where the lock makes a static map true.)

**A "this pet is finished" test**, and it is `+N` **and** `EXP >= 100` read off the pet's panel — see
§12 for why `+9` alone is the wrong test, and why the compare is inclusive.

**The confirmation.** When a pet finishes, opening the breeder shows a message saying so, dismissed with
Enter — the same shape as the out-of-food message, so the Enter the reload already sends after opening
the window may already cover it. Unverified.

### The cycle

```
open the breeder                      目錄 → pet icon → Enter
read the boarded pet's panel          stage, +N, EXP
  finished?  (+N == 9 and EXP >= 100)
      → advance the queue, take the next pet's cell
  not finished?
      → the same pet, as now
place that pet                        right-click its cell
load two stacks                       page tab → right-click → MAX → Enter  (×2)
start and close                       the start button → the X
```

Everything except "which cell does the pet come from" is the existing reload, unchanged.

### Open questions

1. ~~**When a pet is mailed, does the bag compact?**~~ **Answered:** it leaves a **hole** (player,
   2026-09-18). Which is what makes a static map possible at all — and also what the icon list does not
   have to care about, since it looks for the pet rather than for a position.
2. **Is the finish confirmation the same dialog as the out-of-food one?** If it is, nothing new is
   needed; if it is a distinct one, it may need its own dismissal.
3. **What ends the run?** When the queue empties — stop and say so, or wait and re-check? Stopping is
   the honest answer, since the alternative is polling forever for a pet that is not there.
