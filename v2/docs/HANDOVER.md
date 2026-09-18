# Handover — paste this into a new session

Written 2026-09-19, at the end of the session that shipped **v2.10** and built most of the Pet Feeder.
Copy the block below as the first message of the next session.

---

> Continue Seal Tools v2. Read `v2/docs/PROGRESS.md` first (newest entry first — it records what was
> decided and why, which the code and commit titles don't), then `v2/docs/TODO.md` for what is open.
> Do not re-derive what those say.
>
> **Where things are.** `main` is at **v2.10** (tagged and released, app + firmware assets only — the
> `-local` zip is built for backup and never uploaded). The new tool this session is the **Pet
> Feeder**, and its design lives in `v2/docs/PLAN-PET-AUTOFEED.md` — read §11–13 before touching it,
> because those sections carry corrections that are not visible in the code.
>
> The launcher runs from `v2/SealTools.Launcher/bin/Debug/net8.0-windows/` and is **very often
> already running, which locks the build output** — stop it before building
> (`Get-Process SealTools.Launcher | Stop-Process -Force`) or the build fails with "file is locked".
> A **Release** build is the compile check that works while it runs.
>
> **Two gotchas that cost real time this session:**
>
> - **A launcher startup crash shows no window and logs to `bin/.../logs/error.log`, NOT `v2/logs`.**
>   Process alive + empty `MainWindowTitle` = the constructor threw and `App` swallowed it.
> - **A Python write that opens with `'w'` truncates before it can fail.** One did, and a commit
>   recorded 130 deleted lines of `PLAN-HOVER-INFO.md`. Write to a temp path, or build the whole
>   string before opening the file.
>
> **How I want you to work** (unchanged, and it still matters most):
>
> - Commit every small victory as it lands, one concern per commit, with a message that says *why* and
>   **what was measured** — the style of the existing commits.
> - Keep `v2/docs/PROGRESS.md` current in the same session. Update the plan doc in the same commit as
>   the change it describes, and **correct the plan when the live game proves it wrong** — three
>   sections of the pet plan are corrections of my own earlier writing, and one doc was fixed twice in
>   opposite directions.
> - Ask when a request is ambiguous. Say plainly what you verified live and what you only reasoned
>   about; several "obvious" causes this session were wrong.
>
> **Open threads, in order of value:**
>
> 1. **The parser and the `+10` guard.** A read yields `（6）真蔚蓝凤凰+7[52.18%]` — the stage, growth
>    and EXP are all numbers and all read cleanly, while the Chinese does not (see
>    `PLAN-HOVER-INFO.md`). Nothing parses that yet, and nothing stops the tool reloading a finished
>    pet. It is small, self-contained, and it is what makes every read so far worth having.
> 2. **The icon scan and the queue** — `PLAN-PET-AUTOFEED.md` §13. `IconMatch` exists and is unused;
>    it needs a crop capture on the Pet tab. This is what lets the tool run while the character farms,
>    which the marked-cell design cannot.
> 3. **The feeder-count read** — schedule from what is actually in the feeder instead of `600 ÷ 3`.
>    The Test read proves the OCR works; the tool does not use it.
> 4. **The two-writers race.** Calibrator buttons share the Arduino port with a running tool —
>    pressing Test read mid-run interleaves commands with the tool's placement. Not what bit us, but
>    small to close.
> 5. **Fill `text_fixes` in `attributes.yaml`** from the OCR logs. The `simplified_traditional` list
>    already exists with the right comment; it just lacks the pairs we measured (`藍 → 藍`, `鳳 → 鳳`).
>
> **Unfinished on the machine right now.** The Pet tab's **Timing** fields need setting — *Wait after
> empty* = **2**, *Action wait* = **1200** — then Save. The **empty pet slot reference is captured and
> stored**, so the "did the pet go in" check is live. A 12-hour run on 2026-09-18 lost roughly half
> its boarded time to reloads that fed nothing and reported success; the log is at
> `bin/.../logs/pet.log`, and an earlier failure trace at `cursor_trace.txt`.
>
> **Do not trust the plan over the game.** Everything in `PLAN-PET-AUTOFEED.md` that says "verified" was
> verified on the live game with the player watching; everything else is design.
