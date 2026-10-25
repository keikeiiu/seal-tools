# Protocol — v3 core ⇄ shell

**Status: DRAFT v2, written 2026-10-25.** The commands the core serves today are implemented (`hello`, `shutdown`, `config.validate`/`config.apply`/`config.reload`, `ui.window`, `tool.start`/`tool.stop`); the status paragraph in §4 names the spec-only commands that wait on the Windows and OCR crates. The step-1 spikes (DPI mapping, IME, hotkey location) can still change it.

The design rationale is PLAN-V3-CORE §4. This file is the contract; both halves are written
against it, and the golden fixtures in §10 are how they are kept in agreement.

---

## 1. Transport and lifecycle

- The shell (`SealTools.exe`) spawns the core (`seal-core.exe`) from its own directory, no
  arguments.
- Messages are **UTF-8 newline-delimited JSON**, one object per line, on the core's stdin and
  stdout. stderr carries logs and panics only, never protocol messages. A decoder may tolerate one
  trailing carriage return on a line (Windows pipes); the core never writes one.
- The core discovers its root the way v2 does: walk up from the executable for
  `config/defaults.yaml`, else `config/defaults.yaml.example`.
- **Single instance (designed, not built yet — [OPEN.md](../backlog/OPEN.md) §3):** the core will
  hold an exclusive OS file lock on `<root>/config/.seal-core.lock`. A second core exits with
  code `2` and one stderr line; the shell shows it. The lock is released by the OS when the
  process dies, so a crash leaves no stale lock to clear by hand.
- **Handshake:** the shell's first message is `hello`. Any request before a successful handshake
  fails with `not_ready`. A protocol-version mismatch fails `hello` with `bad_version`; the
  shell shows both versions rather than guessing.
- **Shutdown:** `shutdown` asks the core to exit cleanly. EOF on stdin does the same implicitly:
  cancel the current tool through its normal stop path ("abandon the work, not the cleanup"),
  release held keys and buttons through the board guard, flush logs, exit `0`.
- **Core crash:** the shell keeps its last rendered state, shows the failure and the captured
  stderr tail, and offers "restart core" — a new process and a new handshake. The tools' real
  state is the game and the board, so a restart loses nothing that was not already lost.
- **Boot refusal:** a fatal config problem found before `hello` — files that cannot be read, a
  merged config the validator refuses, a dictionary that cannot load — exits non-zero with the
  reason on stderr and nothing on stdout. The shell shows that text on the crash channel above; it
  is not a protocol message and must not be parsed as one. A reload that meets an invalid file is
  different: it is a response with `config_invalid` (§6).
## 2. Envelope

Every message is one JSON object. `v` is always present and is the protocol version (`1`).

Request:

```json
{"v":1,"id":"7","cmd":"window.measure","args":{}}
```

Response — success carries `result`, failure carries `error`; `id` always echoed:

```json
{"v":1,"id":"7","ok":true,"result":{"display":{...}}}
{"v":1,"id":"7","ok":false,"error":{"code":"window_not_found","message":"the game window was not found","detail":{}}}
```

Event — unsolicited, no `id`:

```json
{"v":1,"event":"state","data":{"tool":"pet","running":true,"phase":"running","message":"..."}}
```

`id` values are chosen by the shell, unique for the life of the connection, opaque to the core.

## 3. Types

```jsonc
// Rect — integer pixels. Physical screen pixels unless the field name says otherwise.
{"x": 12, "y": 47, "w": 2865, "h": 1789}

// DisplayInfo — what the core measured; the shell uses only this to map frames to coordinates.
{
  "scale": 1.5,                 // physical px per logical px, measured (never cached)
  "monitor_dpi": 144,           // 96 * scale, shown as a cross-check
  "screen": {"w": 3840, "h": 2160},   // primary screen, physical
  "frame": {...},               // window frame rect, physical
  "physical_client": {...},     // client area, physical
  "logical_client": {...}       // client area as a DPI-unaware process sees it
}

// Frame — an image the shell may draw. origin is the frame's top-left in physical screen px.
{
  "format": "png",              // "png" for now; "bgra" is a later option, measured first
  "width": 2865, "height": 1789,
  "origin": {"x": 12, "y": 47},
  "data": "<base64>",
  "frame_id": "f1"              // retained by the core; see §8
}
```

Config is **opaque JSON**: the exact shape the core reads from and writes to
`defaults.yaml.example` / `local.yaml` / `attributes.yaml`. The shell mirrors the blocks it
edits; it never re-serialises the whole file (§7).

## 4. Commands

| cmd | args | result |
|---|---|---|
| `hello` | `{"protocol":1,"client":"0.1.0"}` | `{"protocol":1,"core_version":"0.1.0","root":"C:\\...","rev":7,"config":{...}}` |
| `config.validate` | `{"sections":{"quest":{...}}}` | `{"errors":[],"warnings":[]}` |
| `config.apply` | `{"rev":7,"sections":{"quest":{...},"settings":{...}}}` | `{"rev":8,"sections":{"quest":{...},"settings":{...}}}` |
| `config.reload` | `{}` | `{"rev":9,"config":{...}}` |
| `window.find` | `{}` | `{"found":true,"title":"Seal Online ..."}` |
| `window.measure` | `{}` | `{"display":{...}}` |
| `capture` | `{"region":{"x":..,"y":..,"w":..,"h":..}}` | `{"frame":{...},"display":{...}}` |
| `calibration.save` | `{"frame_id":"f1","kind":"bag_grid","region":{...}}` or `{"kind":"focus_point","point":{...}}`; optional `display` writes the environment | `{"rev":10,"kind":"bag_grid","mark":[...],"derived":{"pitch":{...}},"artifact":{...}\|null}` |
| `calibration.fetch` | `{"kind":"bag_grid"}` | `{"kind":"bag_grid","mark":[...],"artifact":{"file":..,"png_base64":..}\|null,"rev":10}` |
| `attributes.get` | `{}` | `{"variant":"tw","dictionary":"attributes.yaml","attributes":{...}}` |
| `board.probe` | `{}` | `{"port":"COM5","baud":115200,"vid":"2341","pid":"8036","level":2}` |
| `board.test` | `{"kind":"click"}` / `{"kind":"move","dx":100,"dy":0}` / `{"kind":"key","key":"E"}` | `{"ok":true,"detail":"..."}` |
| `ui.window` | `{"left":100,"top":100,"width":900,"height":700,"pinned":false}` | `{}` |
| `tool.start` | `{"tool":"spammer"}` | `{}` |
| `tool.stop` | `{"tool":"spammer"}` | `{}` |
| `ocr.scan` | `{"frame_id":"f1","mode":"attributes"}` or `{"region":{...},"mode":"count"}` | `{"lines":[...],"attributes":[[...]],"grade":"G","remaining":null,"capture_path":"logs/captures/..."}` |
| `shutdown` | `{}` | `{}` then the process exits |

Notes:

- `capture` does **not** hide the shell's window — the shell hides itself, waits one frame, then
  asks. The core still refuses to blit unless the game window is the foreground window
  (`game_not_foreground`), which is what makes the v2 guards work without a shared process.
- `window.find`/`window.measure` are separate because the calibrator shows "found/not found" long
  before it measures, and the game may be started later.
- `board.probe` reports the firmware level from `V` (absent = level 1) and the VID/PID the adapter
  read as hex strings (or `null` when the driver did not report USB IDs); `board.test` is the
  calibrator's Test Click / movement button, its `key` argument takes `"enter"`/`"E"` (the grammar's
  `E` **is** Enter), `"F1"`-`"F12"`, or one printable character, and its result carries the detail
  string v2 logs. A named `arduino.port` that is not present is `board_unavailable` with the
  port-choice refusal, never a fall-through to a different board.
- `ui.window` is the only way window geometry and pin state reach `local.yaml`; the core is the
  single writer (§7). `height` is the height to restore when the config region is expanded — v2's
  `ui.expanded_height`; the collapsed and mini heights are computed by the window and never stored,
  so `local.yaml` keeps its v2 key names and no `height:` key is written.
- `tool.start` on a second tool while one runs fails with `tool_busy`; `tool.stop` with no `tool`
  stops whatever is running. `holdspace` is a valid tool id (§9).
- **`config.apply` may carry the portable half** as a `"settings": {...}` section alongside owned
  top-level blocks. The core computes the effective merged config (owned blocks replace, `settings`
  overlays field-wise — §7), runs the authoring rules on it, then resolves the variant, loads the
  attribute dictionary that variant names, and only **then** writes: owned sections through the
  block writer, the portable overlay through the settings projection. One rev bump covers both; the
  result echoes the effective sections for the names in the request, in the request's structure. A
  failed dictionary load is `io` with the loader's line — nothing written, no rev consumed. That
  order is v2's `SwitchVariant` (resolve → validate → load → write), the only one that cannot
  half-switch a variant.
- `config.reload` re-reads and re-merges; an invalid on-disk config is answered with
  `config_invalid` (§6) carrying the validator's lines in `detail.errors`, and the session keeps
  the config it had.

**Status of §4.** Implemented today: `hello`, `config.validate`, `config.apply`,
`config.reload`, `ui.window`, `window.find`, `window.measure`, `capture`, `board.probe`,
`board.test`, `attributes.get`, `calibration.save`/`calibration.fetch`, `ocr.scan`, `tool.start`,
`tool.stop`, `shutdown`. A command in this table is the contract; the implemented list is the current
fact, and nothing in §4 is spec-only today.

## 5. Events

| event | data | when |
|---|---|---|
| `state` | `{"tool":"pet","running":true,"phase":"running","message":"row 2: looking","countdown_s":null,"rows":[<row>, ...],"error":null}` — the row object is specified below | whenever the published state changes — the whole 750 ms-poll surface, pushed |
| `tool.finished` | `{"tool":"pet","reason":"done"|"cancelled"|"error"|"guard","message":"DONE — 4 row(s)"}` | a run ends; the message is the one v2 writes to its log |
| `config.changed` | `{"rev":11,"block":"pet","values":{...}}` | specified, no emitter yet (DESIGN §3): the core wrote a block the UI displays (pet session state, calibration, `ui:`) |
| `log` | `{"level":"info"|"warn"|"error","line":"..."}` | a line the UI may show in its status/log surface |
| `core.stopping` | `{"reason":"eof"|"shutdown"|"fatal"}` | last event before exit; `fatal` has no producer yet |

The plan's `core.ready` event is folded into the `hello` result (ruling 1, §12): the handshake
already returns everything it carried, and a second message with the same content is a thing
that can disagree with the first.

### `state`'s `rows`

`rows` is the per-row card surface (the pet feeder's four rows today). Each entry is v2's
`ToolRowState` — `name`, `status`, `detail`, `nextAt` — with the absolute `nextAt` made relative to
the publish:

```json
{
  "tool": "pet",
  "running": true,
  "phase": "running",
  "message": "row 2: looking",
  "countdown_s": null,
  "rows": [
    {"name": "row 1", "status": "boarding", "detail": "藍天 · 3/min", "next_action_in_seconds": 1800.0},
    {"name": "row 2", "status": "waiting for a reload", "detail": null, "next_action_in_seconds": null}
  ],
  "error": null
}
```

| field | type | nullable | meaning |
|---|---|---|---|
| `name` | string | no | The row's label on the card (v2 `ToolRowState.Name`) |
| `status` | string | no | The short state word the card shows — `boarding`, `waiting for a reload`, `given up` (v2 `Status`) |
| `detail` | string | yes | What the row is doing when there is more to say — the pet boarded, its rate — absent when the card needs only the status word (v2 `Detail`) |
| `next_action_in_seconds` | number | yes | Seconds from THIS publish until the row next needs attention; `null` when never again (v2 `NextAt`, an absolute `DateTime?`, made relative so the shell counts down without sharing the core's clock) |

Before the first publish — and for any tool with no rows — `rows` is `[]`. An empty list means
"no rows to show", never "not known yet": the shell draws the card without row entries. The
pinned fixtures are `event.state` (two rows) and `event.state.empty` (`rows: []`).

`phase` in this payload is the **lifecycle** phase (`idle`/`starting`/`running`/`stopping`); a
tool-specific sub-phase, if one is ever needed, travels in `message`/`rows`, not in `phase`.

## 6. Errors

```jsonc
{"code":"conflict","message":"config changed while you were editing","detail":{"rev":12,"sections":{"quest":{...}}}}
```

| code | meaning |
|---|---|
| `bad_request` | malformed envelope, unknown cmd, bad args |
| `bad_version` | protocol version mismatch at `hello`; `detail.expected` is the CORE's version the shell must speak, i.e. what the shell sent was wrong, not the core |
| `not_ready` | request before a successful handshake |
| `conflict` | `config.apply` with a stale `rev`; `detail` carries the current `rev` and the conflicting sections |
| `window_not_found` | the game window is not present |
| `game_not_foreground` | capture/placement refused because another window is in front |
| `board_unavailable` | no board on the configured port / port unusable |
| `ocr_unavailable` | the OCR models are not available on this machine; `detail.reason` is `models_env_unset`, `models_dir_missing` or `model_load_failed:<file>`. Shown like `board_unavailable`: the command exists but this machine cannot serve it |
| `tool_busy` | `tool.start` while another tool runs |
| `not_running` | `tool.stop` when nothing runs (not an error the UI shows as a failure) |
| `io` | a file write failed; `detail.path` says which |
| `config_invalid` | the on-disk config is invalid; `detail.errors` carries the ported validator's player-facing lines (a boot-time failure uses the stderr channel in §1, not this code) |
| `internal` | a bug; `message` is safe to show, `detail` goes to the log |

## 7. Config merge semantics

`local.yaml` is not only settings: the pet feeder persists run state mid-run and the launcher
persists `ui:` on every move. The core is the only writer and the only merger.

- The shell holds a working copy of the blocks it is allowed to edit and sends **only those**
  blocks: `config.apply {rev, sections}`. Never the whole file, never a whole block it does not
  own.
- The core re-runs the authoritative rules (`seal-domain`), merges **field-wise** into the
  current block — so a session half it wrote a second ago survives the tab's static half — writes
  atomically with the existing header/foreign-block rules, bumps `rev`, and returns the merged
  sections.
- `config.apply` may carry `"settings"` — the portable half (§4). Owned blocks replace; `settings`
  overlays field-wise into the file's `settings:` block, and one `rev` covers both halves. The
  result echoes the effective sections for the requested names, not the whole file (§12.2).
- A stale `rev` is refused with `conflict`; the shell reconciles and tells the player what changed
  underneath before saving again.
- Core-initiated writes will emit `config.changed {rev, block, values}` (specified, no emitter yet —
  DESIGN §3). A shell that is not editing that block applies it silently; one that is editing shows
  the conflict.
- `ui:` is fully core-owned; it is written only through `ui.window`.
- Hand edits to the file are picked up at startup or by `config.reload`.
- `config.apply` commits both halves — the owned blocks and the portable projection — in ONE file
  update, so a failed apply writes nothing and consumes no `rev` (seal-config's composed writer,
  `46df1ee`).

## 8. Images and the frame cache

Frames are PNG, base64 in JSON, for v1. At a 2865×1789 client a frame is ~1–3 MB, ~1.3× that as
base64; calibration is human-paced, so this is fine. Raw BGRA framing is a later optimisation,
measured before adopted.

Specified, not built: the core will keep the last **four** frames by `frame_id` (FIFO). `calibration.save` references the
frame the calibrator just captured instead of re-sending megabytes; a `frame_id` older than the
cache fails with `bad_request: unknown frame_id` and the shell re-captures.

## 9. Tool ids

Exactly the eight the v2 runner map accepts, and nothing else:

`spammer`, `holdspace`, `buy`, `sell`, `tuner`, `gem`, `quest`, `pet`

`buy` and `sell` are the two modes of the Shop tool. `holdspace` is the toggle with no card
(its status line and release-failure reporting are part of the Spammer card's surface).

## 10. Golden fixtures

The two halves must not drift, so the protocol is tested from both sides:

- `v3/protocol/fixtures/*.json` — one file per message shape: `request`, `response`, or `event`,
  plus a case name.
- A Rust test writes every fixture (serialising the real types) and fails if the file differs
  from what is committed.
- A Dart test decodes every fixture into the shell's types and fails if a field is missing or a
  type is wrong.
- Planned (not built): a Rust test that replays a recorded command fixture list against the
  dispatcher with a fake core state, asserting the sequence is accepted.

## 11. Open items for the step-1 spikes

1. **Hotkeys** — whether the core (a background process) can read keys while the launcher is
   focused, or whether polling must live in the shell and send `hotkey.down`/`hotkey.up` commands.
   The measurement decides whether this protocol grows two commands (PLAN-V3-CORE §6.5).
2. **DPI mapping** — the frame→physical-offset conversion is proven on this machine at 150%; if it
   is not exact, `DisplayInfo` grows the field the conversion needs rather than the shell deriving
   it from Win32.
3. **Frame size** — measure a real capture's size and transfer time before deciding PNG stays.
4. **IME** — not a protocol item, but it decides whether name fields are free text (PLAN-V3-CORE §13.2); if
   they are not, no protocol change is needed.
5. **Hotkey typed-name validation** — the Hotkeys tab parses typed names (`F12`, `q`) into VK
   codes, and that rule has no protocol path: the config carries codes, and validating a stored
   code would refuse configs v2 ran (v2's `ConfigValidator` had no hotkey rule; the rule belonged
   to the tab's text). A `hotkey.parse`-style command — names in, codes or player-facing error
   lines out — is the natural shape. Not defined yet; until it is, "no widget keeps a rule" has one
   documented exception in the shell's hotkey editor.

## 12. Rulings against the plan (recorded, not silent)

1. **`core.ready` folded into the `hello` result** — one source of truth; a second message that can
   disagree with the first is a defect waiting to happen.
2. **`config.apply` returns the merged sections, not the whole config** — the shell only owns a
   few blocks; returning the file would invite it to keep a whole-file copy, which §7 forbids.
3. **A frame cache with `frame_id`** — added so calibration does not ship megabytes twice; not in
   the plan, small surface.
4. **`config.apply` carries the portable half as a `settings` section, and the dictionary loads
   before the write** — the authoring-rule seam already treated a payload's `settings` as an
   overlay, so this makes the implicit shape explicit rather than adding one; v2's switch order
   (resolve → validate → load → write) is the only one that cannot half-switch a variant.