using SealTools.Core;
using Xunit;

namespace SealTools.Tests;

// Which tools can leave something held down is a decision the stop path makes, not the tool — so it
// lives in Core where LauncherService can ask it and this project can pin it. The rule is small; what
// it guards is not. Getting it wrong in the "false" direction leaves a key or a mouse button stuck
// down on the player's machine with the tool already gone, which is the failure the whole change
// exists to remove.
public class HeldKeysTests
{
    // Every id LauncherService.StartToolAsync accepts EXCEPT the two that hold something — those are
    // asserted true below. Duplicated deliberately: a new tool that holds a key has to be added here,
    // and the failure to notice that is what this test is for.
    private static readonly string[] EveryToolId =
        { "tuner", "gem", "spammer", "buy", "sell" };

    [Fact]
    public void TheToolsThatHoldSomethingNeedARelease()
    {
        // holdspace holds the spacebar; the pet feeder's food DRAG holds the left mouse button.
        Assert.True(HeldKeys.NeedsReleaseOnStop("holdspace"));
        Assert.True(HeldKeys.NeedsReleaseOnStop("pet"));
    }

    [Fact]
    public void NoOtherToolDoes()
    {
        foreach (var id in EveryToolId)
        {
            Assert.False(HeldKeys.NeedsReleaseOnStop(id));
        }
    }

    [Fact]
    public void NothingRunningIsNotAHeldKey()
    {
        Assert.False(HeldKeys.NeedsReleaseOnStop(null));
        Assert.False(HeldKeys.NeedsReleaseOnStop(""));
        Assert.False(HeldKeys.NeedsReleaseOnStop("HoldSpace")); // ids are exact, and case-sensitively so
    }
}
