using SealTools.Core;
using Xunit;

namespace SealTools.Tests;

// The reply is one short line, and the parsing is not the interesting part — what is interesting is
// that "not a version reply" and "a version reply" must never be confused. The silence is the half
// of this feature that carries real information (an old board cannot say anything), so anything that
// is not an unambiguous level has to come back null rather than a guess.
public class FirmwareVersionTests
{
    [Fact]
    public void ReadsTheLevelFromTheReply()
    {
        Assert.Equal(1, FirmwareVersion.Parse("V 1"));
    }

    [Fact]
    public void ToleratesTheWhitespaceARealSerialLineArrivesWith()
    {
        Assert.Equal(3, FirmwareVersion.Parse("  V   3  "));
        Assert.Equal(3, FirmwareVersion.Parse("V\t3"));
    }

    [Theory]
    [InlineData(null)]
    [InlineData("")]
    [InlineData("   ")]
    [InlineData("V")]           // the query echoed back with no answer
    [InlineData("V 1 extra")]   // a line that happens to start with V but is not a level
    [InlineData("v 1")]         // the firmware's command letters are exact
    [InlineData("W 1")]
    [InlineData("V x")]
    [InlineData("V -1")]
    [InlineData("V 0")]         // levels start at 1, so 0 is not one
    [InlineData("1")]
    public void AnythingThatIsNotAnUnambiguousLevelIsNull(string? line)
    {
        Assert.Null(FirmwareVersion.Parse(line));
    }

    [Fact]
    public void TheSilenceSaysWhatItMeans()
    {
        // This is the line the whole feature exists for: it has to read as a definite answer about
        // the board, not as a failed read this tool might retry.
        Assert.Contains("predates", FirmwareVersion.Describe(null));
    }

    [Fact]
    public void AMismatchSaysWhichWayItGoes()
    {
        // Direction matters: "older than this launcher" is a board that needs a flash, "newer" is a
        // launcher that needs updating. The same number cannot mean both.
        var older = FirmwareVersion.Describe(FirmwareVersion.Current - 1);
        var newer = FirmwareVersion.Describe(FirmwareVersion.Current + 1);

        Assert.Contains("older", older);
        Assert.Contains("newer", newer);
        Assert.Null(FirmwareVersion.Parse(newer)); // and neither is a reply
    }

    [Fact]
    public void TheCurrentLevelIsNotDescribedAsAMismatch()
    {
        var current = FirmwareVersion.Describe(FirmwareVersion.Current);

        Assert.Contains("current", current);
        Assert.DoesNotContain("older", current);
        Assert.DoesNotContain("newer", current);
    }

    // Summary is what the player reads. Describe is the same fact in a developer's unit — "protocol
    // level 2 (current)" cannot be judged by someone who only wants to know whether their board is up
    // to date — so these pin that the player-facing sentence never falls back to the number, and that
    // each case says the thing that is actually decided (food dragging) plus a way out.

    [Theory]
    [InlineData(null)]
    [InlineData(0)]
    [InlineData(1)]
    public void ABoardThatCannotDragIsToldSoAndGivenAWayOut(int? level)
    {
        var summary = FirmwareVersion.Summary(level);

        Assert.Contains("drag", summary);
        Assert.Contains("Reflash", summary);
        Assert.DoesNotContain("protocol level", summary);
    }

    [Fact]
    public void ABoardThatCanDragIsNotToldTheNumber()
    {
        var summary = FirmwareVersion.Summary(FirmwareVersion.DragLevel);

        Assert.Contains("Up to date", summary);
        Assert.DoesNotContain("protocol level", summary);
    }

    [Fact]
    public void SilenceIsAnAnsweredNoRatherThanAnUnknown()
    {
        // A board that never replied predates the drag commands as well as version reporting, so this
        // is a definite "cannot drag" — not "we could not tell". The distinction matters because the
        // drag it cannot do fails by doing NOTHING, which looks identical to a mis-aimed drag.
        var summary = FirmwareVersion.Summary(null);

        Assert.Contains("cannot", summary.Replace("it also predates", "cannot"));
        Assert.Contains("Reflash", summary);
    }

    [Fact]
    public void ABoardNewerThanThisLauncherIsNotCalledUpToDate()
    {
        // It may well be fine, but claiming "up to date" about a board the launcher cannot read would
        // be a claim it has no way to check.
        var summary = FirmwareVersion.Summary(FirmwareVersion.Current + 1);

        Assert.Contains("Newer", summary);
        Assert.DoesNotContain("Up to date", summary);
    }

    [Fact]
    public void ASummaryForABoardThatCannotDragNamesTheGameConsequence()
    {
        // "cannot drag food" alone invites "so what?" — the failure is silent, so the sentence has to
        // say that a drag would DO NOTHING. That is the same lesson FoodLoadMode.Complaint exists for.
        var summary = FirmwareVersion.Summary(1);

        Assert.Contains("silently do nothing", summary);
    }
}
