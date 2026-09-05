using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;
using System.IO.Ports;
using System.Threading;
using SealTools.Core;
using SealTools.Core.Config;

namespace SealTools.Spammer;

// Port of skill_spammer/skill_spammer.py. Control in-memory (CancellationToken),
// state in a shared ToolState; Console.WriteLine is only for logs.

public sealed class SkillSpammer
{
    private readonly AppConfig _cfg;
    private bool _quitPressed;

    public SkillSpammer(AppConfig cfg)
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

        var cooldowns = _cfg.Spammer.Keys;
        bool running = false;
        int count = 0;
        string current = "";
        var last = new Dictionary<string, double>();
        foreach (var k in cooldowns.Keys) last[k] = 0;

        bool f12Was = Hotkeys.IsDown(_cfg.Hotkeys.Start);
        var sw = Stopwatch.StartNew();

        Console.WriteLine("\nSkill Spammer");
        foreach (var (k, cd) in cooldowns)
            Console.WriteLine($"  {k}: every {cd:g}s");
        Console.WriteLine("[F12] start/stop  [F11] quit\n");

        void Reset()
        {
            foreach (var k in cooldowns.Keys) last[k] = sw.Elapsed.TotalSeconds;
            count = 0;
            current = "";
            state.Cycle = 0;
            state.Current = null;
        }

        state.Running = false;

        try
        {
            while (true)
            {
                SleepCheck(0.02);
                if (_quitPressed || ct.IsCancellationRequested) break;

                // Sync with the launcher's in-memory start/stop signal.
                if (state.Running && !running)
                {
                    running = true;
                    _quitPressed = false;
                    Reset();
                    Console.WriteLine("[Panel] START");
                    Beep(523, 100);
                }
                else if (!state.Running && running)
                {
                    running = false;
                    Console.WriteLine("[Panel] STOP");
                    Beep(1000, 150);
                }

                bool f12Now = Hotkeys.IsDown(_cfg.Hotkeys.Start);
                if (f12Now && !f12Was)
                {
                    running = !running;
                    state.Running = running;
                    if (running) { _quitPressed = false; Reset(); Console.WriteLine($"[GO] {string.Join(", ", cooldowns.Keys)}"); Beep(523, 100); }
                    else { Console.WriteLine("[STOP]"); Beep(1000, 150); }
                }
                f12Was = f12Now;

                if (!running) continue;

                double now = sw.Elapsed.TotalSeconds;
                foreach (var (k, cd) in cooldowns)
                {
                    if (now - last[k] >= cd)
                    {
                        current = k;
                        SendKey(ser, k);
                        last[k] = now;
                        count++;
                        state.Current = k;
                        state.Cycle = count;
                    }
                }
            }
        }
        finally
        {
            state.Running = false;
        }

        Console.WriteLine("\nDone.");
        return 0;
    }

    private static void SendKey(SerialPort ser, string key)
    {
        bool fast = key.StartsWith('*');
        if (fast) key = key.Substring(1);

        string cmd;
        if (key.StartsWith('F'))
            cmd = (fast ? "f " : "F ") + int.Parse(key.AsSpan(1), CultureInfo.InvariantCulture) + "\n";
        else
            cmd = (fast ? "k " : "K ") + key + "\n";
        ser.Write(cmd);
    }

    private void SleepCheck(double seconds)
    {
        var steps = Math.Max(1, (int)(seconds / 0.05));
        var ms = Math.Max(1, (int)(seconds / steps * 1000));
        for (int i = 0; i < steps; i++)
        {
            Thread.Sleep(ms);
            if (Hotkeys.IsDown(_cfg.Hotkeys.Quit)) { _quitPressed = true; return; }
        }
    }

    private static void Beep(int freq, int ms)
    {
        try { Console.Beep(freq, ms); } catch { }
    }
}
