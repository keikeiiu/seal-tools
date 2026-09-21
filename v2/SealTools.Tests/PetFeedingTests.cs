using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using SealTools.Core;
using Xunit;

namespace SealTools.Tests;

// Pins the feeding arithmetic that decides when a pet is revisited.
//
// The numbers below are not invented: they are PET-DATA.md's own, read off the scraped table on
// 2026-09-15 and measured per level on 2026-09-16. `wyz` is fixed by species and stage, and the whole
// run to `+9` at 100 % is `wyz × 14.5` — the nine transitions plus the `+9` bar's own fill.
public class PetFeedingTests
{
    private const string Fixture =
        "species,pet_id,pet_name,stage,wyz_first_level,feed_value_total,food_items,minutes\n" +
        "线甲,1,X,6,5200,75400,2514,838\n" +
        "线乙,2,Y,6,10000,145000,4834,1682\n" +
        "线丙,3,Z,7,33000,478500,15950,3190\n";

    private static IReadOnlyList<PetFeeding.Line> Table() => PetFeeding.Parse(Fixture);

    /// <summary>`+0` at 0 % is the table's own total — the number PET-DATA.md prints per pet.</summary>
    [Fact]
    public void ANewPetNeedsTheWholeFourteenPointFive()
    {
        Assert.Equal(5200 * 14.5, PetFeeding.RemainingValue(5200, 0, 0), 3);
    }

    /// <summary>`+9` at 100 % is DONE — the goal state, and the one the run exists to reach. It must be
    /// exactly zero, not "nearly": a pet at the top has nothing left to feed.</summary>
    [Fact]
    public void AFinishedPetNeedsNothing()
    {
        Assert.Equal(0, PetFeeding.RemainingValue(5200, PetPanel.MaxGrowth, 100), 6);
    }

    /// <summary>A `+9` pet at 10 % has only its own bar to fill: `wyz × 1.9 × 0.9`.</summary>
    [Fact]
    public void ATopPetWithAPartialBarNeedsOnlyThatBar()
    {
        Assert.Equal(5200 * 1.9 * 0.9, PetFeeding.RemainingValue(5200, 9, 10), 3);
    }

    /// <summary>The example that prompted all of this: a stage-6 pet at `+9` 10 %. The table says its
    /// whole run is 838 minutes, so the remaining 8,892 of 75,400 is just under 99 minutes — not the
    /// 505 the configured cycle would have waited.</summary>
    [Fact]
    public void ATopPetIsRevisitedInAboutAnHourAndAHalfNotEightHours()
    {
        var line = PetFeeding.Find(Table(), "线甲", 6);

        var minutes = PetFeeding.MinutesToFinish(line, 9, 10);

        Assert.NotNull(minutes);
        Assert.InRange(minutes!.Value, 98, 100);
    }

    /// <summary>At `+0` the pet needs everything, so the estimate is the table's whole-run figure.</summary>
    [Fact]
    public void ANewPetNeedsItsWholeRun()
    {
        var minutes = PetFeeding.MinutesToFinish(PetFeeding.Find(Table(), "线甲", 6), 0, 0);

        Assert.Equal(838, minutes!.Value, 3);
    }

    /// <summary>An EXP% outside 0-100 is a misread. Clamped rather than rejected, because the safe
    /// direction is "done" — a pet whose 100 % read as 110 must not be given more time than one at 100.
    /// </summary>
    [Fact]
    public void AnImpossibleExpIsClampedNotTrusted()
    {
        Assert.Equal(PetFeeding.RemainingValue(5200, 9, 100), PetFeeding.RemainingValue(5200, 9, 140), 6);
        Assert.Equal(PetFeeding.RemainingValue(5200, 9, 0), PetFeeding.RemainingValue(5200, 9, -20), 6);
    }

    /// <summary>The stage-7 `.G` pets sit at 33000 and cannot be boarded. Dropping them on load is what
    /// makes `(species, stage)` a key: with them in, seven of the sixty-nine pairs are ambiguous.</summary>
    [Fact]
    public void TheUnboardablePetsAreDroppedSoSpeciesAndStageIsAKey()
    {
        var table = Table();

        Assert.DoesNotContain(table, l => l.Wyz == 33000);
        Assert.Equal(2, table.Count);
    }

    /// <summary>A species spans five stages or seven, so a stage outside its range is a wrongly named
    /// species rather than a missing pet — and it must answer null rather than pick a neighbour.</summary>
    [Fact]
    public void AStageTheSpeciesDoesNotHaveIsNotFound()
    {
        Assert.Null(PetFeeding.Find(Table(), "线甲", 7));
        Assert.Null(PetFeeding.Find(Table(), "nonsense", 6));
        Assert.Null(PetFeeding.MinutesToFinish(null, 9, 10));
    }

    /// <summary>The table the app ships must parse, and must stay KEYABLE — read from the repo rather
    /// than a fixture, because this is the file that will be loaded at run time and a fixture would keep
    /// passing while the shipped csv rotted.
    ///
    /// The key is `(species, stage) → wyz`, and NOT `(species, stage) → one pet`: a line holds several
    /// pets at the same stage, which is why the first version of this test failed. What has to hold is
    /// that they all agree on `wyz` — the number the schedule actually uses — so any of them can
    /// answer for the group.</summary>
    [Fact]
    public void TheShippedTableParsesAndEverySpeciesStageAgreesOnOneWyz()
    {
        var path = FindTable();
        Assert.True(path != null, "docs/pet-data.csv was not found above the test directory");

        var table = PetFeeding.Load(path!);

        Assert.True(table.Count > 250, $"expected the full table, got {table.Count} rows");

        var disagreeing = table.GroupBy(l => (l.Species, l.Stage))
            .Where(g => g.Select(l => l.Wyz).Distinct().Count() > 1)
            .ToList();
        Assert.True(disagreeing.Count == 0,
            "these (species, stage) groups disagree on wyz: " +
            string.Join(", ", disagreeing.Select(g => $"{g.Key.Species} +{g.Key.Stage}")));

        // …and the same group must agree on what that wyz costs and takes, or the answer would depend
        // on which row happened to be found first.
        var inconsistent = table.GroupBy(l => (l.Species, l.Stage))
            .Where(g => g.Select(l => l.TotalValue).Distinct().Count() > 1 ||
                        g.Select(l => l.Minutes).Distinct().Count() > 1)
            .ToList();
        Assert.True(inconsistent.Count == 0,
            "these groups disagree on their total value or minutes: " +
            string.Join(", ", inconsistent.Select(g => $"{g.Key.Species} +{g.Key.Stage}")));

        Assert.All(table, l => Assert.True(l.Wyz > 0 && l.TotalValue > 0 && l.Minutes > 0));
    }

    private static string? FindTable()
    {
        var dir = new DirectoryInfo(AppContext.BaseDirectory);
        while (dir != null)
        {
            var candidate = Path.Combine(dir.FullName, "docs", "pet-data.csv");
            if (File.Exists(candidate)) return candidate;
            dir = dir.Parent;
        }
        return null;
    }
}
