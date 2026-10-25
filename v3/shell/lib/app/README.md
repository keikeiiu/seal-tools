# The shell's state layer

The one app-wide model and the contracts the tab surfaces are built from. It owns the
[protocol client](../protocol/README.md) and speaks the protocol; it imports **no widgets**. The UI
renders these models; where a decision is a UI convenience rather than protocol bookkeeping, the code
says so at the point of judgement.

## `ShellModel` — the one `ChangeNotifier`

| sub-model | owns | fed by |
|---|---|---|
| `ConnectionModel` | lifecycle (`idle/starting/handshaking/ready/failed`), typed failure, core version/root, stderr tail, crash count | spawn/handshake results, the client's event-stream close |
| `ToolsModel` | the eight cards' last `ToolSnapshot`, per-tool + last `ToolFinished`, `core.stopping`, refusals, in-flight flags | `state` / `tool.finished` / `core.stopping` events |
| `ConfigModel` | merged config mirror, session `rev`, validate/apply/reload results, last error, conflict | `hello`, config command answers, `config.changed` events |
| `LogBuffer` | bounded ring of `log` events + stderr + shell notes, with origin | client event/log streams |

The model exposes the commands widgets call (`startTool`/`stopTool`, `saveConfig` (validate → apply),
`validateConfig`/`applyConfig`/`reloadConfig`, `saveWindow`, `findWindow`/`measureWindow`/`probeBoard`)
and a `transportFactory` seam: `main.dart` passes the real process spawner, tests a fake.
`connect()` is single-flight and renders failures instead of throwing; `restart()` is one new process;
`shutdown()` sends `shutdown` then EOF.

## Drafts — `ConfigDraft`, `ConfigDraftGroup`, `ConfigScope`

A tab never edits the mirror. It edits a **draft** seeded from the mirror and filtered to the keys the
tab owns:

- `ConfigScope.ownedBlock` → payload `{block: value}` (Spammer, Buy/Sell, Quest, `arduino.port`);
- `ConfigScope.portableSection` → payload `{settings: {block: value}}` (Tuner, Gem, Pet, Setup's
  window/game/arduino ids — the merged mirror flattens these, but the overlay is where they are saved).

Dirty = value ≠ baseline (JSON equality); `reset()` is Cancel; `saved(echo)` re-baselines from the
apply echo; `seedRevision` bumps on every external re-seed so `DraftFieldSlot` can recreate field state
and never show a stale `TextEditingController`; `hasExternalChange` marks a core-side change that
arrived while the draft was dirty.

`ConfigDraftGroup` fans out over several drafts when one tab owns several sections (Setup). Dirty /
external are "any part"; one Save builds one merged `sections` payload; a successful apply re-baselines
each part from its own echo.

## `ConfigTabState` — the shared tab frame

`config_tab_scaffold.dart` provides the mixin + `ConfigTabScaffold`: header + scrolling editor + Save
bar, dirty marker, Cancel, validate/apply order, the core's validation/refusal lines **verbatim**, the
conflict banner (core rev + changed names + explicit reconcile), and the external-change banner for
kept-alive tabs. Tabs mix in `ConfigTabState` + `AutomaticKeepAliveClientMixin` (so drafts and banners
survive tab switches) and implement `model` + `draft`/`draftGroup`.

## Action results — `CoreActionResult<T>`

Read-only commands (`window.find`, `window.measure`, `board.probe`, and the app-local `attributes.get`
decode) return `CoreActionResult<T>`: either the decoded `value` or the core's typed `ErrorObject`
(plus a shell-side `note` when the command could not be sent). Widgets render `errorLine` verbatim;
nothing invents a refusal.

## Related

- `core_path.dart` — release-beside-the-shell vs development `v3/core/target/debug` executable and the
  `--root` discovery (`SEAL_CORE`/`SEAL_CORE_ROOT` dart-defines override).
- `core_actions.dart` — the typed action result shapes (`WindowFindResult`, `BoardProbeResult`).
- `config_json.dart` — pure clone/equality/coercion helpers used by the drafts (headless-testable).
