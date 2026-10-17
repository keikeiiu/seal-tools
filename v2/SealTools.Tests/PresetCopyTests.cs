using System.Collections.Generic;
using SealTools.Core.Config;
using Xunit;

namespace SealTools.Tests;

/// <summary>The Duplicate button's two halves, tested apart from the button.
///
/// Neither failure mode is visible on screen: a shallow copy makes an edit to the copy land on the
/// original as well, and a colliding name REPLACES the preset it was meant to sit beside — a plain
/// dictionary assignment overwrites rather than refusing. Both are silent, and both are pure logic over
/// the config types, so both are pinned here rather than left to a manual run.</summary>
public class PresetCopyTests
{
    // The names already in play at the call. A List rather than an array literal, because the analyzer
    // treats a constant array passed as an argument as a per-call allocation.
    private static List<string> Taken(params string[] names) => new(names);

    [Fact]
    public void TheFirstCopyIsNamedCopy()
    {
        Assert.Equal("mine (copy)", PresetCopy.CopyName("mine", Taken("mine")));
    }

    [Fact]
    public void TheCountRisesUntilTheNameIsFree()
    {
        Assert.Equal("mine (copy 2)", PresetCopy.CopyName("mine", Taken("mine", "mine (copy)")));
        Assert.Equal("mine (copy 3)",
            PresetCopy.CopyName("mine", Taken("mine", "mine (copy)", "mine (copy 2)")));
    }

    /// <summary>The whole reason the suffix is stripped before counting. Copying the copy must not
    /// produce "mine (copy) (copy)" — a name that grows every time and says nothing.</summary>
    [Fact]
    public void CopyingACopyContinuesTheCount()
    {
        Assert.Equal("mine (copy 2)", PresetCopy.CopyName("mine (copy)", Taken("mine (copy)")));
    }

    /// <summary>And it must not walk BACKWARDS: with "mine (copy)" free and the source at
    /// "mine (copy 2)", restarting the count would hand back a name that sorts above the preset it was
    /// copied from, which reads as a different preset entirely.</summary>
    [Fact]
    public void CopyingANumberedCopyDoesNotWalkBackwards()
    {
        Assert.Equal("mine (copy 3)",
            PresetCopy.CopyName("mine (copy 2)", Taken("mine", "mine (copy)", "mine (copy 2)")));
    }

    /// <summary>The pickers sort ignore-case, so "mine (copy)" beside "Mine (copy)" is two rows the
    /// player cannot tell apart. The dictionaries are keyed case-SENSITIVELY and would accept both, so
    /// the generated name is the one place that has to refuse.</summary>
    [Fact]
    public void ANameThatOnlyDiffersByCaseIsTreatedAsTaken()
    {
        Assert.Equal("mine (copy 2)", PresetCopy.CopyName("mine", Taken("mine", "Mine (Copy)")));
    }

    /// <summary>A name the player typed that merely ends in something paren-shaped keeps all of it: the
    /// digits are what mark a suffix as ours, and "chapter (copy x)" has none.</summary>
    [Fact]
    public void ALookAlikeSuffixIsLeftAlone()
    {
        Assert.Equal("chapter (copy x) (copy)",
            PresetCopy.CopyName("chapter (copy x)", Taken("chapter (copy x)")));
    }

    /// <summary>"(copy 0)" is not a suffix we wrote — the count starts at 1 — so it is a literal name
    /// and the copy is appended after it. Pins the <c>n &gt;= 1</c> half of the guard.</summary>
    [Fact]
    public void ACopyZeroSuffixIsLeftAlone()
    {
        Assert.Equal("mine (copy 0) (copy)",
            PresetCopy.CopyName("mine (copy 0)", Taken("mine (copy 0)")));
    }

    /// <summary>The parsed index is an int, and int.MaxValue is valid input. The counter used to be an
    /// int too, so start + 1 wrapped to int.MinValue and the copy was named "(copy -2147483648)". The
    /// counter is a long now; this pins it so a later "simplification" back to int fails here.</summary>
    [Fact]
    public void TheCopyCountDoesNotOverflowAtTheIntegerBoundary()
    {
        var name = PresetCopy.CopyName("mine (copy 2147483647)", Taken("mine (copy 2147483647)"));

        Assert.Equal("mine (copy 2147483648)", name);
        Assert.DoesNotContain('-', name);
    }

    // ── the copy itself ─────────────────────────────────────────────────────

    [Fact]
    public void ACopiedFlowSharesNothingWithItsSource()
    {
        var source = new QuestPreset
        {
            InitialWaitSeconds = 7.5,
            Loops = 500,
            Steps = new List<QuestStep>
            {
                new() { Action = SealTools.Core.QuestAction.Click, DelaySeconds = 0.3 },
                new() { Action = SealTools.Core.QuestAction.Key, Value = "1", DelaySeconds = 0.2 },
            },
        };

        var copy = PresetCopy.CopyQuestPreset(source);

        Assert.Equal(7.5, copy.InitialWaitSeconds);
        Assert.Equal(500, copy.Loops);
        Assert.Equal(2, copy.Steps.Count);
        Assert.Equal("1", copy.Steps[1].Value);
        // Key is not QuestAction's default (Click), so a copy that dropped Action is caught here;
        // without this the rest of the assertions pass on a step that silently became a click.
        Assert.Equal(SealTools.Core.QuestAction.Key, copy.Steps[1].Action);
        // Every field, and none of these numbers is the type's default (10 / 1 / 0.25): an assertion
        // whose expected value IS the default passes against a copy that dropped the field entirely.
        // This one is not decoration — deleting the DelaySeconds line from CopyQuestPreset was tried,
        // and until this assertion existed the whole suite still went green.
        Assert.Equal(0.3, copy.Steps[0].DelaySeconds);
        Assert.Equal(0.2, copy.Steps[1].DelaySeconds);
        Assert.NotSame(source.Steps, copy.Steps);
        Assert.NotSame(source.Steps[0], copy.Steps[0]);

        // Edit the copy the way the step editor does, and the source must not move. This is the whole
        // test: a shared list passes every equality assertion above and fails only here.
        copy.Steps[0].Action = SealTools.Core.QuestAction.Enter;
        copy.Steps[1].DelaySeconds = 9.5;
        copy.Steps.Add(new QuestStep { Action = SealTools.Core.QuestAction.Wait });

        Assert.Equal(SealTools.Core.QuestAction.Click, source.Steps[0].Action);
        Assert.Equal(0.2, source.Steps[1].DelaySeconds);
        Assert.Equal(2, source.Steps.Count);
    }

    [Fact]
    public void ACopiedSequenceSharesNothingWithItsSource()
    {
        var source = new QuestSequence
        {
            InitialWaitSeconds = 12.5,
            Loops = 5,
            Entries = new List<QuestSequenceEntry>
            {
                new() { Preset = "a", Times = 3 },
                new() { Preset = "b", Times = 2 },
            },
        };

        var copy = PresetCopy.CopyQuestSequence(source);

        Assert.Equal(12.5, copy.InitialWaitSeconds);
        Assert.Equal(5, copy.Loops);
        Assert.Equal(2, copy.Entries.Count);
        Assert.NotSame(source.Entries, copy.Entries);
        Assert.NotSame(source.Entries[0], copy.Entries[0]);
        Assert.Equal("a", copy.Entries[0].Preset);
        Assert.Equal(3, copy.Entries[0].Times);
        Assert.Equal("b", copy.Entries[1].Preset);
        Assert.Equal(2, copy.Entries[1].Times);

        copy.Entries[0].Times = 9;
        copy.Entries.RemoveAt(1);

        Assert.Equal(3, source.Entries[0].Times);
        Assert.Equal(2, source.Entries.Count);
    }

    [Fact]
    public void ACopiedSpammerPresetSharesNothingWithItsSource()
    {
        var source = new Dictionary<string, double> { ["*1"] = 0.2, ["*F1"] = 3.5 };

        var copy = PresetCopy.CopySpammerKeys(source);

        Assert.Equal(0.2, copy["*1"]);
        Assert.Equal(3.5, copy["*F1"]);

        // A Dictionary constructor copies the ENTRIES, so this holds; it is asserted because the day
        // someone "optimises" the copy into handing the same map to the caller, this is the only line
        // that notices — and the two presets would then edit each other's cooldowns invisibly.
        copy["*1"] = 9.5;
        copy.Remove("*F1");

        Assert.Equal(0.2, source["*1"]);
        Assert.True(source.ContainsKey("*F1"));
    }
}
