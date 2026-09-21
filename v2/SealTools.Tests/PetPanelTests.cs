using SealTools.Core;
using Xunit;

namespace SealTools.Tests;

// Pins the pet-panel parser. The primary fixture is the ONE read that was measured on the live game
// (2026-09-18, recorded in PLAN-HOVER-INFO.md):
//
//     on screen   (6階) 真蔚藍鳳凰 +7 [52.18%]
//     read        （6）真蔚蓝凤凰+7[52.18%]
//
// Every digit exact, the Chinese not — and that is the whole reason the parser reads numbers and
// ignores every surrounding character. The tests below are mostly about the ways a panel can arrive
// that is NOT understood, because the caller fails open: an unparsed panel reloads as normal, so the
// dangerous bug is a panel that parses WRONG, not one that parses not at all.
public class PetPanelTests
{
    /// <summary>The measured read, verbatim.</summary>
    private const string LiveRead = "（6）真蔚蓝凤凰+7[52.18%]";

    private static PetPanel? P(params string[] lines) => PetPanel.Parse(lines);

    [Fact]
    public void ParsesTheLiveRead()
    {
        var panel = P(LiveRead);

        Assert.NotNull(panel);
        Assert.Equal(6, panel!.Stage);
        Assert.Equal(7, panel.Growth);
        Assert.Equal(52.18, panel.Exp, 3);
        Assert.False(panel.IsFinished);
    }

    /// <summary>The panel is several rows and the numbers are in the first; parsing joins the lines
    /// rather than guessing which one carries them, because that is the recogniser's business.</summary>
    [Fact]
    public void ParsesAcrossSeveralLines()
    {
        var panel = P(
            LiveRead,
            "所有瞬業皆可使用",   // 職業 misread, and it must not matter
            "贩直價格",           // 販賣 misread, likewise
            "1,234");

        Assert.NotNull(panel);
        Assert.Equal(7, panel!.Growth);
    }

    /// <summary>Our "+10" — the name appears nowhere in the game, which shows +9 with a full bar.</summary>
    [Fact]
    public void FullBarOnTopGrowthIsFinished()
    {
        var panel = P("（6）真蔚蓝凤凰+9[100%]");

        Assert.NotNull(panel);
        Assert.True(panel!.IsFinished);
    }

    /// <summary>A growth read one digit too long reads as its FIRST digit — which for "+99" is the 9
    /// that is really there, so a finished pet still reads as finished.
    ///
    /// MEASURED live, 2026-09-21: a +9/100% pet came back as "+99, 100%" (the sweep's report is
    /// logs\reads\scan_20260921_152331.txt) and the owner confirmed the pet is +9/100%. Because
    /// IsFinished wants EQUALS 9, 99 fell through as NOT finished and the run would have right-clicked
    /// a pet that cannot be boarded, straight into the error dialog.
    ///
    /// The two-digit pattern was what allowed it, and it was wrong in both directions: it also read the
    /// paid extension's "+15 Days" duration label as a growth of 15. See GrowthPattern.</summary>
    [Fact]
    public void AGrowthReadOneDigitTooLongStillReadsTheDigit()
    {
        var panel = P("（6）真蔚蓝凤凰+99[100%]");

        Assert.NotNull(panel);
        Assert.Equal(PetPanel.MaxGrowth, panel!.Growth);
        Assert.True(panel.IsFinished);
    }

    /// <summary>Both halves must hold. Each of these is finished by one number and not the other,
    /// and reading either alone would get the pet fed forever or dropped while it still needs food.</summary>
    [Theory]
    [InlineData("（6）真蔚蓝凤凰+9[99.99%]")]   // top growth, bar still filling
    [InlineData("（6）真蔚蓝凤凰+7[100%]")]    // full bar, seven more levels to go
    [InlineData("（6）真蔚蓝凤凰+8[100%]")]
    public void OneNumberAloneIsNotFinished(string line)
    {
        var panel = P(line);

        Assert.NotNull(panel);
        Assert.False(panel!.IsFinished);
    }

    /// <summary>A growth above the game's maximum is a misread, and it must never read as finished —
    /// "at least 9" would answer "done" to a garbage panel and stop feeding a pet that needs it.
    /// The text below is real: it is the paid extension's label in the same window.</summary>
    [Fact]
    public void GrowthAboveTheMaximumIsNeverFinished()
    {
        var panel = P("（6）真蔚蓝凤凰+15 Days[100%]");

        Assert.NotNull(panel);
        Assert.False(panel!.IsFinished);
    }

    // ── What must NOT parse ─────────────────────────────────────────────────

    [Fact]
    public void NothingReadGivesNull() => Assert.Null(PetPanel.Parse(Array.Empty<string>()));

    [Fact]
    public void NullLinesGiveNull() => Assert.Null(PetPanel.Parse(null));

    /// <summary>A food stack's tooltip: a count, but no growth and no percentage. It shares the same
    /// offset and the same box, so a scan that hovers the wrong cell lands here — and it must read as
    /// "not understood" rather than as some default pet.</summary>
    [Fact]
    public void AFoodPanelIsNotAPetPanel() => Assert.Null(P("高級寵物食物", "300"));

    /// <summary>A growth with no EXP is half a panel. Returning null keeps the finish test from being
    /// answered out of a read we did not fully understand.</summary>
    [Fact]
    public void GrowthWithoutExpGivesNull() => Assert.Null(P("（6）真蔚蓝凤凰+7"));

    // ── The whole panel, as the live game actually produced it (2026-09-19) ──
    //
    // These five lines are one read verbatim, from the Pet tab's Test read. It is the fixture that
    // matters most because it is the only panel we have that was NOT hand-written, and it corrected
    // two things the earlier tests had wrong.

    private static readonly string[] LiveZeroPet =
    {
        "（6踏）真蔚蓝米鲁[0.06%]",
        "所有瞬業皆可使用",
        "等級限制1150",
        "名望限制51595",
        "贩直價格-500000s",
    };

    /// <summary>A +0 pet renders NO "+N". Requiring one rejected every pet at the start of its run,
    /// which is the pet this tool feeds most often — and it failed in a way that looked like the read
    /// being broken rather than the parser being wrong.</summary>
    [Fact]
    public void AZeroPetHasNoGrowthAndParsesAsZero()
    {
        var panel = PetPanel.Parse(LiveZeroPet);

        Assert.NotNull(panel);
        Assert.Equal(6, panel!.Stage);
        Assert.Equal(0, panel.Growth);
        Assert.Equal(0.06, panel.Exp, 3);
        Assert.False(panel.IsFinished);
    }

    /// <summary>The 階 was misread as 踏 and the stage still has to come out. It is read off the
    /// panel's own first row, so a mangled unit must not cost the one value that identifies which
    /// food and which rate this pet needs.</summary>
    [Fact]
    public void AMangledStageUnitStillParsesTheStage() => Assert.Equal(6, PetPanel.Parse(LiveZeroPet)!.Stage);

    /// <summary>The other two panels the same session produced, both from hovering the wrong thing.
    /// Neither carries a bracketed percentage, so neither is a pet — which is the whole of what keeps
    /// a scan that hovers the wrong cell from inventing one.</summary>
    [Fact]
    public void TheOtherTooltipsTheSessionReadAreNotPets()
    {
        // A bag item, and a quest letter. Neither has a bracketed percentage anywhere.
        Assert.Null(P("能量的舜渣", "促希菌特大陵各每收集而来的神秘掉落物：",
            "邂子城的?能量守恒者?知道這是什应東西?拿去给他看看吧"));
        Assert.Null(P("奥藤的請求信", "奥藤為了找回冬天，正在收集大量的",
            "有效期間", "2026年9月月20日0時58分"));
    }

    /// <summary>The panel states the sell price, the level limit and the fame limit — three large
    /// numbers that a parser hunting for digits would happily mistake for something. Anchoring on the
    /// bracket and the + is what makes them impossible to pick up.</summary>
    [Fact]
    public void ThePanelsOtherNumbersAreNotMistakenForGrowthOrExp()
    {
        var panel = PetPanel.Parse(LiveZeroPet);

        Assert.Equal(0, panel!.Growth);      // not 150, not 51595, not 500000
        Assert.Equal(0.06, panel.Exp, 3);    // and not one of those either
    }

    /// <summary>The BRACKET is what makes a percentage an EXP bar rather than any other number that
    /// happens to carry a %. Both live reads kept theirs, and the recogniser is reliable on ASCII
    /// punctuation where it is not on Chinese — so requiring it costs nothing real and it is the one
    /// thing standing between a scan and an item whose description mentions a rate.</summary>
    [Fact]
    public void AnUnbracketedPercentageIsNotAPet() => Assert.Null(P("某個物品", "增加 10% 的掉落率"));

    // ── Shapes the recogniser plausibly returns ─────────────────────────────

    /// <summary>A full-width digit is a different character and the same number. Folding it is
    /// cheaper than discovering that a correct read was thrown away for being wide.</summary>
    [Fact]
    public void FullWidthDigitsParse()
    {
        var panel = P("（６）真蔚蓝凤凰＋９［１００％］");

        Assert.NotNull(panel);
        Assert.Equal(6, panel!.Stage);
        Assert.Equal(9, panel.Growth);
        Assert.True(panel.IsFinished);
    }

    /// <summary>The 階 was dropped by the recogniser in the measured read and is optional.</summary>
    [Theory]
    [InlineData("（6）真蔚蓝凤凰+7[52.18%]")]
    [InlineData("(6階)真蔚蓝凤凰+7[52.18%]")]
    [InlineData("(6阶)真蔚蓝凤凰+7[52.18%]")]
    public void StageUnitIsOptional(string line) => Assert.Equal(6, P(line)!.Stage);

    /// <summary>The stage is the one value nothing acts on yet, so losing ONLY the stage still
    /// yields a usable panel — the two numbers the finish test needs both survived.</summary>
    [Fact]
    public void AMissingStageStillParses()
    {
        var panel = P("真蔚蓝凤凰+7[52.18%]");

        Assert.NotNull(panel);
        Assert.Null(panel!.Stage);
        Assert.Equal(7, panel.Growth);
    }

    /// <summary>Spacing is not ours to rely on: the game's own panel has spaces where the read does
    /// not, and OCR moves them around.</summary>
    [Fact]
    public void SpacingIsIrrelevant()
    {
        var panel = P("(6階) 真蔚藍鳳凰 +9 [ 100 %]");

        Assert.NotNull(panel);
        Assert.Equal(9, panel!.Growth);
        Assert.Equal(100, panel.Exp, 3);
        Assert.True(panel.IsFinished);
    }
}
