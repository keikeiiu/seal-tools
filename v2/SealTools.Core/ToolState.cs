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
