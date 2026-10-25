# v3 plans — proposals

**Work not yet done, or partly done.** A document here describes something that is *not* how the
system works today — a to-do with its reasoning attached. Read one for what it proposes and why; do
NOT read one to find out how something works. That answer is in [../spec/](../spec/DESIGN.md).

**The table below is the STATUS of every live v3 plan, and the only place one is written down.** The
reason is the same as [v2's board](../../../v2/docs/plans/README.md): a status written into a plan's
own text drifts, and this repo has read a shipped plan as pending work before. When a plan's work
ships, its durable reasoning graduates into `../spec/` and its row here goes away.

| plan | state |
|---|---|
| [PLAN-V3-SHELL.md](PLAN-V3-SHELL.md) | **scaffold landed** (`15ee238`): both installs done (Flutter stable 3.47.8 at `C:\src\flutter`; VS Build Tools 2022 + SDK 10.0.26100), `flutter doctor` clean for Windows, generated app + the three Dart gates passing. Next: the hand-written protocol client (`task-25`), the board OS adapter (`task-26`), then the UI surfaces; the five ⚑ player questions are still open. |

The v3 **core** plan is not here: it is [v2's PLAN-V3-CORE.md](../../../v2/docs/plans/PLAN-V3-CORE.md),
and its status stays on [v2's board](../../../v2/docs/plans/README.md). One plan, one status row.
