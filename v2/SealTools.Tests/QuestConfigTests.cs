using System.Collections.Generic;
using SealTools.Core.Config;
using Xunit;

namespace SealTools.Tests;

/// <summary>The check that stops a Save from quietly re-pointing a sequence entry.
///
/// It lives in Core because the Launcher is not in this test project, and the failure it prevents is
/// silent: the entry editor used to fall back to the first flow it knew, the next Save wrote that in,
/// and a sequence then ran a different quest — with nothing on screen to say it had changed. The editor
/// now shows the missing name and the Save button calls this; the two halves that matter are "found"
/// and "found in a stable order", so both are pinned here.</summary>
public class QuestConfigTests
{
    private static QuestConfig WithFlows(params string[] names)
    {
        var config = new QuestConfig();
        foreach (var name in names) config.Presets[name] = new QuestPreset();
        return config;
    }

    private static QuestSequence SequenceWith(params string[] flows)
    {
        var sequence = new QuestSequence();
        foreach (var flow in flows) sequence.Entries.Add(new QuestSequenceEntry { Preset = flow });
        return sequence;
    }

    [Fact]
    public void ASequenceThatNamesRealFlowsIsNotDangling()
    {
        var quest = WithFlows("a", "b");
        quest.Sequences["run"] = SequenceWith("a", "b");

        Assert.Empty(quest.DanglingSequenceEntries());
    }

    [Fact]
    public void AnEntryWhoseFlowIsGoneIsReportedWithItsPosition()
    {
        var quest = WithFlows("a", "b");
        quest.Sequences["run"] = SequenceWith("a", "deleted");

        var dangling = quest.DanglingSequenceEntries();

        var only = Assert.Single(dangling);
        Assert.Equal("run", only.Sequence);
        // 1-based, because this number goes straight into a message the player reads next to the rows.
        Assert.Equal(2, only.Entry);
        Assert.Equal("deleted", only.Flow);
    }

    [Fact]
    public void EveryDanglingEntryIsReportedNotJustTheFirst()
    {
        var quest = WithFlows("a");
        quest.Sequences["run"] = SequenceWith("gone1", "a", "gone2");

        Assert.Equal(new[] { (1, "gone1"), (3, "gone2") },
            quest.DanglingSequenceEntries().Select(d => (d.Entry, d.Flow)).ToArray());
    }

    /// <summary>A blank entry is not a flow named "": the run looks that name up and finds nothing, so
    /// it has to be refused for the same reason a deleted one is. It is also a state the rows editor can
    /// produce, because a row's picker can be left on an empty item.</summary>
    [Fact]
    public void AnEmptyFlowNameIsDanglingToo()
    {
        var quest = WithFlows("a");
        quest.Sequences["run"] = SequenceWith("");

        Assert.Single(quest.DanglingSequenceEntries());
    }

    /// <summary>YamlDotNet writes null — not "" — into a <c>string</c> property whose key is present with
    /// no value, and local.yaml is hand-editable by contract. A null must be reported like a blank name,
    /// not thrown on: <c>Dictionary.ContainsKey(null)</c> throws, and that exception would escape the
    /// Save handler's InfoBar and turn the button into a silent no-op.</summary>
    [Fact]
    public void ANullFlowNameIsReportedAsEmptyInsteadOfThrowing()
    {
        var quest = WithFlows("a");
        var sequence = new QuestSequence();
        sequence.Entries.Add(new QuestSequenceEntry { Preset = null! });
        quest.Sequences["run"] = sequence;

        var only = Assert.Single(quest.DanglingSequenceEntries());
        Assert.Equal("", only.Flow);
    }

    /// <summary>A bare `entries:` key loads as a NULL list, not an empty one. There is nothing to
    /// judge, and throwing here would escape the Save handler's InfoBar and make the button a silent
    /// no-op for the rest of the session. The sequence is refused at Start instead, where
    /// <c>QuestTool.RunSequence</c> treats a null list as "no entries".</summary>
    [Fact]
    public void ANullEntryListIsNotDanglingAndDoesNotThrow()
    {
        var quest = WithFlows("a");
        quest.Sequences["run"] = new QuestSequence { Entries = null! };

        Assert.Empty(quest.DanglingSequenceEntries());
    }

    /// <summary>A bare `-` list item loads as a null ENTRY. It is reported like a blank name rather
    /// than throwing on the way to the lookup, so the Save gate can name the row. (The runner refuses
    /// it too — <c>RunSequence</c> reports an "empty entry" — but this Save no longer lets it get there.)</summary>
    [Fact]
    public void ANullEntryElementIsReportedAsEmpty()
    {
        var quest = WithFlows("a");
        var sequence = new QuestSequence();
        sequence.Entries.Add(null!);
        quest.Sequences["run"] = sequence;

        Assert.Equal("", Assert.Single(quest.DanglingSequenceEntries()).Flow);
    }

    /// <summary>The check must agree with the runner. `QuestTool.RunSequence` trims the name before
    /// looking it up, so a padded name that runs must not stop every Save on the tab.</summary>
    [Fact]
    public void APaddedNameTheRunnerWouldTrimIsNotDangling()
    {
        var quest = WithFlows("a");
        quest.Sequences["run"] = SequenceWith(" a ");

        Assert.Empty(quest.DanglingSequenceEntries());
    }

    /// <summary>The order is fixed so the same defect reads the same way twice — the message lists every
    /// hit, and a message whose lines move between saves is one the player cannot compare.</summary>
    [Fact]
    public void TheReportIsOrderedBySequenceThenPosition()
    {
        var quest = WithFlows();
        quest.Sequences["zeta"] = SequenceWith("missing");
        quest.Sequences["alpha"] = SequenceWith("missing", "missing");

        Assert.Equal(new[] { ("alpha", 1), ("alpha", 2), ("zeta", 1) },
            quest.DanglingSequenceEntries().Select(d => (d.Sequence, d.Entry)).ToArray());
    }

    /// <summary>The check covers EVERY sequence, not the active one: Save writes the whole quest block,
    /// so a dangling entry in a sequence the player is not looking at is still about to be saved.</summary>
    [Fact]
    public void AnInactiveSequenceIsCheckedToo()
    {
        var quest = WithFlows("a");
        quest.Sequences["active"] = SequenceWith("a");
        quest.Sequences["other"] = SequenceWith("gone");
        quest.ActiveSequence = "active";

        var dangling = quest.DanglingSequenceEntries();

        Assert.Equal("other", Assert.Single(dangling).Sequence);
    }
}
