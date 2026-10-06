# Plan — the input arbiter

Written 2026-10-06. **NOT BUILT — plan only.**

The design below is the **player's**, given 2026-10-06. It came out of the same external read-only
audit of `cc6551b` as [PLAN-OWNERSHIP.md](PLAN-OWNERSHIP.md) — it was that audit's answer to *"any other
integration like MQ / database / other IT methods to make the program more solid?"* — and was then
developed with the player into the flow recorded here.

**Where it sits, so it is not read as a third competing design.**

[PLAN-SCHEDULING.md](PLAN-SCHEDULING.md) §4 designs the **policy**: the gate carries a *reason*, and
`ShouldStandDown(owner)` answers *who yields*. This document designs the **unit** — what an ownership
claim covers, and the one component that grants it. They are not alternatives. The audit's own flow
already assumes §4: the pet feeder's box says *"has a deadline"*, and the arbiter's reply to the spammer
is *"a deadline-holder is waiting"*. **§4 is what the arbiter decides with.**

**What already exists, so nothing here is invented.** `PortGate` (`SealTools.Core/PortGate.cs`) is a
single-owner named claim with a waiting signal — `TryAcquire` / `Release` / `Owner` /
`AnnounceWaiting` / `WithdrawWaiting` — with 28 tests in `PortGateTests.cs`. It is injected into exactly
two tools, the pet feeder and the spammer (`LauncherService.cs:138`, `:142`), and constructed once at
`LauncherService.cs:174`. The audit says of it: *"the existing `PortGate` is a starting point, but every
input path, including calibration and Stop, would need to use the arbiter."* That is the whole job.

---

## 1. The flow

```
   Skill Spammer            Pet Feeder              Gem Composer
   one combo at a time      one reload visit        one full run
   can yield                has a deadline          cannot resume
          \                       |                       /
           \--- request exclusive input ownership ------/
                           |
                  +------------------+
                  |  InputArbiter    |
                  |  - one owner     |
                  |  - waiters       |
                  |  - one Arduino   |
                  +------------------+
                           |
                     Arduino -> game
```

**What it is.** One C# component inside the launcher that decides who may control the Arduino and the
game cursor. It needs no MQ, no database, and no separate service or process.

**"Operation-level"** means ownership lasts for a **complete action**, not one serial write. The pet
feeder owns the input through `press_mouse_button → move_food → release → confirm → check_the_result`.
A calibration test or another tool waits until that action reaches a **safe boundary**. The skill spammer
can yield **between key presses**.

**The key rule:** never let another caller insert a command halfway through a gesture.

---

## 2. The two worked examples

**Example A — Spammer + Pet.**

```
Spammer gets ownership  ->  presses one complete combo
Pet reload becomes due  ->  asks arbiter for ownership
Arbiter tells Spammer a deadline-holder is waiting
Spammer finishes combo  ->  releases
Pet gets ownership      ->  reads, reloads, verifies  ->  releases
Spammer gets ownership again  ->  continues
```

**Example B — Gem + Pet.**

```
Gem gets ownership      ->  combines gems
Pet becomes due         ->  asks arbiter; must wait (gem cannot resume)
You move the mouse, with Gem's policy set to "stop"
Gem cancels further input  ->  releases ownership; result may be unknown
Pet gets ownership      ->  re-reads game state  ->  reloads
```

Example B is the reason "one full run, cannot resume" is written on the Gem Composer's box. A tool that
cannot resume must never be interrupted mid-run — it waits its turn instead. That distinction belongs to
the tool, not the arbiter: the arbiter is told the tool's resume behaviour, it does not infer it.

---

## 3. What it is NOT

Stated because each of these was proposed at some point and is deliberately declined:

- **Not a separate process, service, or project.** It is a component inside the existing app. A separate
  project is only worth it if the component grows substantially or needs to be reused elsewhere.
- **Not MQ, not a database.** RabbitMQ or Kafka would add failure modes to a single-PC, single-cursor
  system, and delayed clicks are the worst of them. (Same conclusion as
  [PLAN-SCHEDULING.md](PLAN-SCHEDULING.md) §5, which rejects a dispatcher, a broker and an in-process
  job queue for the same reason.)
- **Not a launcher.** It does not start tools, keep a cron schedule, or decide *what* to do.
- **Not an override of a tool's own settings.** It does not override a tool's mouse-movement policy; the
  tool still controls its own workflow and still decides what action to perform.

**The boundary, and it holds whichever way §6 is decided: the arbiter governs *access*, never
*behaviour*.** Every tool keeps its existing logic — what it clicks, what it reads, what it decides. The
arbiter knows nothing about pet slots, gem combining, or spammer combos, and must never learn. It knows
only *who asked*, *what they declared as their unit of work*, and *whether they are done*.

The sharpest form of that rule: **the arbiter never cancels a tool.** Example B in §2 is the proof — the
arbiter tells the Gem Composer a deadline-holder is waiting, and the composer cancels *itself* because
its own policy is set to `stop`. Refusing a claim and telling a holder someone is waiting is the
arbiter's whole vocabulary. It never aborts a run on a tool's behalf.

So the division is:

| Decided by the arbiter | Decided by the tool |
|---|---|
| who may send input now | what that input does |
| when a lease starts and ends | how long its own operation is |
| refusing a claim while held | whether to yield, or to cancel |
| telling a holder a waiter exists | its own mouse-movement / stop policy |

---

## 4. The one enforceable rule

> **No tool and no UI button writes directly to the Arduino outside the arbiter.**

That is the property worth having, and it is the one that can be checked mechanically — by grepping that
nothing outside the arbiter reads the port directly. It is also the rule the launcher **already fails**:
`MainWindow` never references `.Gate` (zero occurrences) while 19 call sites in it obtain the port and
send input directly — which is PLAN-OWNERSHIP item 4, and is why item 4 is the first slice of this
rather than a separate guard. A guard bolted on beside `PortGate` would be exactly the *"two competing
ownership rules"* the audit warns against.

**Normal Stop vs emergency stop.** These are different operations and the arbiter must not conflate them:

- a **normal Stop** finishes, or safely aborts, the current gesture, and only then releases;
- a **separate emergency stop** releases held inputs immediately, because a stuck key or button is worse
  than a half-finished gesture.

---

## 5. Where it lives, and how it is wired

- `InputArbiter` in **`SealTools.Core`**, alongside the current `PortGate`.
- `LauncherService` creates **one instance** and passes it to tools and to the calibration controls. It
  is one instance because there is one Arduino — a second instance would be a second owner.
- It **grants exclusive leases for complete operations**; each tool still decides what action to perform.
- `PortGate` is **evolved or replaced**, not kept alongside. Keeping both is the failure mode.

---

## 6. The open decision that sets the size

The audit says two things that are not quite the same, and which one is meant decides whether this is an
M or an L:

| | Reading A — the arbiter owns the port | Reading B — the arbiter owns permission |
|---|---|---|
| Audit prose | *"the arbiter would **own access to the shared serial port** and grant exclusive leases"* | — |
| Audit diagram | — | *"**request exclusive input ownership**"*, "one owner" |
| Tools receive | a lease object instead of a `SerialPort` | the `SerialPort`, as today, for the duration of a lease |
| Cost | all 7 `Run(SerialPort ser, …)` signatures, ~18 `SerialPort` parameters inside `PetTool` alone, every `HidPointer` call site | the arbiter decides *when*; the port stays direct |

**Reading B is recommended**, and it is the one the diagram describes. It also keeps the property
`PortGate` was built on deliberately (`PortGate.cs:17-21`):

> *"Serial writes stay direct and unlocked, and the gate is the guarantee rather than a wrapper around
> each write. … A gate that made interleaving look safe would be worse than no gate."*

Reading A would put a wrapper around every write, which is the opposite of that. The enforceable rule in
§4 survives Reading B unchanged: the only way to *obtain* the port is from the arbiter.

**Neither reading touches tool logic.** The cost column for Reading A is *plumbing* — how a tool receives
the port, not what it does with it. §3's boundary holds either way, and it is the test to apply to any
design here: if a proposal makes the arbiter aware of pet rows, gem counts, or combo timings, the
proposal is wrong, whichever reading it is wearing.

---

## 7. Order of work

Staged so each step is verifiable on its own, and so the first steps cannot regress the tools:

1. **The deadline on a claim, and `ShouldStandDown`** — `PortGate` only, with tests. No tool changes.
   *This is [PLAN-SCHEDULING.md](PLAN-SCHEDULING.md) §9 step 1; it should not be built twice.*
2. **The arbiter's entry point** — port acquisition moves behind the arbiter, and the UI's 19 direct
   call sites route through it. This is PLAN-OWNERSHIP item 4, absorbed.
3. **The unit** — leases become operation-level (a whole gesture), with the normal-stop /
   emergency-stop distinction from §4 above.
4. **Live check** — the pet feeder against a running spammer: the pause lasts as long as the reload, and
   the spammer resumes on its own. Per PLAN-SCHEDULING §9 step 3, this is the step that decides whether
   the policy is right and it is the one that cannot be reasoned.

---

## 8. Verification

- **The policy and the leases are testable on the service** with no board: refuse-while-held,
  deadline-orders-the-waiters, an operation-level lease spanning a whole gesture.
- **The enforceable rule is checked by grep** — no UI call site reads the port directly, which is the
  same check PLAN-OWNERSHIP item 4 was found by.
- **The live check is the only test that proves the behaviour** and it is step 4. Everything before it
  is necessary and none of it is sufficient.

**Risk, stated plainly.** A refusal a player meets has to say what is running and how to stop it, or it
reads as a broken button — the failure this repo has paid for repeatedly. That is why the refusal message
is written before the sites route through it.
