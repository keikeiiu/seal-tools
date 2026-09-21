# Archive — finished plans

Plans that are **built and shipped**. They are kept because the reasoning inside them is the reason the
code looks the way it does — and a design decision without its reasoning gets reversed by whoever finds
it inconvenient next.

**Nothing here is pending work.** How it works now is [DESIGN.md](../DESIGN.md). What is still open is
[TODO.md](../TODO.md). What was done, and when, is [PROGRESS.md](../PROGRESS.md).

| plan | what it built | landed |
|---|---|---|
| [PLAN-BUY-SELL.md](PLAN-BUY-SELL.md) | the buy/sell tool — the scroll-and-match flow, presets, the attribute filter | v2.9, verified live 2026-09-14 |
| [PLAN-HOVER-INFO.md](PLAN-HOVER-INFO.md) | reading the hover tooltip — the calibrated offset, the hover read, `PetPanel` | 2026-09-18, parser 2026-09-19 |
| [PLAN-RESIDENT-PET.md](PLAN-RESIDENT-PET.md) | running tools alongside the pet feeder — per-tool records, slot-aware `StopTool`, the port gate, the deferral | v2.11, 2026-09-21 |

**The rule this folder exists to keep:** a plan is a *proposal*. When one completes, its durable
reasoning belongs in `DESIGN.md` and the plan comes here — so that "how does this work?" is never
answered by reading a document that is half aspiration. That failure has already happened once: the
resident-pet plan shipped in v2.11 and was still being read as pending work the next morning.
