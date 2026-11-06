# v3 flow diagrams

The same eight run-flow pages as v2 (`v2/docs/diagrams/flows/`), rebuilt for the v3 architecture.

They are generated with **Archify 3.0.1** from the facts in
[`../spec/TOOL-FLOWS.md`](../spec/TOOL-FLOWS.md) and the cited Rust sources, at commit
`45cd0a9`. Each page is a self-contained HTML file; open it directly in a browser.

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

Every page passed Archify's `validate`, `deliver`, strict `check` and real-browser
`browser-check` gates for the final candidate.

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
```

Regenerate one page with:

```powershell
node <archify>/bin/archify.mjs finalize workflow `
  v3/docs/diagrams/sources/buy.workflow.json `
  v3/docs/diagrams/flows/buy-flow.html `
  --repo-root C:\Git_repo\seal-tools --quality showcase --json
```

The candidate's `meta.output` points at the matching HTML path.
