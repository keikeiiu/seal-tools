# Handover — paste this into a new session

Written 2026-09-21, at the end of the session that shipped **v2.11**: the Pet Feeder went from one row
to four, learned to read how much food is left and schedule its reload from that, became resident, and
the count read was rebuilt four times until one held. Copy the block below as the first message.

> **This is a snapshot and it has an age.** The two things below that go stale fastest, and what they
> actually were when this was last checked (2026-09-22): **`main` is no longer "at the v2.11 release"**
> — it is ~80 commits past the tag, with that work unreleased, so re-run `git log` rather than
> trusting the count here. **The running-process details (PID, start time) are from the session that
> wrote this**, not from whatever is running now — check with `Get-Process SealTools*` before you
> assume the feeder in the log is the one you are looking at.

---

> Continue Seal Tools v2. Read `v2/docs/PROGRESS.md` first (newest entry first — it records what was
> decided and why, which the code and commit titles don't), then `v2/docs/TODO.md`. Do not re-derive
> what those say.
>
> **A pet feeder is RUNNING LIVE as you read this.** `SealTools.Launcher` PID 35532, started 10:48:58;
> its log is `SealTools.Launcher/bin/Debug/net8.0-windows/logs/pet.log`. **Do not stop the launcher and
> do not build Debug into it** — that kills the run. A **Release** build works while it runs and is the
> compile check. Restarting it needs the player's agreement.
>
> **Where things are.** The last **release** is **v2.11** (tagged and pushed, 121 commits past v2.10)
> — but `main` is *ahead* of that tag, so there is unreleased work; `git log v2.11..main` is the real
> answer and the count here is only true as of 2026-09-22. The branch `v2-pet-drag` is fully merged and
> can be deleted. `gh` is authenticated now, which it was not for v2.10.
>
> **Everything planned is built.** `PLAN-RESIDENT-PET.md`'s three parts all shipped in v2.11 — that
> document is now reasoning rather than a plan. What is left is verification and the items below.
>
> **Open, in value order:**
>
> 1. **Row 1 slot 1 intermittently reads nothing.** Visible in the log as
>    `feeder counts: [— / 300] -> 300 (1 of 2 slots read)` — seen twice, and the schedule then runs
>    early (105 min instead of 132). Harmless to the pet and it costs food cells sooner. The **row
>    geometry** pane on Calibrate Pet shows the per-row crop pixels and lets you nudge the strip's `x`
>    by typing; the Test read pane shows every slot's raw reading. Start there.
> 2. **`Food load → Drag` and the firmware `V` command need a reflash** and are inert until then. Both
>    default to today's behaviour. Flash `arduino/seal_mouse/seal_mouse.ino` (FW_VERSION 2) on both
>    boards; the Arduino tab then reports what each board says. The other PC's board is the one worth
>    flashing — an old board has no host-gone release, so a killed launcher leaves the spacebar down.
> 3. **The tuner and composer have NOT been started live in this build.** The residency work rewrote
>    the shared Start/Stop path (`LauncherService`: per-tool records, slot-aware `StopTool`, the port
>    gate), and only the pet tool has exercised it. Give the tuner a short supervised run before
>    trusting it — the visible surface is a Start, a Stop, and two cards.
> 4. **The feeder has not actually run resident beside another tool.** The deferral, the waiting card
>    message and the Quit-hotkey rule are all compile-checked only. Start the feeder, then start
>    buy/sell, and watch that the feeder's card keeps its standing line.
> 5. **One pet reads low at 0.320** against the crops where its neighbours are 0.000. It counts now, so
>    it is not urgent, and nobody has confirmed whether it is a rendering difference or something in
>    the way.
> 6. **~57 food cells a day** across four rows, against a 64-cell bag holding the pets. `local.yaml` had
>    **8 of 15 cells left** when this was written. A reload that runs out of cells is the one that
>    leaves a pet unboarded.
> 7. **`text_fixes` in `attributes.yaml` still wants filling** from the OCR logs.
>
> **Traps that cost real time — all written into PROGRESS, but they will cost you again if you skim.**
>
> - **A crop cut to the digits' height finds NOTHING.** Measured on three separate crops, each holding a
>   perfectly legible number, each returning zero detected boxes. **The mechanism is not known** — the
>   engine's own "text below ~20px" note does not explain it, since the digits are ~66px tall upscaled.
>   Full slot height is a measured rule with no explanation, and it is written down as one.
> - **The band of read positions that works is about three pixels wide.** Outside it, one direction
>   finds nothing and the other returns a **confidently wrong number** — a clipped `138` came back as
>   `3` at 0.99, the same confidence as a correct read. Four attempts to let a human put that position
>   in by drawing a box all failed. It is computed now, and `Count crop starts at` is a setting.
> - **A hand-written config projection drops a field on LOAD, silently** — three times across two
>   sessions (`LocalPet.From`, `LocalPetSlot.ToConfig`, and `IsPoint` used on a rectangle). **And
>   removing a config field deletes the player's stored value on the next save**, which is how the
>   player's drawn count slot was lost.
> - **A capture reads the SCREEN.** Whatever is in front is what gets matched, so a log saying "no pet
>   in the bag" can be the right verdict from the wrong image. Three things now refuse or correct for
>   it: foreground guards, parking the cursor off the bag, and the Test read refusing to judge a
>   capture that is not the game.
> - **Numbers picked by reasoning were wrong; numbers measured were right.** `MatchLimit` was 0.12 by
>   inheritance and cost half of every scan — the real separation is 0.0–0.15 against 0.83+.
> - **The EXP% is per LEVEL, not per stage.** Four rows of live evidence, matching the game's own ETA
>   to the minute.
> - **`SealTools.Pet/PetTool.cs` is LF while `MainWindow.xaml.cs` is CRLF.** A multi-line patch written
>   against the wrong one silently does nothing — which cost an hour of "fixes that appeared to do
>   nothing". Match the file's own endings.
>
> **How I want you to work (unchanged, and it still matters most):**
>
> - Commit every small victory as it lands, one concern per commit, saying why and what was measured.
> - Keep `PROGRESS.md` current in the same session; update the plan in the same commit as the change,
>   and **correct the plan when the game proves it wrong** — it did again here.
> - **Present the plan before editing.** The player asked for this explicitly and it was earned: one
>   patch of mine duplicated 535 lines of `MainWindow.xaml.cs`, and fixing forward made it worse.
> - **Ask when a request is ambiguous**, and say plainly what you verified live and what you only
>   reasoned about. Several "obvious" causes were wrong here, and one of them was written into a plan.
