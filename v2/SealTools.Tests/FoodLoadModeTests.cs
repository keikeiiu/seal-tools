using System.Globalization;
using SealTools.Core;
using Xunit;

namespace SealTools.Tests;

// The food-load setting is the one place where a BOARD VERSION changes what the tool does rather than
// merely being printed. Getting the "can this board drag?" answer wrong in the permissive direction is
// the expensive one: the drag letters are ignored by an old board, the button is never pressed, and
// the stack is never picked up — which looks exactly like a mis-aimed drag, so the feeder would report
// successful reloads while loading no food at all.
public class FoodLoadModeTests
{
    [Fact]
    public void TheDefaultIsTheModeThatWorksOnEveryBoard()
    {
        // Not a style preference: a right-click works on a board that has never been reflashed, and a
        // silent no-op is the failure this setting must not default into.
        Assert.False(FoodLoadMode.IsDrag(new SealTools.Core.Config.PetConfig().FoodLoadMode));
        Assert.True(FoodLoadMode.IsDrag(FoodLoadMode.Drag));
    }

    [Theory]
    [InlineData("drag")]
    [InlineData("DRAG")]
    [InlineData("  drag  ")]
    public void TheValueIsReadLoosely(string mode) => Assert.True(FoodLoadMode.IsDrag(mode));

    [Theory]
    [InlineData(null)]
    [InlineData("")]
    [InlineData("right_click")]
    [InlineData("rightclick")]
    [InlineData("dragn")]
    [InlineData("drags")]
    public void AnythingElseIsNotADrag(string? mode) => Assert.False(FoodLoadMode.IsDrag(mode));

    [Fact]
    public void ThePickerOffersEveryMode()
    {
        // The Pet tab builds its dropdown from this, so anything missing here is unreachable in the UI.
        Assert.Contains(FoodLoadMode.RightClick, FoodLoadMode.All);
        Assert.Contains(FoodLoadMode.Drag, FoodLoadMode.All);
    }

    [Fact]
    public void ABoardAtTheDragLevelCanDrag()
    {
        Assert.True(FoodLoadMode.BoardCanDrag(FirmwareVersion.DragLevel));
        Assert.True(FoodLoadMode.BoardCanDrag(FirmwareVersion.Current));
        Assert.True(FoodLoadMode.BoardCanDrag(FirmwareVersion.DragLevel + 5));
    }

    [Fact]
    public void ABoardBelowItCannot()
    {
        Assert.False(FoodLoadMode.BoardCanDrag(FirmwareVersion.DragLevel - 1));
        Assert.False(FoodLoadMode.BoardCanDrag(0));
    }

    [Fact]
    public void ASilentBoardIsADefiniteNoAndNotAnUnknown()
    {
        // The whole point of the V command: the sketch that predates version reporting also predates
        // the drag commands, so silence is an answer. Treating null as "maybe" here is what would let
        // an un-reflashed board be trusted with a drag.
        Assert.False(FoodLoadMode.BoardCanDrag(null));
        Assert.Contains("did not report", FoodLoadMode.Complaint(FoodLoadMode.Drag, null));
    }

    [Fact]
    public void RightClickIsNeverAComplaintWhateverTheBoardSays()
    {
        // Right-click is the fallback that any board can do, so it must never block a start.
        Assert.Null(FoodLoadMode.Complaint(FoodLoadMode.RightClick, FirmwareVersion.DragLevel));
        Assert.Null(FoodLoadMode.Complaint(FoodLoadMode.RightClick, 1));
        Assert.Null(FoodLoadMode.Complaint(FoodLoadMode.RightClick, null));
        Assert.Null(FoodLoadMode.Complaint(null, null));
    }

    [Fact]
    public void DragOnACapableBoardIsNotAComplaint()
    {
        Assert.Null(FoodLoadMode.Complaint(FoodLoadMode.Drag, FirmwareVersion.DragLevel));
        Assert.Null(FoodLoadMode.Complaint(FoodLoadMode.Drag, FirmwareVersion.Current));
    }

    [Fact]
    public void DragOnAnOldBoardSaysWhichBoardAndWhatToDoAboutIt()
    {
        var complaint = FoodLoadMode.Complaint(FoodLoadMode.Drag, FirmwareVersion.DragLevel - 1);

        Assert.NotNull(complaint);
        Assert.Contains("reflash", complaint);
        // The version it saw, so "wrong board" and "wrong setting" can be told apart from the message
        // alone — the player cannot see the board.
        Assert.Contains((FirmwareVersion.DragLevel - 1).ToString(CultureInfo.InvariantCulture), complaint);
    }
}
