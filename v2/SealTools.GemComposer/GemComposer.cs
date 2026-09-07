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

    public int Run(SerialPort ser, ToolState state, CancellationToken ct)
    {
        var grades = _cfg.Gem.Grades;
        int gidx = Math.Max(0, grades.IndexOf(_cfg.Gem.StartGrade));
        bool running = false;
        int cycle = 0;
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
                GemPointer.Move(ser, rc[0], rc[1]);
                SleepCheck(0.2);
                GemPointer.Click(ser);
                SleepCheck(0.8);

                // Back to Register — deregister + register.
                if (_quitPressed || ct.IsCancellationRequested) break;
                var cr = _cfg.Gem.Movements.CombineRegister;
                GemPointer.Move(ser, cr[0], cr[1]);
                SleepCheck(0.2);
                GemPointer.Click(ser);
                SleepCheck(0.3);
                GemPointer.Click(ser);
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
