using SealTools.Core;
using Xunit;

namespace SealTools.Tests;

// Pins the physical-offset -> screen-coordinate conversion (docs/COORDINATES.md). The numbers are
// the real ones measured on the reference machine: 150% display, physical client 2865x1789 @ (14,48),
// logical client 1910x1193 @ (9,32), calibrated N at physical client-relative (1083,998).
public class CursorTargetTests
{
    private static readonly DisplayInfo Reference = new(
        Scale: 1.5,
        MonitorDpi: 144,
        ScreenWidth: 3840,
        ScreenHeight: 2160,
        PhysicalFrame: new WindowRect(2, 2, 2889, 1848),
        PhysicalClient: new WindowRect(14, 48, 2865, 1789),
        LogicalClient: new WindowRect(9, 32, 1910, 1193));

    [Fact]
    public void ConvertsPhysicalOffsetToBothCursorSpaces()
    {
        var t = WindowFinder.ComputeCursorTarget(Reference, 1083, 998);

        Assert.Equal(731, t.LogicalX);   // 9  + round(1083 / 1.5) = 9 + 722
        Assert.Equal(697, t.LogicalY);   // 32 + round(998  / 1.5) = 32 + 665
        Assert.Equal(1097, t.PhysicalX); // 14 + 1083
        Assert.Equal(1046, t.PhysicalY); // 48 + 998
    }

    [Fact]
    public void BothSpacesDescribeTheSamePhysicalPoint()
    {
        var t = WindowFinder.ComputeCursorTarget(Reference, 1083, 998);

        // The logical target, scaled back up, must land within a pixel of the physical one.
        Assert.InRange(t.LogicalX * Reference.Scale, t.PhysicalX - 1.5, t.PhysicalX + 1.5);
        Assert.InRange(t.LogicalY * Reference.Scale, t.PhysicalY - 1.5, t.PhysicalY + 1.5);
    }

    [Fact]
    public void NoScalingAtOneToOne()
    {
        var flat = Reference with
        {
            Scale = 1.0,
            PhysicalClient = new WindowRect(9, 32, 1910, 1193),
        };

        var t = WindowFinder.ComputeCursorTarget(flat, 1083, 998);

        Assert.Equal(1092, t.LogicalX);
        Assert.Equal(1092, t.PhysicalX);
        Assert.Equal(1030, t.LogicalY);
        Assert.Equal(1030, t.PhysicalY);
    }
}
