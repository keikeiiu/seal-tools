# Handover — paste this into a new session

Written 2026-10-01, at the end of a long session that shipped **v2.12** and **v2.13**, built the US-client
frame and the quest sequences. Copy the block below as the first message.

---

> Continue Seal Tools v2. Read `v2/docs/PROGRESS.md` first (newest entry first — it records what was
> decided and why, which the code and commit titles don't), then `v2/docs/TODO.md`. Do not re-derive
> what those say.
>
> **A launcher is RUNNING (PID 32932, started 09-30 21:53).** Its binaries are the current `v2/` code —
> the only commit since that build touches `v1/` — but **do not build Debug into it and do not kill it
> without asking**. A **Release** build is the compile check while it runs; restarting it needs the
> player's agreement. The pet feeder is NOT running in it (its last run ended 09-29 21:04).
>
> **Where things are.** The last release is **v2.13** (tagged and published, 2026-09-28). **`main` is 6
> commits past it and none of them are pushed**: the US-client frame (`d4d23dc`), the quest sequences
> (`67b3c2b`, `c0701ce`, `b872713`), and a check-in rework (`ae7ee30`) that is **someone else's work** —
> it changes `v1/checkin/checkin.py` only, is not part of the app, and is not mine to speak for. Pushing
> is a decision to make first, not a step to assume.
>
> **What is open, in value order:**
>
> 1. **The US client.** The frame is built — `game.variant: tw | us` in `defaults.yaml`, resolved at load
>    and selecting the window title AND the attribute dictionary. **Not built:** the Setup-tab switch
>    (§4.3, so switching is still a file edit) and moving the two hardcoded phrase sets into the
>    dictionary (§4.4). **Blocked on the player:** the English dictionary itself — names, OCR variants
>    and the fixes table, which cannot be written from here. Plan: [PLAN-US-CLIENT.md](PLAN-US-CLIENT.md).
>    **One measurement already taken:** the US client runs at window `1926×1112` / client `1910×1073`
>    against the TW calibration's `1926×1232` / `1910×1193` — **same width, 120 px shorter**. Resize the
>    US window to match *before* judging whether the calibration transfers, then look: the calibrate tabs
>    draw the saved marks on a capture.
> 2. ~~**The food scan (v2.12) has never run against a live bag.**~~ **CORRECTED 2026-10-02: it has.** The
>    player runs it from the Pet tab routinely — *"I have run it always in the tab"*. The v2.12 release
>    note and this line were both stale. It now ALSO runs as part of the tool's own pet-position scan and
>    re-marks the food cells (see PROGRESS entry 47).
> 3. **The quest sequences are unverified live too** — `Test one loop` on a single flow was reported
>    working; a sequence has not been run.
> 4. **The pet feeder's own list** — [PLAN-PET-FEEDER-NEXT.md](PLAN-PET-FEEDER-NEXT.md): §2 the
>    intermittent COUNT read (crop-armed, waiting for its evidence), §3 the cursor placement failures (no
>    cause, instrument first), and **the second time-line read still lives in `Visit` rather than
>    `ReadRows`, so the run-start look gets no retry** — that gap was found and deliberately left.
> 5. **The 2026-09-22 audit's findings, unfixed**, are in [TODO.md](TODO.md): the four `PetConfig`
>    properties no config file can carry, the unlocked `.tmp` race between the UI save and the pet tool's,
>    `AttrMatcher`'s `int.Parse` on unbounded digits, and **the projection guard's reach being only what
>    its fixture remembers**.
> 6. **The launcher UI** — [ANALYSIS-UI.md](ANALYSIS-UI.md) has the measurements and a proposed order:
>    extract the calibration canvas (copy-pasted 5×), collapse the four button factories into one, then
>    accessibility and the layout constants. **The blocking test handlers are the player's call** — the
>    only item that changes observable behaviour.
>
> **Traps that cost real time this session — all of them presented as something else.**
>
> - **A patch against a CRLF file with a `\n` pattern silently does NOTHING.** `ConfigLoader.cs` is CRLF.
>   It surfaced as *a test passing when it should have failed* — a no-op edit and a real one look
>   identical. Verify a removal took, then run the guard.
> - **`StartToolAsync` carries a tool-id WHITELIST that duplicates the launcher's card list.** A new tool
>   registered for its card and tab but missed there looks wired up and does nothing on Start — the
>   exception leaves an `async void` handler, so there is no message box and nothing on the card, only a
>   line in `logs/error.log`. Consolidating the ids into one source is still open.
> - **`publish.bat` copies the config by an EXPLICIT LIST.** A new config file not named there does not
>   ship — and the error message can then tell the player to copy a file that isn't in the zip.
> - **A tool that ends by itself was left "running" forever.** `StartToolCoreAsync` had no `finally`, so
>   Running was cleared only on a crash; it had never shown because every other tool loops until stopped.
> - **The projection drops fields, four times now.** `SaveDefaults`, `LocalQuest`, `LocalPet` and
>   `PetSlotConfig` are all explicit field lists. **The drill that works: remove the copy, watch the guard
>   fail, restore it** — done twice this session, and one of those "checks" was a no-op that looked like a
>   pass.
> - **A field the guard's FIXTURE omits passes whether or not it is copied** — the guard compares local
>   against config, so null on both sides is a pass.
> - **`MakeButton` carries a 10px TOP margin** meant for a button under a card heading. In a row, leave it
>   on one button and override it on the other and they sit at different heights — `MakeInlineButton`
>   exists for this.
> - **An "+ Add" button must go in FIRST, with rows inserted before it.** Appending rows puts a new one
>   AFTER the button, so it drifts up into the middle of the list. This was true of the tuner's rule and
>   override grids for as long as they have existed.
>
> **How I want you to work (unchanged, and it still matters most):**
>
> - **Commit every small victory as it lands, one concern per commit**, saying why and what was measured.
>   Keep `PROGRESS.md` current in the same commit.
> - **Present the plan before editing.** Asked for twice this session and it earned its place both times.
> - **Never rebase, never delete a branch** — *"never rebase... just normal pull request"*. A diverged
>   branch is resolved by merging.
> - **Ask when a request is ambiguous**, and say plainly what you verified live and what you only reasoned
>   about. This session that habit changed a design: I proposed "abort the sequence when a flow fails",
>   the player asked *"how do you know it is failed?"*, and the honest answer — that this tool reads
>   nothing and therefore cannot — removed the rule entirely.
> - **Do not kill or restart the player's running tool without agreement**, and build Release while one is
>   up.
