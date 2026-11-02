# spec — how v3 works now

If a document here disagrees with the code, the CODE is the bug under discussion and the document is
the contract to reconcile — but a stale sentence is a defect too; fix it in the same commit as the
change that made it stale.

| document | subject |
|---|---|
| [PROTOCOL.md](PROTOCOL.md) | the core <-> shell seam (commands, events, envelope, fixtures) |
| [DESIGN.md](DESIGN.md) | the architecture as built: crates, layers, ownership |
| [CONFIG.md](CONFIG.md) | the config files, the merge, the write gate |
| [CALIBRATION.md](CALIBRATION.md) | the coordinate contract, the marks, portability |
| [TOOL-FLOWS.md](TOOL-FLOWS.md) | what each tool decides and what a run does |

The directory is the v3 truth; `v2/docs/spec/` is the previous build's documentation and the RECORD
behind these rules. Where something is not built, [OPEN.md](../backlog/OPEN.md) owns that list — not
this folder.
