# Seal Tools v2 — Fix Execution Status (branch `v2-saving-attempt`)

> **v2.3 (2026-09-11).** The launcher adopts WPF-UI properly — card-based tabs, a collapsible
> configuration region, pin-on-top and a mini view that shrinks to the running tool's card — plus a
> placement that is remembered and two real bugs fixed (see Pass 5).
>
> **v2.2 (2026-09-10).** Cursor positioning moved off `SetCursorPos` onto the Arduino, the composer
> gained a second move set (calibrated points, closed loop), empty-result detection became a pixel
> comparison, and a run ends after the last grade. All of it verified on a live game.
>
> **v2.1 (2026-09-09).** The state described here is tagged `v2.1`: capture, cursor, tuner OCR and
> the composer verified on a live game, plus spammer presets and the Setup tab.

Current state of the fix work. Every row is **one commit**. Build is clean (`0 warnings / 0 errors`)
and all tests pass at every step. Nothing is pushed; the branch is ahead of `origin/v2-saving-attempt`.

For the original design review and the guardrails that must not be reversed, see
[REVIEW.md](REVIEW.md). For the config file split, see [CONFIG.md](CONFIG.md). For per-machine
calibration steps, see [CALIBRATION.md](CALIBRATION.md).

---

## Pass 5 — WPF-UI adoption and the launcher shell (v2.3, latest)

Branch `v2-ui-cleanup`, merged to `main` and tagged `v2.3`. Design and the per-tab checklist:
[PLAN-UI-CLEANUP.md](PLAN-UI-CLEANUP.md).

| # | Commit | Change |
|---|--------|--------|
| 1 | `43c08f7` | Palette: the eight hex brushes deleted, the UI uses WPF-UI's semantic theme brushes |
| 2–3 | `ef0d649` `97fbd57` | Hotkeys and Arduino rebuilt on `Card` + `ui:` controls; `Res`/`Hint`/`Mono`/`UiText`/`Section` helpers |
| 4–7 | `fd06f7d` `141da64` `ebfe803` `8d16e28` | Setup, Gem, Spammer (key grid with one header), Attributes |
| 8–9 | `e53a040` `39768c0` | Tuner, Calibrate Tuner |
| 10 | `dba4c67` | Calibrate Gem split into Capture/Points/Coordinates/Tests/Moves×2/Full-run/Advanced/Result, with the button renames |
| 11 | `415b135` | Config tabs behind a `▸ Configuration` chevron; the window opens as the tool cards |
| 12 | `2a9527d` | Pin on top; placement and size remembered in `local.yaml` |
| 13 | `d2437b2` | Mini mode: while a tool runs, only its card is shown |
| 14 | `73258af` | Placement committed automatically ~0.7 s after a move/resize |
| — | `025e801` | **fix**: the empty check refuses to judge a screen grab that isn't the game |
| — | `25bdde5` | **fix**: the empty check compares only the box's interior, so a moved window can't stall the composer |
| — | `e3f374b` | **fix**: the move-set selector no longer lives inside the card it disables |

**Status 2026-09-11:** verified live — pin and placement survive a relaunch (moved to logical
`(1927,3)`, reopened there at `619×430`, pinned), mini mode drops the window to the running tool's
card, and a full composer run advanced correctly with the inset fix (gem frames `0.68–0.82`, empty
`0.000`).

**Still open:** the v2.3 zip is not built or published; the robustness list and the tuner spring plan
are in [IDEAS.md](IDEAS.md) and [PLAN-TUNER-SPRING.md](PLAN-TUNER-SPRING.md); the tuner's OCR
row-bucket question ([TODO.md](TODO.md)) still needs evidence.

## Pass 4 — Arduino cursor, move sets, empty check (v2.2)

Branch `v2-arduino-moves`. Detail: [CURSOR-INVESTIGATION.md](CURSOR-INVESTIGATION.md),
[MOVE-SETS.md](MOVE-SETS.md), [PROGRESS.md](PROGRESS.md).

| # | Commit | Change |
|---|--------|--------|
| 1 | `624fddd` | The `arduino` move set: `GemRoutes` (each route ends on a calibrated point) + `gem.move_mode` |
| 2 | `2e0b043` | Cursor positioned by the Arduino in a closed loop; a failed placement stops the tool instead of clicking blind; the port hypothesis refuted |
| 3 | `3ca2995` | `PROGRESS.md` — dated log of what was done and why |
| 4 | `ac0f8ec` | **Test Full Cycle (Arduino)** button in Calibrate Gem |
| 5 | `accaf90` | Progress entry |
| 6 | `0afdc46` | Composer switched to `gem.move_mode: arduino` |
| 7 | `f4b5f02` | Empty-result detection by pixel difference vs the saved empty crop; signature sampled from the launcher-hidden screenshot; `Distance` → Euclidean |
| 8 | `9fc03d6` | Progress entry |
| 9 | `6a28ecf` | A run ends after the last grade instead of wrapping to the first |
| 10 | `25db00a` | Progress entry |

**Status 2026-09-10:** verified on a live game — the composer runs N → G → DG in `arduino` mode,
combining each grade until the result box reads empty (`diff 0.44–0.54` with a gem, `0.000` empty,
gate `0.01`), and stops after DG.

**Still open:** why `SetCursorPos` is refused in this process (no longer load-bearing — nothing calls
it), and the tuner's OCR row-bucket question ([TODO.md](TODO.md)).

## Pass 3 — physical coordinate space

Fixing the capture crop led to reworking the coordinate model. Design and measured evidence:
[docs/COORDINATES.md](COORDINATES.md).

| # | Commit | Change |
|---|--------|--------|
| 1 | `a9d296c` | Decision record: the measurements, why the process stays DPI-unaware, the scale formula, what must never be scaled |
| 2 | `6912805` | `Dpi.Measure` — thread-scoped per-monitor-aware peek; verified live (scale 1.5, physical 2865×1789 vs logical 1910×1193) |
| 3 | `f7e0742` | Document the cursor positioning path against v1's working code |
| 4 | `03e4035` | Capture in physical pixels (`CaptureClient` / `CaptureClientRegion` on the aware thread) |
| 5 | `ad88273` | `calibration:` block in `local.yaml` (dpi_scale, monitor_dpi, screen, client_size, measured_at) + round-trip test |
| 6 | `87f34c6` | **Setup** tab: Detect / editable scale + client size / Save / client-size mismatch warning |
| 7 | `61d6624` | Cursor positioned from physical client-relative coordinates (`SetPhysicalCursorPos`, scale-divided fallback) |

**Required after this pass:** recalibrate once (Tuner + Gem). Coordinates written before it are
logical and wrong in the physical space; `gem.movements` (HID counts) are unaffected.

**Status 2026-09-09:** done and verified on the live game — capture, cursor path (`SetCursorPos`
logical), composer Test Click / Test Move, and Check OCR (in-session and saved-geometry). The
one-off debugging aids added while chasing this (`Diagnose capture`, the full-frame OCR dump, the two
Debug cursor buttons) are kept as diagnostics. Remaining live checks are in [TODO.md](TODO.md).

## Pass 2 — review fixes

| # | Commit | Fix |
|---|--------|-----|
| 1 | `647ae69` | `SaveDefaults` kept dropping `ocr_retries` on every launcher Save (3 → 0, silently disabling the OCR re-scan) + round-trip regression test |
| 2 | `ca2305b` | Start/run failures are visible: `LastArduinoError` + MessageBox + `ToolState.Message` on the card (the app has no console) |
| 3 | `b49d77d` | `StartToolAsync` awaits the cancelled tool before starting a new one — two tools can no longer interleave bytes on the shared serial port |
| 4 | `6f742fb` | Malformed OCR geometry fails validation with a pointed message instead of `IndexOutOfRange` / an OpenCV exception; colour crop clamped |
| 5 | `7d366e0` | `CALIBRATION.md` rewritten for the WPF calibrator; README's auto-anchor / `mouse_scale` / DPI claims corrected |
| 6 | `784670f` | `publish.bat` no longer copies a nonexistent `launcher.html` or prints v1 instructions |
| 7 | `3626998` | `CONFIG.md`: removed `display.dpi_scale`, fixed the wrong save-path description, dropped an unverified Admin claim |
| 8 | `d6992eb` | `App.xaml.cs` comment no longer claims it enables DPI awareness |
| 9 | `e2c6963` | `local.yaml.corrupt-backup` gitignored (kept as a backup, not deleted) |
| 10 | `740d091` | No per-move cursor-file writes; `DebugClickLog` before/after titles now real; dead `_gemTestFrom` / `altPath` removed |
| 11 | `7f5a221` | `ColoredGapMin` (the last hardcoded threshold) moved to `gem.colored_gap_min` in config |
| 12 | `8fbbf10` | Removed the redundant gem focus-click — the single click both focuses the game and presses the button |
| 13 | `8472ae8` | Documented the gem focus lifecycle precisely (unfocused at start → first click focuses + presses) |
| 14 | `94a7fd3` | Missing gem calibration now reports on the card ("Move N → Register isn't saved yet…") instead of throwing |
| 15 | `1cc1e8b` | Added the "Diagnose capture" button + `WindowFinder.GetFrameRect` — the evidence-gathering step |
| 16 | `795d13e` | Launcher diagnostics written under the app root (`logs/`), not the build output dir |
| 17 | `94129b7` | Capture switched to `CopyFromScreen`; `PrintWindow` removed — it returned a **black frame** for the game, so the calibrator screenshot was blank |

### Local-only repair (not a commit)

`v2/config/local.yaml` was corrupted (a 51-byte README fragment) and blocked startup. It was renamed
to `local.yaml.corrupt-backup` and the machine calibration was copied from
`v2/dist/SealTools-v2/config/local.yaml`. **`v2/dist/` is read-only — never modify it**; it holds the
only intact copy of the calibration.

---

## Pass 1 — earlier fixes

| # | Commit | Fix |
|---|--------|-----|
| 1 | `0e0880a` | `GetClientRectInScreen` uses the client-area origin (`GetClientRect`+`ClientToScreen`) |
| 2 | `a8b05cb` | `StopTool` disposes the CTS only after the tool task exits |
| 3 | `94f935b` | `ToolState.Attributes` is an immutable snapshot |
| 4 | `2b2c427` | Arduino boot delay non-blocking (no 2 s UI freeze) |
| 5 | `ae67be7` | `AttrMatcher.CheckFilter` no longer drops same-named attrs in any/all mode |
| 6 | `19cde7e` | Spammer rejects unsupported keys instead of silently pressing `'0'` |
| 7 | `41f9f2e` | Removed dead DPI helpers + orphaned Win32 P/Invokes |
| 8 | `2d4e497` | Removed dead `DpiScale` + `MouseScale` config |
| 9 | `628645d` | Removed dead `DgWhiteRatio`/`GWhiteRatio` + `Slot2Dg` config |
| 10 | `e53dde9` | Extracted shared `SleepCheck`/`Beep`/`BeepMany` into `SealTools.Core.ToolBase` |
| 11 | `66a40d5` | Restored `BeepMany` inter-beep gap |
| 12 | `bcc5df3` | Renamed `sameCount` → `consecutiveRepeats` |
| 13 | `05b306b` | `[AllowedValues]` on `EmptyMode`/`MatchMode` |

---

## Open items (deferred, with reasons)

- **Capture method — settled by measurement (2026-09-09).** `PrintWindow` returns a solid black frame
  for this game; `CopyFromScreen` returns the real screen (`logs/captures/diag_printwindow.png` vs
  `diag_copyfromscreen.png`). All capture now uses `CopyFromScreen` in physical pixels. Do not
  reintroduce `PrintWindow`.
- **Recalibration is pending** after the physical-coordinate change (Pass 3) — see
  [TODO.md](TODO.md).
- **OCR row-bucket pooling.** A fixed `row_height` grid can pool two attribute rows. **Evidence first**
  — `ocr_log.jsonl` already records each raw item with its bucket key; do not change the bucketing blind.
- **Machine-specific Debug Cursor / Debug Physical buttons** in the Gem calibrate tab — diagnostic value
  vs cruft; left in place.
- **`ReferenceWindowConfig`** is written to `defaults.yaml` but never read. Removing it means editing the
  docs too; left for a decision (auto-anchor is *not* implemented).
- **`publish.bat` ships `local.yaml`** (your calibration) in the distributable. Deliberate for
  personal/same-machine use; revisit when the build is genuinely release-ready.
- **Focus-click / focus handling:** settled — see the README focus section. Do not re-add.
- **v1 (Python) duplication** — v1 is frozen; out of scope.

## Corrected from earlier reports

- There is **no** hardcoded `2.0/3.0` DPI scale in the code.
- `GemColorAnalyzer.Analyze(...)` is used by the calibrator, not dead.
- `local.yaml` is gitignored; its corruption was a local-only repair.
