# Seal Tools v2 — Fix Execution Status (branch `v2-saving-attempt`)

Final status of the incremental fixes. Each item is one commit, pushed to `v2-saving-attempt`
(not `main`). Build clean (`0 warnings / 0 errors`) and 3/3 tests pass at every step.

## Done (one commit each)

| # | Commit | Fix |
|---|--------|-----|
| 1 | `0e0880a` | `GetClientRectInScreen` uses client-area origin (`GetClientRect`+`ClientToScreen`) not the frame |
| 2 | `a8b05cb` | `StopTool` disposes the CTS only after the tool task exits (no more `ObjectDisposedException`) |
| 3 | `94f935b` | `ToolState.Attributes` is an immutable snapshot (thread-safety) |
| 4 | `2b2c427` | Arduino boot delay made non-blocking (`ArduinoPortAsync`/`StartToolAsync`) — no 2 s UI freeze |
| 5 | `ae67be7` | `AttrMatcher.CheckFilter` no longer drops same-named attrs in any/all mode |
| 6 | `19cde7e` | Spammer rejects unsupported keys (only digits 0–9 + F1–F10) instead of silently pressing `'0'` |
| 7 | `41f9f2e` | Removed dead DPI helpers + orphaned Win32 P/Invokes (`EnablePerMonitorDpiAwareness`, `LogDpiAwareness`, `DebugCursorCalc`, `BringToForeground`, `IsForeground`, …) |
| 8 | `2d4e497` | Removed dead `DpiScale` + `MouseScale` config (loader plumbing, example, test assertions) |
| 9 | `628645d` | Removed dead `DgWhiteRatio`/`GWhiteRatio` + `Slot2Dg` config |
| 10 | `e53dde9` | Extracted shared `SleepCheck`/`Beep`/`BeepMany` into `SealTools.Core.ToolBase` |
| 11 | `66a40d5` | Restored `BeepMany` inter-beep gap (matches v1) |
| 12 | `bcc5df3` | Renamed `sameCount` → `consecutiveRepeats` (clarity) |
| 13 | `05b306b` | `[AllowedValues]` on `EmptyMode`/`MatchMode` — fail loudly on a misspelled value |

## Deferred (not done — with reasons)

- **`GemColorAnalyzer.ColoredGapMin = 40`** — the one hardcoded threshold. Left as-is because it is a
  fundamental pixel-classification parameter that the live-sampled `EmptySignature` already
  self-calibrates; exposing it as config adds tuning surface with little value, and it's ambiguous
  whether it belongs in `defaults.yaml` (portable) or `local.yaml` (machine-specific).
- **OCR row-bucket pooling** — needs measured evidence first (real unconfirmed frames), not a guess.
- **Machine-specific debug-cursor buttons** (`DebugCursor(727,696)` / `DebugPhysicalCursor(1096,1044)`)
  — diagnostic value vs cruft; leave unless you no longer need the DPI debugging.
- **`ReferenceWindowConfig`** — documented in CALIBRATION.md as a planned "auto-anchor" feature but
  not implemented (write-only). Removing it means editing the docs too; left for a decision.
- **v1 (Python) duplication** — v1 is frozen; out of scope.

## Not done (corrected from the original report)

- There is **no** hardcoded `2.0/3.0` DPI scale in the code (an earlier report was wrong).
- `GemColorAnalyzer.Analyze(IntPtr, …)` is **used** (MainWindow calibrator), not dead.
- `local.yaml` is gitignored; its corruption is a local-only repair (delete it — it re-seeds from
  `local.yaml.example`).
