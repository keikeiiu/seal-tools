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

    private CancellationToken _runToken;
    private bool _cancellationMuted;

    /// <summary>Runs teardown that must finish even though the run is being stopped.
    ///
    /// A STOP ABANDONS THE WORK, NOT THE CLEANUP, and this is where that distinction lives.
    /// <see cref="SleepCheck"/> throws so a step in progress can be abandoned — but it throws from ANY
    /// point, and the code that CLOSES A WINDOW or LETS GO OF A BUTTON also sleeps. Without this, a
    /// Stop arriving inside such a teardown aborted the teardown itself:
    ///
    ///   * the pet left its boarding window open on top of the game, and the code says in as many
    ///     words that the next cycle then clicks 目錄 behind it;
    ///   * a `finally` that throws REPLACES the exception it was unwinding, so a real crash arriving
    ///     next to a Stop was filed as a clean stop — the one outcome worse than a noisy failure.
    ///
    /// Nestable, and it restores the previous value rather than clearing it, so a muted section inside
    /// a muted section cannot un-mute the outer one early.</summary>
    protected void WithoutCancellation(Action teardown)
    {
        var wasMuted = _cancellationMuted;
        _cancellationMuted = true;
        try { teardown(); }
        finally { _cancellationMuted = wasMuted; }
    }

    /// <summary>Hands the run's cancellation token to the base, so <see cref="SleepCheck"/> can see a
    /// stop. Every tool calls this as the first line of its Run — without it a sleep is deaf to the
    /// stop signal, which is the whole bug this exists to fix.
    ///
    /// NOT folded into the constructor: the token is not known when the tool is built. The launcher
    /// constructs a tool and immediately calls Run with the token, so construction is the wrong place
    /// and Run is the right one.</summary>
    protected void WatchCancellation(CancellationToken ct) => _runToken = ct;

    /// <summary>When true, SleepCheck never raises <see cref="QuitPressed"/>, so this tool can only be
    /// stopped by its own Stop button.
    ///
    /// The quit hotkey is read from GLOBAL OS key state, which was fine while only one tool could run:
    /// the press meant "stop the tool". With the pet feeder resident there are two loops, and one press
    /// would stop BOTH — so quitting a buy run would silently end a schedule that is feeding four pets.
    /// The pet feeder is the tool that sets this; the hotkey stays fully in charge of the foreground
    /// tool, which is what it was for.</summary>
    protected bool IgnoresQuitHotkey { get; }

    /// <summary>Sleeps in 50 ms chunks, watching for a stop.
    ///
    /// A STOP THROWS RATHER THAN SETTING A FLAG, and that is the fix rather than a style choice. The
    /// tools' loops already read the token, but a loop is only reached when the step it is running has
    /// finished — and a pet feeder's step can be a whole visit: several rows of OCR, then a reload
    /// whose wait alone is ~30 s. So a stop was honoured only at a visit boundary, and the player's
    /// Stop button looked broken for as long as that took. There is no flag the caller could read in
    /// the middle of that work, so the work has to be abandoned from the inside, and an exception is
    /// how you unwind a step in progress. The tools' own finally blocks then run — the pet releases
    /// the game, a drag lets go of the button — which is exactly the teardown a stop wants.
    ///
    /// The token is checked BEFORE the hotkey on every tick, so a stop is not delayed behind a key
    /// read, and a cancelled run cannot be revived by the hotkey paths that clear <see cref="QuitPressed"/>
    /// to mean "pause", not "quit".
    ///
    /// A tool that catches this and carries on is not a hazard: the token stays cancelled, so the very
    /// next SleepCheck throws again.</summary>
    protected void SleepCheck(double seconds)
    {
        var steps = Math.Max(1, (int)(seconds / 0.05));
        var ms = Math.Max(1, (int)(seconds / steps * 1000));
        for (int i = 0; i < steps; i++)
        {
            Thread.Sleep(ms);
            // Muted inside WithoutCancellation: a stop abandons the work, not the cleanup.
            if (!_cancellationMuted) _runToken.ThrowIfCancellationRequested();
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
