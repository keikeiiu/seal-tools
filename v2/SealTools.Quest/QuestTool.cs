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
    private readonly string? _sequenceOverride;
    private readonly string? _flowOverride;

    /// <param name="loopsOverride">How many loops to run instead of the flow's own count. The tab's
    /// **Test one loop** passes 1, so the test is THE SAME CODE as a real run — the guard, the
    /// validation and the sending are not re-implemented beside it, which is where a test and a run
    /// drift apart and the test starts passing things the run would refuse.</param>
    /// <param name="sequenceOverride">What the CARD chose to run, when it chose one. The card's quick
    /// choice wins over the tab's because that is the whole reason it is on the card; null means "use
    /// the tab's choice", which is what the tab's own Test buttons want.</param>
    /// <param name="flowOverride">As above, for a single flow.</param>
    public QuestTool(AppConfig cfg, int? loopsOverride = null, string? sequenceOverride = null,
        string? flowOverride = null)
        : base(cfg.Hotkeys)
    {
        _cfg = cfg;
        _loopsOverride = loopsOverride;
        _sequenceOverride = sequenceOverride;
        _flowOverride = flowOverride;
    }

    public int Run(SerialPort ser, ToolState state, CancellationToken ct)
    {
        var quest = _cfg.Quest;

        // WHAT THE CARD CHOSE, first — it exists so a run can be aimed without opening Configuration,
        // and a choice made there must not be quietly overridden by the tab.
        if (!string.IsNullOrWhiteSpace(_sequenceOverride))
        {
            if (!quest.Sequences.TryGetValue(_sequenceOverride.Trim(), out var fromCard))
                return Refuse(state, $"There is no sequence called \"{_sequenceOverride.Trim()}\" — " +
                                     "pick another on the card, or reload the config.");

            return RunSequence(ser, state, quest, fromCard, _sequenceOverride.Trim(), ct);
        }

        if (!string.IsNullOrWhiteSpace(_flowOverride))
        {
            if (!quest.Presets.TryGetValue(_flowOverride.Trim(), out var fromCard))
                return Refuse(state, $"There is no flow called \"{_flowOverride.Trim()}\" — pick " +
                                     "another on the card, or reload the config.");

            return RunFlow(ser, state, quest, _flowOverride.Trim(), fromCard,
                _loopsOverride ?? Math.Max(1, fromCard.Loops), ct);
        }

        // Then the TAB's choice. A SEQUENCE when one is chosen; an empty `active_sequence` — or a name
        // nobody recognises — runs the single flow, which is what every config written before sequences
        // says.
        if (quest.ActiveSequenceOrNull is { } sequence)
            return RunSequence(ser, state, quest, sequence, quest.ActiveSequence.Trim(), ct);

        return RunFlow(ser, state, quest, quest.Active, quest.ActivePreset,
            _loopsOverride ?? Math.Max(1, quest.ActivePreset.Loops), ct);
    }

    /// <summary>One flow, on its own: position, guard, then its steps for its own loop count.</summary>
    private int RunFlow(SerialPort ser, ToolState state, QuestConfig quest, string flow,
        QuestPreset preset, int loops, CancellationToken ct)
    {
        if (Validate(preset, flow) is { } problem) return Refuse(state, problem);

        state.Schedule = $"flow \"{flow}\" · {preset.Steps.Count} step(s) × {loops} loop(s)";
        Reset(state);

        Log($"\nQuest Hand-in — flow \"{flow}\"");
        Log($"  {preset.Steps.Count} step(s), {loops} loop(s), " +
            $"{preset.InitialWaitSeconds:0.#}s to place the mouse");
        foreach (var step in preset.Steps) Log("  · " + QuestAction.Describe(step));

        if (!Position(state, preset.InitialWaitSeconds, ct))
            return Cancelled(state, "during the positioning wait");

        if (!TryReadGuardReference(quest, state, out var start)) return Finish(state);

        for (int loop = 1; loop <= loops; loop++)
        {
            state.Cycle = loop;
            Log($"\n[loop {loop}/{loops}]");

            switch (RunSteps(ser, state, preset, "", start, quest.GuardPx, ct))
            {
                case PassResult.Done:
                    break;

                case PassResult.Cancelled:
                    return Cancelled(state, $"in loop {loop}");

                default:
                    state.Message = $"{state.Message} Stopped on loop {loop} of {loops}.";
                    Log("[!] " + state.Message);
                    Finish(state);
                    return 0;
            }
        }

        Finish(state);
        Log($"DONE — {loops} loop(s) of \"{flow}\"");
        return 0;
    }

    /// <summary>A sequence: A three times, then B twice, and the whole list repeated by a master loop.
    /// See docs/PLAN-QUEST.md §11.
    ///
    /// **ONE POSITIONING WINDOW, at the start.** A flow's own initial wait means *"when I am run alone"*
    /// and does not apply here: the tool never moves the cursor, so a flow inside a sequence clicks where
    /// the last one did, and a per-flow wait would ask the player to aim at something that is not going
    /// to happen.</summary>
    private int RunSequence(SerialPort ser, ToolState state, QuestConfig quest,
        QuestSequence sequence, string name, CancellationToken ct)
    {
        // EVERY entry, and every flow it names, is checked BEFORE anything is clicked — a sequence is
        // longer than a flow, so a mistake in it costs more.
        if (sequence.Entries.Count == 0)
            return Refuse(state, $"The sequence \"{name}\" has no entries — add some on the Quest tab.");

        var planned = new List<(string Flow, QuestPreset Preset, int Times)>();
        foreach (var entry in sequence.Entries)
        {
            var flowName = entry.Preset?.Trim() ?? "";
            if (!quest.Presets.TryGetValue(flowName, out var preset))
                return Refuse(state,
                    $"The sequence \"{name}\" names a flow \"{entry.Preset}\" that does not exist.");

            if (Validate(preset, flowName) is { } problem) return Refuse(state, problem);

            planned.Add((flowName, preset, Math.Max(1, entry.Times)));
        }

        var loops = Math.Max(1, _loopsOverride ?? sequence.Loops);
        var shape = string.Join(" + ", planned.Select(p => $"{p.Flow}×{p.Times}"));

        state.Schedule = $"sequence \"{name}\" · {loops} × ({shape})";
        Reset(state);

        Log($"\nQuest Hand-in — sequence \"{name}\"");
        Log($"  {loops} master loop(s) × ({shape}), " +
            $"{sequence.InitialWaitSeconds:0.#}s to place the mouse");
        foreach (var (flowName, preset, times) in planned)
        {
            Log($"  · {flowName} ×{times}");
            foreach (var step in preset.Steps) Log("      " + QuestAction.Describe(step));
        }

        if (!Position(state, sequence.InitialWaitSeconds, ct))
            return Cancelled(state, "during the positioning wait");

        if (!TryReadGuardReference(quest, state, out var start)) return Finish(state);

        for (int master = 1; master <= loops; master++)
        {
            state.Cycle = master;
            Log($"\n[master {master}/{loops}]");

            foreach (var (flowName, preset, times) in planned)
            {
                for (int pass = 1; pass <= times; pass++)
                {
                    Log($"  {flowName} {pass}/{times}");

                    switch (RunSteps(ser, state, preset, $"{flowName} {pass}/{times} · ",
                               start, quest.GuardPx, ct))
                    {
                        case PassResult.Done:
                            break;

                        case PassResult.Cancelled:
                            return Cancelled(state, $"at {flowName} {pass}/{times}");

                        default:
                            state.Message = $"{state.Message} Stopped during {flowName} {pass}/{times} " +
                                            $"of master loop {master}/{loops}.";
                            Log("[!] " + state.Message);
                            Finish(state);
                            return 0;
                    }
                }
            }
        }

        Finish(state);
        Log($"DONE — {loops} master loop(s) of \"{name}\"");
        return 0;
    }

    /// <summary>How one pass of a flow's steps ended.</summary>
    private enum PassResult
    {
        Done,
        StoppedByGuard,
        Cancelled,
    }

    /// <summary>One pass of a flow's steps, with the guard checked before every send.
    ///
    /// Shared by the single-flow and sequence runners rather than written twice: the two differ in how
    /// many times and in what order they call this, and in nothing else — and a second copy of the
    /// sending is where the guard would one day be checked in one path and not the other.</summary>
    private PassResult RunSteps(SerialPort ser, ToolState state, QuestPreset preset, string prefix,
        (int X, int Y) start, int guardPx, CancellationToken ct)
    {
        foreach (var step in preset.Steps)
        {
            if (ct.IsCancellationRequested || QuitPressed) return PassResult.Cancelled;

            // Checked BEFORE every send, because the cost of a missed check is a click at the wrong
            // place and the cost of the check is one cursor read.
            if (GuardProblem(start, guardPx) is { } drift)
            {
                state.Message = drift;
                return PassResult.StoppedByGuard;
            }

            state.Current = prefix + QuestAction.Describe(step);
            Send(ser, step);
            if (!Wait(step.DelaySeconds, ct)) return PassResult.Cancelled;
        }

        return PassResult.Done;
    }

    /// <summary>The positioning window — the whole of "positioning". The tool never moves the cursor, so
    /// where it sits when this ends is where every click lands. Counted out loud because the player is
    /// aiming at a moment. False means the run was cancelled.</summary>
    private bool Position(ToolState state, double seconds, CancellationToken ct)
    {
        for (double left = Math.Ceiling(seconds); left > 0; left--)
        {
            state.Current = $"move the mouse into place — {left:0}s";
            Log($"  place the mouse — {left:0}s");
            if (!Wait(1, ct)) return false;
        }

        return true;
    }

    /// <summary>The guard's reference, read ONCE per run and read STRICTLY. An unreadable cursor ends the
    /// run rather than letting it click on at a place nothing is watching — which is the difference from
    /// the tuner's guard: that one needs a game window it may not be able to measure, so it is documented
    /// as failing OPEN, and this one needs nothing but the cursor.</summary>
    private static bool TryReadGuardReference(QuestConfig quest, ToolState state, out (int X, int Y) start)
    {
        if (WindowFinder.LogicalCursorPosition() is { } cursor)
        {
            start = cursor;
            Log($"  cursor at ({start.X},{start.Y}) — moving more than {quest.GuardPx} stops the run");
            return true;
        }

        start = default;
        state.Message = "The cursor couldn't be read, so it can't be watched — stopping rather than " +
                        "clicking at a place nothing is checking.";
        Log("[!] " + state.Message);
        return false;
    }

    /// <summary>Why this flow cannot run, or null. Refused BEFORE anything is clicked: a step that sends
    /// nothing is indistinguishable from the game behaving differently today, and at a few hundred loops
    /// there is no way to tell afterwards.</summary>
    private static string? Validate(QuestPreset preset, string flow)
    {
        if (preset.Steps.Count == 0)
            return $"The flow \"{flow}\" has no steps — add some on the Quest tab.";

        for (int i = 0; i < preset.Steps.Count; i++)
            if (QuestAction.Complaint(preset.Steps[i]) is { } problem)
                return $"Flow \"{flow}\" step {i + 1}: {problem}";

        return null;
    }

    private static void Reset(ToolState state)
    {
        state.Cycle = 0;
        state.Current = null;
        state.Message = null;
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

    /// <summary>Writes a line to `logs/quest.log` beside the launcher, AND to the console.
    ///
    /// The file is the point. A tool's Console output goes NOWHERE in the launcher — the published
    /// WinExe has no console, and nothing in the launcher redirects one — so a run's progress had no
    /// durable record at all. That went unnoticed until this tool became the first that ENDS BY ITSELF:
    /// the card returns to "stopped", the same as a tool somebody stopped, and with no file there was
    /// nothing left to tell a completed run from an interrupted one. The pet feeder keeps pet.log for
    /// the same reason, in the same place.
    ///
    /// Every failure here is swallowed on purpose: logging must never be the reason a tool fails.</summary>
    private static void Log(string line)
    {
        try
        {
            Console.WriteLine(line);

            var dir = System.IO.Path.Combine(AppContext.BaseDirectory, "logs");
            System.IO.Directory.CreateDirectory(dir);
            System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "quest.log"),
                $"{DateTime.Now:yyyy-MM-dd HH:mm:ss} {line}\n");
        }
        catch
        {
            // Never the reason a run fails.
        }
    }

    /// <summary>Ends a run because it was ASKED to — the Stop button, or the quit hotkey. Logged
    /// separately from a completed run, because the card returns to "stopped" for both and this line is
    /// the only place the difference survives.</summary>
    private static int Cancelled(ToolState state, string where)
    {
        Log($"cancelled {where}");
        return Finish(state);
    }

    private static int Refuse(ToolState state, string why)
    {
        state.Message = why;
        Log("[!] " + why);
        return Finish(state);
    }
}
