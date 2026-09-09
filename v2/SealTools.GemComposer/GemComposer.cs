using System;
using System.IO.Ports;
using System.Threading;
using OpenCvSharp;
using SealTools.Core;
using SealTools.Core.Config;

namespace SealTools.GemComposer;

// Port of gem_composer/gem_composer.py. Grade positions are client-area-relative
// (plan §3). Control is in-memory (CancellationToken), state is a shared ToolState.

public sealed class GemComposer : ToolBase
{
    private readonly AppConfig _cfg;

    public GemComposer(AppConfig cfg)
        : base(cfg.Hotkeys)
    {
        _cfg = cfg;
    }

    public int Run(SerialPort ser, ToolState state, CancellationToken ct)
    {
        var grades = _cfg.Gem.Grades;
        int gidx = Math.Max(0, grades.IndexOf(_cfg.Gem.StartGrade));
        bool running = false;
        int cycle = 0;
        int emptyCount = 0;
        bool f12Was = Hotkeys.IsDown(_cfg.Hotkeys.Start);
        bool f9Was = Hotkeys.IsDown(_cfg.Hotkeys.AdvanceGrade);

        Console.WriteLine("\nGem Composer");
        Console.WriteLine("[F12] start/stop  [F9] advance grade  [F11] quit\n");

        state.Cycle = 0;
        state.Grade = grades[gidx];

        // Calibrated points/moves only exist once the user has saved them (Save Gem Composer writes
        // the positions, Save Composer Moves writes the movements). A missing entry used to throw
        // KeyNotFoundException / ArgumentOutOfRangeException, which stopped the tool with no
        // explanation on the card. Report it there and stop instead.
        void Fail(string msg)
        {
            Console.WriteLine("[!] " + msg);
            state.Message = msg;
            running = false;
            state.Running = false;
        }

        bool TryPoint(string what, IReadOnlyDictionary<string, List<int>> map, string key, out int x, out int y)
        {
            x = 0; y = 0;
            if (map.TryGetValue(key, out var p) && p is { Count: >= 2 }) { x = p[0]; y = p[1]; return true; }
            Fail($"{what} '{key}' isn't calibrated yet — open Calibrate Gem and save it.");
            return false;
        }

        bool TryMove(string what, List<int>? d, out int dx, out int dy)
        {
            dx = 0; dy = 0;
            if (d is { Count: >= 2 }) { dx = d[0]; dy = d[1]; return true; }
            Fail($"{what} isn't saved yet — open Calibrate Gem and press Save Composer Moves.");
            return false;
        }

        void SelectGradeAndRegister()
        {
            var rect = GemPointer.Client(_cfg.Window.Title);
            if (rect == null)
            {
                Fail("Game window not found — open the game first.");
                return;
            }

            var mv = _cfg.Gem.Movements;
            if (!TryPoint("Grade position", _cfg.Gem.GradePositions, grades[gidx], out var gx, out var gy)) return;
            if (!TryMove($"Move {grades[gidx]} → Register", mv.RadioToRegister.GetValueOrDefault(grades[gidx]), out var dx, out var dy)) return;

            // v1 sequence: select grade, move to Register, select. No focus handling — the single
            // click on the grade button does both jobs (activates the game window and presses the
            // button), so no separate click-to-focus is needed. Do NOT add a Win32 focus API or a
            // centre-click (a centre-click pins a raw-input game's cursor at centre).
            GemPointer.To(rect, gx, gy);
            SleepCheck(0.3);
            GemPointer.Click(ser);
            SleepCheck(0.5);
            GemPointer.Move(ser, dx, dy);
            SleepCheck(0.3);
            GemPointer.Click(ser);
            SleepCheck(0.5);
        }

        // True when the composed result-gem box is empty (no gem), per the calibrated empty
        // signature. Returns false when empty-detection isn't configured.
        bool IsResultBoxEmpty()
        {
            if (_cfg.Gem.ResultGemArea is not { Count: 4 } area) return false;
            var hwnd = WindowFinder.FindByTitle(_cfg.Window.Title);
            var client = WindowFinder.GetClientRectInScreen(hwnd);
            if (client == null) return false;
            using var crop = ScreenCapture.CaptureScreenRegion(client, new RegionConfig { Left = area[0], Top = area[1], Width = area[2], Height = area[3] });
            var frame = GemColorAnalyzer.Analyze(crop, _cfg.Gem.ColoredGapMin);
            var empty = GemColorAnalyzer.IsEmpty(frame, _cfg.Gem.EmptySignature, _cfg.Gem.EmptyDistance);
            if (_cfg.Gem.SaveEmptyCaptures && _cfg.Gem.EmptySignature is { } sig)
            {
                try
                {
                    var dir = Path.Combine(AppContext.BaseDirectory, "logs", "captures");
                    Directory.CreateDirectory(dir);
                    crop.ImWrite(Path.Combine(dir, $"empty_check_crop_{DateTime.Now:HHmmss_fff}.png"));
                }
                catch { }
                var d = GemColorAnalyzer.Distance(frame, sig);
                try { File.AppendAllText(Path.Combine(AppContext.BaseDirectory, "logs", "empty_check.txt"), $"{DateTime.Now:HH:mm:ss} dist={d:0.000} threshold={_cfg.Gem.EmptyDistance} empty={empty}\n"); } catch { }
            }
            return empty;
        }

        // Right-click the three resource slots to clear any stuck resource gems.
        bool ClearResources()
        {
            var mv = _cfg.Gem.Movements;
            if (!TryMove("Move Register → Resource1", mv.RegisterSlot1, out var x1, out var y1)) return false;
            GemPointer.Move(ser, x1, y1);
            SleepCheck(0.2);
            GemPointer.RightClick(ser);
            SleepCheck(0.3);
            if (!TryMove("Move Resource1 → Resource2", mv.Slot1Slot2, out var x2, out var y2)) return false;
            GemPointer.Move(ser, x2, y2);
            SleepCheck(0.2);
            GemPointer.RightClick(ser);
            SleepCheck(0.3);
            if (!TryMove("Move Resource2 → Resource3", mv.Slot2Slot3, out var x3, out var y3)) return false;
            GemPointer.Move(ser, x3, y3);
            SleepCheck(0.2);
            GemPointer.RightClick(ser);
            SleepCheck(0.3);
            return true;
        }

        // Resource3 → grade movement for the given grade label (null when the grade isn't one of
        // the three configured gem grades, so the caller reports it instead of moving 0,0).
        List<int>? Slot3ToGrade(string grade) => grade switch
        {
            "N" => _cfg.Gem.Movements.Slot3ToN,
            "G" => _cfg.Gem.Movements.Slot3ToG,
            "DG" => _cfg.Gem.Movements.Slot3ToDg,
            _ => null,
        };

        // Advance to the next grade; in "advance_grade_clear" mode, clear the resource slots first.
        void AdvanceGrade()
        {
            if (_cfg.Gem.EmptyMode == "advance_grade_clear" && !ClearResources())
                return;

            gidx = (gidx + 1) % grades.Count;
            state.Grade = grades[gidx];
            Console.WriteLine($"[EMPTY] advancing -> {grades[gidx]}");

            if (_cfg.Gem.EmptyMode == "advance_grade_clear")
            {
                // Cursor is at Resource3 → move to the next grade and select it.
                if (!TryMove($"Move Resource3 → {grades[gidx]}", Slot3ToGrade(grades[gidx]), out var sx, out var sy)) return;
                GemPointer.Move(ser, sx, sy);
                SleepCheck(0.2);
                GemPointer.Click(ser);
                SleepCheck(0.5);
            }
            else
            {
                // Cursor is at Register → re-select the grade absolutely.
                var rect = GemPointer.Client(_cfg.Window.Title);
                if (rect == null)
                {
                    Fail("Game window not found — open the game first.");
                    return;
                }
                if (!TryPoint("Grade position", _cfg.Gem.GradePositions, grades[gidx], out var gx, out var gy)) return;
                GemPointer.To(rect, gx, gy);
                SleepCheck(0.3);
                GemPointer.Click(ser);
                SleepCheck(0.5);
            }

            // Next grade → Register (radio_to_register) and select.
            if (!TryMove($"Move {grades[gidx]} → Register",
                _cfg.Gem.Movements.RadioToRegister.GetValueOrDefault(grades[gidx]), out var dx, out var dy)) return;
            GemPointer.Move(ser, dx, dy);
            SleepCheck(0.3);
            GemPointer.Click(ser);
            SleepCheck(0.5);
        }

        try
        {
            while (true)
            {
                SleepCheck(0.05);
                if (QuitPressed || ct.IsCancellationRequested) break;

                // Sync with the launcher's in-memory start/stop signal.
                if (state.Running && !running)
                {
                    running = true;
                    f12Was = true;
                    Console.WriteLine("[Panel] START");
                    Beep(523, 100);
                    SelectGradeAndRegister();
                }
                else if (!state.Running && running)
                {
                    running = false;
                    Console.WriteLine("[Panel] STOP");
                    Beep(1000, 150);
                }

                bool f12Now = Hotkeys.IsDown(_cfg.Hotkeys.Start);
                bool f9Now = Hotkeys.IsDown(_cfg.Hotkeys.AdvanceGrade);

                if (f12Now && !f12Was)
                {
                    running = !running;
                    state.Running = running;
                    if (running)
                    {
                        Console.WriteLine($"[GO] Grade: {grades[gidx]}");
                        Beep(523, 100);
                        SelectGradeAndRegister();
                    }
                    else { Console.WriteLine("[STOP]"); Beep(1000, 150); }
                }

                if (f9Now && !f9Was)
                {
                    gidx = (gidx + 1) % grades.Count;
                    Console.WriteLine($"[GRADE] -> {grades[gidx]}");
                    state.Grade = grades[gidx];
                    SelectGradeAndRegister();
                }

                f12Was = f12Now;
                f9Was = f9Now;

                if (!running) continue;

                cycle++;
                state.Cycle = cycle;

                if (QuitPressed || ct.IsCancellationRequested) break;
                if (Hotkeys.IsDown(_cfg.Hotkeys.Start))
                {
                    f12Was = true;
                    running = false;
                    state.Running = false;
                    Console.WriteLine("[STOP]");
                    Beep(1000, 150);
                    continue;
                }

                // Combine.
                if (!TryMove("Move Register → Combine", _cfg.Gem.Movements.RegisterCombine, out var rcx, out var rcy)) break;
                GemPointer.Move(ser, rcx, rcy);
                SleepCheck(0.2);
                GemPointer.Click(ser);
                SleepCheck(0.8);

                // Auto-advance when the result box is empty (no gem created).
                bool advanceNow = false;
                if (_cfg.Gem.EmptyMode != "stop" && _cfg.Gem.EmptySignature != null)
                {
                    if (IsResultBoxEmpty())
                    {
                        emptyCount++;
                        if (emptyCount >= _cfg.Gem.EmptyStreak)
                        {
                            emptyCount = 0;
                            advanceNow = true;
                        }
                    }
                    else emptyCount = 0;
                }

                // Move back to Register (the advance flow must start from the Register button).
                if (QuitPressed || ct.IsCancellationRequested) break;
                if (!TryMove("Move Combine → Register", _cfg.Gem.Movements.CombineRegister, out var crx, out var cry)) break;
                GemPointer.Move(ser, crx, cry);
                SleepCheck(0.2);

                if (advanceNow)
                {
                    AdvanceGrade();
                    continue;
                }

                // Normal: deregister + register.
                GemPointer.Click(ser);
                SleepCheck(0.3);
                GemPointer.Click(ser);
                SleepCheck(0.5);

                if (cycle % 10 == 0) Console.WriteLine($"  Cycle: {cycle}");
                if (PauseRequested)
                {
                    Console.WriteLine("[PAUSE] graceful stop");
                    running = false; state.Running = false;
                    PauseRequested = false;
                    break;
                }
            }
        }
        finally
        {
            state.Running = false;
        }

        Console.WriteLine($"\nDone. {cycle} cycles.");
        return 0;
    }
}
