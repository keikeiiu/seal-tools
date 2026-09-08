# Seal Tools v2 — Verified Findings & Fix Checklist

> Working checklist for the `v2-saving-attempt` branch. Every item below is a **single
> incremental fix** and gets its own commit. Items are verified against the actual source
> (file:line). This supersedes any inaccurate statements in REVIEW.md — see "Corrections".

---

## Corrections to REVIEW.md (verified)

- **There is NO `ScaleGemCoordsToLogical` / hardcoded `2/3` DPI scale.** I misread the git history. Gem coordinates are used as-is in logical space (process is DPI-unaware). The real DPI issue is *dead config + dead helpers* (item 7 below).
- `v2/config/local.yaml` is **gitignored** (`v2/.gitignore` → `config/local.yaml`) and not tracked. Its corruption is a **local-only** repair (delete it; `ConfigLoader.Load` re-seeds from `local.yaml.example`), not a commit.

---

## Findings (verified, ranked)

### A. Critical — concurrency & lifecycle

1. **`StopTool` disposes the `CancellationTokenSource` while the tool thread reads it.**
   [LauncherService.cs:102-115](v2/SealTools.Launcher/LauncherService.cs#L102-L115) — `_cts.Cancel()` then `_cts.Dispose()` immediately, then `Thread.Sleep(300)`. The tool loop reads `ct.IsCancellationRequested` (e.g. [SealTuner.cs:58](v2/SealTools.Tuner/SealTuner.cs#L58)) → `ObjectDisposedException` on a disposed CTS, swallowed by the catch in [LauncherService.cs:92-97](v2/SealTools.Launcher/LauncherService.cs#L92-L97) and logged as "crashed".
   **Fix:** track the `Task`; dispose the CTS only after the task completes (a continuation).

2. **`Thread.Sleep` blocks the WPF UI thread.**
   [LauncherService.cs:61](v2/SealTools.Launcher/LauncherService.cs#L61) `Thread.Sleep(2000)` (Arduino boot delay) and [LauncherService.cs:114](v2/SealTools.Launcher/LauncherService.cs#L114) `Thread.Sleep(300)` — both run on the UI thread from button handlers.
   **Fix:** non-blocking boot delay (`await Task.Delay`), or move off the UI thread.

3. **Race on shared `ToolState`.**
   [ToolState.cs](v2/SealTools.Core/ToolState.cs) has unsynchronized mutable fields. [SealTuner.cs:158](v2/SealTools.Tuner/SealTuner.cs#L158) replaces `state.Attributes` (a `List<string>`) wholesale from the tool thread while `MainWindow`'s 750 ms `DispatcherTimer` enumerates it → mid-enumeration `InvalidOperationException` possible.
   **Fix:** lock, or make `Attributes` an immutable snapshot.

### B. High — correctness

4. **Arduino `K`/`k` handler only presses digits 0–9.**
   [seal_mouse.ino:128-135](arduino/seal_mouse/seal_mouse.ino#L128-L135) does `atoi(&buf[1])` then `'0' + (n % 10)`; a letter key silently becomes `'0'`. `SendKey` [SkillSpammer.cs:120-131](v2/SealTools.Spammer/SkillSpammer.cs#L120-L131) sends `"K <key>"` for any non-F key. The `F` handler only supports F1–F10 while `ParseVk` accepts F1–F24.
   **Fix:** validate/restrict key input (digits + F1–F10) with a clear error, or extend firmware.

5. **`AttrMatcher.CheckFilter` drops later same-named attributes.**
   [AttrMatcher.cs:262-272](v2/SealTools.Tuner/AttrMatcher.cs#L262-L272) — unconditional `break` after the name matches; if the first same-named attr fails min/max, the second is never checked.
   **Fix:** `continue` instead of `break` (without double-counting).

### C. Medium — dead code / quality

6. **Dead DPI helpers + dead DPI config.**
   - [WindowFinder.cs:130-134](v2/SealTools.Core/WindowFinder.cs#L130-L134) `EnablePerMonitorDpiAwareness` (would re-enable PerMonitorV2 → regression trap), never called.
   - [WindowFinder.cs:225-237](v2/SealTools.Core/WindowFinder.cs#L225-L237) `LogDpiAwareness`, [193-207](v2/SealTools.Core/WindowFinder.cs#L193-L207) `DebugCursorCalc`, [86-94](v2/SealTools.Core/WindowFinder.cs#L86-L94) `BringToForeground`, [110-121](v2/SealTools.Core/WindowFinder.cs#L110-L121) `IsForeground` — all dead; their P/Invokes (`ClipCursor`, `SetForegroundWindow`, `ShowWindow`, `IsIconic`, `GetForegroundWindow`, `GetThreadDpiAwarenessContext`, `GetAwarenessFromDpiAwarenessContext`) become dead too.
   - `DisplayConfig.DpiScale` [AppConfig.cs:21-24](v2/SealTools.Core/Config/AppConfig.cs#L21-L24) loaded ([ConfigLoader.cs:104](v2/SealTools.Core/Config/ConfigLoader.cs#L104)) but never read.
   - `GemConfig.MouseScale` [AppConfig.cs:189-192](v2/SealTools.Core/Config/AppConfig.cs#L189-L192) — parsed + asserted in [ConfigLoaderTests.cs:57-58](v2/SealTools.Tests/ConfigLoaderTests.cs#L57-L58), never used in coordinate math; doc comment claims "(point - Register) * scale / 100" which is **false** — the composer sends raw HID counts.
   **Fix:** remove dead helpers/P/Invokes + dead config; update the stale doc comment and test.

7. **Dead config fields.**
   - `GradeDecisionConfig.DgWhiteRatio`/`GWhiteRatio` [AppConfig.cs:126,128](v2/SealTools.Core/Config/AppConfig.cs#L126) — removed from `DecideGrade`.
   - `ReferenceWindowConfig` [AppConfig.cs:32](v2/SealTools.Core/Config/AppConfig.cs#L32) — loaded/saved, never read.
   - `MovementsConfig.Slot2Dg` [AppConfig.cs:204](v2/SealTools.Core/Config/AppConfig.cs#L204) — never referenced.
   - `GemColorAnalyzer.Analyze(IntPtr hwnd, …)` [GemColorAnalyzer.cs:29-33](v2/SealTools.Core/GemColorAnalyzer.cs#L29-L33) — unused overload.
   **Fix:** remove (each as a commit, or grouped as one "dead code" commit).

8. **Duplicated `SleepCheck` + `Beep`.**
   Identical private helpers in [SealTuner.cs:230-250](v2/SealTools.Tuner/SealTuner.cs#L230-L250), [GemComposer.cs:296-311](v2/SealTools.GemComposer/GemComposer.cs#L296-L311), [SkillSpammer.cs:133-148](v2/SealTools.Spammer/SkillSpammer.cs#L133-L148). The F12/F11 start/stop + countdown skeleton is also repeated.
   **Fix:** extract a shared base/helper into `SealTools.Core`.

9. **String-typed enums.**
   `GemConfig.EmptyMode` (`"stop"|"advance_grade"|"advance_grade_clear"`, [AppConfig.cs:176](v2/SealTools.Core/Config/AppConfig.cs#L176)) and `FilterConfig.MatchMode` (`"any"|"all"|"per_attr"`, [AppConfig.cs:147](v2/SealTools.Core/Config/AppConfig.cs#L147)) compared via raw strings ([GemComposer.cs:134,141,245](v2/SealTools.GemComposer/GemComposer.cs#L134), [AttrMatcher.cs:225,274](v2/SealTools.Tuner/AttrMatcher.cs#L225)). Misspelling fails only at runtime.
   **Fix:** typed enums.

10. **Hardcoded magic values.**
    - `GemColorAnalyzer.ColoredGapMin = 40` [GemColorAnalyzer.cs:26](v2/SealTools.Core/GemColorAnalyzer.cs#L26) (every other threshold is config-driven).
    - `MainWindow.xaml.cs` `DebugCursor(727,696)` / `DebugPhysicalCursor(1096,1044)` — machine-specific DPI-debug leftovers baked into buttons.
    **Fix:** move `ColoredGapMin` to config; remove/guard debug buttons.

11. **`BeepMany` dropped the inter-beep sleep.**
    [SealTuner.cs:247-250](v2/SealTools.Tuner/SealTuner.cs#L247-L250) fires 5 beeps back-to-back; v1 sleeps 0.1 s between them. **Fix:** add the delay.

12. **`sameCount` naming.**
    [SealTuner.cs:197-202](v2/SealTools.Tuner/SealTuner.cs#L197-L202) `if (sameCount >= 2)` prints "same result x3". Logic correct, name/message confusing. Minor.

### D. Local-only (no commit)

13. **`v2/config/local.yaml` is corrupted** (a single mangled README line). Delete it so `ConfigLoader.Load` re-seeds from `local.yaml.example`. Gitignored, so local repair only.

---

## Fix order (each row = one commit)

| # | Fix | Risk | File(s) |
|---|---|---|---|
| 1 | ✅ DONE — client-area origin | — | WindowFinder.cs |
| 2 | StopTool CTS dispose race | low | LauncherService.cs |
| 3 | ToolState thread-safety | low | ToolState.cs (+ SealTuner.cs) |
| 4 | Non-blocking boot delay (UI freeze) | med | LauncherService.cs (+ callers) |
| 5 | CheckFilter `break` → `continue` | low | AttrMatcher.cs |
| 6 | Restrict spammer keys (digits + F1–F10) | low | SkillSpammer.cs (+ MainWindow) |
| 7 | Remove dead DPI helpers + DpiScale/MouseScale | low | WindowFinder.cs, AppConfig.cs, ConfigLoader.cs, tests |
| 8 | Remove dead config fields | low | AppConfig.cs, GemColorAnalyzer.cs |
| 9 | Extract shared SleepCheck/Beep helper | med | 3 tools + new Core type |
| 10 | Typed enums (EmptyMode/MatchMode) | med | AppConfig.cs, GemComposer.cs, AttrMatcher.cs |
| 11 | ColoredGapMin → config | low | GemColorAnalyzer.cs, config |
| 12 | BeepMany inter-beep sleep | trivial | SealTuner.cs |
| 13 | sameCount naming | trivial | SealTuner.cs |

**Deferred (need evidence / decision first):**
- OCR row-bucket pooling (measure real frames first — do NOT guess).
- Removing the machine-specific debug-cursor buttons (diagnostic value vs cruft).
- v1 Python duplication (v1 is frozen).

---

## Verification per fix

- Build: `dotnet build v2/SealTools.sln` (note `TreatWarningsAsErrors=true` + `AnalysisMode=Recommended`).
- Tests: `dotnet test v2/SealTools.Tests` (update/remove the `MouseScale` assertion when that config goes away).
- Manual: start/stop a tool repeatedly (no "crashed" log, no UI freeze); run a Gem calibrate + one combine cycle to confirm coordinates unchanged.
