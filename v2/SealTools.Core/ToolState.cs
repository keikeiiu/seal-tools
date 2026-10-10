using System;
using System.Collections.Generic;
using System.Linq;

namespace SealTools.Core;

// Live state of a running tool, shared between the tool thread and the WPF launcher.
// The launcher UI reads this directly — it does NOT parse the tool's console logs.
//
// Ownership model: single WRITER (the tool thread) and single READER (the UI thread via a
// 750 ms DispatcherTimer). Every field is only ever value/reference REPLACED, never mutated
// in place, so reads are atomic. Attributes is an immutable snapshot — the tool replaces the
// whole list each attempt and the UI enumerates the reference it read, so no lock is needed.
//
// One of these per running tool, not one per launcher: the resident Pet Feeder runs alongside a
// foreground tool, and each owns its own instance. The model above is per instance and is unchanged
// by that — what the launcher must not do is read a tool's state from a thread that is not the UI's.
public sealed class ToolState
{
    private string[] _attributes = Array.Empty<string>();

    public bool Running { get; set; }
    public string? Grade { get; set; }
    public int? Remaining { get; set; }
    public int Attempt { get; set; }
    public int Cycle { get; set; }
    public string? Current { get; set; }
    public string FilterStatus { get; set; } = "";

    // The tool's STANDING plan, as opposed to Message which is about the last thing that happened.
    // The pet feeder is the reason it exists: a reload is hours apart, so a card that only reported
    // the last event said nothing at all for most of the run. Schedule is the text; NextActionAt is
    // the moment, so the card can count down to it while the UI ticks.
    public string? Schedule { get; set; }
    public DateTime? NextActionAt { get; set; }

    // ONE ENTRY PER ROW, for the tools that have rows — the pet feeder's four breeding rows are the
    // only ones. The player asked for it in as many words: *"if I have 4 pets pending to feed …
    // list out all 4 conditions"* — the summary above says which row is soonest, and it cannot say
    // that row 4 is boarding with 675 items while row 2 is waiting for a pet. Each entry carries its
    // OWN moment so the card can recompute every row's countdown on each UI tick, exactly as
    // NextActionAt does for the summary. Same replace-wholesale rule as Attributes: the tool thread
    // swaps the whole list, never mutates one in place.
    private IReadOnlyList<ToolRowState> _rows = Array.Empty<ToolRowState>();
    public IReadOnlyList<ToolRowState> Rows
    {
        get => _rows;
        set => _rows = value?.ToArray() ?? Array.Empty<ToolRowState>();
    }

    // One-off user-visible notice shown on the launcher card — for things the user must see but
    // that would otherwise only reach Console.WriteLine, which is invisible in the published
    // WinExe (no console attached). Null/empty when there is nothing to report.
    public string? Message { get; set; }

    // Immutable snapshot: the tool thread replaces the whole list each attempt (never mutates
    // it in place), so the UI can safely enumerate the reference it read. The setter copies to
    // a fresh array so no caller can mutate the shared instance after the fact.
    public IReadOnlyList<string> Attributes
    {
        get => _attributes;
        set => _attributes = value.ToArray();
    }
}

/// <summary>One row of a tool's standing plan — the pet feeder's four breeding rows.
///
/// <paramref name="Status"/> is the short state word the card shows ("boarding", "not boarding",
/// "given up"); <paramref name="Detail"/> is the last reading about that row (the pet it boarded and
/// its feeder count, or null when there is none to show); <paramref name="Why"/> is WHY the next time
/// is what it is — the arithmetic and the reading behind it, written where the number was produced so
/// the two cannot drift; <paramref name="NextAt"/> is when this row next needs attention, or null
/// when it never will again (a row that was dropped).</summary>
public sealed record ToolRowState(string Name, string Status, string? Detail, string? Why,
    DateTime? NextAt);
