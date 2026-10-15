# v2.15 — the pet feeder's visit flow, rebuilt from its own log

The feeder's whole visit flow is reworked — the plan's four steps — and most of what changed was not in
the plan at all: three weeks of `pet.log` were mined and the fixes follow what the log actually says.
The feeder's card now lists every breeding row, the count read stops failing on knife-edge digits, and
the suite can be pointed at the **US client** from the Setup tab.

**Nothing here needs a reflash.** The board's protocol level is unchanged at **2** — the only
sketch change since v2.14 is an 11-line comment recording how the level is pinned (by
`FirmwareSketchTests`) and tagged (`fw1`, `fw2`), so a board already flashed stays correct. The sketch
still ships with the release, for a new board.

**What was verified, and what was not.** Stated honestly, because a rework of code that runs unattended
for days is exactly where a claim needs to be checkable.

| | |
|---|---|
| **The build and the tests** | `dotnet build v2/SealTools.sln` — 0 warnings, 0 errors. `dotnet test` — **292 tests pass**. |
| **Two adversarial reviews** | 4+2 reviewers over the branch, every finding independently verified: 11 fixes, then 9 more. Two of those found real defects in what the reviews were reviewing. |
| **Live, overnight** | The **veto fired for real** (Row 1, 36 min of food left → the reload was skipped). The **count ladder recovered two slots in one look** (at 9× and 24×). The **deferral kept two rows safe for two hours** while the game was down — no clicks, no strikes. |
| **The newest batch** | The join, the open-deferral, the ladder and the client switch are deployed and lightly exercised (the switch was tested on the English client and reported fine). A watched multi-row visit with a join has **not** been seen yet. |
| **The US dictionary** | Ships **empty**, deliberately, and says so on the card. See §5. |

---

## 1. The visit flow, in the plan's order

**The close guard.** The X is now clicked twice — the measured failure was the cursor failing to
*converge* on it (17 times in the log), and a retry re-aims. When no click can be sent at all, the run
remembers that the window may still be open, and the next open **probes** the X against a reference crop
it learned from a window whose contents it had just read. Only a confident match skips the two blind
clicks; everything else opens exactly as before. Design record: `PLAN-PET-CLOSE-GUARD.md`.

**The fresh read can cancel a reload.** A due row is skipped when the visit's own reading shows food
SEEN — the slot read as occupied, a count read above zero — with more than **30 minutes** to go. The
margin is measured, not chosen: the log's visit-opening readings are 165 at 0 minutes, one at 12, and
thirty at 41-plus; the stretch between is empty, so 30 sits in the middle of it. Every firing *and*
every near-firing is written to the log, so the number can be corrected from the next run's evidence.

**One bag sweep per visit.** A plural batch used to pay one full three-page sweep **per row**. The sweep
is now taken by the visit's first placement and shared — with every stored cell **re-read before it is
clicked** (a cell an earlier row emptied is skipped, and said so), and one fallback sweep for a row whose
own pet came back after the sweep ran. `PLAN-PET-SWEEP.md`.

**The look and the first visit share one window.** A run started because a row was due used to look
(open, read, close), then immediately reopen for the very row it had just read. The look's own reading
now reloads the due rows inside the window it already has open.

## 2. What the log found that the plan did not

**The feeder's biggest row-killer was a covered window.** 77 of the 131 in-visit failures in the log had
**no reason written anywhere** — and all of them came through one branch: after pressing the end toggle,
the slot read said the pet had not come back out. The slot read was the **only screen read in the tool
without the foreground gate** its two siblings have — so a covered game answered "a pet is in the
loader" from whatever was covering it, at exactly 1.000 (194 such reads), and the reload then pressed
the end toggle at that window. That path is **57 of the 90 strikes** that dropped a row across three
weeks. Fixed: the gate is on every screen read; the run waits for the game at startup; a visit whose
window is not in front defers five minutes without a click or a strike; and **every failure now writes
its reason** — a row's give-up carries the last one with it.

**Two more incidents, both seen live.** A visit whose own read found a **second row due now** could not
act on it — the batch is chosen before the window opens — so the second row got its own open/close 40
seconds later; the reading can now **join** the visit. And a boarding window that never came up (the
player had the game's own menu open, and 目錄 is a toggle) used to carry on into the reads, which
described the game **world** and failed two rows; it now **defers**, and clears the X reference after
two such opens in a row so a stale crop can never gate every open.

## 3. The card, and the numbers on it

Each breeding row now has its own line: the pet it boarded — species from the queue entry's preset and
the hover panel's `stage 6, +7, 52.18%` — the **`→ +N`** the game's time line counts to, the row's own
**`N/min (game line)`** feeding rate, and its own countdown, recomputed as the card refreshes.

Three numbers sit close together and are now told apart: `(6階)` is the **stage**; `+7` and `到9為止`
are the **餵養值/growth** (+0…+9, where +9 at 100 % is a finished pet); `52.18%` is that bar's fill.

**The rate is read per row**, from the window's own sentence — so rows holding pets at different feeding
stages are timed by their own rates, and the Pet tab's items-per-minute is only the fallback.

## 4. The count read

A slot whose count read as *nothing* while plainly showing digits led to a full investigation: the
saved crops are pixel-for-pixel indistinguishable from the ones that read at 0.998, and the recogniser
is **knife-edge** on them — one crop reads at 3× and at nothing else, another reads only at 24×, and the
same slot read fine seven minutes later. The read now walks **3× → 9× → 24×** when a rung comes back
empty, with the foreground re-checked between rungs; a slot that reads normally still pays exactly the
one read it always did. Anything above 300 is still rejected, so a garbled or reversed read becomes *no
reading* rather than a confident wrong number.

## 5. The US client

**Setup → Game client**: one button per client (`tw — TW_LIVE`, `us — US_LIVE`). The choice sets the
window title and the Tuner's wordings, and it is written to the config. Every tool that reads **no
text** — Gem Composer, Buy, Sell, Spammer, Quest, and the pet reads — runs on the US client with no
setup at all; that is why `config/attributes.us.yaml` ships **empty**, with its header saying what it is
for and how to fill it. The **Tuner** is the one tool that needs it filled, from that client, by running
it and letting it record its own misreads — the method is in the file. Switching **refuses** if a
client's dictionary file is missing, because a config pointing at a missing dictionary is a launcher
that will not start.
