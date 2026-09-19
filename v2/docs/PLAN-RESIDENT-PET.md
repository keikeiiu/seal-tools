# Plan — running tools alongside the pet feeder

Status: **Part 1 built 2026-09-20 (compile + tests only, not live-verified); Parts 2 and 3 planned.**
Three parts, in the order they should be built. Part 1 is a live bug and the smallest. Part 2 needs a
reflash, which also delivers one of Part 1's cases. Part 3 is the feature the player actually asked
for, and depends on neither.

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

## Why

The other PC's board is probably running an older sketch, and **there is no way to ask it**. The
host-gone release — `Keyboard.releaseAll()` when `Serial` goes false, `seal_mouse.ino:116-120` — was
added after the first boards were flashed, and it is the only thing that frees the spacebar when the
launcher is **killed** rather than stopped. So a force-close on an old board leaves space held with no
host-side path at all, and neither the player nor this tool can tell whether that is the cause.

## The changes

**Firmware** (`arduino/seal_mouse/seal_mouse.ino`) — the first thing the sketch has ever written *back*:

```
#define FW_VERSION 3        // a PROTOCOL level, not a build counter — see below
…
V            →  Serial.println("V 3");
```

**Launcher** — after the port opens (`LauncherService.ArduinoPortAsync`, `:77-105`), send `V` and read a
line back:

- **a reply arrives** → keep it, show it on the Arduino tab beside the existing Diagnose, and log it as
  the pet run's first line
- **nothing arrives** → *"the board did not answer — its firmware predates version reporting"*

**That second line is the point.** Today the question has no answer; afterwards, silence is a definite
**no**, and a current board prints its number.

## Two details that will bite otherwise

- **The board resets when the port opens.** `setup()` waits `delay(3000)` (`seal_mouse.ino:106`) and the
  launcher only waits 2 s (`LauncherService.cs:103`), so the query can arrive before the board is
  listening. Send, wait, **retry once** — otherwise a silence that is only timing reads as "old board",
  which is the exact wrong conclusion from the wrong evidence.
- **The number must mean something.** A build counter tells you nothing. `3` meaning *"has the host-gone
  release and the `V` command"* — incremented when behaviour changes — is what makes the answer
  actionable. A date stamp works too, if it is compared against something.

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

Flash this PC's board, open the Arduino tab, and confirm it prints a version. Then flash the other PC's
and confirm the same — and if the launcher is killed while holding space, the key must be released.

---

# Part 3 — the pet feeder becomes resident

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

**5. Docs and comments asserting the invariant** — `MainWindow.xaml.cs:62`, `:508-511`, `:692-694`;
`LauncherService.cs:17`, `:107`, `:118-123`, `:190-192`; `ToolState.cs:10-13`; `README.md:41`;
`USER_GUIDE.md:11, :69`; `PLAN-WATCHER.md:18`.

## Verification

1. **Tests in `Core`:** the gate — claim/release, a second claim refused, a release by the wrong owner
   ignored, `PortOwner` readable while held.
2. **Live, pet feeder alone** — the case that must not regress. A reload must look exactly as today.
3. **Live, with a second tool:** start the pet feeder, then buy/sell; the pet card must still show its
   next reload, and the feeder must still be alive afterwards.
4. **Live, the deferral:** with a tool running, let a pet row come due; the card says it is waiting,
   nothing is clicked while the other tool runs, and the reload happens once it stops.
5. **Live, Quit:** with the feeder resident and a buy run going, Quit must stop the buy run and **not**
   the feeder.
