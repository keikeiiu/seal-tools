using System;
using System.IO.Ports;
using System.Threading;
using SealTools.Core;
using SealTools.Core.Config;

namespace SealTools.GemComposer;

// Port of gem_composer/gem_composer.py. Grade positions are client-area-relative
// (plan §3). Control is in-memory (CancellationToken), state is a shared ToolState.

public sealed class GemComposer
{
    private readonly AppConfig _cfg;
    private bool _quitPressed;
    private bool _pauseRequested;

    public GemComposer(AppConfig cfg)
    {
        _cfg = cfg;
    }

    public int Run(ToolState state, CancellationToken ct)
    {
        var port = Arduino.Find(_cfg.Arduino.Vid, _cfg.Arduino.Pid);
        if (port == null)
        {
            Console.WriteLine("[!] Arduino not found");
            return 1;
        }
        using var ser = Arduino.Open(port, _cfg.Arduino.Baud);
        Thread.Sleep(2000);
        Console.WriteLine($"[OK] Arduino on {port}");

        var grades = _cfg.Gem.Grades;
        int gidx = Math.Max(0, grades.IndexOf(_cfg.Gem.StartGrade));
        bool running = false;
        int cycle = 0;
        bool f12Was = Hotkeys.IsDown(_cfg.Hotkeys.Start);
        bool f9Was = Hotkeys.IsDown(_cfg.Hotkeys.AdvanceGrade);

        Console.WriteLine("\nGem Composer");
        Console.WriteLine("[F12] start/stop  [F9] advance grade  [F11] quit\n");

        state.Cycle = 0;
        state.Grade = grades[gidx];

        void SelectGradeAndRegister()
        {
            var rect = WindowFinder.GetClientRectInScreen(WindowFinder.FindByTitle(_cfg.Window.Title));
            if (rect == null) return;
            var pos = _cfg.Gem.GradePositions[grades[gidx]];
            WindowFinder.MoveCursor(rect, pos[0], pos[1]);
            SleepCheck(0.3);
            ser.Write("C\n");
            SleepCheck(0.5);
            var d = _cfg.Gem.Movements.RadioToRegister[grades[gidx]];
            ser.Write($"D {d[0]} {d[1]}\n");
            SleepCheck(0.3);
            ser.Write("C\n");
            SleepCheck(0.5);
        }

        try
        {
            while (true)
            {
                SleepCheck(0.05);
                if (_quitPressed || ct.IsCancellationRequested) break;

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

                if (_quitPressed || ct.IsCancellationRequested) break;
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
                ser.Write($"D {rc[0]} {rc[1]}\n");
                SleepCheck(0.2);
                ser.Write("C\n");
                SleepCheck(0.8);

                // Back to Register — deregister + register.
                if (_quitPressed || ct.IsCancellationRequested) break;
                var cr = _cfg.Gem.Movements.CombineRegister;
                ser.Write($"D {cr[0]} {cr[1]}\n");
                SleepCheck(0.2);
                ser.Write("C\n");
                SleepCheck(0.3);
                ser.Write("C\n");
                SleepCheck(0.5);

                if (cycle % 10 == 0) Console.WriteLine($"  Cycle: {cycle}");
                if (_pauseRequested)
                {
                    Console.WriteLine("[PAUSE] graceful stop");
                    running = false; state.Running = false;
                    _pauseRequested = false;
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

    private void SleepCheck(double seconds)
    {
        var steps = Math.Max(1, (int)(seconds / 0.05));
        var ms = Math.Max(1, (int)(seconds / steps * 1000));
        for (int i = 0; i < steps; i++)
        {
            Thread.Sleep(ms);
            if (Hotkeys.IsDown(_cfg.Hotkeys.Quit)) { _quitPressed = true; return; }
            if (Hotkeys.IsDown(_cfg.Hotkeys.Pause)) { _pauseRequested = true; }
        }
    }

    private static void Beep(int freq, int ms)
    {
        try { Console.Beep(freq, ms); } catch { }
    }
}
