using System.Collections.Generic;
using System.Linq;

namespace SealTools.Core;

/// <summary>How many compose cycles a run has spent on each grade.
///
/// The composer's single Cycle counter says how much work a run has done and not where it went, and the
/// interesting number is per grade: a run that took 40 cycles on N and 3 on DG is telling you something
/// a total of 43 cannot. The player asked for it in as many words — *"N how many cycle, G how many
/// cycle, DG how many cycle"*.
///
/// Separate from the composer so the counting and its two renderings can be tested without a serial
/// port, a game window or a capture: the tool is none of those things' friend.</summary>
public sealed class GradeCycles
{
    private readonly Dictionary<string, int> _counts = new();

    /// <summary>One more cycle spent on <paramref name="grade"/>.</summary>
    public void Add(string grade) => _counts[grade] = CountFor(grade) + 1;

    /// <summary>Cycles spent on <paramref name="grade"/> so far — 0 for one not reached yet.</summary>
    public int CountFor(string grade) => _counts.TryGetValue(grade, out var n) ? n : 0;

    /// <summary>A new run: every count back to zero. Called when the run is started — by the Start key
    /// and by the panel's Start — so the numbers on the card are this run's.</summary>
    public void Reset() => _counts.Clear();

    /// <summary>One entry per grade, in the order the composer runs them, for the launcher card. Every
    /// grade appears including the ones not reached: "DG: 0 cycles" is information, and a list that
    /// grows as the run proceeds moves under the reader's eye.</summary>
    public IReadOnlyList<ToolRowState> Rows(IReadOnlyList<string> grades) =>
        grades.Select(g => new ToolRowState(g, Describe(CountFor(g)), null, null)).ToList();

    /// <summary>The whole tally on one line, for the log — `N 12 · G 8 · DG 3 · 23 total`. The total is
    /// the sum of the grades rather than a separate counter, so it cannot disagree with them.</summary>
    public string Summary(IReadOnlyList<string> grades) =>
        string.Join(" · ", grades.Select(g => $"{g} {CountFor(g)}")) + $" · {Total(grades)} total";

    /// <summary>Cycles this tally has counted, across its grades.</summary>
    public int Total(IReadOnlyList<string> grades) => grades.Sum(CountFor);

    /// <summary>`1 cycle` / `4 cycles` — one wording, so the card and the log cannot disagree, and
    /// "1 cycles" never reaches a reader.</summary>
    public static string Describe(int cycles) => cycles == 1 ? "1 cycle" : $"{cycles} cycles";
}
