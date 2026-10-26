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
| [CUTOVER.md](CUTOVER.md) | **in progress**: decisions taken (zip-only v3.0, naming, templates, no global hotkeys, docs set), packaging script `-Verify` RED on seven named release inputs, capture/hide blocker + OCR model fallback + naming tasks in flight, docs corrections landing. |
| [PLAN-V3-SHELL.md](PLAN-V3-SHELL.md) | **all PLAN §6 surfaces landed** (`62ffc4d`, 129→134 commits): the hand-written protocol client, 12 tabs (Cards + all config tabs + Pet/Attributes/Arduino/Setup/Hotkeys/Calibrate), the config-draft machinery with keep-alive and external-change guards, the calibrate frame/mark/gesture surface, live card status with the shared countdown ticker, and card extras. Gates: `flutter analyze` No issues, `flutter test` 142/142, format clean. Remaining shell work is blocked on core readers (shop buy needs its protocol choice; pet rows need the pet loop) or on the mini-mode geometry decision; the five ⚑ player questions are still open (they shape polish, not function). |

The v3 **core** plan is not here: it is [v2's PLAN-V3-CORE.md](../../../v2/docs/plans/PLAN-V3-CORE.md),
and its status stays on [v2's board](../../../v2/docs/plans/README.md). One plan, one status row.
