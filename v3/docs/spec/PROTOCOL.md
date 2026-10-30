# Protocol — v3 core ⇄ shell

**Status: current contract, 2026-10-27.** Every command in §4 is implemented and served by the core —
there is no spec-only command left — and both halves are written against this file, with the golden
fixtures in §10 keeping them from drifting. "DRAFT" describes v3 before cut-over, not a half-built
seam. The step-1 measurements that shaped it are recorded in §11; every place this file changed
against the plan is a ruling in §12.

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
- **Single instance (designed, not built yet — [OPEN.md](../backlog/OPEN.md) §2):** the core will
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
- **Logs are files, not protocol commands.** The core appends under `<root>/logs/`:
  `ocr_log.jsonl` (one record per completed scan), `ocr_errors.jsonl` (a completed scan that did not
  produce its mode's target), optional `captures/*.png` while `tuner.save_captures` is on (the scan
  result carries the relative `capture_path`), `error.log` (internal errors and tool opening
  failures — never player-correctable refusals, which would bury them), `holdspace.log` (its own
  diagnostic trail) and `<tool>.log` (one timestamped line each for the run's start, every CHANGED
  tool status message, and its final reason: `finished — …`, `stopped — …`, `failed — …`). Text lines
  use `yyyy-MM-dd HH:mm:ss `. A failed log write is swallowed and never fails a command or a run;
  `shutdown` flushes the files.

  `logs/reads/` holds command-caused Pet scan evidence when a scan command runs: a `scan_page{N}_{stamp}.png`
  page capture plus a `scan_page{N}_{stamp}.txt` report (see the two exceptions in §2). Nothing else
  writes there.

  Two explicit exceptions to "logs are not commands". `logs.cleanup_captures` (§4) exists so the Tuner
  tab can be told to delete its own capture images: explicit, Tuner-only housekeeping that opens no
  game/board, deletes only direct regular `*.png` files under `logs/captures` (no recursion, never
  follows links or reparse points), treats a missing directory as success with `deleted: 0`, and logs
  per-file failures instead of returning them. And a Pet bag SCAN writes its own evidence as part of the
  command (`pet.scan_food_page`/`pet.scan_queue_page`, §4): a best-effort page PNG and report under
  `logs/reads/`, returned in the result's `evidence`; an evidence write failure is swallowed and never
  fails the scan. These are command-caused artifacts, not a general log API.
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
| `calibration.save` | `{"frame_id":"f1","kind":"bag_grid","region":{...}}` or `{"frame_id":"f1","kind":"scroll_point","point":{...}}`; `{"frame_id":"f1","kind":"result_gem_area","region":{...},"empty":true}`; a named instance carries `target`, e.g. `{"kind":"gem_grade","point":{...},"target":"N"}`; optional `display` writes the environment. **Every save needs the `frame_id` of a live capture** — the core keeps the last four frames; an evicted id is `bad_request: unknown frame_id` and the shell must capture again (it never re-captures automatically) | `{"rev":10,"kind":"bag_grid","mark":[...],"derived":{"pitch":{...}},"artifact":{...}\|null}` (a targeted save echoes `"target"`) |
| `calibration.fetch` | `{"kind":"bag_grid"}`; a named instance needs the same `target`, e.g. `{"kind":"pet_toggle","target":"2"}` | `{"kind":"bag_grid","mark":[...],"artifact":{"file":..,"png_base64":..}\|null,"rev":10}` (a targeted fetch echoes `"target"`) |
| `keys.parse` | `{"names":{"start":"F12","quit":"F11",...}}` | `{"codes":{"start":123,...},"errors":[{"field":"pause","input":"Zulu","line":"…"}]}` |
| `keys.names` | `{"codes":{"start":123,...}}` | `{"names":{"start":"F12",...}}` |
| `attributes.get` | `{}` | `{"variant":"tw","dictionary":"attributes.yaml","attributes":{...}}` |
| `board.probe` | `{}` | `{"port":"COM5","baud":115200,"vid":"2341","pid":"8036","level":2}` |
| `board.test` | `{"kind":"click"}` / `{"kind":"move","dx":100,"dy":0}` / `{"kind":"key","key":"E"}` | `{"ok":true,"detail":"..."}` |
| `board.details` | `{}` | `{"baud":115200,"expected":{"vid":"2341","pids":["8036"],"port":null},"ports":[{"port":"COM5","vid":"2341","pid":"8036","product":"Arduino Leonardo","manufacturer":"Arduino LLC","matches_ids":true,"chosen":true}],"choice":{"state":"ready","port":"COM5","refusal":null},"firmware":null}` — READ-ONLY: enumerates serial ports and reports what a Start would use; never opens the port. `choice.state` is `ready`/`named_missing`/`not_found` and the core owns its `refusal` sentence; `firmware` is `null` (not checked yet — normal) or `{level, checked}` from the memory-only cache. Additive `v:1` |
| `pointer.place` | `{"x":1213,"y":1077}` | `{}` — closed-loop placement at a CLIENT-RELATIVE PHYSICAL point; never `SetCursorPos` |
| `pointer.click` | `{"x":1213,"y":1077}` | `{}` — place, then a bare left click; never clicks after a failed placement |
| `ui.window` | `{"left":100,"top":100,"width":900,"height":700,"pinned":false}` | `{}` |
| `pet.preflight` | `{}` | `{"ready":false,"calibration":[{"ok":true,"label":"Pet menu point","detail":"marked"}],"session":[{"ok":true,"label":"return slot","detail":"page 1, cell 6"}],"lines":["no pet icon queued — the tool will use the return slot instead of looking for the pet by icon"]}` — READ-ONLY: no board, window or OCR access; computes the Pet tab's Setup-so-far/Ready-to-run facts and v2's ready lines (ready is true only when no line fires AND calibration is complete); absent marks are facts with `ok:false`, not errors |
| `pet.capture_icon` | `{"kind":"queue"|"food","page":1,"cell":34}` (page/cell 1-BASED) | `{"kind":"queue","page":1,"cell":34,"rect":[10,40,10,10],"png_base64":"<standard base64 PNG>"}` — CAPTURE-ONLY: validates the input lease, the calibrated `pet.bag_grid` and the page's 2-point tab mark, then one foreground-gated client capture cropped in memory with the run's own BagGrid centre/pitch derivation; never clicks, moves the cursor, opens the board or writes logs. `rect` is client-relative PHYSICAL [x,y,w,h]. Errors: `bad_request` naming `kind`/`page`/`cell`/`bag_grid`, existing `window_not_found`/`game_not_foreground`, `tool_busy` with the holder's line (`detail.line`; `detail.command` names the refused command), `internal` for frame/encode bugs. The shell hides itself; the requested ITEM page must already be up — the shell does not click the tab today (OPEN §6 G22), and the core only validates the tab mark. No OCR/match here (scan increments) |
| `pet.scan_food_page` | `{"page":1}` | `{"page":1,"cells":[13,36],"report":["page 1: 2 cell(s) — 13, 36","  cell 13: 0.000 (limit 0.500)","  runner-up: cell 1 at 1.000"],"evidence":["logs/reads/scan_page1_2026....png","logs/reads/scan_2026....txt"]}` — READ-ONLY: input lease, one foreground-gated capture, the configured `pet.food_icon_png` matched against every bag cell with the existing v2 matcher; cells are 1-BASED in CELL order; evidence files are best-effort. Missing icon -> `bad_request` field `food_icon_png`; grid/page mark -> `bag_grid`/`page`; lease -> `tool_busy`; window/foreground as usual; no config write (read-only except the best-effort evidence above). The shell hides itself; the requested page must already be up (the shell does not click the ITEM tab today — OPEN §6 G22), and the core does NOT click or verify the tab, so a wrong page yields a plausible scan under the requested page |
| `pet.scan_queue_page` | `{"page":1}` | `{"page":1,"per_entry":[{"index":0,"label":"red","hits":[{"cell":13,"score":0.0,"runner_up":1.0}]}],"claimed":[],"deduced":["red ×1"],"evidence":["..."]}` — READ-ONLY, same policy; `index` is 0-based queue order, `hits[].cell` 1-based, `score` the matcher fraction, `runner_up` the second-best raw score (or null); `claimed` lists cells matched by more than one queued icon; no queue icons -> empty arrays, not an error; read-only except the best-effort evidence, and the page-up/wrong-page caveat is the same as the food scan (the core validates the tab mark but does not click or verify the page) |
| `pet.test_read_panel` | `{"page":1,"cell":34,"delay_ms":700}` — page/cell 1-BASED; `delay_ms` optional, default 700, bounded 0..5000 | `{"region":{"x":33,"y":88,"width":120,"height":80},"lines":["(6階) 真蔚藍鳳凰 +7 [52.18%]"],"panel":{"growth":7,"exp":52.18,"line":"stage 6, +7, 52.18%"},"numbers":["6","7","52.18"],"evidence":"logs/reads/petpanel_20261030_120000.png"}` — READ-ONLY diagnostic: input lease (`tool_busy` with the holder's line), validates `pet.bag_grid`, the page's tab mark and the calibrated `tooltip` BEFORE the board/window/OCR work (`bad_request` naming `bag_grid`/`page`/`tooltip`), loads the shared OCR pipeline (`ocr_unavailable` before any hover), then one board hover over the cell centre (closed-loop placement, **never a click**), waits `delay_ms`, one foreground-gated capture of the tooltip region (cell centre + calibrated offset/size) and OCR through the run's `PetPanel::parse`. `panel` is null when the panel is not fully understood; `lines` and `numbers` still report exactly what the recogniser produced, so a wrong region is distinguishable from an empty panel. `line` is the parser's own description (stage/growth/EXP/finished). `evidence` is the best-effort PNG of the exact captured region under `logs/reads/`, or null when the write failed; it never fails the read. No config write. The game must already be in front and the requested ITEM page up: like the scans, the core does NOT click the page tab or a focus point (OPEN §6 G22). Other errors: existing `window_not_found`/`game_not_foreground`/`board_unavailable`/`ocr_unavailable`/`internal` |
| `pet.apply_food_scan` | `{"cells":[[1,1],[1,34]]}` (1-BASED page/cell) | `{"rev":21,"block":{...merged pet block...}}` — THE explicit write: converts to the stored 0-based `pet.food_slots`, replaces them and resets `pet.food_slots_used` to 0 in ONE A1 field-precise write (never a whole-block apply), returns the merged block and emits exactly one `config.changed {block:"pet"}` AFTER the response. An EMPTY selection is `bad_request` field `cells` (nothing-found must not clear a live run's cells); malformed/out-of-range pairs, missing page-tab marks, a cell outside 1..64 are `bad_request` naming `cells` + index; an I/O failure answers `io` and leaves the file untouched |
| `pet.set_boarding` | `{"row":1,"running":true}` | `{"rev":21,"block":{...merged pet block...}}` — writes exactly `pet.slots[row-1].boarding_running` through the single writer (field-precise, not a whole-block apply), keeps the rest of `pet`/file/foreign blocks, bumps the session revision, emits exactly one `config.changed {block:"pet"}` AFTER the response, and returns the merged block. `row` is 1-based; bad/zero/non-bool `bad_request` naming the field; out-of-range row `bad_request` naming `row`; an I/O failure leaves the file untouched and answers `io` |
| `tool.start` | `{"tool":"spammer"}`, for `buy` `{"tool":"buy","preset":"大紅","count":3}`, for `sell` `{"tool":"sell","slots":[3,7,12]}` | `{}` |
| `tool.stop` | `{"tool":"spammer"}` | `{}` |
| `ocr.scan` | `{"frame_id":"f1","mode":"attributes"}` or `{"region":{...},"mode":"count"}` | `{"lines":[...],"attributes":[[...]],"grade":"G","remaining":null,"capture_path":"logs/captures/..."}` |
| `logs.cleanup_captures` | `{}` | `{"deleted":N,"dir":"logs/captures"}` — deletes only direct regular `*.png` files under `logs/captures`; a missing directory is `deleted: 0` (success); per-file failures are logged, never returned (§2's one explicit log-housekeeping exception) |
| `shutdown` | `{}` | `{}` then the process exits |

Notes:

- `capture` does **not** hide the shell's window — the shell hides itself, waits one frame, then
  asks. (The Windows runner serves a small `seal/host` `hideWindow`/`showWindow` channel for exactly
  this; the Dart side contains no Win32, and the show is unconditional — a refused capture must not
  leave the shell hidden.) The core still refuses to blit unless the game window is the foreground
  window
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
- `tool.start` follows the arbiter's one-game rules: a second FOREGROUND tool while one runs fails
  `tool_busy`; a RESIDENT start (`pet`) while a foreground tool runs is accepted (armed, not holding
  the game); and a foreground start while the resident feeder holds the game mid-visit is
  **DEFERRED** — the reply is still `{}`, a `log` line names the waiter, and the start happens when
  the feeder releases the game. A 45 s pending timeout publishes a `log` warning; the start then does
  not happen. Deferred is not an error and not `tool_busy`. `tool.stop` with no `tool` stops whatever
  is running. `holdspace` is a valid tool id (§9).
- **`tool.start` carries optional per-run arguments, and `buy` and `sell` are the two tools with
  any.** `buy`: `preset` (a string, a key of `buy_sell.presets`) and `count` (an integer, at least
  1) — a buy start without `preset` answers the domain's own `No buy preset selected.` line;
  without `count` it uses the preset's configured `count`; `count` below 1 is `bad_request` rather
  than silently clamped, because a clamp would buy a number the player did not ask for. `sell`:
  `slots` (an array of bag cell indexes `0..63`; order and duplicates are irrelevant — the core
  dedupes, sorts highest-first and the run's `buy_sell.sell_cap` still applies). **The selection
  changes between sells**, which is why it is a run argument and never a saved setting: v2 kept it
  in memory only, and persisting it risks selling the slots the player selected last time — a
  data-loss bug, not a convenience. `slots` is validated before the tool starts: a non-array, a
  non-integer, an out-of-range value or an empty array is `bad_request` naming the field, because
  an empty selection has nothing to sell and a shell that sends it has a bug. With `slots` absent
  the run has NO selection and the start answers the domain's own `No slots are selected to sell.`
  line. The config's `buy_sell.sell_slots` is deliberately NEVER read by a run: v2 never wrote it,
  v3.0.1 briefly did, and a stale value would silently sell the slots chosen last time — the
  data-loss path this ruling exists to prevent. The field stays in the typed config for migration
  compatibility and has no writer.
  Any run argument on a tool that does not take it is `bad_request` naming the offending field —
  a shell bug must not silently buy or sell something else. This is ruling §12.6: v2's Buy card
  and Buy/Sell tab picked these from the run, and `local.yaml` had no `active`-style field for
  either.
- **`pointer.place` / `pointer.click` take the input lease for the COMMAND only.** Both move or
  click at a client-relative PHYSICAL point through the board's closed-loop placement (never
  `SetCursorPos`), and both require integers >= 0. While a foreground tool is running or the
  resident feeder holds the game they answer `tool_busy` naming the holder — the same line a
  tool start gets — instead of steering the game behind a run. Session-wide holding/queueing
  (a calibration action that spans several commands) is a session increment's job, not faked here
  (ruling §12.9).
- **Some calibration kinds are NAMED INSTANCES, selected by `target`** (§12.8): `pet_page_tab`
  (`ITEM1`..`ITEM3`), `pet_toggle`/`pet_boarding_slot`/`pet_feeder_strip` (row `"1"`..`"4"`),
  `gem_grade` (`N`,`G`,`DG`,`Register`,`Combine`) and `gem_resource` (`"1"`..`"3"`). A kind that takes
  a target refuses a save/fetch without one; a kind that does not refuses a `target` naming the field.
  Targets are 1-based/player-facing strings in the protocol and map to 0-based arrays inside the core;
  a target that names an instance the config does not have is refused (the core never invents a row or
  a page tab). Every write preserves the sibling fields: a per-row mark sets exactly its own key and
  leaves the row's stacks, reference crops and its other marks alone.
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
`board.test`, `board.details`, `pointer.place`/`pointer.click`, `logs.cleanup_captures`,
`pet.preflight`/`pet.set_boarding`,
`pet.capture_icon`,
`pet.scan_food_page`/`pet.scan_queue_page`/`pet.test_read_panel`/`pet.apply_food_scan`,
`keys.parse`/`keys.names`, `attributes.get`, `calibration.save`/`calibration.fetch`, `ocr.scan`,
`tool.start`, `tool.stop`, `shutdown`. A command in this table is the contract; the implemented list
is the current fact, and nothing in §4 is spec-only today.

## 5. Events

| event | data | when |
|---|---|---|
| `state` | `{"tool":"pet","running":true,"phase":"running","message":"row 2: looking","countdown_s":null,"rows":[<row>, ...],"error":null}` — the row object is specified below | whenever the published state changes — the whole 750 ms-poll surface, pushed. A live tool's `message`/`countdown_s`/`rows` come from the tool's own `status()` (the runner publishes it when it changes, before performing the decision it belongs to), so a run's own words reach the card without the tool writing to the bus |
| `tool.finished` | `{"tool":"pet","reason":"done"|"cancelled"|"error"|"guard","message":"DONE — 4 row(s)"}` | a run ends; the message is the one v2 writes to its log |
| `config.changed` | `{"rev":11,"block":"pet","values":{...}}` | the core wrote a block the UI displays: `config.apply` (one event per written block, `settings` fans out to its inner blocks), `ui.window` (`ui`) and `calibration.save` (`buy_sell`/`gem`/`tuner`/`pet`), sent AFTER that command's response; or a tool's mid-run RUN-STATE write (`pet`, no request at all), sent immediately after the file write — the shell may receive it while idle. `config.reload` emits nothing — its answer already carries the whole config |
| `log` | `{"level":"info"|"warn"|"error","line":"..."}` | a line the UI may show in its status/log surface |
| `core.stopping` | `{"reason":"eof"|"shutdown"|"fatal"}` | last event before exit; `fatal` is produced only by replay drift (§13) — a normal session ends `eof` or `shutdown` |

The plan's `core.ready` event is folded into the `hello` result (ruling 1, §12): the handshake
already returns everything it carried, and a second message with the same content is a thing
that can disagree with the first.

`log` is the LIVE shell line; the run also records its own post-mortem in `<root>/logs/<tool>.log`
(§1) — start, changed tool status messages, and the final reason, with a stop written as
`stopped — …` and a normal finish as `finished — …`. The file is not repeated on the wire, and no
tool writes it: the supervisor's worker derives it from the status the runner already publishes.

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
| `ocr_unavailable` | the OCR pipeline cannot serve this read on this machine; `detail.reason` is one of `models_env_unset`, `models_dir_missing`, `model_load_failed`, `runtime_failed`, `text_fixes_invalid` (all failures of building the pipeline); `detail.message` carries the resolver's line naming every directory it tried, and for a load/runtime failure the model/file that failed (the player needs to know WHERE it looked and WHICH file). A failure while SCANNING an already-built pipeline (`bad_image`, `det_output_shape`) is a bad frame or a bug, not a machine that cannot serve OCR, and is reported as `internal` with the OCR message. Resolution order: `SEAL_OCR_MODELS` when set (it wins or fails — never silently ignored), then `<config root>/models`, then `<seal-core.exe directory>/models`; a directory counts only with all three model files. Shown like `board_unavailable`: the command exists but this machine cannot serve it |
| `tool_busy` | `tool.start` (or a pointer action) while another tool holds the game; `detail` names the holder |
| `not_running` | `tool.stop` when nothing runs (not an error the UI shows as a failure) |
| `io` | a file read/write or parse failed (config load, dictionary load, artifact write); `detail.path` says which path when one is known |
| `config_invalid` | the on-disk config is invalid; `detail.errors` carries the ported validator's player-facing lines (a boot-time failure uses the stderr channel in §1, not this code) |
| `internal` | a bug; `message` is safe to show, `detail` goes to the log |

## 7. Config merge semantics

`local.yaml` is machine state the CORE alone writes and merges. The only writers are `config.apply`
(the owned blocks plus the portable `settings:` projection), `ui.window` (`ui`) and
`calibration.save` (the block it marks, plus its binary artifact); tool run state lives in memory and
is not persisted mid-run (v2's pet `ApplySession` half is not carried over).

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
- Core-initiated writes emit `config.changed {rev, block, values}` AFTER their own response
  (`config.apply`, `ui.window`, `calibration.save`) — a shell that is not editing that block applies
  it silently; one that is editing shows the conflict.
- **Run-state writes have no request.** The Pet Feeder persists its own machine state mid-run
  (`pet.food_slots_used`, `pet.slots[i].boarding_running`, and the close-guard pair
  `pet.close_button_png`/`pet.boarding_window_may_be_open`) through the same single writer; each
  successful write bumps `rev` and emits `config.changed {rev, block:"pet", values}` immediately,
  because there is no response to follow. The shell may receive it while idle. Only those fields are
  touched: the rest of `pet`, the rest of the file and its foreign blocks are preserved; a malformed
  config or I/O failure writes nothing and logs to `error.log` — persisting state is a safety net, it
  never fails the run. A shell `config.apply` carrying the pre-write rev gets `conflict` and the
  current rev; a dirty draft shows the external-change banner.
- `ui:` is fully core-owned; it is written only through `ui.window`.
- Hand edits to the file are picked up at startup or by `config.reload`.
- `config.apply` commits both halves — the owned blocks and the portable projection — in ONE file
  update, so a failed apply writes nothing and consumes no `rev` (seal-config's composed writer,
  `46df1ee`).

## 8. Images and the frame cache

Frames are PNG, base64 in JSON, for v1. At a 2865×1789 client a frame is ~1–3 MB, ~1.3× that as
base64; calibration is human-paced, so this is fine. Raw BGRA framing is a later optimisation,
measured before adopted.

The core keeps the last **four** frames by `frame_id` (FIFO, `win::FrameCache`). `calibration.save` references the
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

## 11. The step-1 measurements

**Resolved since this list was written:** item 2 — the DPI/frame→physical-offset conversion is proved
on this machine at 150 % and is what every placement now uses; `DisplayInfo` did not need a new field.
Item 5 — the hotkey typed-name rule LANDED as `keys.parse`/`keys.names` (§12.5), so the Hotkeys tab no
longer needs an exception to "no widget keeps a rule".
**Still open:** item 1 (hotkeys — v3.0 ships WITHOUT a global poller; PLAN §6.5's measurement decides
whether polling lives in the core or in the shell, and until it is taken the Stop button is the control
that always works: OPEN), item 3 (frame size/transfer — PNG+base64 is retained and the four-frame cache
exists; no measurement has superseded it), item 4 (IME — the shell's name fields are free text today;
the .NET 10 composition regression is deferred with the v2 bump per PLAN §10 step 0).

1. **Hotkeys** — whether the core (a background process) can read keys while the launcher is
   focused, or whether polling must live in the shell and send `hotkey.down`/`hotkey.up` commands.
   The measurement decides whether this protocol grows two commands (PLAN-V3-CORE §6.5).
2. **DPI mapping** — the frame→physical-offset conversion is proven on this machine at 150%; if it
   is not exact, `DisplayInfo` grows the field the conversion needs rather than the shell deriving
   it from Win32.
3. **Frame size** — measure a real capture's size and transfer time before deciding PNG stays.
4. **IME** — not a protocol item, but it decides whether name fields are free text (PLAN-V3-CORE §13.2); if
   they are not, no protocol change is needed.
5. **RESOLVED — Hotkey typed-name validation.** The rule landed behind the protocol as
   `keys.parse`/`keys.names` (§12.5): names in, codes or player-facing error lines out, so the
   Hotkeys tab has no exception to "no widget keeps a rule" any more.

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
5. **`keys.parse`/`keys.names` exist because the config stores VK CODES while the Hotkeys tab
   edits NAMES.** v2 kept both directions inside `MainWindow` (`ParseVk`/`VkName`); v3 puts them
   behind the protocol so the typed-name rule stays in `seal-domain` (the same reason `config.validate`
   deliberately does NOT validate stored codes — v2 accepted any code, `GetAsyncKeyState` takes any
   code, and refusing one would refuse a config v2 ran). The commands answer every parseable field
   plus one line per bad box rather than a bare refusal, because a four-box save with one typo must
   name that box.

6. **`tool.start` gained optional run arguments for `buy` and `sell`**, because both tools'
   parameters are genuinely per-run and v2 had no config field to persist them: the Buy card chose
   the item and count (`ShopPlan::buy` took both from the run), and the Buy/Sell tab's sell-slot
   selection changed between sells (v2 kept it in memory only). The alternatives were worse: a
   `buy_sell.active`-style field would persist a choice v2 never persisted and make the cards config
   editors over settings no tool reads mid-run; the sell case is stronger still, because persisting
   the selection risks selling the slots chosen last time (a data-loss bug, not a convenience); a
   special `tool.start_buy`/`start_sell` command would duplicate the start lifecycle
   (busy/deferred/stopping) for two tools. The ruling therefore keeps the lifecycle in one command
   and makes the run arguments explicit: `preset`/`count` (buy) and `slots` (sell), each validated
   before the tool starts, wrong on any other tool (`bad_request` naming the field), with the
   domain's own refusal line when a required selection is missing. `buy_sell.sell_slots` stays in
   the typed config for migration compatibility but NO run reads it; a sell run is exactly the
   shell's `slots` or nothing. This is a DRAFT-shape change (the spec was v2); the golden fixtures
   cover the envelope, not per-command argument semantics, and the argument rules are covered by
   core tests on both sides of the seam.

7. **The calibration kind table was repaired, not extended silently.** Three findings from the
   calibration gap audit changed the spec: (a) the `focus_point` kind wrote `buy_sell.focus_point`,
   which no model field or consumer reads; the live shop reads `buy_sell.scroll_point` (v2's
   `ScrollPoint`), so the kind is **`scroll_point`** and writes that path — no second focus field was
   invented. (b) `max_button` was a mark CALIBRATION.md described but no kind wrote; it is now a kind
   (a point on `buy_sell.max_button`), shared by buy/sell and the pet feeder's readiness rule. (c)
   `result_gem_area` lost v2's empty verdict: v3 saved whatever was in the box as the "empty"
   reference, so calibrating on a busy box poisoned the strong test. The save now carries
   `empty: true|false`; `true` saves the crop and samples `empty_signature`, `false` samples the
   signature only. Two items were **withdrawn rather than faked**: `tuner_region` (v2 derived the
   region from three dragged boxes; a single rect wrote an unusable shape) until the three-box
   session exists, and CALIBRATION.md's invented "Quest step marks" row (both codebases' `QuestStep`
   carries only action/value/delay). Pet and gem point marks were the reason `calibration.save`/`fetch` grew the optional `target` —
LANDED as §12.8, not left as a silent omission.

8. **Calibration kinds gained an optional `target`, because v2's marks are named instances.** The
   calibration audit's increment 2 is the pet's marks and the gem's points; their config shapes are
   `page_tabs[0..2]`, `slots[i].{toggle_label, boarding_pet_slot, feeder_strip}`,
   `grade_positions[name]` and `resource_gems[0..2]`. Fifteen fixed kind names would encode the same
   information twice (kind and index), so the protocol keeps one kind per semantic mark plus an
   optional **`target`** string: 1-based and player-facing on the wire (`"1"`..`"4"` rows,
   `ITEM1`..`ITEM3` page tabs, `"1"`..`"3"` resources), 0-based arrays inside the core, and gem grade
   points keep v2's own names (`N`/`G`/`DG`/`Register`/`Combine`). Rules: a targeted kind requires a
   target; a non-targeted kind refuses one naming the field; an unknown target is refused with the
   allowed values in the line; a target naming an instance the config does not carry is refused rather
   than invented (rows and page tabs come from the config until the Pet-tab editor exists). A
   per-instance write sets exactly its own key and preserves the siblings — `slots[i]` keeps its
   `stacks`, crops and its other two marks, and row order is untouched. This is why the pet's crops,
   the food cells/queue and the sell-slot selection are NOT calibration kinds: they are Pet-tab/Sell-tab
   features whose home is a live-screen session, not a frame mark (OPEN).

9. **`pointer.place`/`pointer.click` exist for the calibration sessions' tests and are deliberately
   per-command.** v2's Calibrate Gem "Place cursor + click" and Calibrate Tuner "Test Click" need
   to steer the cursor without running a tool; the pointer commands are that primitive. They take
   the same input lease a tool takes (refuse `tool_busy` naming the holder while a foreground tool
   runs or the resident feeder holds the game) but hold it only for the command: a calibration
   action that spans several commands is a session increment and gets its own ruling when it lands,
   rather than being half-faked behind a different command. They steer only through the board's
   closed-loop placement — v3 forbids `SetCursorPos` (CALIBRATION.md §2).

## 13. Replay mode (development)

`seal-core --replay <script.json>` serves the SAME stdio protocol from a script instead of from the
machine: no config file is loaded, no board opened, no window measured, no frame captured, no OCR
model run. It is the plan §8's replay core — the basis for widget and golden tests and for
developing the shell without the game. It is a development mode of the SAME binary and the SAME
protocol; it adds no command, event or field, and a release build that never receives the flag
behaves exactly as before.

- **Script.** One JSON object: `{"protocol": 1, "turns": [<turn>, ...]}`. Each turn is
  `{"expect": {"cmd": "hello"}, "response": <body>, "events": [<event>, ...], "delay_ms": 0}`, every
  field but `response` optional. `expect` is a matcher on the incoming request (`cmd` required if
  present; other keys compared shallowly against the request's `args`). `response` is a BODY —
  `{"result": ...}` answers `ok`, `{"error": {"code": ..., "message": ..., "detail": ...}}` answers
  that error, and an omitted/empty body answers `{"result": {}}`; the core supplies `v` and the
  incoming request's `id`, so a script never invents an id. `events` are emitted verbatim, in order,
  AFTER the response, which is how a script can drive a card through `state`/`tool.finished`/`log`
  sequences. `delay_ms` (0..=10_000) delays before the response, so a demo can be watched.
- **Matching.** One turn answers one request line, in order. If `expect` does not match, the core
  answers that request with `internal` (the line names expected vs actual), emits
  `core.stopping {"reason":"fatal"}`, writes the reason to stderr, and exits non-zero — a script that
  drifts from the shell must fail loudly, never mislead.
- **A scripted `shutdown` turn ends the session like a normal one**: its response is written, its
  events follow, and the core exits 0 — the script supplies the `core.stopping {"reason":"shutdown"}`
  event, and no `eof` event is synthesised after it. Every other script ends on EOF.
- **Lifecycle.** EOF ends the loop like a normal session (`core.stopping {"reason":"eof"}`, exit 0);
  the script running out of turns while requests keep arriving is a drift, handled as above. A
  missing/unreadable/malformed script exits non-zero with the reason on stderr and nothing on stdout
  (the boot-refusal rule of §1).
- **When both flags are present**, `--replay` wins and `--root` is ignored; the replay mode never
  writes `local.yaml`, `logs/` or a calibration artifact. A committed example script lives in
  `v3/protocol/replay/` and the core's e2e test drives the binary through it.

**Status.** Implemented (`4d4ab1f`), with the committed example script `v3/protocol/replay/smoke.json`
and ten e2e tests that drive the real binary. It adds no command, event or field.
