using System.Globalization;

namespace SealTools.Core.Config;

/// <summary>Making a copy of a named preset: the name the copy gets, and the copy itself.
///
/// It lives here rather than beside the buttons that call it, because the Launcher is net8.0-windows and
/// stays out of the test project — and both ways a copy goes wrong are pure logic over the config types:
/// a SHALLOW copy leaves the copy and the original sharing one step list (so editing either edits both),
/// and a generated name that collides with a key already in the dictionary silently REPLACES it, because
/// a plain <see cref="Dictionary{TKey,TValue}"/> assignment overwrites rather than refusing.
///
/// A name is the identity of a flow, a sequence and a spammer preset alike, which is why the name is
/// generated here and not left to a prompt.</summary>
public static class PresetCopy
{
    /// <summary>The name a copy of <paramref name="source"/> gets: "<c>&lt;name&gt; (copy)</c>", then
    /// "<c>&lt;name&gt; (copy 2)</c>", up to the first one that is free.
    ///
    /// The source's own suffix is stripped FIRST, and the count resumes from it. Without the strip the
    /// second copy of a flow would be "mine (copy) (copy)"; without resuming, copying "mine (copy 2)"
    /// would walk backwards into a free "mine (copy)" and produce a name that sorts above the preset it
    /// came from.
    ///
    /// <paramref name="taken"/> is compared case-INSENSITIVELY even though the dictionaries are keyed
    /// case-sensitively, because the pickers sort ignore-case: "mine (copy)" beside "Mine (copy)" is two
    /// rows the player cannot tell apart, and a generated name should never be what creates one.</summary>
    public static string CopyName(string source, IEnumerable<string> taken)
    {
        var stem = source;
        var start = 0;

        const string CopySuffix = " (copy)";
        if (stem.EndsWith(CopySuffix, StringComparison.Ordinal))
        {
            stem = stem[..^CopySuffix.Length];
            start = 1;
        }
        else
        {
            // " (copy <n>)" — the numbered form. Anything else ending in ")" (a name the player typed
            // that happens to look like it) is left alone, because the digits are what make it ours.
            const string numbered = " (copy ";
            var open = stem.LastIndexOf(numbered, StringComparison.Ordinal);
            if (open >= 0 && stem.EndsWith(')') &&
                int.TryParse(stem[(open + numbered.Length)..^1], NumberStyles.None,
                    CultureInfo.InvariantCulture, out var n) && n >= 1)
            {
                stem = stem[..open];
                start = n;
            }
        }

        for (var k = start + 1; ; k++)
        {
            var candidate = k == 1 ? $"{stem} (copy)" : $"{stem} (copy {k})";
            if (!taken.Any(t => string.Equals(t, candidate, StringComparison.OrdinalIgnoreCase)))
                return candidate;
        }
    }

    /// <summary>A copy of one flow that shares nothing with it.
    ///
    /// Element by element, because <see cref="QuestPreset.Steps"/> is a list of <see cref="QuestStep"/>
    /// objects: copying the list alone would leave the copy and the original editing the same steps, and
    /// nothing on screen would say so.</summary>
    public static QuestPreset CopyQuestPreset(QuestPreset source) => new()
    {
        InitialWaitSeconds = source.InitialWaitSeconds,
        Loops = source.Loops,
        Steps = source.Steps.Select(s => new QuestStep
        {
            Action = s.Action,
            Value = s.Value,
            DelaySeconds = s.DelaySeconds,
        }).ToList(),
    };

    /// <summary>A copy of one sequence that shares nothing with it — the same element-by-element reason
    /// as <see cref="CopyQuestPreset"/>.
    ///
    /// Its entries still name the SAME flows. Duplicating a sequence is not a reason for every flow in
    /// it to be duplicated too, and pointing a sequence at a copy is one edit in its entry editor.</summary>
    public static QuestSequence CopyQuestSequence(QuestSequence source) => new()
    {
        InitialWaitSeconds = source.InitialWaitSeconds,
        Loops = source.Loops,
        Entries = source.Entries.Select(e => new QuestSequenceEntry
        {
            Preset = e.Preset,
            Times = e.Times,
        }).ToList(),
    };
}
