# Plans — proposals

**Work not yet done, or partly done.** Every document here describes something that is *not* how the
system works today — a to-do with its reasoning attached.

Read one for what it proposes and why. Do NOT read one to find out how something works: the answer is in
[../spec/](../spec/DESIGN.md), and a plan that has shipped moves to [../archive/](../archive/README.md)
so that "how does this work?" is never answered by a document that is half aspiration.

**The table below is the STATUS of every live plan, and the only place one is written down.** A plan's
own text carries its design and its reasoning; it does not say whether it is built, because that claim
drifts — the pet-feeder plan's §13 read "design, not built" long after the queue it describes had
shipped. So: when a plan is added, add its row; when its state changes, change the row; and when its
work ships, the reasoning graduates into [../spec/](../spec/DESIGN.md), the plan gets an ARCHIVED banner
and moves to [../archive/](../archive/README.md), and this row goes away.

| plan | state |
|---|---|
| [PLAN-DEPLOY.md](PLAN-DEPLOY.md) | steps 1–6 built on `feature/frozen-templates`; **not shipped** until the release |
| [PLAN-PATHS.md](PLAN-PATHS.md) | **declined** — its relocation is rejected by PLAN-DEPLOY; kept for the diagnosis |
| [PLAN-ARBITER.md](PLAN-ARBITER.md) | steps 1–2 shipped; 3–4 not, so the deadline machinery is inert |
| [PLAN-PET-AUTOFEED.md](PLAN-PET-AUTOFEED.md) | mostly built — the reload path is verified live; **the queue in §13 shipped too**, so what is left of that section wants a pass |
| [PLAN-PET-FEEDER-NEXT.md](PLAN-PET-FEEDER-NEXT.md) | partly built |
| [PLAN-RELOAD-VISIT.md](PLAN-RELOAD-VISIT.md) | partly built |
| [PLAN-SCHEDULING.md](PLAN-SCHEDULING.md) | step 2 not started |
| [PLAN-TUNER-SPRING.md](PLAN-TUNER-SPRING.md) | features 1–2 shipped in v2.4; the OCR-band cursor parking is open |
| [PLAN-UI-CLEANUP.md](PLAN-UI-CLEANUP.md) | steps 0–9 shipped, the navigation rail tried and reverted; the button convention is left |
| [PLAN-US-CLIENT.md](PLAN-US-CLIENT.md) | the switch shipped; the English dictionary is the player's to fill |
| [PLAN-PRESET-DUPLICATE.md](PLAN-PRESET-DUPLICATE.md) | **built on `plan/preset-duplicate`** — Quest flow, Quest sequence and Spammer presets all have Duplicate; Buy and Game Variants stay out; all three ⚑ questions answered and built (ignore-case name scan, Save refuses a missing flow, one shared picker) |
| [PLAN-V3-CORE.md](PLAN-V3-CORE.md) | **code-complete on `v3-core`, cut-over in progress** (165 commits) — nine Rust crates + the Flutter shell; **every PROTOCOL §4 command is served**, no spec-only command left; **all eight tools live** (spammer, holdspace, tuner, quest, sell, buy with §12.6 run arguments, gem, pet); per-run file logs (G8), the 18-kind calibration surface with named-instance targets, and the `--replay` development core all landed; the shell reaches every PLAN-V3-SHELL surface (12 tabs, 164 tests) and the capture/hide lifecycle now works from the UI; `v3/publish.ps1` is hardened and selftested, `-Verify` RED only on the three guide docs. Remaining: USER_GUIDE/INSTALL + zh-TW, the staging/release run, the release record/root README, and **live verification in the game** — the last needs the player (⚑ §13.10 OCR recordings also open) |
| [PLAN-AGENT.md](PLAN-AGENT.md) · [PLAN-INPUT-BACKEND.md](PLAN-INPUT-BACKEND.md) · [PLAN-V3-RUST.md](PLAN-V3-RUST.md) | not started — **all three want the same seam**, which PLAN-V3-CORE now cuts; PLAN-V3-RUST is superseded in shape and kept for its diagnosis and its invariant table |
| [PLAN-SEAL-KB-BOT.md](PLAN-SEAL-KB-BOT.md) · [PLAN-WATCHER.md](PLAN-WATCHER.md) | not started |
