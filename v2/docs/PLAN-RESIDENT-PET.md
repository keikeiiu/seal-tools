# Plan — running tools alongside the pet feeder

Status: **ALL THREE PARTS BUILT AND RELEASED in v2.11 (2026-09-21).** This document is kept for the
reasoning, not as a plan — what is left is verification, and the open items are at the foot of it.
Three parts, in the order they should be built. Part 1 is a live bug and the smallest. Part 2 needs a
reflash, which also delivers one of Part 1's cases. Part 3 is the feature the player actually asked
for, and depends on neither.

Part 2 is the one that **cannot be verified by building it**: a board without `V` cannot report
anything, so the change is only observable after a flash. Until then the launcher's half is safe
against an old board — an unrecognised letter is ignored by the sketch, which is exactly the silence
the launcher reads as "predates version reporting".

The origin, 2026-09-20:

> while pet tool running we could very well need to buy sell and spam, and some gem compose

---

# Part 1 — the Hold Space toggle inverts instead of stopping

**Built 2026-09-20.** Built as written below, with one correction the code forced (the fourth note
under *The changes*) and one addition (a fourth change, the toggle's own label).

## What the player reports

**On the other PC, the Hold Space toggle button will not stop it.** So the spacebar stays held and the
game keeps auto-picking.

## What is actually happening

Not a refusal — an **inversion**. The toggle (`MainWindow.xaml.cs:292-301`) is:

```csharp
if (_service.CurrentId == "holdspace" && _service.CurrentState?.Running == true)
{
    _service.ReleaseSpace();
    _ = _service.StopTool();
}
else if (!await _service.StartToolAsync("holdspace")) …      // ← restarts it, sending P again
```

The guard asks **two** questions and only needs one. Miss either and the press falls to the `else`,
which **starts hold space again** — so pressing Stop on a held spacebar re-holds it.

**And it is not the older version.** v2.9.1's copy is character-for-character this, read from the tag —
so the other PC's older launcher is a red herring for this bug.

## When the guard misses

`Running` goes false while space is still held whenever the tool's `finally` ran but its `Release()` did
not land — a port write that threw. The tool sets `Running = false` unconditionally
(`HoldSpace.cs:69-73`), so:

```
space held  +  Running == false  →  the button takes the START branch  →  P again
```

And `LauncherService.ReleaseSpace()` (`LauncherService.cs:248`), the one path that would have released
unconditionally, is **only reachable from that same branch** — so it can never rescue this case.

## The changes

1. **`StopTool` releases space whenever the tool it stops is hold space** (`LauncherService.cs:193-229`).
   Then every stop path releases — the card's Stop, Quit, the app closing — instead of the button being
   the only one that can. **This is the fix that matters.**
2. **The button's guard drops `Running == true`.** `CurrentId == "holdspace"` is the question it means to
   ask: *is hold space the active tool?* Whether its loop has flagged itself running is not the button's
   business, and asking is what causes the inversion.
3. **`ReleaseSpace` stops swallowing its exception** (`:248`). It is `try { … } catch { }` while the
   tool's own `Release()` reports on the card (`HoldSpace.cs:32`). The path that must never fail silently
   is the silent one. **Corrected while building:** that second half is not true — Hold Space has **no
   card** (it is the top-right button, and `Tools` does not contain it), so `state.Message`, which only
   `FormatStatus` renders, has nowhere to be drawn. The tool's release message is written and never
   read; the service's was the *only* one with a surface, and it was the one throwing the report away.
   `LauncherService.LastSpaceReleaseError` is now that surface, rendered on the status line beside the
   toggle.
4. **The toggle's label follows the same question as its guard.** It keyed on `holding` — `CurrentId`
   *and* `Running` — so in exactly the state the bug creates it read **"Hold Space"** while pressing it
   now stops. Text and action key on `CurrentId` alike. The dot stays on `Running`: "holding" is a claim
   about the key being down, and the tool only knows that while its loop says so.

## Behaviour after

| | today | after |
|---|---|---|
| Toggle pressed while space is held, `Running` true | releases ✓ | releases ✓ |
| Toggle pressed while space is held, **`Running` false** | **re-holds it** ✗ | releases ✓ |
| Card's Stop / Quit / app close | the tool's `finally` | the tool's `finally` **and** the service |
| A release that fails | the tool reports, the service doesn't | both report |

## Verification

**Done:** the release-on-stop rule is `Core.HeldKeys.NeedsSpaceRelease` — LauncherService is not
reachable from the test project, so the decision is somewhere that is. Three tests pin it, including
one that lists every tool id `StartToolAsync` accepts and asserts none of them holds a key, so a new
tool cannot start holding one without a test failing. Build clean, 84/84 pass.

**Not done — this part has never run.** `MainWindow` is unreachable from the test project, so the
toggle's guard and the status line are a compile check and nothing more. Live, in order:

1. Start hold space; confirm the spacebar is held.
2. Press the toggle once. It must release, and the button must read **"Stop Space"** throughout.
3. Stop it *without* the toggle — the Quit hotkey, or closing the launcher — and confirm the key comes
   up. This is the case that is new: the tool's `finally` used to be the only path.
4. The inversion itself needs `Running == false` with space still held, which only happens when a
   release write fails. It can be faked by unplugging the board mid-hold; without that, note that the
   guard no longer *can* take the start branch while `CurrentId == "holdspace"`, which is the whole fix.

---

# Part 2 — the firmware reports its version

**Built 2026-09-20.** The sketch compiles for the Pro Micro (`arduino-cli compile --fqbn
arduino:avr:micro`, 11878 bytes / 41 % of flash). Nothing has run on a board.

## Why

The other PC's board is probably running an older sketch, and **there is no way to ask it**. The
host-gone release — `Keyboard.releaseAll()` when `Serial` goes false, `seal_mouse.ino:116-120` — was
added after the first boards were flashed, and it is the only thing that frees the spacebar when the
launcher is **killed** rather than stopped. So a force-close on an old board leaves space held with no
host-side path at all, and neither the player nor this tool can tell whether that is the cause.

## The changes

**Firmware** (`arduino/seal_mouse/seal_mouse.ino`) — the first thing the sketch has ever written *back*:

```
#define FW_VERSION 1        // a PROTOCOL level, not a build counter — see below
…
V            →  Serial.print("V "); Serial.println(FW_VERSION);
```

**Launcher** — after the port opens (`LauncherService.ArduinoPortAsync`), send `V` and read a
line back:

- **a reply arrives** → keep it, show it on the Arduino tab beside the existing Diagnose, and log it on
  the pet run's first line
- **nothing arrives** → *"did not answer — its firmware predates version reporting"*

**That second line is the point.** Today the question has no answer; afterwards, silence is a definite
**no**, and a current board prints its number.

The parsing and the sentences live in `Core.FirmwareVersion`, not in the launcher — `LauncherService`
is unreachable from the test project, and "is this line a version reply, or is it something else that
happened to arrive?" is the decision worth pinning. The launcher half is only the write and the read.

## Deviations and corrections, 2026-09-20

- **The level is 1, not 3.** The plan's `3` was counting back over behaviour changes that predate
  reporting — but no board in the field can report anything, so levels 2 and 3 cannot exist and cannot
  be told apart from 1. A future reader would go looking for two sketches that were never flashed. The
  *meaning* is what the plan was right about: a protocol level, bumped when the board's behaviour
  changes, so the number answers "does this board have the feature I need?" and not "how old is it?".
- **"The board resets when the port opens" is not true of this board**, and the sketch is more
  forgiving than the plan assumed. `Arduino.Open`'s own comment records that the 32U4 does *not* reset
  on DTR, and `setup()` calls `Serial.begin(115200)` *before* its `delay(3000)` — so a `V` written
  during that delay waits in the USB CDC buffer and is read the moment `loop()` starts. It is not
  lost. The retry is kept anyway, because it is cheap and it is the DTR-resetting case that would
  genuinely lose it; what actually matters is the **read window**, which is sized to reach past the
  3 s mark. That is the failure the plan named, and the fix is the timeout rather than the retry.
- **The pet-run log line is wired through the constructor** (`PetTool(..., firmware:)`) rather than
  read from the service, because a tool has no reference to it. It lands on the existing `run started`
  line in `pet.log`.

## What it buys

- The sticky-spacebar question stops being a guess, for this PC and the other one.
- Every future firmware feature — the pet tool's timings, any new command — stops being unverifiable.
  Today a board is a black box.

## The catch

**It needs a reflash to take effect**, and a board without `V` cannot report anything, including its own
age. So it does not retroactively answer anything — it makes the *next* flash verifiable. Reflashing the
other PC's board with the current sketch also gives it the host-gone release, which covers Part 1's
killed-launcher case.

## Verification

**Done:** 10 tests on the parse and the sentences, mutation-checked (loosening the two-token rule makes
`"V 1 extra"` parse and the suite fails). The sketch compiles for the Pro Micro. Release build clean.

**Not done — this needs a board, and it needs the port.** In order:

1. Flash this PC's board (this needs the launcher stopped — it holds the port open for its lifetime).
2. Open the Arduino tab. It must read *"Firmware: protocol level 1 (current)"*. If it says **not asked
   yet**, the port has not been opened: press a tool's Start or a test button and Refresh.
3. **The silence case is the one worth forcing**, and it is the harder one to test because it needs a
   board that predates `V`. If no old board is to hand, it can be simulated by flashing a sketch whose
   `V` branch is removed — the launcher must say *"did not answer — its firmware predates version
   reporting"* and must NOT say "not asked yet", which is the state it confuses with a silent board if
   the query never runs.
4. Then the other PC's board, and confirm the same — and if the launcher is killed while holding space,
   the key must be released (the host-gone release, which the reflash also delivers).

**Open question the flash will answer:** how much the cold start slows down. The query adds up to two
1.5 s read windows on the *first* port open of a launcher session, and nothing thereafter — but that is
arithmetic, not a measurement. If 3 s on a cold start is objectionable, the fix is to send `V` and read
it without holding the tool start behind it.

---

# Part 3 — the pet feeder becomes resident

**RELEASED in v2.11, 2026-09-21.** Built on `v2-resident-pet`, merged into `v2-pet-drag`, and shipped.
The pet feeder has run resident alongside nothing else yet — see the open items below — so the parts
that ARE verified live are the ones the released run exercised.

**Verified live:** a run reading its counts and scheduling from them (`381 items -> 132 min`), and the
feeder surviving a Start of another tool has NOT been exercised — that is the first thing to try.
**Verified by test:** `Core.PortGate`, 9 tests, mutation-checked.

## Context

Today it is impossible: `StartToolCoreAsync` opens with `StopTool(fromStart: true)`
(`LauncherService.cs:139`), so pressing Start on any tool **kills the pet feeder**. Everything else
assumes one tool — `_cts` / `_toolTask` / `_state` / `_currentId` are single slots, `ToolState` documents
itself as "single WRITER, single READER", and the mini-mode comment says *"Only one tool can run at a
time, so nothing is hidden that could be used anyway."*

**Why this is not a cursor lease.** That design was for the **tuner**, and it died on a fact from the
game: the boarding window and the tuner's window cannot both be open, and the tuner's must be closed
first. The tuner also runs ~a day — longer than a feeder load — and the player has decided against
automating its setup. **The tuner stays a manual schedule; out of scope.** See
[PLAN-WATCHER.md](PLAN-WATCHER.md) for the recorded reasoning.

Every other tool is different in one decisive way: **the pet feeder holds nothing between reloads.** It
needs the game ~30 seconds, five times a day. So no lease, no yield boundaries, no cursor arbitration —
only a guarantee that **one tool writes at a time**, with the pet feeder as the one that waits.

## The behaviour after

| | today | after |
|---|---|---|
| Start a tool while the pet feeder runs | **the pet feeder is killed** | the tool runs; the pet keeps its schedule |
| Stop that tool | — | it stops; the pet feeder is untouched |
| A pet reload comes due while a tool runs | n/a | **the pet feeder waits**, and says so on its card |
| Two non-pet tools | the first is killed | the second replaces the first — unchanged |
| The feeder runs dry with a tool still running | — | **it says so and keeps waiting** (player's call) |
| The Quit hotkey | stops the running tool | stops the *foreground* tool only — **not** the pet feeder |

The pet feeder **never opens the boarding window while another tool runs**, so the window problem found
for the tuner is never reached.

## The changes

**1. `LauncherService` — two slots, one gate.** The four single fields (`:24-27`) become a per-tool
record keyed by id, with the rule *at most one non-pet tool*. `StartToolAsync` stops only its own slot;
`StopTool(id)` becomes slot-aware (today it nulls all four, `:209-212`); `ResidentId`/`ResidentState`
join `CurrentId`/`CurrentState`; and `_startInProgress` (`:124-125`), which today returns *success
having started nothing*, becomes per-slot.

The gate — `TryAcquirePort(owner)` / `ReleasePort(owner)` / `PortOwner` — closes the race between "is
anything running?" and acting on it. The launcher claims it for a non-pet tool; the pet feeder claims it
for `InspectRows` and each `ReloadRow`. **A Start pressed while the pet holds it waits**, bounded by a
reload's ~30 s, with the card saying so — never a silent no-op. It belongs in **`Core`**, because
`LauncherService` is not reachable from the test project.

**Built:** `Core.PortGate` — `TryAcquire(id)` / `Release(id)` / `Owner`. Two decisions that are the
class's whole content:

- **A claim is refused, never queued or reference-counted**, including from the owner asking twice. A
  refused claim leaves nothing to undo; a tool that claimed twice and released once would leave the
  gate free while it still ran, which is the failure this exists to prevent.
- **A release from anyone but the owner is ignored.** A stale tool finishing late must not free the
  gate its competitor is waiting on — that is exactly how two tools end up writing to the port at once.
  The return value says whether it actually released, so a caller cannot report a release that did not
  happen.

The gate is deliberately **not a lock around each serial write**, and the waiting is the caller's
(poll `TryAcquire`, `Task.Delay`, bounded) rather than the gate's. Two tools *interleaving* on the
cursor is still wrong; the gate only makes them take turns, which is why the pet feeder defers for a
whole reload instead of sharing one.

**Deviations from the plan above, all decided while building it:**

- **`ResidentId`/`ResidentState` did not get added.** `StateFor(id)` already answers both, and the card
  loop wants the per-id form — `ResidentState` had no caller, so it would have been dead API. The
  public shape is `StateFor(id)` plus the `ResidentId` constant. `IsRunning(id)` was likewise written
  and then deleted for the same reason: the pet asks the **gate**, not `_running`, because `_running`
  is only safe on the dispatcher thread and the pet runs on its own.
- **`_startInProgress` stayed global, not per-slot.** One port, one boot delay, and two concurrent
  starts must still not both open it — so a single flag is the correct shape, not a limitation. The
  plan's per-slot version would have allowed exactly the double-open the flag exists to prevent. Its
  wart is unchanged: a Start during another start is still a silent no-op.
- **The gate is released in the stop *continuation*, not at `Cancel()`.** Releasing at the moment a
  stop is requested would let the pet start clicking while the dying tool is still sending its last
  command. The cost is real and recorded in the code: a loop that never exits keeps the game, and the
  pet says `waiting for <tool>` on its card until it goes.
- **`_currentId` is never the resident.** The pet is background furniture, not a run the player is
  watching, and mini mode keys off `CurrentId` — collapsing the window onto a schedule that runs for
  days would hide everything else for the length of it. A pet running alone therefore leaves the window
  full size, which is the one behaviour change nothing in the table above predicted.
- **The keeper is a predicate, not a name.** `StopAll(keep:)`, not `StopAll(keeper:)`, because starting
  the pet must stop a *previous pet* while keeping every other tool — the two cases need opposite
  rules, and a single "keeper id" cannot express both. Leaving it as a name reintroduced the
  orphaned-loop bug for the pet card's own Start.

Serial writes stay direct and unlocked: the gate is the guarantee, and a write lock would imply a safety
this design does not provide — two tools *taking turns* on the cursor is still wrong, which is why the
pet feeder defers rather than interleaves.

**2. `PetTool` — defer, in two places.** `InspectRows` and `ReloadRow` both claim the game; a run started
while another tool is active **starts and waits to look**. The reload's edit point is exact —
`PetTool.cs:181`, between `SleepUntil(next[row], ct)` and the `ReloadRow` after it. Deferring leaves
`next[row]` in the past, so the next pass re-picks the same row with no bookkeeping. The card carries the
state — *"Row 2 is due — waiting for gem composer (4 min)"* — because silence would look like a stall.
**Never interrupt** (player's call): past the slack it says so, and the moment is exactly computable with
no new state — a row is scheduled at `next[row]` = fill + `LoadMinutes + WaitAfterEmpty`, so the feeder
goes dry at **`next[row] − WaitAfterEmptyMinutes`**. Slack: a free row holds 3 h 20 m, a paid row 8 h 20 m.

**3. `PetTool` must ignore the Quit hotkey.** `ToolBase.SleepCheck` sets `QuitPressed` from **global OS
key state** (`ToolBase.cs:20-30`), so with two loops alive one Quit press stops **both** — quitting a buy
run would kill the resident feeder. A flag on `ToolBase` keeps the decision visible.

**4. The UI.** The cards are already id-keyed (`_toolCards`/`_statusBlocks`, `:63-64`), so per-tool
rendering extends naturally. The work is `RefreshStatus` (`:504-543`), whose `else` at `:532-533`
overwrites every non-current card with `"stopped"` on every tick — **a running pet feeder would read
"stopped" while feeding pets**; mini mode (`:508-514`, `_miniToolId` `:599`, `ApplyWindowLayout`
`:690-724`), which collapses the second running tool; and the Hold Space status (`:538-542`) and toggle
(`:294-301`), which key on the single `CurrentId`.

**5. Docs and comments asserting the invariant** — done: `MainWindow.xaml.cs:62` and the mini-mode
comment, `LauncherService`'s class summary, `ToolState.cs`'s ownership model, `README.md:41`,
`USER_GUIDE.md:11` and `:69`, `PLAN-WATCHER.md:18`. `PLAN-WATCHER` keeps its reasoning and gains the
exception, because the argument it makes — *a tool holds the port for its whole run, so the watcher must
not* — is unaffected by the pet feeder waiting instead of being killed.

## Verification

1. **Tests in `Core`** — **done.** The gate: claim and release, a second claim refused, the same owner
   asking twice refused, a release by the wrong owner ignored *and the gate still held*, `Owner`
   readable while held, releasing a free gate is not a release, an owner is required, and 30 callers
   racing for it produce exactly one winner. 9 tests, mutation-checked — dropping the ownership test
   from `Release` fails two of them. **Not verifiable by test:** the atomicity itself. The racing test
   is the reason the check and the claim share one lock, but a check-then-act mutation is not reliably
   caught by 30 threads, so that test is evidence rather than proof.
2. **Live, pet feeder alone** — the case that must not regress. A reload must look exactly as today.
3. **Live, with a second tool:** start the pet feeder, then buy/sell; the pet card must still show its
   next reload, and the feeder must still be alive afterwards.
4. **Live, the deferral:** with a tool running, let a pet row come due; the card says it is waiting,
   nothing is clicked while the other tool runs, and the reload happens once it stops.
5. **Live, Quit:** with the feeder resident and a buy run going, Quit must stop the buy run and **not**
   the feeder.

**Not covered by any of the above, and worth knowing before trusting it:**

- **Starting the pet while a tool runs** (the reverse direction): the pet must join without stopping
  that tool, and its first "look" must wait rather than read a screen the other tool is covering.
- **Starting the pet twice:** the second Start must replace the first, not leave two loops. This is the
  case that produced the keeper bug, and it is exactly what a test cannot reach.
- **Stopping the pet mid-reload:** its own claim must be handed back by its `finally`, not by the
  launcher, or the gate leaks and nothing can start until the launcher restarts.

