using System;
using System.Threading;
using SealTools.Core.Config;

namespace SealTools.Core;

// Shared hotkey-aware sleep + beep helpers for the three tools. Each tool loop ticks in 50 ms
// chunks, checking the quit/pause hotkeys so it can stop promptly; centralizing this means a
// change to the hotkey check applies to every tool at once instead of drifting per copy.
public abstract class ToolBase
{
    private readonly HotkeysConfig _hotkeys;

    protected ToolBase(HotkeysConfig hotkeys, bool ignoresQuitHotkey = false)
    {
        _hotkeys = hotkeys;
        IgnoresQuitHotkey = ignoresQuitHotkey;
    }

    // Set by SleepCheck when the quit/pause hotkey is seen; the tools poll these each tick.
    protected bool QuitPressed { get; set; }
    protected bool PauseRequested { get; set; }

    /// <summary>When true, SleepCheck never raises <see cref="QuitPressed"/>, so this tool can only be
    /// stopped by its own Stop button.
    ///
    /// The quit hotkey is read from GLOBAL OS key state, which was fine while only one tool could run:
    /// the press meant "stop the tool". With the pet feeder resident there are two loops, and one press
    /// would stop BOTH — so quitting a buy run would silently end a schedule that is feeding four pets.
    /// The pet feeder is the tool that sets this; the hotkey stays fully in charge of the foreground
    /// tool, which is what it was for.</summary>
    protected bool IgnoresQuitHotkey { get; }

    protected void SleepCheck(double seconds)
    {
        var steps = Math.Max(1, (int)(seconds / 0.05));
        var ms = Math.Max(1, (int)(seconds / steps * 1000));
        for (int i = 0; i < steps; i++)
        {
            Thread.Sleep(ms);
            if (!IgnoresQuitHotkey && Hotkeys.IsDown(_hotkeys.Quit)) { QuitPressed = true; return; }
            if (Hotkeys.IsDown(_hotkeys.Pause)) { PauseRequested = true; }
        }
    }

    protected static void Beep(int freq, int ms)
    {
        try { Console.Beep(freq, ms); } catch { }
    }

    protected static void BeepMany()
    {
        for (int i = 0; i < 5; i++)
        {
            Beep(1200, 200);
            Thread.Sleep(100); // 0.1 s gap so the five beeps read as distinct (matches v1)
        }
    }
}
