# v3 documentation

`v3/docs/` holds one folder per kind of document, matching the rule in `v2/docs/`:

| Folder | Holds | Rule |
|---|---|---|
| [spec/](spec/README.md) | How v3 works now | Read this before changing behaviour; update it in the same commit. |
| [plans/](plans/README.md) | Proposals and work not yet done | A plan has no status of its own; its row on the plans board is the status. |
| [backlog/](backlog/README.md) | Decided or named work, not ideas | [OPEN.md](backlog/OPEN.md) is the living list. |
| [investigations/](investigations/README.md) | Measurements and analyses | Findings stay true whether or not anything is built from them. |
| [diagrams/](diagrams/README.md) | Generated run-flow and system pages | v2's eight tool flows and calibration; runtime architecture, lifecycle, start/stop sequence and config dataflow; all Archify-checked. |
| [records/](records/README.md) | Dated history | Records remember; they do not describe the present. |

The v3 docs tree is the current shipped baseline (v3.0.14). The v2 docs tree
([../../v2/docs/](../../v2/docs/)) remains for the previous generation and its fallback release; v3 keeps
its own fresh tree rather than moving v2's (PLAN-V3-CORE §13.5).
