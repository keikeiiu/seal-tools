using System;
using System.Collections.Generic;
using SealTools.Core;
using Xunit;

namespace SealTools.Tests;

// Pins the parser for the boarding window's time line — the game's own answer to "when should this row
// be looked at again".
//
// THE STRINGS BELOW ARE REAL. They were read off a live window on 2026-09-21, one row each:
//
//   row 3, a pet about to finish  代養完成預計所需時間：約14分
//   row 1, a pet mid-run          到2為止預計所需時間：約 20分
//   every row                    該欄位約27日9時51分16 秒後到期。
//   every row                    每1分 攝取3個。
//
// The completion form is the one that matters — a pet that finishes is mailed and the row goes empty —
// and the expiry line is the trap: it also carries a 約 and a number.
public class FeederEtaTests
{
    private static FeederEta.Reading? P(params string[] lines) => FeederEta.Parse(lines);

    /// <summary>The line that prompted all of this: row 3's pet, 14 minutes from being mailed.</summary>
    [Fact]
    public void TheCompletionFormIsRead()
    {
        var r = P("代養完成預計所需時間：約14分");

        Assert.NotNull(r);
        Assert.True(r!.IsCompletion);
        Assert.Equal(14, r.Minutes);
    }

    /// <summary>The level form parses — and must NOT report completion, or the row would be revisited
    /// every twenty minutes for the whole life of the pet.</summary>
    [Fact]
    public void TheLevelFormIsReadButIsNotCompletion()
    {
        var r = P("到2為止預計所需時間：約 20分");

        Assert.NotNull(r);
        Assert.False(r!.IsCompletion);
        Assert.Equal(20, r.Minutes);
        Assert.Null(FeederEta.CompletionMinutes(r));
    }

    /// <summary>The 餵養值/growth the level form counts to — what the card shows as `→ +9`, and the
    /// reason the parser keeps the line verbatim. It is the GROWTH (+0 … +9, ending at a finished
    /// pet), NOT the stage: the player corrected both readings on 2026-10-11 — *"the pet feeding
    /// level is not the pet rank level"* and *"there is no stage 8 stage 9"*. The stage comes from
    /// hovering the pet icon (the panel's `(6階)`), which is a different number entirely. Display
    /// only: the level form drives nothing.</summary>
    [Theory]
    [InlineData("到2為止預計所需時間：約 20分", 2)]
    [InlineData("到6為止预計所需時間：約19分", 6)]
    [InlineData("到9為止预計所需時間：約94分", 9)]
    [InlineData("到3为止预计所需时间：约 12分", 3)]   // the OCR's Simplified 止, mixed scripts
    [InlineData("代養完成預計所需時間：約14分", null)]   // the completion form names no growth
    [InlineData("該欄位約27日9時51分16秒後到期。", null)] // the shelf life is not one either
    [InlineData("到6為止", 6)]
    [InlineData("到12為止預計所需時間：約 20分", null)]   // above the game's own maximum (+9)
    public void TheLevelFormNamesTheGrowthItCountsTo(string line, int? expected)
    {
        Assert.Equal(expected, FeederEta.GrowthTarget(line));
    }

    /// <summary>The feeding rate the window prints beside every row — the line that answers the
    /// player's *"the reason why it is 3 food per pet is those pets are stage 6 pets"*, read per row
    /// because rows may hold pets at different stages.</summary>
    [Theory]
    [InlineData("每1分 攝取3個", 3)]                    // the measured line, verbatim
    [InlineData("每 1 分 攝取 5 個", 5)]
    [InlineData("每2分 攝取6個", 3)]                    // "3 per minute", stated as 6 in 2
    [InlineData("每1分攝取4个", 4)]                     // Simplified 个, no spaces
    [InlineData("該欄位約27日9時51分16 秒後到期。", null)]
    [InlineData("代養完成預計所需時間：約14分", null)]
    [InlineData("到6為止预計所需時間：約19分", null)]
    [InlineData("每1分 攝取300個", null)]               // implausible: rejected, not clamped
    [InlineData("每0分 攝取3個", null)]
    public void TheRateLineIsReadPerRow(string line, int? expected)
    {
        Assert.Equal(expected, FeederEta.ParseRate(new[] { line }));
    }

    /// <summary>THE TRAP: the food's shelf life carries a 約 and a number near a 分, and reading it as a
    /// countdown would schedule a row twenty-seven days out.</summary>
    [Fact]
    public void TheFoodExpiryIsNotATimeTo()
    {
        Assert.Null(P("該欄位約27日9時51分16 秒後到期。"));
    }

    /// <summary>The rate line has no 約 and no countdown.</summary>
    [Fact]
    public void TheRateLineIsNotATimeTo()
    {
        Assert.Null(P("每1分 攝取3個。"));
    }

    /// <summary>The real window, all four lines together, in the order they appear — the expiry first,
    /// and it must not win merely for arriving first.</summary>
    [Fact]
    public void TheWholeRowReadsAsCompletion()
    {
        var r = P("該欄位約27日9時51分20 秒後到期。", "每1分 攝取3個。", "代養完成預計所需時間：約14分");

        Assert.NotNull(r);
        Assert.True(r!.IsCompletion);
        Assert.Equal(14, r.Minutes);
    }

    /// <summary>Nothing to read is an ordinary answer, not an error: the caller keeps the food figure,
    /// which is what it used before any of this existed.</summary>
    [Fact]
    public void NothingReadIsNull()
    {
        Assert.Null(P());
        Assert.Null(P(""));
        Assert.Null(P("每1分 攝取3個。", "该栏位约27日9时51分16 秒后到期。"));
        Assert.Null(FeederEta.Parse(null));
    }

    /// <summary>OCR mangles Chinese, so the completion test must survive a damaged character or two —
    /// a missed completion costs a cycle, which is what happens today, and never a wrong answer.</summary>
    [Fact]
    public void AMangledCompletionLineStillReads()
    {
        var r = P("代養完戌預計所需時間：約14分");

        Assert.NotNull(r);
        Assert.True(r!.IsCompletion);
        Assert.Equal(14, r.Minutes);
    }

    /// <summary>The region is derived from the strip as RATIOS of its height, so a machine whose UI
    /// scales gets a proportionally placed box with nothing to calibrate. A 60-high strip puts it 12
    /// below the slots and 72 tall — measured positions are +34 and (on a row carrying an extra line)
    /// about +57, and the band has to contain BOTH.</summary>
    [Fact]
    public void TheRegionIsDerivedFromTheStripAsRatios()
    {
        var strip = new List<int> { 344, 645, 326, 60 };

        var region = FeederLayout.EtaRegion(strip);

        Assert.NotNull(region);
        Assert.Equal(344 - 6, region![0]);
        Assert.Equal(645 + 60 + 12, region[1]);
        Assert.Equal(282, region[2]);
        Assert.Equal(72, region[3]);
    }

    /// <summary>Widening the band is what stops a 7-pixel difference between rows being fatal, and the
    /// old band has to stay INSIDE the new one — a row that read before must not stop reading. This is
    /// the regression this test exists for: the old band was +32..+56, and a live row's line was at
    /// ~+57.</summary>
    [Fact]
    public void TheRegionContainsTheBandItUsedToAimAt()
    {
        var strip = new List<int> { 344, 645, 326, 60 };

        var region = FeederLayout.EtaRegion(strip)!;
        var top = region[1];
        var bottom = region[1] + region[3];

        Assert.True(top <= 645 + 60 + 32, "the band must still start at or above the old +32");
        Assert.True(bottom >= 645 + 60 + 56, "and must still reach the old band's end of +56");
    }

    /// <summary>…and it scales: double the slots and everything below them moves and grows with it.</summary>
    [Fact]
    public void TheRegionScalesWithTheSlotHeight()
    {
        var tall = FeederLayout.EtaRegion(new List<int> { 344, 645, 652, 120 });

        Assert.NotNull(tall);
        Assert.Equal(645 + 120 + 24, tall![1]);
        Assert.Equal(144, tall[3]);
    }

    [Fact]
    public void AnUnusableStripGivesNoRegion()
    {
        Assert.Null(FeederLayout.EtaRegion(null));
        Assert.Null(FeederLayout.EtaRegion(new List<int> { 1, 2 }));
    }
}
