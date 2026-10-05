# Plan — who gets the game, and when

Written 2026-10-06. **NOT BUILT — plan only.**

The ask was *"implement both scheduling like cron and also the MQ to queue when jobs are arriving at the
same time → it should solve any job execution race issue"*.

**It would not solve the race, and the first section is why.** The race here is not jobs arriving at the
same moment — two simultaneous arrivals are already serialised correctly by `PortGate`, which is what it
exists for. The race is that **a job with a deadline can starve behind a job that never ends**, and the
pet feeder nearly running dry behind a spammer is that failure, observed. A queue does not fix it: FIFO
makes it *worse*, since a deadline job then sits behind whatever arrived first.

What does fix it is small, and it is most of §4: **the reason you want the port is a deadline, and the
earliest deadline wins.** No dispatcher, no queue, no restructuring of Start/Stop.

---

## 1. What the two proposed pieces each answer

| Proposed | Answers | Does not answer |
|---|---|---|
| **Cron** | *when* does this job run? | anything about contention — it only decides a start time |
| **A queue (MQ)** | *in what order* are arrivals served? | **starvation**, unless the queue is a *priority* queue with a reason on each job |

Both are fine ideas that would make the program nicer. Neither is the fix for the failure that exists,
and §6 keeps cron as the separate, optional half it is.

**The scarce resource is one Arduino and one cursor.** Exactly one job may drive it at a time — that is
not a throughput problem to be scheduled away, it is a physical fact.

---

## 2. What exists today, so this does not invent a parallel mechanism

**The gate already arbitrates.** `PortGate` is a single-owner claim: `TryAcquire` / `Release` / `Owner`.
Two jobs wanting the port *at once* are already ordered correctly by it, and the tests pin that.

**The waiting signal already exists.** Added 2026-10-02 (PROGRESS entry 46): `AnnounceWaiting(owner)` /
`WithdrawWaiting(owner)` / `Waiting`. A claim can be *refused* but never interrupted, so a waiter was
invisible until it could say so — and the pet feeder, whose reload is ~30 s, was waiting out entire
spammer runs.

**The yielding machinery already exists**, and it is the hard part:

- the **spammer** releases the game when someone is waiting, presses nothing, re-acquires when it comes
  free, and re-opens through its own rules ([SkillSpammer.cs](../SealTools.Spammer/SkillSpammer.cs),
  the `yielded` block);
- the **pet** announces itself *before* polling and withdraws when it is done;
- **completion chaining already exists** as `Task`: `StopTool` returns a `ContinueWith` continuation, and
  `StartToolCoreAsync` awaits one `Task` per stopped tool before claiming the port itself.

So "when action A completes, do B" needs no new machinery — `await` is the mechanism and it is already
the design.

**And the rule is already implemented, just not named.** The spammer yields when *anyone* is waiting, and
the only waiter today is the pet — which is the only participant with a deadline. So "anything without a
deadline yields to anything with one" is what the code *does*; it simply looks like a special case
because there has only ever been one case.

---

## 3. Where it stops working

**Every yielding rule is a hand-written pair.** Today: the pet defers to whatever holds the game; the
spammer yields to the pet. That is correct for two participants and it is *only* correct for two,
because nothing states the general rule and nothing catches the pair somebody forgets.

**And a third resident feature is already designed.** [IDEAS.md](IDEAS.md) §1 sketches a **passive
watcher** — death and pet notifications — explicitly *"a launcher-level service rather than a tool card,
because it must poll without holding the Arduino and only take the port in order to act."* It is a
second resident thing that needs the port occasionally. The gate has no way to arbitrate between two of
them, because it has no notion of **why** anyone wants the port.

That is the gap. It is a *policy* gap, not a plumbing one.

---

## 4. The design — the gate carries a REASON

Three changes, all inside `PortGate` and the tools that already yield. **Nothing is restructured.**

**§4.1 A claim carries a deadline, or none.**

The pet's deadline is real, computable, and already computed: `WaitingForRow` prints *"N min of food
left"* from `next[soonest]` minus `WaitAfterEmptyMinutes`. That same arithmetic, surfaced as a
`DateTime?`, is the deadline. A tool that simply wants the port has **no** deadline — nothing bad happens
if the tuner starts a minute later.

**§4.2 The gate answers one question.**

`ShouldStandDown(owner)` → does some waiter have a *sooner* deadline than mine? A holder with no deadline
always yields to a waiter that has one. That single comparison replaces every pair-wise rule.

**§4.3 The holder asks that instead of comparing strings.**

The spammer's `yielded` machinery is unchanged — release, pause, re-acquire, `Reset(payLeadIn: false)`.
Only the *condition* changes, from `Waiting != null` to `ShouldStandDown(GateOwner)`. `AnnounceWaiting`
gains a deadline argument; everything else is additive.

**The yield is the hard part and it is already built.** What changes is *when* the decision is made.

---

## 5. What is deliberately NOT here

- **No dispatcher.** A single component that owns a queue and starts the next job would centralise
  arbitration, and it would touch `StartToolAsync` / `StopAll` — the path the residency work already
  rewrote once. The rule above needs none of it.
- **No broker.** One process, one port, one user. An external MQ would add a service to install and
  supervise and break the self-contained-exe property `publish.bat` exists to produce.
- **No in-process job queue.** A queue without a priority is FIFO, which is the discipline that starves
  the deadline job. Adding a *priority* queue is a dispatcher by another name.

---

## 6. The cron half — separate, optional, genuinely small

A trigger table (time → tool id) and a timer that calls the **existing** `StartToolAsync`. It shares
nothing with §4 and can be built before, after, or never.

Two things it must respect rather than rediscover: starting a tool stops whatever else is running
(`StopAll`), and a trigger firing while the pet holds the game waits up to `GameWaitMs`. A schedule that
silently does nothing because something else was running is exactly the "looks wired up and does
nothing" failure this repo has paid for twice.

---

## 7. What this does not fix, stated plainly

- **A holder that cannot pause cannot yield, and almost none of them can.** "Pause" in the tuner and the
  composer is a **graceful STOP** — `PauseRequested` breaks out of the loop and the run is over
  ([SealTuner.cs:252](../SealTools.Tuner/SealTuner.cs#L252), [GemComposer.cs:469](../SealTools.GemComposer/GemComposer.cs#L469)) —
  and the tuner's mouse guard ends it outright. A yield would therefore *end* their run rather than
  suspend it, which is not a trade the gate may make on a tool's behalf. They hold the port and the pet
  waits, exactly as today.

  **So the rule governs only tools that can be interrupted AND resumed, and today that is the spammer
  alone** — its `yielded` state is the only resume in the suite. The gate can ask; it cannot force. That
  is a smaller win than it first sounds, and it is the honest size of it.
- **It is a policy, not a proof.** With two participants the outcome is checkable by hand. With three or
  four, "earliest deadline wins" is a choice somebody owns, and whether it behaves as expected is what
  an experiment is for.
- **It does not touch the transport.** The Arduino link still has no flow control — a 64-byte RX ring and
  a blocking `delay()` per command. A *queue* would make that worse, which is a second reason not to
  build one.

---

## 8. Decide first

- **The tie-break.** Two waiters with the same deadline — first announced, or a stated priority? Only
  matters at three participants, so it can be deferred with the rule written down.
- **Does a deadline expire?** If the pet's deadline has passed, does it outrank everything absolutely, or
  is there a point where the run in progress should be allowed to finish? Today's behaviour is "it
  waits", which is a decision already made and documented.
- **Whether to name the rule without building it.** §3's gap is real but has no victim yet — the watcher
  does not exist. A paragraph in IDEAS may be the honest first step, and this plan the second.

---

## 9. Order of work

1. **§4.1 + §4.2** — the deadline on a claim, and `ShouldStandDown`. `PortGate` only, with tests; no tool
   changes, so nothing can regress.
2. **§4.3** — the spammer asks the new question. One condition changes.
3. **Live check** — the pet feeder against a running spammer: the pause should last exactly as long as
   the reload, and the spammer should resume on its own. **This is the step that decides whether the
   policy is right**, and it is the one that cannot be reasoned.
4. **§6 cron**, if it is still wanted once the above behaves.
