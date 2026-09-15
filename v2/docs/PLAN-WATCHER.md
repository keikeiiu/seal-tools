# Watch — passive monitoring and unattended actions

Status: **design only, not built.** Two triggers are agreed and the architecture falls out of a
constraint that already exists in the codebase; the open questions at the end are the ones that decide
how much of it is safe to build.

Goal: notice two things about the game while you are not looking, and either act or tell you.

| Trigger | Detected by | Because |
|---|---|---|
| **Character died** | **OCR** on a calibrated region | You said there is text to read, so the read is the direct signal |
| **Pet needs attention** | **Pixel diff** on a menu-bar icon | The game puts a notification on that icon, and a diff is the cheapest way to see it appear |

---

## The constraint that decides the architecture

**One Arduino port, one tool at a time.** `LauncherService` holds a single `SerialPort`, and starting a
second tool is refused. So:

- **The watcher is not a tool card.** A tool holds the port for its whole run; the watcher must *poll
  without the port* and only take it in order to act. It is therefore a launcher-level service with a
  toggle in the chrome row beside Hold Space — not a Start/Stop card. That also keeps it out of the
  one-tool-at-a-time rule, so it can keep watching while the composer runs.
- **It acts only when no tool is running.** If a tool holds the port, the watcher notifies and defers
  rather than queueing a click.

> Acting *alongside* a running tool was considered and set aside. Two things driving the board means
> interleaving commands on one serial line and deciding which wins — a much larger change, with
> failure modes that are hard to reason about from a log.

## How it watches while another tool is running

**Watching needs no Arduino at all.** Detection is pure screen reading — a capture, then OCR or a
pixel diff. The port is only needed to *click*. So the watcher polls continuously no matter what else
is running; only the **action** is gated on a tool not running.

That makes the gate far less limiting than it first looks, because **stopping a tool does not need the
port either** — `StopTool()` cancels a token, it writes nothing to the board. So on a death while the
composer is running, the watcher does not have to queue and wait for the tool to end:

1. **stop the running tool** — no port needed; this is also the right thing on its own, since a
   composer clicking through a death dialog is doing damage;
2. the port is now free;
3. **act** — revive, then optionally leave things stopped, or restart.

That removes most of why acting *alongside* a tool looked hard. What it does not remove is two writers
on one serial line at the same instant, so the rule stays: the watcher takes the port only when nothing
else holds it.

Two things this depends on:

- **A running tool can pollute a watched region.** The composer drives the gem UI, so a watched box
  that overlaps anything it animates would see the tool's own clicks as a state change. The death
  region in particular must be somewhere the tools never touch.
- **The poll must not run on the UI thread.** The status timer is a `DispatcherTimer`, and an OCR read
  is long enough that doing it there would visibly stutter the window. The watcher needs its own
  background loop. (The calibrator's **Check OCR** does block the UI thread today — fine for a button
  press, not for a poll.)

## The two action flows

### Pet feed — boarding (代养), not feeding

The automation is: **the pet's boarding runs out of food, restock the feeder with 2 stacks.**

It is **not** the manual feeding system, and the two do not mix — a boarding food item cannot be used
for 喂养. Conflating them sends the flow to the wrong window, so it is worth stating plainly:

| | 喂养 (manual feeding) | **代养 (boarding) — this feature** |
|---|---|---|
| Opened by | clicking the pet cartoon image (bottom-right) | the **宠物代养** button |
| You do | place 1 item, click 喂食, repeat | stock the feeder, click 开始代养 |
| Then | — | the game **auto-feeds once a minute** |

**The game does the feeding; the tool only restocks.** That is what makes this worth automating at all:
the job is *stocking*, not clicking a dialog over and over.

#### What it eats, and how fast

One feed per minute, fixed, and — unlike manual hunger — **not** paused by town maps, 飞越广场, the
arena, fishing, 摆摊 or selling. So the rate is just the per-feed count × 60:

| Stage | Food (喂养值) | Per feed | Per hour | 600 food (2×300) lasts |
|---|---|---|---|---|
| 1–3 | 一般宠物食物 (5) | 2 | 120 | 5 h 00 m |
| 4–5 | 营养满分宠物食物 (15) | 1 | 60 | 10 h 00 m |
| 6 | 高级宠物食物 (30) | 3 | 180 | **3 h 20 m** |
| 7 | 高级宠物食物 (30) | 4 | 240 | **2 h 30 m** |
| 7G | **cannot be boarded** | — | — | — |

Stage 6 and 7 take the **same food** and differ only in count — x3 and x4, confirmed against the item
pages for 真蔬果男妖精仙子 (stage 6) and the stage-7 spirit, which agree with the news page's table.

**The refill interval is a property of the pet, not the tool.** The same two stacks buy 10 hours on a
stage-4/5 pet and 2.5 on a stage-7, so the poll cadence should be derived from the configured pet
rather than hardcoded.

**Boarding auto-stops** when the pet reaches **+9 (100%+)**, when the **character logs off**, and —
unconfirmed — when the **food runs out**. On a stop, the pet and the leftover food are **mailed back**.

**Restocking a +9 pet is wasted food**, so "is it at +9?" is a real guard rather than a formality.

#### The flow

```
icon lights up  ──►  open the 代养 window  ──►  read it
（cheap diff on        （calibrated button,       （OCR: EXP% + feeder state）
  the cartoon image）     NOT the icon click）          │
                                        feeder empty ──┴── pet at +9
                                              │                    │
                                restock 2 stacks, close      close, do not restock
```

**The icon click is the trap.** Clicking the cartoon image opens the **喂养** window, which is about the
other mechanism and holds none of the boarding food — so the icon is a *signal to act*, not the control
that gets you there. The boarding window is opened by its own button, and that button is the calibrated
point.

This is still the pairing the rest of the watcher uses: **a nearly-free check wakes an expensive one**,
and the expensive one — not the cheap one — is what authorises a click.

**Restocking is two bag slots handed over one at a time** — two right-click transactions, the Sell shape
twice. So there are two designated slots to keep stocked and locked, not one.

**The trigger state persists, which makes this the easy half.** The prompt and the exclamation icon stay
up until the pet is fed — nothing resolves it on its own, because the character stays online — so the
pet detector can poll slowly, and a missed poll costs a minute rather than the event. The cadence worry
attached to the death trigger does not apply here.

**The alternative worth noting:** skip the icon entirely and open the boarding window on a timer.
Simpler — no icon region to calibrate and no diff to tune — but it opens a UI window on a schedule while
you are playing, whether or not anything has happened. Watching the icon means the window is only
opened when the game says something changed.

**One thing that still needs a guard:** the icon stays lit until the state is dealt with, so a state
that never clears would open the window on every poll. A per-trigger cooldown, and treating "opened it,
nothing to do" as handled, is what stops that becoming a loop.

### Feed — the transaction

**Moving the food to the feeder is a right-click transaction**, the same shape as buying and selling:
right-click the item in the backpack, then work the dialog. That matters more than it sounds, because
**the firmware cannot drag**: `C` is a press and release in one command (`Mouse.press` … `delay` …
`Mouse.release`), with no separate button-down/button-up. A drag would need new commands and a reflash
of every board. A right-click transaction needs neither.

So this is close to `ShopTool`'s `SellPass` — right-click a bag slot, run the shared `MaxEnterEnter` —
with two differences to measure rather than assume:

- **the dialog may not have a MAX.** Placing a stack into the feeder is not obviously the same
  quantity handover the shop uses, so the step after the right-click may be a confirm button rather
  than MAX → Enter → Enter. The count of steps needs measuring on a real dialog, not inferring from
  the sell one.
- **two slots, each one transaction**, not a selection of many.

**Where the food is: a designated slot, held there by the backpack's lock button.** The lock is what
makes a fixed slot index trustworthy — without it a compacting bag moves the item and the index points
at something else. Still the sharpest risk in the whole feature, because **the failure is feeding the
wrong item**, and unlike a mis-aimed sale there is no undo. Two things follow:

- the lock is a **prerequisite**, not a nicety, and the watcher should say so if it cannot be assumed;
- the handover dialog names the item, so if a cheap confirmation is ever wanted, that is the natural
  place to check before confirming — OCR on the dialog, not on the bag. (The project deliberately has
  no bag OCR; see [PLAN-BUY-SELL.md](PLAN-BUY-SELL.md) on why the sell selection is yours and visible.)

#### Backing out of a dialog

The dialog's **X is just another calibrated point**, so closing one costs no firmware change — the tool
clicks it like any other. `Esc` would be tidier and the firmware does not have it (`C R D H E T S K F X
W Q Z P U`), but adding it would buy nothing the X does not, and every added command is a reflash of
every board.

### Locating the food: the designated slot, and what search would take

**The default is the designated slot.** It is confirmed doable, needs nothing read, and the lock holds
the item there.

**Search:** the button *dims every non-matching item*, so items do not move and the match is the cell
that stays at full colour.

**The obvious way to find it does not work.** "Find the cell that is not dimmed" assumes full colour
reads as brighter than dimmed — and **the pet food's own colour is close to the dimmed colour**, so a
brightness threshold cannot separate the food from some other item dimmed. Measured by the user; it is
not something the code could have inferred.

**The version that does work costs more than it saves.** Compare *before and after* rather than testing
brightness: capture the bag, run the search, capture again. The non-matching cells change (they dim)
and the match is a cell that **did not change** — relative to each cell's own appearance, so the food's
colour stops mattering. That is the empty-check mechanism again. But **empty cells do not change
either**, so they read as matches too, and excluding them needs a fourth calibrated thing: the
appearance of an empty slot.

So the surviving version is a before/after diff *plus* an empty-slot test, and it still sits on top of
the typing problem below. That is a lot of new failure surface for a feature whose only job is to avoid
maintaining one slot's position — which the bag lock already does.

**And the search box clears its text every time it is opened**, so every feed would have to type the
name. That is where the rest of it gets hard:

- the board sends **HID scancodes, not characters**, so producing 中文 means driving the IME — for 速成,
  a radical prefix and then a **candidate number** that depends on the IME's dictionary and ordering.
  It breaks if the IME is in English mode, if the candidate order differs, or if the game does not route
  IME input to the field at all;
- **unless the item has an ASCII name the box accepts**, in which case `K c` types it a character at a
  time and there is no IME involved — worth checking first, because it would make search a
  straightforward win;
- otherwise it needs **paste** (clipboard set by the launcher, then Ctrl+V). The firmware has no
  modifier combos today, but `X` is Alt+Tab, so they are possible: one new command, one reflash. The
  unknown is whether the game accepts a paste.

**Decision:** build the designated slot. Search is not the cheaper path it first looked like — it needs
either an ASCII name the box accepts or a firmware paste command to get the name in, and then a
before/after diff plus an empty-slot test to find the result, against a colour that does not read as a
simple brightness difference. Revisit only if the name turns out to be typeable in ASCII, since that
removes the hardest part.

### Revive — undefined

**Not yet designed.** What reviving consists of decides whether this is a click or a sequence:
a button in a death dialog is one calibrated point; anything that involves walking, a town, or a
choice of revival type is a scripted flow with its own failure modes. Needs answering before phase 2.

## Why this is mostly assembly, not new machinery

| Need | Already exists |
|---|---|
| Read a region as text | `OcrEngine.Scan(OcrGeometry)` — the calibrator's **Check OCR** already calls it with arbitrary geometry, not just the tuner's |
| Compare a region against a reference | The empty check: a saved crop, the fraction of differing pixels, and a **6px inset** because a one-pixel window shift once made an *empty* box score 7.7 % |
| Drag a region to calibrate it | The Buy / Sell and Gem calibrators' canvas drag, and their `SizeChanged` overlay redraw |
| Click a calibrated point | `HidPointer.To` + `Click` — the shared closed-loop placement both other tools use |
| Right-click a bag slot, work a dialog | `ShopTool.SellPass` + `MaxEnterEnter` — the pet-feed action is this shape, one slot instead of a selection |
| Tell you | The card status text plus `Beep()`. No new dependency, no tray icon, no toast |
| Take and release the port | `ArduinoPortAsync` / `Dispose` on `LauncherService` |

The calibration for a trigger is the same shape as everything else in this app: capture, drag a box,
save a reference crop, mark the action point, and a **Test** button that shows the verdict without
clicking.

## Risks, in the order they matter

1. **A false positive here is unrecoverable and unattended.** This is worse than the Sell tool: Sell
   runs once, while you are watching it. A watcher misreads a busy screen and clicks while you are in
   another room. Detection quality is the whole feature; everything else is plumbing.
2. ~~**A revive costs something.**~~ **Downgraded** — the user's revive cost is close to nothing, so
   spending a resource is not the objection. What is left is the part that is not a resource: acting
   on a *wrong* read, or repeatedly, which is risk 1 wearing a different hat.
3. **The capture reads the screen**, so the watch is blind whenever anything covers the game — the
   launcher included. A watcher that silently stops seeing is worse than one that says so.
4. **OCR cadence.** The engine is cached but a read is not free. Death wants a short interval and OCR is
   the wrong thing to run every second — which argues for the diff-then-confirm pairing below.
5. `OcrEngine` lives in `SealTools.Tuner` and its `ScanResult` is tuner-shaped (`Grade`, `Attributes`,
   `Remaining`). A watcher wants the raw text lines of a region. That is a small addition, and probably
   a move of the engine to `Core` so the launcher is not reaching into the tuner.

### The pairing worth considering for death

Diff first, OCR to confirm. The diff runs every few seconds and is nearly free; only when a region
*changes* does the OCR wake up and read it. That keeps the cheap check at a high frequency and the
expensive one rare — and it is what stops a single bad read from becoming a click. (This is the "both"
option; it is more moving parts than either alone.)

## Open questions

1. ~~**What does the pet action actually do?**~~ **Answered:** boarding, not manual feeding — stock the
   代养 feeder with 2 stacks and let the game auto-feed. What is still unknown: whether the feeder is
   **two positions or one filled twice**, whether the item dialog **has a MAX**, and whether boarding
   **stops or merely idles when the food runs out** (the flow above assumes a stop, which is what makes
   the +9 branch worth guarding).
2. **How long does the death state last?** If the game revives you automatically after some seconds,
   the window to act is short and the poll interval decides whether it is catchable at all.
3. **What is the death text**, and how stable is the region it appears in? A phrase to match is easy; a
   region that is also quiet when nothing is happening is the part that needs measuring.
4. ~~**Does the pet trigger act, or only notify?**~~ **Answered:** it acts, but only after the dialog
   confirms the state.
5. **Does opening the boarding window interrupt play?** If it takes focus or pauses something, an
   icon-triggered open is much less intrusive than a timer — which is an argument for watching the icon
   rather than polling the window.
6. **What happens on repeated failure** — retry forever, or give up and say so? Unattended retrying is
   how a misread becomes a loop of clicks.

## Phasing

**Agreed (2026-09-15): the first phase is notify-only, because the checking is the hard part.** The
detectors get wired to a beep and a status line and nothing else, so they can be watched getting it
wrong for a day and the false-positive rate measured — rather than trusted.

1. **Notify-only watcher**, both detectors, no port and no clicks. A false positive costs a beep. It
   also settles questions 2, 3 and 4 by observation instead of guesswork, which is the point: none of
   them can be answered from a desk.
2. **Acting**, guarded — only when no tool runs, or after stopping the one that does, with a
   per-trigger cooldown and a give-up after N attempts.
3. **The pet action sequence**, once question 1 has an answer.
