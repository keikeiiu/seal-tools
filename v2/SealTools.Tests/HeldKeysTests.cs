using SealTools.Core;
using Xunit;

namespace SealTools.Tests;

// Which tools hold a key down is a decision the stop path makes, not the tool — so it lives in Core
// where LauncherService can ask it and this project can pin it. The rule is small; what it guards is
// not. Getting it wrong in the "false" direction leaves a key stuck down on the player's keyboard
// with the tool already gone, which is the failure the whole change exists to remove.
public class HeldKeysTests
{
    // Every id LauncherService.StartToolAsync accepts. Duplicated deliberately: a new tool that holds
    // a key has to be added here, and the failure to notice that is what this test is for.
    private static readonly string[] EveryToolId =
        { "tuner", "gem", "spammer", "buy", "sell", "pet" };

    [Fact]
    public void HoldSpaceNeedsItsRelease() => Assert.True(HeldKeys.NeedsSpaceRelease("holdspace"));

    [Fact]
    public void NoOtherToolDoes()
    {
        foreach (var id in EveryToolId)
        {
            Assert.False(HeldKeys.NeedsSpaceRelease(id));
        }
    }

    [Fact]
    public void NothingRunningIsNotAHeldKey()
    {
        Assert.False(HeldKeys.NeedsSpaceRelease(null));
        Assert.False(HeldKeys.NeedsSpaceRelease(""));
        Assert.False(HeldKeys.NeedsSpaceRelease("HoldSpace")); // ids are exact, and case-sensitively so
    }
}
