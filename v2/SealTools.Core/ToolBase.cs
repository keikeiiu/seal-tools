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

    protected ToolBase(HotkeysConfig hotkeys) => _hotkeys = hotkeys;

    // Set by SleepCheck when the quit/pause hotkey is seen; the tools poll these each tick.
    protected bool QuitPressed { get; set; }
    protected bool PauseRequested { get; set; }

    protected void SleepCheck(double seconds)
    {
        var steps = Math.Max(1, (int)(seconds / 0.05));
        var ms = Math.Max(1, (int)(seconds / steps * 1000));
        for (int i = 0; i < steps; i++)
        {
            Thread.Sleep(ms);
            if (Hotkeys.IsDown(_hotkeys.Quit)) { QuitPressed = true; return; }
            if (Hotkeys.IsDown(_hotkeys.Pause)) { PauseRequested = true; }
        }
    }

    protected static void Beep(int freq, int ms)
    {
        try { Console.Beep(freq, ms); } catch { }
    }

    protected static void BeepMany()
    {
        for (int i = 0; i < 5; i++) Beep(1200, 200);
    }
}
