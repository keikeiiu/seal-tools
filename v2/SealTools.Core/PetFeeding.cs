using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;

namespace SealTools.Core;

/// <summary>How much a pet still needs, and how long that is — from the scraped table in
/// docs/pet-data.csv.
///
/// THE TABLE IS THE ONLY SOURCE, and that is the player's decision rather than a shortcut: *"the base
/// feeding value is predetermined, nowhere can you find it in the game. We believe the data."*
///
/// What the table records, all measured (PET-DATA.md, scraped 2026-09-15, per-level cost measured
/// 2026-09-16):
///
///     cost(+n)      = wyz × (1 + n/10)      n = 0 … 9
///     to +9 / 100%  = wyz × 14.5            the +9 bar fills too — 14.5, not 9
///
/// which collapses to one line here, because the table already carries the total value AND the total
/// minutes for the pet's whole run: **whatever fraction of the total value is left is the same fraction
/// of the total minutes**. No per-item value and no per-minute rate has to be derived or configured.
///
/// WHY (species, stage) AND NOT THE NAME. The game's Chinese is Traditional and this table's is
/// Simplified, and the recorded rule is never to string-match one against the other — the OCR
/// normalises and mangles what it cannot map. The player names the species once per queued pet and the
/// panel supplies the stage, so nothing is matched at run time.</summary>
public static class PetFeeding
{
    /// <summary>One pet LINE at one stage. `(Species, Stage)` identifies it: every pet on a line at a
    /// stage shares one `wyz` (PET-DATA.md, measured across four pets and four orders of magnitude).</summary>
    public sealed record Line(
        string Species, int PetId, string Name, int Stage,
        double Wyz, double TotalValue, double FoodItems, double Minutes);

    /// <summary>The `.G` pets' value. They sit at stage 7, they cannot be boarded, and leaving them in
    /// makes `(species, stage)` ambiguous for seven of the sixty-nine pairs — so they are dropped on
    /// load rather than defended against on every lookup.</summary>
    private const double UnboardableWyz = 33000;

    /// <summary>The table, from the csv the app ships beside its config. Empty when the file is
    /// missing: a schedule that cannot be computed falls back to the configured cycle, which is what
    /// the tool did before any of this existed.</summary>
    public static IReadOnlyList<Line> Load(string path)
    {
        try
        {
            return File.Exists(path) ? Parse(File.ReadAllText(path)) : Array.Empty<Line>();
        }
        catch
        {
            // A table that will not parse must not stop a run feeding pets.
            return Array.Empty<Line>();
        }
    }

    /// <summary>Parses the table. Public so the tests can feed it a fixture rather than depend on where
    /// the app happens to be running from.</summary>
    public static IReadOnlyList<Line> Parse(string text)
    {
        var lines = new List<Line>();
        var rows = text.Replace("\r\n", "\n").Split('\n');

        for (int i = 1; i < rows.Length; i++)   // row 0 is the header
        {
            var f = rows[i].Split(',');
            if (f.Length < 8) continue;

            if (!int.TryParse(f[3], NumberStyles.Integer, CultureInfo.InvariantCulture, out var stage)) continue;
            if (!Num(f[4], out var wyz) || !Num(f[5], out var total) ||
                !Num(f[6], out var items) || !Num(f[7], out var minutes)) continue;

            if (wyz == UnboardableWyz) continue;

            lines.Add(new Line(f[0].Trim(), int.TryParse(f[1], out var id) ? id : 0, f[2].Trim(),
                stage, wyz, total, items, minutes));
        }

        return lines;
    }

    private static bool Num(string s, out double v) =>
        double.TryParse(s.Trim(), NumberStyles.Float, CultureInfo.InvariantCulture, out v);

    /// <summary>The line for a species at a stage, or null when the table has no such pet — a species
    /// spans five stages or seven, so a stage outside its range is a species that was named wrongly
    /// rather than a pet the table is missing.</summary>
    public static Line? Find(IEnumerable<Line> table, string? species, int stage) =>
        species == null ? null : table.FirstOrDefault(l => l.Species == species && l.Stage == stage);

    /// <summary>What is left to feed, in the table's own units.
    ///
    /// The levels above the pet's own, plus the part of its current bar still to fill:
    ///
    ///     Σ(k = growth+1 … 9) wyz × (1 + k/10)   +   wyz × (1 + growth/10) × (1 − exp/100)
    ///
    /// At `+9` 100 % this is zero — the pet is done, which is the whole goal and the state the run
    /// exists to reach. At `+0` 0 % it is `wyz × 14.5`, the table's own total.</summary>
    public static double RemainingValue(double wyz, int growth, double expPercent)
    {
        if (growth >= PetPanel.MaxGrowth)
            return wyz * (1 + PetPanel.MaxGrowth / 10.0) * (1 - Clamp(expPercent) / 100.0);

        var remaining = wyz * (1 + growth / 10.0) * (1 - Clamp(expPercent) / 100.0);
        for (int k = growth + 1; k <= PetPanel.MaxGrowth; k++) remaining += wyz * (1 + k / 10.0);
        return remaining;
    }

    /// <summary>How long the pet still needs, in minutes — or null when the line cannot answer, which
    /// the caller treats as "no estimate" rather than as zero.
    ///
    /// The fraction of the total value that is left is the fraction of the total minutes that is left,
    /// because both describe the same run. That is why nothing here needs a per-item value or a
    /// per-minute rate: the table already paid for them.</summary>
    public static double? MinutesToFinish(Line? line, int growth, double expPercent)
    {
        if (line == null || line.TotalValue <= 0 || line.Minutes <= 0) return null;

        var remaining = RemainingValue(line.Wyz, growth, expPercent);
        return remaining / line.TotalValue * line.Minutes;
    }

    /// <summary>An EXP% outside 0-100 is a misread; clamped rather than rejected, because the direction
    /// that matters is the safe one — a `100 %` that reads as `110` must still mean "done".</summary>
    private static double Clamp(double expPercent) => Math.Max(0, Math.Min(100, expPercent));
}
