# Protocol — v3 core ⇄ shell

**Status: DRAFT v1, written 2026-10-25 before the walking skeleton exists.** This is the step-1
deliverable named in [PLAN-V3-CORE.md](../../../v2/docs/plans/PLAN-V3-CORE.md) §10; the step-1
spikes (DPI mapping, IME, hotkey location) may change it. Nothing here is implemented yet.

The design rationale is PLAN-V3-CORE §4. This file is the contract; both halves are written
against it, and the golden fixtures in §10 are how they are kept in agreement.

---

## 1. Transport and lifecycle

- The shell (`SealTools.exe`) spawns the core (`seal-core.exe`) from its own directory, no
  arguments.
- Messages are **UTF-8 newline-delimited JSON**, one object per line, on the core's stdin and
  stdout. stderr carries logs and panics only, never protocol messages.
- The core discovers its root the way v2 does: walk up from the executable for
  `config/defaults.yaml`, else `config/defaults.yaml.example`.
- **Single instance:** the core holds an exclusive OS file lock on
  `<root>/config/.seal-core.lock`. A second core exits with code `2` and one stderr line; the
  shell shows it. The lock is released by the OS when the process dies, so a crash leaves no
  stale lock to clear by hand.
- **Handshake:** the shell's first message is `hello`. Any request before a successful handshake
  fails with `not_ready`. A protocol-version mismatch fails `hello` with `bad_version`; the
  shell shows both versions rather than guessing.
- **Shutdown:** `shutdown` asks the core to exit cleanly. EOF on stdin does the same implicitly:
  cancel the current tool through its normal stop path ("abandon the work, not the cleanup"),
  release held keys and buttons through the board guard, flush logs, exit `0`.
- **Core crash:** the shell keeps its last rendered state, shows the failure and the captured
  stderr tail, and offers "restart core" — a new process and a new handshake. The tools' real
  state is the game and the board, so a restart loses nothing that was not already lost.

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
{"v":1,"event":"state","data":{"tool":"pet","running":true,"phase":"visiting","message":"..."}}
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
| `config.apply` | `{"rev":7,"sections":{"quest":{...}}}` | `{"rev":8,"sections":{"quest":{...}}}` |
| `config.reload` | `{}` | `{"rev":9,"config":{...}}` |
| `window.find` | `{}` | `{"found":true,"title":"Seal Online ..."}` |
| `window.measure` | `{}` | `{"display":{...}}` |
| `capture` | `{"region":{"x":..,"y":..,"w":..,"h":..}}` | `{"frame":{...},"display":{...}}` |
| `calibration.save` | `{"frame_id":"f1","region":{...},"kind":"bag"}` | `{"rev":10,"derived":{...}}` |
| `calibration.fetch` | `{"kind":"bag"}` | `{"frame":{...}}` |
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
- `board.probe` reports the firmware level from `V` (absent = level 1); `board.test` is the
  calibrator's Test Click / movement button, and its result carries the detail string v2 logs.
- `ui.window` is the only way window geometry and pin state reach `local.yaml`; the core is the
  single writer (§7).
- `tool.start` on a second tool while one runs fails with `tool_busy`; `tool.stop` with no `tool`
  stops whatever is running. `holdspace` is a valid tool id (§9).

## 5. Events

| event | data | when |
|---|---|---|
| `state` | `{"tool":"pet","running":true,"phase":"visit","message":"row 2: looking","countdown_s":null,"rows":[{...}],"error":null}` | whenever the published state changes — the whole 750 ms-poll surface, pushed |
| `tool.finished` | `{"tool":"pet","reason":"done"|"cancelled"|"error"|"guard","message":"DONE — 4 row(s)"}` | a run ends; the message is the one v2 writes to its log |
| `config.changed` | `{"rev":11,"block":"pet","values":{...}}` | the core wrote a block the UI displays (pet session state, calibration, `ui:`) |
| `log` | `{"level":"info"|"warn"|"error","line":"..."}` | a line the UI may show in its status/log surface |
| `core.stopping` | `{"reason":"eof"|"shutdown"|"fatal"}` | last event before exit |

The plan's `core.ready` event is folded into the `hello` result (ruling 1, §12): the handshake
already returns everything it carried, and a second message with the same content is a thing
that can disagree with the first.

## 6. Errors

```jsonc
{"code":"conflict","message":"config changed while you were editing","detail":{"rev":12,"sections":{"quest":{...}}}}
```

| code | meaning |
|---|---|
| `bad_request` | malformed envelope, unknown cmd, bad args |
| `bad_version` | protocol version mismatch at `hello` |
| `not_ready` | request before a successful handshake |
| `conflict` | `config.apply` with a stale `rev`; `detail` carries the current `rev` and the conflicting sections |
| `window_not_found` | the game window is not present |
| `game_not_foreground` | capture/placement refused because another window is in front |
| `board_unavailable` | no board on the configured port / port unusable |
| `tool_busy` | `tool.start` while another tool runs |
| `not_running` | `tool.stop` when nothing runs (not an error the UI shows as a failure) |
| `io` | a file write failed; `detail.path` says which |
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
- A stale `rev` is refused with `conflict`; the shell reconciles and tells the player what changed
  underneath before saving again.
- Core-initiated writes emit `config.changed {rev, block, values}`. A shell that is not editing
  that block applies it silently; one that is editing shows the conflict.
- `ui:` is fully core-owned; it is written only through `ui.window`.
- Hand edits to the file are picked up at startup or by `config.reload`.

## 8. Images and the frame cache

Frames are PNG, base64 in JSON, for v1. At a 2865×1789 client a frame is ~1–3 MB, ~1.3× that as
base64; calibration is human-paced, so this is fine. Raw BGRA framing is a later optimisation,
measured before adopted.

The core keeps the last **four** frames by `frame_id` (FIFO). `calibration.save` references the
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
- A Rust test replays a recorded command fixture list against the dispatcher with a fake core
  state, asserting the sequence is accepted.

## 11. Open items for the step-1 spikes

1. **Hotkeys** — whether the core (a background process) can read keys while the launcher is
   focused, or whether polling must live in the shell and send `hotkey.down`/`hotkey.up` commands.
   The measurement decides whether this protocol grows two commands (PLAN-V3-CORE §6.5).
2. **DPI mapping** — the frame→physical-offset conversion is proven on this machine at 150%; if it
   is not exact, `DisplayInfo` grows the field the conversion needs rather than the shell deriving
   it from Win32.
3. **Frame size** — measure a real capture's size and transfer time before deciding PNG stays.
4. **IME** — not a protocol item, but it decides whether name fields are free text (§13.2); if
   they are not, no protocol change is needed.

## 12. Rulings against the plan (recorded, not silent)

1. **`core.ready` folded into the `hello` result** — one source of truth; a second message that can
   disagree with the first is a defect waiting to happen.
2. **`config.apply` returns the merged sections, not the whole config** — the shell only owns a
   few blocks; returning the file would invite it to keep a whole-file copy, which §7 forbids.
3. **A frame cache with `frame_id`** — added so calibration does not ship megabytes twice; not in
   the plan, small surface.
