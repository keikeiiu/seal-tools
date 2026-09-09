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

public sealed class SkillSpammer : ToolBase
{
    private readonly AppConfig _cfg;

    public SkillSpammer(AppConfig cfg)
        : base(cfg.Hotkeys)
    {
        _cfg = cfg;
    }

    public int Run(SerialPort ser, ToolState state, CancellationToken ct)
    {
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
                if (QuitPressed || ct.IsCancellationRequested) break;

                // Sync with the launcher's in-memory start/stop signal.
                if (state.Running && !running)
                {
                    running = true;
                    QuitPressed = false;
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
                    if (running) { QuitPressed = false; Reset(); Console.WriteLine($"[GO] {string.Join(", ", cooldowns.Keys)}"); Beep(523, 100); }
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
                        SendKey(ser, k, state);
                        last[k] = now;
                        count++;
                        state.Current = k;
                        state.Cycle = count;
                    }
                }
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

        Console.WriteLine("\nDone.");
        return 0;
    }

    private static readonly HashSet<string> WarnedKeys = new();

    private static void SendKey(SerialPort ser, string key, ToolState state)
    {
        bool fast = key.StartsWith('*');
        if (fast) key = key.Substring(1);

        string cmd;
        if (key.StartsWith('F'))
        {
            if (!int.TryParse(key.AsSpan(1), NumberStyles.Integer, CultureInfo.InvariantCulture, out var f) || f is < 1 or > 10)
            {
                WarnUnsupported(key, state);
                return;
            }
            cmd = (fast ? "f " : "F ") + f + "\n";
        }
        else if (key.Length == 1 && key[0] is >= '0' and <= '9')
        {
            cmd = (fast ? "k " : "K ") + key + "\n";
        }
        else
        {
            WarnUnsupported(key, state);
            return;
        }
        ser.Write(cmd);
    }

    // The Arduino firmware's K/k handler parses a digit (0–9) and its F handler supports F1–F10
    // only — a letter key would otherwise be silently pressed as '0'. Warn once per bad key, on
    // the launcher card as well as the console (which the published WinExe doesn't have).
    private static void WarnUnsupported(string key, ToolState state)
    {
        if (WarnedKeys.Add(key))
        {
            var msg = $"Spammer key '{key}' unsupported — only digits 0–9 and F1–F10 are sent.";
            Console.WriteLine("[!] " + msg);
            state.Message = msg;
        }
    }

}
