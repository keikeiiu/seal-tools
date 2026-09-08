# Seal Tools — Design, Review & Action Plan

> Analysis-only document. No code was changed as part of this review.
> Generated 2026-09-09 after a full read of v1 (Python), v2 (C#/.NET 8 WPF), the Arduino firmware, config, and recent git history.

---

## Part A — What this project is (design record)

### Purpose

A bundle of **GameGuard-safe automation tools for the MMORPG Seal Online (希望Online / Seal Online TW)**. Because the game's GameGuard anti-cheat blocks synthetic input (`SendInput`, `SetCursorPos` while focused), the tools drive an **Arduino Pro Micro (ATmega32U4)** that enumerates as a *genuine USB HID mouse/keyboard* — its clicks/keystrokes are indistinguishable from human input and cannot be blocked.

Three core tools + one daily script:

- **Magic Tuner** — auto-rolls the 發條 (magic tuning) UI: click confirm + Enter, OCR-reads grade (N/G/DG/XG/SG) + 3 attribute lines, applies filter rules, auto-stops at target grade.
- **Gem Composer** — auto-combines gems (N/G/DG radio + Register + Combine) via calibrated cursor moves.
- **Skill Spammer** — presses configured skill keys on a loop.
- **Check-in** — headless Playwright daily event check-in (v1 Python only, out of scope for v2).

### Two generations

| | v1 (Python) | v2 (C# / .NET 8 WPF) |
|---|---|---|
| Location | repo root (`launcher.py`, `tuner/`, `gem_composer/`, `skill_spammer/`, `checkin/`) | `v2/` |
| Runtime | Python 3.12 + pip | self-contained native Windows `.exe` |
| UI | Flask web panels (`:5002`/`:5000`) | WPF + WPF-UI 4.3.0 single window |
| Control/state | out-of-process `control.txt`/`state.json` files | in-memory `CancellationToken` + shared `ToolState` |
| OCR | `rapidocr-onnxruntime` | `RapidOCRSharpOnnx` + ONNX Runtime + OpenCvSharp (same PP-OCRv4 models) |
| Status | working, untouched | **active** (all recent commits `v2:`) |

v2 is the current/active version. Check-in is explicitly out of scope for v2.

### v2 module map (namespace per project)

- **`SealTools.Core`** — shared infra:
  - `Arduino.cs` — WMI VID/PID discovery + `System.IO.Ports`.
  - `Config/` — YAML via YamlDotNet (defaults + local overlay + attributes), fails loudly on invalid input.
  - `WindowFinder.cs` — Win32 `EnumWindows`/`GetClientRect`/`ClientToScreen`.
  - `ScreenCapture.cs` — `PrintWindow` (calibration) + `CopyFromScreen` (OCR/loop).
  - `Hotkeys.cs` — `GetAsyncKeyState`.
  - `GemPointer.cs` — shared Arduino "mouse tool" calls (absolute move, `C`, `R`, `D`).
  - `GemColorAnalyzer.cs` — color-composition fingerprint (empty vs filled result box).
  - `ToolState.cs` — live mutable state; `FileLogger.cs` — thread-safe logger.
- **`SealTools.Tuner`** — `SealTuner.cs` (loop), `OcrEngine.cs` (RapidOCR + grade color), `AttrMatcher.cs`, `TextCleaner.cs`.
- **`SealTools.GemComposer`** — `GemComposer.cs` (combine loop).
- **`SealTools.Spammer`** — `SkillSpammer.cs` (key loop).
- **`SealTools.Launcher`** — WPF-UI app: `App.xaml(.cs)`, `MainWindow.xaml(.cs)`, `LauncherService.cs` (lifecycle).
- **`SealTools.Tests`** — xUnit (only `ConfigLoaderTests.cs` currently).

### Coordinate model (the crux)

Canonical coordinate space = **client-area top-left in screen space** (`GetClientRect` + `ClientToScreen`, *not* `GetWindowRect`, which includes the title bar/border). The whole process runs **DPI-unaware** (`app.manifest` `dpiAwareness=unaware`), so everything is in **logical pixels** (v1 convention). `SetCursorPos` handles absolute cursor positioning; the Arduino handles actual clicks (`C`/`R`) and relative HID moves (`D dx dy`, **raw counts, not pixels**).

Input pipeline: `SetCursorPos(client.Left+x, client.Top+y)` → Arduino `C`/`D`/`R` via serial. The game stays **unfocused** while tools run (launcher holds focus).

Arduino firmware protocol (`arduino/seal_mouse/seal_mouse.ino`, 115200 baud): `C`=left click, `R`=right, `D dx dy`=relative move, `E`=Enter, `T`=Tab, `K/k n`=digit key, `F/f n`=F-key, `X`=Alt+Tab, `W ms`=wait, `H`=Bezier human-like move (unused).

---

## Part B — Findings (inconsistencies & issues)

Ranked by impact.

### Critical (immediate bugs)

1. **`StopTool` disposes the `CancellationTokenSource` while the tool thread still reads it** — `LauncherService.cs:102-115` calls `_cts.Cancel()` then `_cts.Dispose()` immediately, then `Thread.Sleep(300)`. Reading `ct.IsCancellationRequested` on a disposed CTS throws `ObjectDisposedException`, swallowed by the generic catch and logged as "crashed". Timing-dependent bug on every Stop. **Fix: dispose only after the task completes (await first).**

2. **`Thread.Sleep` blocks the WPF UI thread** — `LauncherService.ArduinoPort()` does `Thread.Sleep(2000)` (`:61`) called synchronously from button handlers; `StopTool` sleeps `300` ms (`:114`) on the UI thread. Freezes UI. **Fix: `await Task.Delay`.**

3. **`v2/config/local.yaml` in the working tree is corrupted** — a single mangled line (a README instruction fragment). `ConfigLoader` deserializes it as empty `LocalOverrides`, then `ConfigValidator` throws "grade_area x2/y2 must be greater than x1/y1". Breaks a fresh checkout/run. `dist/` copy is fine. **Fix: restore from `local.yaml.example`; confirm gitignored.**

4. **Race conditions on shared `ToolState`** — `LauncherService.StartTool` mutates `ToolState` (`Running`/`Grade`/`Attributes`/`Cycle`) from the tool thread while `MainWindow`'s 750 ms `DispatcherTimer` reads it. `ToolState.Attributes` is a `List<string>` replaced wholesale mid-enumeration with no lock/`volatile`/`ImmutableList`. Can throw. **Fix: synchronize or use a single-owner/immutable-snapshot model.**

### High (correctness)

5. **Arduino `K`/`k` handler only presses digits 0–9** — `seal_mouse.ino:128-135` does `atoi(&buf[1])` then `'0' + (n % 10)`. A letter key → `'0'`. `SkillSpammer.SendKey` sends `"K <key>"` for any non-F key, and the Settings tab advertises "a single letter/digit". Latent (defaults are digits) but silent. Also `F` handler only supports `F1–F10` while `ParseVk` accepts `F1–F24`. **Fix: restrict UI to digits/F1–F10, or extend firmware.**

6. **`AttrMatcher.CheckFilter` "any"/"all" mode drops later matches of the same name** — `AttrMatcher.cs:262-272` has an unconditional `break` after the name matches; if the first same-named attr fails min/max, the second is never considered. Faithful port of `attr_matcher.py:300-316` (same flaw). **Fix: continue to next match instead of break (careful not to double-count).**

7. **DPI/coordinate handling is contradictory and half-removed** — the `7f9e1c7` DPI churn left landmines:
   - `WindowFinder.EnablePerMonitorDpiAwareness()` still sets `PerMonitorV2` but is never called — a regression trap if anyone "fixes" DPI by calling it.
   - `LogDpiAwareness()`, `DebugCursorCalc()`, `BringToForeground()`, `IsForeground()` and the related `SetForegroundWindow`/`ShowWindow`/`IsIconic`/`ClipCursor` P/Invokes are all dead.
   - The "scale stored physical gem coords to logical" approach from `7f9e1c7` was silently dropped; vestigial `DisplayConfig.DpiScale`/`DisplayOverrides.DpiScale` and `GemConfig.MouseScale` remain (loaded but never read; `MouseScale`'s doc comment is now stale/false).
   - `ScaleGemCoordsToLogical` uses a **hardcoded `2.0/3.0`** (96/144) rather than reading DPI — wrong on any non-150% display.
   - **Fix: remove the dead DPI helpers + vestigial config; replace hardcoded 2/3 with measured DPI scale (or document 150%-only).**

8. **OCR row-bucket may pool two attribute rows** — `BuildLines` buckets by `y / rowHeight`; a fixed bucket grid can merge two nearby attr rows. **Evidence first — do not guess.**

### Medium (quality / duplication)

9. **`SleepCheck` + `Beep` copy-pasted** across `SealTuner.cs`, `SkillSpammer.cs`, `GemComposer.cs`; F12/F11 start/stop + countdown skeleton repeated in all three `Run` loops. **Extract shared base/helper.**

10. **`find_arduino()`/`key()`/`sleep_check()` duplicated across four v1 Python tools** (v1 only — leave unless touching v1).

11. **String-typed "enums"** — `GemConfig.EmptyMode` (`"stop"|"advance_grade"|"advance_grade_clear"`) and `FilterConfig.MatchMode` (`"any"|"all"|"per_attr"`) compared via raw strings; misspelling fails only at runtime. **Convert to typed enums.**

12. **Magic numbers / hardcoded coords** — `MainWindow.xaml.cs:664` `DebugCursor(727,696)` / `:673` `DebugPhysicalCursor(1096,1044)` (machine-specific DPI-debug leftovers); `GemColorAnalyzer.cs:26` `ColoredGapMin=40` hardcoded; many inline delay constants. **Move to config / remove debug buttons.**

13. **Swallowed exceptions** — `Beep` is `try{...}catch{}` in three files (audio failure invisible); `Arduino.Find` WMI fallback uses `catch{}`. At minimum log them.

14. **`sameCount` naming vs message** — `SealTuner.cs:197-202` `if (sameCount >= 2)` prints "same result x3". Logic correct; name/message confusing. Minor.

15. **`BeepMany` dropped the inter-beep sleep** — v1 sleeps 0.1 s between the five beeps; v2 fires back-to-back → single ~200 ms tone. Minor.

### Dead code (safe removals)

- `GradeDecisionConfig.DgWhiteRatio`/`GWhiteRatio` (rule removed from `DecideGrade`), `ReferenceWindowConfig`, `MovementsConfig.Slot2Dg`, `ScreenCapture`/`GemColorAnalyzer` `IntPtr hwnd` overload (unused).
- No `TODO`/`FIXME`/`HACK` markers exist — clean on that axis.

---

## Part C — Action plan

### Step 0 — Save this report (already done by this file)

### Step 1 — Fix immediate bugs (critical, no behavior change)

1. `LauncherService.StopTool` — await the tool `Task` before `_cts.Dispose()`; remove `Thread.Sleep(300)`.
2. `LauncherService.ArduinoPort` + `StopTool` — replace `Thread.Sleep` with `await Task.Delay` (make callers async); keep the 2 s boot delay.
3. Repair `v2/config/local.yaml` from `local.yaml.example`; verify it's in `.gitignore`.
4. Add synchronization to `ToolState` (lock or single-owner immutable snapshot) so the UI timer can't observe a mid-replacement `Attributes` list.

### Step 2 — Correctness fixes (small, targeted)

5. Arduino firmware: restrict key input to digits + F1–F10 in the UI (or extend firmware); align `SkillSpammer`/`ParseVk` with what firmware supports.
6. `AttrMatcher.CheckFilter` — fix the `break` so same-named attrs aren't dropped (verify against v1 behavior first).
7. DPI cleanup: remove dead helpers + vestigial `DpiScale`/`MouseScale`/white-ratio config; replace hardcoded `2.0/3.0` with measured DPI scale or documented 150%-only constraint.
8. OCR row-bucket: **gather evidence first** (save real unconfirmed frames, inspect actual row y-positions) before changing bucketing.

### Step 3 — Quality / consolidation (lower priority)

9. Extract shared `SleepCheck`/`Beep`/loop skeleton into a base helper in `SealTools.Core`.
10. Convert `EmptyMode`/`MatchMode` to typed enums.
11. Remove dead config fields; move `ColoredGapMin` and inline delay constants into config; remove/guard machine-specific debug buttons.
12. Log (not swallow) `Beep`/WMI fallback exceptions; restore `BeepMany` inter-beep sleep.

---

## Part D — What NOT to do (explicit guardrails)

These are settled by evidence/memory — **do not reverse them**:

1. **Do NOT touch v1 (Python)** — it works, is a reference/fallback, and is treated as frozen. Check-in is v1-only and stays out of v2.
2. **Do NOT re-enable DPI awareness (PerMonitorV2)** — root cause of the coordinate regression `7f9e1c7` fixed. Keep the process DPI-unaware + logical pixels.
3. **Do NOT move clicks from Arduino HID to `SendInput`/`SetCursorPos`-while-focused** — GameGuard blocks synthetic input; the design depends on the Arduino being a genuine HID device.
4. **Do NOT add a focus-click before gem clicks** — focus is *not* the issue; the coordinate is. v1 is unfocused too and works.
5. **Do NOT re-introduce pixel-computed relative-move (`(point - Register) * scale / 100`)** — that is what broke relative moves; the composer intentionally sends raw HID counts.
6. **Do NOT "fix" OCR row-bucketing blind** — evidence first.
7. **Do NOT change the PrintWindow capture path** for calibration back to CopyFromScreen — PrintWindow is the DirectX-safe fix.
8. **Do NOT fold the "Save Composer Moves" button / advanced-mode** work into this pass — separately tracked future task.

---

## Part E — Verification

- **Build:** `dotnet build v2/SealTools.sln` clean (note `TreatWarningsAsErrors=true` + `AnalysisMode=Recommended` — removals must not leave unused-symbol warnings).
- **Tests:** `dotnet test v2/SealTools.Tests` — `ConfigLoaderTests` still pass (the `MouseScale` assertion in `ConfigLoaderTests.cs:57-58` must be updated/removed when `MouseScale` config is removed).
- **Stop/start loop:** run the Launcher, start + stop a tool repeatedly — confirm no "crashed" log and no UI freeze.
- **Fresh-checkout smoke:** confirm `local.yaml` loads without the `ConfigValidator` exception.
- **Gem/OCR spot check:** run Gem Composer calibrate + one combine cycle on the 150% display to confirm coordinates unchanged after DPI cleanup.
