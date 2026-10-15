# Specifications — how it works NOW

Each of these describes the system as it stands and **has to stay true**: when behaviour changes, the
document changes in the same commit. That is what separates this folder from its neighbours — a
[plan](../plans/README.md) *proposes*, an [investigation](../investigations/README.md) *measures*, a
[record](../records/README.md) *remembers*.

The map — which document answers which question, and what each kind is — is
[DESIGN.md §7](DESIGN.md#7-where-the-rest-lives).

| document | what it covers |
|---|---|
| [DESIGN.md](DESIGN.md) | the architecture, the invariants, and the index to everything else |
| [TOOL-FLOWS.md](TOOL-FLOWS.md) | what each tool sends to the game, and where it branches |
| [CONFIG.md](CONFIG.md) | which file holds what, and how a save merges |
| [COORDINATES.md](COORDINATES.md) | coordinate space, DPI and capture — read before touching geometry |
| [CALIBRATION.md](CALIBRATION.md) | calibrating a machine, step by step |
| [MOVE-SETS.md](MOVE-SETS.md) | the composer's two move sets: tuned counts, or Arduino placement |
| [REVIEW.md](REVIEW.md) | the guardrails — decisions not to reverse |
| [INSTALL.md](INSTALL.md) | install, first run, updating |
| [USER_GUIDE.md](USER_GUIDE.md) · [zh-TW](USER_GUIDE.zh-TW.md) | every card, tab and button |
| [PET-DATA.md](PET-DATA.md) | the scraped per-pet feeding table |
| [PET-TAB-DESIGN.md](PET-TAB-DESIGN.md) | the Pet tab, the schedule and the reload flow |
| [AUDIT-PET-FLOW.md](AUDIT-PET-FLOW.md) | the feeder's in-visit detail, TOOL-FLOWS' companion — named for the audit that wrote it, but it is a specification |
