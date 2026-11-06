# v3 diagrams

Archify pages for the v3 architecture. Each page is a self-contained HTML file; open it directly in
a browser.

## Run-flow pages

The same eight tool flows as v2 (`v2/docs/diagrams/flows/`), rebuilt for v3, plus a calibration page.

| Diagram | Scope |
|---|---|
| [suite-flow.html](flows/suite-flow.html) | Whole suite: Flutter shell → protocol → supervisor → tool decision/runner → game and Arduino board |
| [buy-flow.html](flows/buy-flow.html) | Buy: start gate, ordered transaction plan, focus/scroll, count, one pass |
| [sell-flow.html](flows/sell-flow.html) | Sell: slots argument, selection/cap plan, highest-slot-first, MAX/Enter, one pass |
| [tuner-flow.html](flows/tuner-flow.html) | Magic Tuner attempt: countdown/guard, click/Enter, OCR read, match/limit stops |
| [gem-flow.html](flows/gem-flow.html) | Gem Composer: select → combine → empty-result read → streak → advance → all grades done |
| [quest-flow.html](flows/quest-flow.html) | Quest Hand-in: validate, positioning + cursor guard, replay steps, loop verdict |
| [pet-flow.html](flows/pet-flow.html) | Pet Feeder: preflight, row reads, schedule, visit/reload, wait, row drop |
| [spammer-flow.html](flows/spammer-flow.html) | Skill Spammer: tick, yield gate, combo/single walk, key writes, exits |
| [calibration-flow.html](flows/calibration-flow.html) | Calibrate: capture + assign, draw mark/session, validate, save + derive, verify/delete |

## System views

| Type | Diagram | Scope |
|---|---|---|
| Architecture | [system-architecture.html](system-architecture.html) | Shell ↔ core process, Rust crates, Win32/OCR/board, game and Arduino |
| Lifecycle | [tool-lifecycle.html](tool-lifecycle.html) | `idle → starting → running → stopping → finished`, refusals and exits |
| Sequence | [start-stop-sequence.html](start-stop-sequence.html) | Shell → supervisor → tool → board, Stop/cancel, `tool.finished` |
| Dataflow | [config-dataflow.html](config-dataflow.html) | defaults / setup / machine → merged config → readers → one write gate → file + event |

All pages passed Archify's `validate`, `deliver`, strict `check` and real-browser `browser-check`
gates for their final candidates. Each candidate pins the repository revision it was checked at.

## Sources

The Archify candidates live beside this file:

```text
sources/suite.workflow.json
sources/buy.workflow.json
sources/sell.workflow.json
sources/tuner.workflow.json
sources/gem.workflow.json
sources/quest.workflow.json
sources/pet.workflow.json
sources/spammer.workflow.json
sources/calibration.workflow.json
sources/architecture.json
sources/lifecycle.json
sources/sequence.json
sources/dataflow.json
```

Regenerate one page with:

```powershell
node <archify>/bin/archify.mjs finalize workflow `
  v3/docs/diagrams/sources/buy.workflow.json `
  v3/docs/diagrams/flows/buy-flow.html `
  --repo-root C:\Git_repo\seal-tools --quality showcase --json
```

Use `architecture`, `lifecycle`, `sequence` or `dataflow` in place of `workflow` for the system
views. The candidate's `meta.output` points at the matching HTML path.
