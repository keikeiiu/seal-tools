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
        // The game is ALWAYS unfocused when the composer starts (the launcher window holds
        // focus). v1 never calls a Win32 focus API — a CLICK is what focuses the game. So we
        // click the grade position once to bring the game forward, then run the v1 sequence.
        bool gameFocused = false;

        Console.WriteLine("\nGem Composer");
        Console.WriteLine("[F12] start/stop  [F9] advance grade  [F11] quit\n");

        state.Cycle = 0;
        state.Grade = grades[gidx];

        void SelectGradeAndRegister()
        {
            var rect = GemPointer.Client(_cfg.Window.Title);
            if (rect == null)
            {
                Console.WriteLine("[!] game window not found");
                return;
            }
            var pos = _cfg.Gem.GradePositions[grades[gidx]];

            // Click-to-focus (shared GemPointer + v1 model): the game is always UNFOCUSED while the
            // tool runs (the launcher holds focus), so a CLICK brings it forward — no Win32 focus
            // API, no centre-click (that would pin a raw-input game's in-game cursor at centre).
            if (!gameFocused)
            {
                GemPointer.To(rect, pos[0], pos[1]);
                SleepCheck(0.3);
                GemPointer.Click(ser);
                SleepCheck(0.4);
                gameFocused = true;
            }

            // v1 sequence: select grade, move to Register, select.
            GemPointer.To(rect, pos[0], pos[1]);
            SleepCheck(0.3);
            GemPointer.Click(ser);
            SleepCheck(0.5);
            var d = _cfg.Gem.Movements.RadioToRegister[grades[gidx]];
            GemPointer.Move(ser, d[0], d[1]);
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
            var frame = GemColorAnalyzer.Analyze(crop);
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
        void ClearResources()
        {
            var mv = _cfg.Gem.Movements;
            GemPointer.Move(ser, mv.RegisterSlot1[0], mv.RegisterSlot1[1]);
            SleepCheck(0.2);
            GemPointer.RightClick(ser);
            SleepCheck(0.3);
            GemPointer.Move(ser, mv.Slot1Slot2[0], mv.Slot1Slot2[1]);
            SleepCheck(0.2);
            GemPointer.RightClick(ser);
            SleepCheck(0.3);
            GemPointer.Move(ser, mv.Slot2Slot3[0], mv.Slot2Slot3[1]);
            SleepCheck(0.2);
            GemPointer.RightClick(ser);
            SleepCheck(0.3);
        }

        // Resource3 → grade movement for the given grade label.
        List<int> Slot3ToGrade(string grade) => grade switch
        {
            "N" => _cfg.Gem.Movements.Slot3ToN,
            "G" => _cfg.Gem.Movements.Slot3ToG,
            "DG" => _cfg.Gem.Movements.Slot3ToDg,
            _ => new List<int> { 0, 0 },
        };

        // Advance to the next grade; in "advance_grade_clear" mode, clear the resource slots first.
        void AdvanceGrade()
        {
            if (_cfg.Gem.EmptyMode == "advance_grade_clear")
                ClearResources();

            gidx = (gidx + 1) % grades.Count;
            state.Grade = grades[gidx];
            Console.WriteLine($"[EMPTY] advancing -> {grades[gidx]}");

            if (_cfg.Gem.EmptyMode == "advance_grade_clear")
            {
                // Cursor is at Resource3 → move to the next grade and select it.
                var slot3 = Slot3ToGrade(grades[gidx]);
                GemPointer.Move(ser, slot3[0], slot3[1]);
                SleepCheck(0.2);
                GemPointer.Click(ser);
                SleepCheck(0.5);
            }
            else
            {
                // Cursor is at Register → re-select the grade absolutely.
                var rect = GemPointer.Client(_cfg.Window.Title);
                if (rect == null) return;
                var pos = _cfg.Gem.GradePositions[grades[gidx]];
                GemPointer.To(rect, pos[0], pos[1]);
                SleepCheck(0.3);
                GemPointer.Click(ser);
                SleepCheck(0.5);
            }

            // Next grade → Register (radio_to_register) and select.
            var d = _cfg.Gem.Movements.RadioToRegister[grades[gidx]];
            GemPointer.Move(ser, d[0], d[1]);
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
                var rc = _cfg.Gem.Movements.RegisterCombine;
                GemPointer.Move(ser, rc[0], rc[1]);
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
                var cr = _cfg.Gem.Movements.CombineRegister;
                GemPointer.Move(ser, cr[0], cr[1]);
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
