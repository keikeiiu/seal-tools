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
        var cooldowns = _cfg.Spammer.ActiveKeys;
        if (cooldowns.Count == 0)
        {
            // ActiveKeys no longer substitutes another preset, so an empty set means the active one
            // is missing (or none is configured). Say which, and don't run: pressing some other
            // rotation silently would be worse than pressing nothing.
            var msg = _cfg.Spammer.Presets.Count == 0
                ? "No spammer presets are configured — add one in the Spammer tab."
                : $"The active preset '{_cfg.Spammer.Active}' was not found — pick one in the Spammer tab.";
            Console.WriteLine("[!] " + msg);
            state.Message = msg;
            state.Running = false;
            return 0;
        }

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

        // Do NOT touch state.Running here: LauncherService starts the tool with Running = true and
        // this loop picks that up to begin. Clearing it would leave the tool paused on Start.
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

                bool disconnected = false;
                double now = sw.Elapsed.TotalSeconds;
                foreach (var (k, cd) in cooldowns)
                {
                    if (now - last[k] >= cd)
                    {
                        current = k;
                        last[k] = now; // advance the cooldown either way, so an unusable key isn't retried every tick

                        bool sent;
                        try
                        {
                            sent = SendKey(ser, k, state);
                        }
                        catch (Exception ex)
                        {
                            // The port is gone — unplugged, or the handle died. Nothing this tool does
                            // means anything now, so stop with a reason on the card rather than let the
                            // exception unwind to the launcher's generic handler.
                            state.Message = $"Arduino disconnected — stopped ({ex.Message})";
                            running = false;
                            state.Running = false;
                            disconnected = true;
                            break;
                        }

                        // Only count a press that actually went out. SendKey bails on a key the
                        // firmware cannot send, and counting it made the card report a rising Cycle
                        // and a changing Current while nothing was being pressed at all.
                        if (!sent) continue;

                        count++;
                        state.Current = k;
                        state.Cycle = count;
                    }
                }
                if (disconnected) break;
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

    /// <summary>Sends one key press. False when the key can't be sent at all — the caller must not
    /// count that as a press. A serial write failure is deliberately left to throw, so the caller
    /// stops the tool with a reason instead of pretending the press happened.</summary>
    private static bool SendKey(SerialPort ser, string key, ToolState state)
    {
        bool fast = key.StartsWith('*');
        if (fast) key = key.Substring(1);

        string cmd;
        if (key.StartsWith('F'))
        {
            if (!int.TryParse(key.AsSpan(1), NumberStyles.Integer, CultureInfo.InvariantCulture, out var f) || f is < 1 or > 10)
            {
                WarnUnsupported(key, state);
                return false;
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
            return false;
        }
        ser.Write(cmd);
        return true;
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
