using System;
using System.IO.Ports;
using System.Threading;
using SealTools.Core;
using SealTools.Core.Config;

namespace SealTools.Quest;

/// <summary>Hands in quests repeatedly by replaying a flow at the quest NPC.
///
/// **It reads nothing** — no capture, no OCR, no window lookup — which is why it has no calibration at
/// all. The player puts the mouse where the clicks have to land during the initial wait, the tool clicks
/// wherever it is, and the cursor is then watched so a run STOPS if it moves. See docs/PLAN-QUEST.md.
///
/// The trade that buys, in the plan's words: **a wrong flow does <c>Loops</c> wrong actions and the game
/// accepts every one of them.** Nothing in here can notice that — only the player, watching one test
/// loop before committing to the count. That is what "Test one loop" on the tab is for.
///
/// <see cref="ToolState.Running"/> is NOT set here. LauncherService creates the state already running
/// and tears it down when this returns — a tool that set it would be fighting the service for the field.</summary>
public sealed class QuestTool : ToolBase
{
    private readonly AppConfig _cfg;
    private readonly int? _loopsOverride;

    /// <param name="loopsOverride">How many loops to run instead of the flow's own count. The tab's
    /// **Test one loop** passes 1, so the test is THE SAME CODE as a real run — the guard, the
    /// validation and the sending are not re-implemented beside it, which is where a test and a run
    /// drift apart and the test starts passing things the run would refuse.</param>
    public QuestTool(AppConfig cfg, int? loopsOverride = null)
        : base(cfg.Hotkeys)
    {
        _cfg = cfg;
        _loopsOverride = loopsOverride;
    }

    public int Run(SerialPort ser, ToolState state, CancellationToken ct)
    {
        var quest = _cfg.Quest;
        var flow = quest.Active;
        var preset = quest.ActivePreset;

        // A FLOW THAT CANNOT BE SENT IS REFUSED BEFORE ANYTHING IS CLICKED. A bad step sends nothing, and
        // in a flow replayed 500 times "sends nothing" is indistinguishable from the game behaving
        // differently today — so it is caught here, where the message can still be read and acted on.
        for (int i = 0; i < preset.Steps.Count; i++)
        {
            if (QuestAction.Complaint(preset.Steps[i]) is { } problem)
                return Refuse(state, $"Step {i + 1}: {problem}");
        }

        if (preset.Steps.Count == 0)
            return Refuse(state, $"The flow \"{flow}\" has no steps — add some on the Quest tab.");

        var loops = Math.Max(1, _loopsOverride ?? preset.Loops);

        state.Schedule = $"flow \"{flow}\" · {preset.Steps.Count} step(s) × {loops} loop(s)";
        state.Cycle = 0;
        state.Current = null;
        state.Message = null;

        Console.WriteLine($"\nQuest Hand-in — flow \"{flow}\"");
        Console.WriteLine($"  {preset.Steps.Count} step(s), {loops} loop(s), " +
                          $"{preset.InitialWaitSeconds:0.#}s to place the mouse");
        foreach (var step in preset.Steps) Console.WriteLine("  · " + QuestAction.Describe(step));

        // THE POSITIONING WINDOW, and it is the whole of "positioning". The tool never moves the cursor,
        // so where it sits when this ends is where every click in the flow lands — the tuner's `manual`
        // spring mode, working the same way. Counted out loud because the player is aiming at a moment.
        for (double left = Math.Ceiling(preset.InitialWaitSeconds); left > 0; left--)
        {
            state.Current = $"move the mouse into place — {left:0}s";
            Console.WriteLine($"  place the mouse — {left:0}s");
            if (!Wait(1, ct)) return Finish(state);
        }

        // THE GUARD'S REFERENCE, read ONCE here and read STRICTLY. An unreadable cursor ends the run
        // rather than letting it click on at a place nothing is watching — which is the difference from
        // the tuner's guard: that one needs a game window it may not be able to measure, so it is
        // documented as failing OPEN, and this one needs nothing but the cursor itself.
        if (WindowFinder.LogicalCursorPosition() is not { } start)
        {
            return Refuse(state,
                "The cursor couldn't be read, so it can't be watched — stopping rather than clicking at " +
                "a place nothing is checking.");
        }

        Console.WriteLine($"  cursor at ({start.X},{start.Y}) — moving more than {quest.GuardPx} stops the run");

        var stopped = false;
        for (int loop = 1; loop <= loops; loop++)
        {
            state.Cycle = loop;
            Console.WriteLine($"\n[loop {loop}/{loops}]");

            foreach (var step in preset.Steps)
            {
                if (ct.IsCancellationRequested || QuitPressed) return Finish(state);

                // Checked BEFORE every send, because the cost of a missed check is a click at the wrong
                // place and the cost of the check is a cursor read.
                if (GuardProblem(start, quest.GuardPx) is { } drift)
                {
                    stopped = true;
                    state.Message = $"{drift} Stopped on loop {loop} of {loops}.";
                    Console.WriteLine("[!] " + state.Message);
                    break;
                }

                state.Current = QuestAction.Describe(step);
                Send(ser, step);
                if (!Wait(step.DelaySeconds, ct)) return Finish(state);
            }

            if (stopped) break;
        }

        Finish(state);
        Console.WriteLine(stopped ? "\nStopped by the mouse guard." : $"\nDone — {loops} loop(s).");
        return 0;
    }

    /// <summary>Why the cursor says the run should stop, or null when it is where the player left it.
    ///
    /// Both reads are in the same space, which is what matters: this process is DPI-unaware, so both are
    /// logical pixels and the threshold is in those. On a 150 % display that makes the guard stricter in
    /// physical pixels than its number suggests, which is the safe direction for something whose whole
    /// job is to notice a hand on the mouse.</summary>
    private static string? GuardProblem((int X, int Y) from, int guardPx)
    {
        if (WindowFinder.LogicalCursorPosition() is not { } now)
            return "The cursor stopped being readable, so it can't be watched any more.";

        var moved = Math.Max(Math.Abs(now.X - from.X), Math.Abs(now.Y - from.Y));
        return moved > guardPx
            ? $"The mouse moved — it is at ({now.X},{now.Y}) and started at ({from.X},{from.Y})."
            : null;
    }

    /// <summary>One step's input. The board's protocol is one command per line: <c>C</c> and <c>R</c>
    /// click, <c>E</c> is Enter, and <c>K</c> presses one printable character — which is why a key step
    /// carries exactly one, and anything else was refused before the run started.</summary>
    private static void Send(SerialPort ser, QuestStep step)
    {
        switch (step.Action?.Trim().ToLowerInvariant())
        {
            case QuestAction.Click:
                HidPointer.Click(ser);
                break;
            case QuestAction.RightClick:
                HidPointer.RightClick(ser);
                break;
            case QuestAction.Enter:
                ser.Write("E\n");
                break;
            case QuestAction.Key:
                ser.Write($"K {step.Value}\n");
                break;
            default:
                // A wait, or anything else — and anything else cannot get here: it was refused above.
                break;
        }
    }

    /// <summary>SleepCheck ticks 50 ms and watches the HOTKEYS, but it knows nothing about the
    /// CancellationToken — so a ten-second positioning wait would ignore the Stop button for ten
    /// seconds, which is precisely the moment a player is most likely to change their mind. This ticks
    /// the same way and checks both. False means stop.</summary>
    private bool Wait(double seconds, CancellationToken ct)
    {
        var ticks = Math.Max(1, (int)Math.Round(seconds / 0.05));
        for (int i = 0; i < ticks; i++)
        {
            if (ct.IsCancellationRequested || QuitPressed) return false;
            SleepCheck(0.05);
        }

        return !ct.IsCancellationRequested && !QuitPressed;
    }

    /// <summary>Clears the standing lines so a stopped run stops reporting a plan and a current step.
    /// The Message is left alone: it is how a guard stop or a refusal says what happened, and it is the
    /// one thing the player needs to read afterwards.</summary>
    private static int Finish(ToolState state)
    {
        state.Schedule = null;
        state.Current = null;
        return 0;
    }

    private static int Refuse(ToolState state, string why)
    {
        state.Message = why;
        Console.WriteLine("[!] " + why);
        return Finish(state);
    }
}
