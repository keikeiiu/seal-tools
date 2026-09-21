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
    /// scales gets a proportionally placed box with nothing to calibrate. Measured: a 60-high strip puts
    /// the line at +32 and 24 tall, x 6 left of the strip.</summary>
    [Fact]
    public void TheRegionIsDerivedFromTheStripAsRatios()
    {
        var strip = new List<int> { 344, 645, 326, 60 };

        var region = FeederLayout.EtaRegion(strip);

        Assert.NotNull(region);
        Assert.Equal(344 - 6, region![0]);
        Assert.Equal(645 + 60 + 32, region[1]);
        Assert.Equal(282, region[2]);
        Assert.Equal(24, region[3]);
    }

    /// <summary>…and it scales: double the slots and everything below them moves and grows with it.</summary>
    [Fact]
    public void TheRegionScalesWithTheSlotHeight()
    {
        var tall = FeederLayout.EtaRegion(new List<int> { 344, 645, 652, 120 });

        Assert.NotNull(tall);
        Assert.Equal(645 + 120 + 64, tall![1]);
        Assert.Equal(48, tall[3]);
    }

    [Fact]
    public void AnUnusableStripGivesNoRegion()
    {
        Assert.Null(FeederLayout.EtaRegion(null));
        Assert.Null(FeederLayout.EtaRegion(new List<int> { 1, 2 }));
    }
}
