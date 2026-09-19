# Handover — paste this into a new session

Written 2026-09-20, at the end of the session that built the four-row Pet Feeder end to end and watched
it feed for the first time. Copy the block below as the first message of the next session.

---

> Continue Seal Tools v2. Read `v2/docs/PROGRESS.md` first (newest entry first — it records what was
> decided and why, which the code and commit titles don't), then `v2/docs/TODO.md` for what is open. Do
> not re-derive what those say.
>
> **A pet feeder is RUNNING LIVE as you read this.** PID and start time are in the launcher's process;
> its log is `SealTools.Launcher/bin/Debug/net8.0-windows/logs/pet.log`. **Do not stop the launcher, and
> do not build Debug into it** — that kills the run. A **Release** build works while it runs and is the
> compile check. If you need to restart it for a real reason, ask the player first.
>
> **Where things are.** `main` is at **v2.10 plus ~49 commits**, none of it released — the Pet Feeder
> grew from one row to four, gained an icon queue and a feeder-count read, and is now genuinely feeding
> four pets on a schedule. `v2.10` is tagged; nothing since is.
>
> **What is planned and NOT built: `v2/docs/PLAN-RESIDENT-PET.md`**, three parts in order.
>
> 1. **Hold Space's toggle inverts instead of stopping** — a live bug on the other PC. The guard asks
>    `CurrentId == "holdspace"` *and* `Running == true`, and when `Running` has gone false with space
>    still held the press falls to the `else` and **re-holds it**. Not a version difference — v2.9.1's
>    tag carries the identical code. The fix that matters is `StopTool` releasing space on every stop
>    path, not just that button's.
> 2. **Firmware version reporting** — a `V` command, because a board currently cannot be asked anything
>    and the other PC's may predate the host-gone release. The valuable output is the *silence*: no
>    answer is a definite no. Needs a reflash, so it only makes the *next* flash verifiable.
> 3. **The pet feeder becomes resident** — the player wants buy/sell, spam and compose to run **while**
>    the pet keeps its schedule. Nothing to do with the cursor-lease design I abandoned: that was for
>    the **tuner**, whose window the game will not open alongside the boarding window. See
>    `PLAN-WATCHER.md` for that reasoning.
>
> **Traps that cost real time this session — all four are already written into PROGRESS or the plan, but
> they will cost you again if you skim.**
>
> - **A hand-written config projection drops a field on LOAD, silently.** It happened twice:
>   `LocalPet.From`, then `LocalPetSlot.ToConfig`. The symptom is that a setting "was never saved" when
>   it *was* — every load discards it and the next save writes the blank back. The reflection test only
>   guarded the outer type; it covers the nested ones now.
> - **A capture reads the SCREEN.** Whatever is in front is what gets matched — a log that says "no pet
>   in the bag" can be the right verdict from the wrong image. Three things now refuse or correct for
>   it: a foreground guard on the feeder read and the bag scan, and parking the cursor off the bag
>   (a pet under the pointer reads as a *different pet*, which cost an hour).
> - **Numbers picked by reasoning were wrong; numbers measured were right.** `MatchLimit` was 0.12 by
>   inheritance and cost half of every scan — the real separation is 0.0–0.15 against 0.83+. And when a
>   match failed, the fix that looked obvious (compare only the middle of the cell) made it *strictly
>   worse*; only the measurement said so.
> - **The EXP% is per LEVEL, not per stage** — the plan said otherwise and was wrong. Four rows of live
>   evidence, matching the game's own ETA to the minute.
>
> **How I want you to work** (unchanged, and it still matters most):
>
> - Commit every small victory as it lands, one concern per commit, saying *why* and **what was
>   measured** — the style of the existing commits.
> - Keep `v2/docs/PROGRESS.md` current in the same session. Update the plan doc in the same commit as
>   the change, and **correct the plan when the live game proves it wrong** — it did three times today.
> - Ask when a request is ambiguous.
> - **Say plainly what you verified live and what you only reasoned about.** Several "obvious" causes
>   today were wrong, and two were conclusions I had already written down.
>
> **Open threads, in value order:**
>
> 1. **The three parts of `PLAN-RESIDENT-PET.md`**, in the order written.
> 2. **The feeder-count read is built and never verified against a live feeder.** It reads each row's
>    number boxes and schedules from them; a row it cannot read falls back to a full-load assumption and
>    says so. The Pet tab's **Test read** is how to check the regions before trusting a run.
> 3. **One pet still reads low** (0.320 against the crops, where its neighbours are 0.000) and nobody
>    has confirmed whether that is a rendering difference or something in the way. It counts now, so it
>    is not urgent — but it is unexplained.
> 4. **~57 food cells a day** across four rows, against a 64-cell bag holding the pets. Restocking and
>    re-marking is a daily chore, not a one-off, and a reload that runs out of cells is the one that
>    leaves a pet unboarded.
> 5. `text_fixes` in `attributes.yaml` still wants filling from the OCR logs — the Simplified pairs
>    (`藍 → 藍`, `鳳 → 鳳`) are measured and missing.
>
> **Two gotchas from earlier sessions that still bite:**
>
> - **A launcher startup crash shows no window and logs to `bin/.../logs/error.log`, NOT `v2/logs`.**
>   Process alive + empty `MainWindowTitle` = the constructor threw and `App` swallowed it.
> - **A Python write that opens with `'w'` truncates before it can fail.** One did, and a commit
>   recorded 130 deleted lines. Write to a temp path, or build the whole string before opening.
>
> **Do not trust the plan over the game.** Everything in `PLAN-PET-AUTOFEED.md` that says "verified" was
> verified with the player watching; everything else is design, and this session found three places
> where the design was simply wrong.
