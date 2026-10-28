# Plan — card live detail (B5) and the Arduino connection card (B3)

**Proposal.** Nothing here is built; its state is the row on the [plans board](README.md).

Closes audit gaps **B5** and **B3** ([PARITY-AUDIT-2026-10-28.md](../records/PARITY-AUDIT-2026-10-28.md)
:42-49; [OPEN.md](../backlog/OPEN.md) §5): v2's cards printed facts the v3 `state` payload cannot
carry, and v2's Connection card — deliberately fixed in a past release — is one probe line. Both are
additive to protocol `v:1`; no version bump (§4).

---

# Part 1 — live detail on the cards (B5)

## 1.1 What v2 showed, per tool

State is `v2/SealTools.Core/ToolState.cs` (`Running, Grade, Remaining, Attempt, Cycle, Current,
FilterStatus, Schedule, NextActionAt, Rows, Message, Attributes`); `FormatStatus`
(`v2/SealTools.Launcher/MainWindow.xaml.cs:855-891`) renders `● RUNNING`/`● paused`, schedule +
countdown, one line per row, then `Grade:`, `Remaining:`, `Attempt:`, `Cycle:`, `Current:`, one
`· name=value` per matched attribute, `Filter:`, `⚠ message`. The guide states that list
(`v2/docs/spec/USER_GUIDE.md:106`), one line per part for pet rows and gem grades (`:123`).

| Tool | v2 live detail (writer) | v3 `ToolStatus` today | Gap |
|---|---|---|---|
| Tuner | Grade, Remaining, matched attributes, FilterStatus (`SealTuner.cs:202-205`); Attempt (`:122`) | message = countdown / filter line / "Attempt N" (`seal-core/src/tuner.rs:250-273`); the read's grade/remaining/attributes are consumed by `TunerRun::decide` and dropped | **Real** |
| Gem | Grade, Cycle (`GemComposer.cs:88-100,367,461-471`) + rows per grade | message "Grade G — cycle N" + rows (`seal-core/src/gem.rs:524-556`) | **Partial** (same facts, one message) |
| Spammer | lead-in schedule + countdown (`SkillSpammer.cs:121-127`), Current/Cycle (`:308-339`), paused (`:264`), unsupported-key warning on Message (`:383-411`) | **no `status()`** (`seal-core/src/spammer.rs:145-181`) → default empty | **Real** |
| Buy/Sell | Cycle, Current (`ShopTool.cs:168-169,212-213`), error/done messages (`:185,227`) | message = step words only (`seal-core/src/shop.rs:140-171`) | **Partial** |
| Quest | Schedule + Cycle(loop) (`QuestTool.cs:91,106,118,173`) | message only (`seal-core/src/quest.rs:200-210`) | **Real** |
| Pet | Schedule + NextActionAt + rows (`PetTool.cs:504-517`) | schedule line in message + rows + per-row countdown (`seal-core/src/pet.rs:477-485,1889-1898`) | **Nearly parity**; summary countdown absent |
| Hold Space | top-row dot/tooltip, five states incl. red "● input may be stuck" (`MainWindow.xaml.cs:808-854`; `LauncherService.LastReleaseError:785-885`) | no card; Spammer card holds phase/message (`tool_labels.dart:74-90`); failed release is **log-only** (`seal-core/src/holdspace.rs:44-46`; v3 guide §2.3) | **Real**, failure case only (⚑5) |

The unsupported-key ⚠ is a start refusal in v3 (CHANGED, better) and a paused message cannot exist
until the spammer yield path does (OPEN §2).

## 1.2 What the contract carries today

`ToolStatus` = `message` + `countdown_s` + `rows` (`seal-tools/src/runner.rs:54-80`); `ToolRow`/
`ToolSnapshot` in `seal-tools/src/tool.rs:114-170`. The runner publishes a snapshot when the status
changes (`runner.rs:224-245`); `events::state_data` serialises exactly those fields
(`seal-core/src/events.rs:129-145`); PROTOCOL §5 defines the event and the `rows` object (`:218,244-280`);
fixtures `v3/protocol/fixtures/event.state*.json` are the cross-language gate
(`seal-proto/tests/fixtures.rs:560-600`; regenerate with `UPDATE_FIXTURES=1`). The card renders phase
chip → message → countdown → rows in a 360 px cell (`tool_card.dart:90-260`; `card_grid.dart:182`),
rows with a shared 1 s ticker.

## 1.3 Proposed shape — one additive `detail` list

```json
"detail": [
  {"label": "Grade", "value": "DG"},
  {"label": null, "value": "attack speed=+10"},
  {"label": "Filter", "value": "matched"}
]
```

- One new field `detail: Vec<ToolDetail>`, `ToolDetail { label: Option<String>, value: String }`,
  through `ToolStatus` → `ToolSnapshot` → `state` data. `[]`/absent = "no detail to show" (the
  `rows` rule, [PROTOCOL.md](../spec/PROTOCOL.md) §5); an older core's missing field defaults to
  `[]`, so `v:1` handshake stays valid across the pair.
- Labels and values are the **core's** (v2 `FormatStatus` wording ported to the decision halves); a
  label-less entry is v2's `· name=value` attribute line, rendered as-is. The shell only lays out
  `label: value` — "no widget keeps a rule".
- `message` stays "the last thing that happened"; no `schedule` field — pet already uses
  message+`countdown_s`, and Spammer's lead-in can too (one field added, not two).
- **Rejected**: reusing `rows` (rows are the per-item plan and `[]` means "no rows to show", so
  scalar facts would make one block mean two things and v2's plan order would be lost); a new
  `tool.detail` event (second ordering surface beside `state`); per-tool typed fields (eight optional
  sets in the shell).
- Docs/fixtures: PROTOCOL §5 state table + example, a `detail` object table beside `rows`, note the
  additive rule; regenerate both state fixtures and add `event.state.detail` (Tuner). No new error
  codes; §13 replay unaffected.

## 1.4 Domain changes (facts the decision half must keep)

| Tool | Change |
|---|---|
| Tuner | store last read `grade`/`remaining`/matched `(name,value)` attributes (`TunerRead` has them, `seal-core/src/tuner.rs:427-466`) + `filter_line`, `attempt`; publish `Grade`, `Remaining` (Some), `Attempt` (>0), one label-less line per attribute, `Filter` (v2: `disabled` when off, `SealTuner.cs:205`); `message` returns to v2's "last event" (spring/guard/mouse lines) |
| Gem | add `Grade`/`Cycle` detail lines; message/rows unchanged |
| Spammer | implement `Tool::status`: lead-in → message "lead-in — starting shortly" + `countdown_s` from `SpammerRun::lead_in_remaining`; running → `Current` (last key) + `Cycle` (presses accepted, counted from `tick().presses`); no paused line (OPEN §2) |
| Shop | `ShopPlan` gains per-step milestones `(at_step, cycle, current label)` from the buy/sell planners ("大紅 3/10", "slot 12 (2/6)"); `status()` publishes v2's `Cycle`/`Current`; message keeps the v3 step words (CHANGED: more, never fewer) |
| Quest | `Plan` line (v2's flow/sequence shape, ported from `QuestTool.cs:91,173` over `QuestPlan.kind/loops/steps`) + `Loop` (`loop_index+1` of `loops`); message keeps the last action |
| Pet | `countdown_s` = soonest row's remaining (summary countdown); rows unchanged |
| Hold Space | failed release is the only v2 status surface with no protocol path; recommended optional `warning` on `tool.finished` (supervisor publishes the final word after the effects' `Drop`) — ⚑5 |

## 1.5 Shell card changes

- `_DetailView`: `label: value` lines (label `onSurfaceVariant`, value `onSurface`); label-less
  entries as the value; bodySmall; no per-line ellipsis (see layout).
- Card order: phase chip → message (hidden when empty) → countdown → error → body (rows then
  detail) → extras/hold-space/finished/refusal → buttons. v2's order (schedule → rows → facts →
  message) differs; recorded as adaptive.
- Layout: rows + detail share ONE scrollable body inside the 360 px card (`card_grid.dart:182`), so a
  6-line Tuner detail scrolls instead of clipping or growing the grid; empty body keeps the "No rows
  to show" quiet line.
- Countdown fix: top `countdownLabel` (`tool_labels.dart:62`) is static between events — subtract
  the grid's `elapsedTicks` as the rows do (`tool_card.dart:326`) so lead-in/start countdowns tick.
- Tests: extend `tool_card_test.dart`/`card_status_test.dart`: labels+values, label-less line, empty
  detail unchanged, long detail scrolls at 360, countdown ticks, older-core payload (no `detail`)
  decodes; fake transport already exists (`test/support/fake_transport.dart`).

## 1.6 Increments (B5)

| # | Deliverable | Acceptance |
|---|---|---|
| 1 | Foundation: `ToolDetail`/`detail` through status→snapshot→event; PROTOCOL §5 + fixtures (+`event.state.detail`); Dart `ToolDetail` + `ToolSnapshot.detail` defaulting to `[]`; card body + countdown tick + tests | core round-trip unit; fixture gate green; Dart fixture decode; widget tests; no visual change for tools without detail |
| 2 | Tuner: store + publish the five facts; message semantics | unit: scripted reads, retry numbering, filter disabled, attribute lines; live: a few real attempts, card matches v2's field list |
| 3 | Gem/Spammer/Shop/Quest/Pet per §1.4 | unit status per tool (Spammer lead-in/countdown; Shop milestones for a 3-item buy and 2-slot sell; Quest plan line; Gem lines; pet countdown); live pass per tool |
| 4 | Hold Space failure surface (⚑5) + docs: USER_GUIDE/zh card + Hold Space text, TOOL-FLOWS per-tool Card lines, PROTOCOL §5, release note | chosen option tested; both guides updated |

---

# Part 2 — the Arduino connection card (B3)

## 2.1 What v2 had (all `MainWindow.xaml.cs:1154-1391`)

- **Light + verdict + advice** (`:1234-1263`): green "Ready — Start will open COM5"; red "Start will
  refuse — the named port is missing" + the name-the-port sentence; red "Start will refuse — no
  Arduino found" + charge-only-cable advice. The `:1238` comment records the fix: one green/red light
  could not express a named-port refusal while another board matched.
- **Port list** (`:1265-1294`): every OS port, chosen first, `●`/`○`, name, "— Start will open this
  one" / "matches the expected ID"; PnP id as a tooltip only.
- **What the launcher looks for** (`:1296-1305`): `VID 0x2341  PID 0x8036 / 0x8037`; `arduino.port`
  unset ("found by the ID above") or named ("a named port wins").
- **Board firmware** (`:1307-1328`): "Not checked yet — this is normal…" or `OK/Not OK — <summary>` +
  `Reported: <detail>`; cached `FirmwareLevel/FirmwareReport` from the first tool open
  (`LauncherService.cs:261-263,317-321`), not this tab.
- **Refresh** (`:1331-1346`) re-enumerates; **input test** (`:1348-1391`) unchanged.
- Enumeration was WMI `Win32_SerialPort` (`SealTools.Core/Arduino.cs:25-76`,
  `ArduinoDevice(Port,PnpId,Name,IsMatch)` = `Diagnose`); the choice ports to
  `seal-board/src/port_choice.rs` (named port present wins, absent refuses).

## 2.2 What v3 serves today

`board.probe` **opens the port**, runs `V` twice, answers `{port,baud,vid,pid,level}`
(`seal-core/src/board.rs:119-135`; PROTOCOL §4:121); the Arduino tab calls it on open
(`arduino_tab.dart:30-80`) and shows one line; Setup has the same probe (`setup_tab.dart:315-354`).
The card's data mostly exists: `PortInfo {name,vid,pid,product,manufacturer}` from `serialport 4`'s
`available_ports()` (`seal-board/src/enumerate.rs:20-45`, the one live line), `matches_ids`/`discover`
(`:58-110`), `port_choice::choose` with v2's refusal (`port_choice.rs:20-50`), `Firmware` +
`summary`/`describe`/`can_drag` (ported wording, `seal-board/src/firmware.rs`), and
`Settings::from_config` (`board.rs:20-80`). Missing: the read-only command, a firmware cache, the tab.

## 2.3 Proposal — new read-only `board.details {}`; probe unchanged

Probe stays the "open and ask" action (Setup) because it is heavy and exclusive: it opens the COM
port, pays the boot wait and can answer `board_unavailable` while a tool runs — the moment the player
most wants the connection facts. A second command owns the tab:

```json
{"baud":115200,
 "expected":{"vid":"2341","pids":["8036","8037"],"port":null},
 "ports":[{"port":"COM5","vid":"2341","pid":"8036","product":"Arduino Leonardo",
           "manufacturer":"Arduino LLC","matches_ids":true,"chosen":true}],
 "choice":{"state":"ready","port":"COM5","refusal":null},
 "firmware":null}
```

- `choice.state` = `ready` / `named_missing` (refusal sentence from `port_choice::choose`) /
  `not_found`; the shell derives the three v2 light/verdict states and owns their wording (⚑9).
- Port rows: chosen first, then enum order; `matches_ids` gives "matches the expected ID";
  product/manufacturer replace WMI's `Name`; **PnPDeviceID is not ported** (tooltip-only in v2,
  not exposed by `serialport`, would need SetupAPI/WMI for no decision — ⚑7).
- **Firmware cache**: in-memory `CoreState` fact (`null` = "not checked yet — normal"), updated by
  `board.probe` and the pet session's existing `firmware::detect` (`registry.rs:265-278`). v3 does
  not query `V` on every tool start (that would add the boot wait + two attempts to every Start);
  v2's fresh launcher behaved the same, and an explicit query is available (⚑8).
- Never fails for a missing board (pure: settings + port list + cache); no new error codes.
- Test seam: pure `board::details(settings, ports: &[PortInfo], firmware: Option<Firmware>) -> Value`;
  the dispatcher only injects `enumerate::available_ports()`. Core tests over hand-built lists:
  chosen-first, named-missing, no ports, `matches_ids`, null/cached firmware. Shell: typed
  `BoardDetailsResult` (pattern `v3/shell/lib/app/core_actions.dart:54-90`;
  `shell_model.probeBoard` `:337-338`) and a `cmd.board.details` fixture decoded by
  `fixture_decoders.dart` (one entry per fixture — the cross-language gate is automatic).

## 2.4 Shell Arduino-tab rebuild (keys + tests)

Connection (dot `arduino-connection-status`, verdict, advice `arduino-advice`); Ports this machine
has (rows `arduino-port-row-COM5`, chosen first, `●`/`○`, product, reason); What the launcher looks
for (`arduino-looking-for`); Board firmware (`arduino-firmware`, `arduino-firmware-detail`); Refresh
(`arduino-refresh-details` → `board.details`, replacing "Refresh probe"); existing Input test boxes
unchanged. Fetch on open + Refresh; no polling (v2: "It is not live"). Optional secondary "Query
firmware" = `board.probe` (NOT-IN-V2; ⚑8). Widget tests with the fake transport: ready / named-missing
/ not-found / no-ports / not-checked / cached firmware / chosen-first / refusal verbatim; keep Setup's
probe tests green.

## 2.5 Increments + batch fit (B3)

| # | Deliverable | Acceptance |
|---|---|---|
| 1 | core: pure `details`, dispatch arm, firmware-cache plumbing, `cmd.board.details` fixture, PROTOCOL §4 row | unit cases; fixture gate; `cargo test`/`clippy -D warnings`/`fmt` |
| 2 | shell: Connection card rebuild + typed result + model method + keys + tests | `flutter analyze`/`test`; widget cases; Setup probe untouched |
| 3 | docs + live: USER_GUIDE §5.8/§11 + zh-TW, TOOL-FLOWS Arduino, CONFIG arduino block, release note; live with/without board, misnamed `arduino.port`, while a tool runs | live worksheet; docs gates |

**Batch fit.** B3 is additive (new command + tab, no existing payload touched) and can land in the
next release on its own. B5 changes the `state` contract (fixtures, Dart model, card layout) — the
audit proposes v3.1; both remain additive `v:1` (no `bad_version`, both halves ship together).
Recommended: **B3 first**, **B5 next**, unless batched.

---

## 3. ⚑ Questions (player/Lead)

**⚑1 — detail shape.** Recommended: one additive `detail: [{label?, value}]` with labels core-side;
alternatives rejected (rows reuse, second event). Confirm.

**⚑2 — Tuner message semantics.** Recommended: move Filter/Attempt/Grade into `detail` and return
`message` to "last event" (v2's `⚠` line), so no fact is said twice. Confirm nothing is lost.

**⚑3 — card layout.** Recommended: rows+detail in one scrollable body inside the 360 px cell;
alternative is taller cards (changes the grid just live-passed). Confirm.

**⚑4 — exact lines.** Recommended: port v2's labels verbatim (`Grade:`, `Remaining:`, `Attempt:`,
`Cycle:`, `Current:`, `Filter:`, `· name=value`, Quest/Spammer plan sentences) into the core; shell
adds no punctuation. Confirm English labels (values may be CJK).

**⚑5 — Hold Space release failure.** Recommended: optional `warning` on `tool.finished` (supervisor
publishes after the effects' `Drop`, so the log-only limitation v3 USER_GUIDE §2.3 records can go);
alternatives: log event only, or document-only. A failed release is a real stuck input — pick a
surface.

**⚑6 — B3 split.** Recommended: read-only `board.details` (enumeration + cached firmware + choice,
never opens the port) with `board.probe` kept for the explicit open-and-ask. Alternative: grow probe
(then the tab's facts fail whenever a tool runs).

**⚑7 — PnP id.** Recommended: drop it (v2 tooltip-only; not exposed by `serialport`; SetupAPI/WMI is
a new dependency for no decision). Say now if it is wanted.

**⚑8 — firmware freshness.** Recommended: memory-only, updated by probe and the pet session; the card
says "not checked yet — normal" and offers an explicit query. Alternative: `V` on every open (2 s
boot wait + up to 2×1.5 s attempts per Start).

**⚑9 — verdict/advice wording home.** Recommended: core owns the rule + refusal sentence; shell owns
the three verdict/advice lines (test-pinned, guides updated). Alternative: core returns sentences.

**⚑10 — release batch.** Recommended: B3 next release, B5 v3.1 (audit); B5 must not ship as a silent
patch (fixtures + card layout move).

---

## 4. Out of scope

Spammer card rotation picker (B1), window placement/mini-mode (OPEN §5), the pet session plan,
calibration increments, dry runs / Quest test loop, and the spammer yield path (OPEN §2) whose
paused card line cannot exist yet.

---

## 5. Lead decisions (2026-10-28)

All ten questions answered so implementation can freeze the wire. Two are player-facing surface
choices and are recorded as such; the rest are settled.

- **^1 detail shape — DECIDED: one additive `detail: [{label?, value}]`.** It rides ToolStatus ->
  ToolSnapshot -> the `state` event; labels and values from the core (ported v2 wording), label-less
  entries are the attribute lines, `[]` means no detail, and an old core's missing field decodes to
  `[]` so `v:1` handshakes survive with no version bump. `rows` stays per-item plan/empty data; no
  second event.
- **^2 Tuner message — DECIDED: Filter/Attempt/Grade move into `detail`; `message` returns to "last
  event".** The card must not say one fact twice and v2's warning line must still appear in `message`.
- **^3 card layout — DECIDED: rows + detail share ONE scrollable body inside the 360 px cell.**
  Taller cards would change the grid the live pass just approved; the countdown moves into that body's
  scroll with the existing ticker so it cannot go stale.
- **^4 exact labels — DECIDED: port v2's labels verbatim into the core** (`Grade:`, `Remaining:`,
  `Attempt:`, `Cycle:`, `Current:`, `Filter:`, `· name=value`, the Quest/Spammer plan sentences); the
  shell adds no punctuation of its own. English labels, values may be CJK (v2 parity).
- **^5 Hold Space failed release — DECIDED: yes, an optional `warning` on `tool.finished`.** A failed
  release is a real stuck-input risk; the supervisor publishes it after the effects' Drop, so the
  log-only limitation the guide currently records can go (the B5 increment that touches this also
  updates USER_GUIDE §2.3 and OPEN).
- **^6 B3 split — DECIDED: new read-only `board.details {}`; `board.probe` stays the explicit
  open-and-ask.** The tab must show "what Start will do" without opening the port, which is exactly
  what fails while a tool runs.
- **^7 PnP id — DROPPED for now.** v2 showed it only as a tooltip, `serialport` does not expose it,
  and SetupAPI/WMI is a new dependency for no decision. OPEN records it as a player-askable nicety.
- **^8 firmware freshness — DECIDED: memory-only cache**, updated by `board.probe` and the pet
  session's detect; the card says "not checked yet — normal" until a tool runs and offers an explicit
  "Query firmware" action (the only thing that opens the port from the tab).
- **^9 wording home — DECIDED: the core owns the rule and the refusal sentence; the shell owns the
  three verdict/advice lines** (test-pinned, guides updated). No rule in a widget.
- **^10 batch fit — DECIDED: B3 in the next release (v3.0.4 window), B5 in v3.1.** B5 changes the
  `state` contract, fixtures and the card layout; it must not ship as a silent patch.
