using System.Collections.Generic;
using System.Globalization;
using System.Text.RegularExpressions;
using SealTools.Core.Config;

namespace SealTools.Tuner;

// Faithful port of tuner/attr_matcher.py. The attribute dictionary comes from
// attributes.yaml (no hardcoded ATTRIBUTES list).

public sealed class MatchedAttr
{
    public string Name { get; set; } = "";
    public string Category { get; set; } = "";
    public int? Value { get; set; }
    public PerLevelInfo? Extra { get; set; }
    public List<string> Raw { get; set; } = new();
}

public sealed class PerLevelInfo
{
    public int? Levels { get; set; }
    public string? Stat { get; set; }
    public int Bonus { get; set; } = 1;
}

public sealed class FilterHit
{
    public string Rule { get; set; } = "";
    public string? Found { get; set; }
    public int? Value { get; set; }
    public bool Override { get; set; }
}

public sealed class FilterResult
{
    public bool Passed { get; set; }
    public string Reason { get; set; } = "";
    public List<FilterHit> Hits { get; set; } = new();
}

public sealed class AttrMatcher
{
    private static readonly Regex SignNumber = new(@"([+-])\s*(\d+)", RegexOptions.Compiled);
    private static readonly Regex AnyNumber = new(@"(\d+)", RegexOptions.Compiled);
    private static readonly Regex WholeNumber = new(@"^\s*(\d+)\s*$", RegexOptions.Compiled);
    private static readonly Regex PerLevelInterval = new(@"每\s*(\d+)\s*[等級级]", RegexOptions.Compiled);
    private static readonly Regex PerLevelInterval2 = new(@"每\s*(\d+)", RegexOptions.Compiled);
    private static readonly Regex PlusTwo = new(@"[+]\s*2", RegexOptions.Compiled);

    private readonly List<AttributeDef> _attributes;
    private readonly List<string> _perLevelStats;

    public AttrMatcher(AttributesConfig cfg)
    {
        _attributes = cfg.Attributes;
        _perLevelStats = cfg.PerLevelStats;
    }

    private static string Normalize(string text)
    {
        var t = text.Trim();
        t = t.Replace("|", "").Replace("`", "");
        t = Regex.Replace(t, @"\s+", "");
        return t;
    }

    private static int? ExtractValue(List<string> texts, string? preferMatcher)
    {
        if (preferMatcher != null)
        {
            foreach (var raw in texts)
            {
                var t = raw.Trim();
                if (!t.Contains(preferMatcher)) continue;
                var m = SignNumber.Match(t);
                if (m.Success) return ParseSigned(m);
                m = AnyNumber.Match(t);
                if (m.Success) return int.Parse(m.Groups[1].Value, CultureInfo.InvariantCulture);
            }
        }
        foreach (var raw in texts)
        {
            var t = raw.Trim();
            var m = SignNumber.Match(t);
            if (m.Success) return ParseSigned(m);
            m = WholeNumber.Match(t);
            if (m.Success) return int.Parse(m.Groups[1].Value, CultureInfo.InvariantCulture);
        }
        return null;
    }

    private static int ParseSigned(Match m)
    {
        var val = int.Parse(m.Groups[2].Value, CultureInfo.InvariantCulture);
        return m.Groups[1].Value == "+" ? val : -val;
    }

    private PerLevelInfo? ExtractPerLevel(List<string> texts)
    {
        var combined = string.Join(" ", texts);
        var m = PerLevelInterval.Match(combined);
        if (!m.Success) m = PerLevelInterval2.Match(combined);
        int? levels = m.Success ? int.Parse(m.Groups[1].Value, CultureInfo.InvariantCulture) : null;

        var bonus = PlusTwo.IsMatch(combined) ? 2 : 1;

        string? stat = null;
        foreach (var s in _perLevelStats)
            if (combined.Contains(s)) { stat = s; break; }

        return levels.HasValue && stat != null
            ? new PerLevelInfo { Levels = levels, Stat = stat, Bonus = bonus }
            : null;
    }

    public List<MatchedAttr> MatchAttributes(List<List<string>> ocrLines)
    {
        var results = new List<MatchedAttr>();

        foreach (var line in ocrLines)
        {
            if (line == null || line.Count == 0) continue;
            var combined = string.Join(" ", line);

            string? bestName = null, bestCategory = null, bestVariant = null;
            double bestScore = 0;

            foreach (var def in _attributes)
            {
                if (def.Category.StartsWith("plv", StringComparison.Ordinal))
                {
                    var statVariant = def.Variants[def.Variants.Count - 1];
                    if (!combined.Contains(statVariant)) continue;
                    if (!combined.Contains('每') && !combined.Contains("等級")) continue;
                    bestName = def.Name;
                    bestCategory = def.Category;
                    bestVariant = statVariant;
                    bestScore = statVariant.Length + 2;
                    break;
                }

                foreach (var variant in def.Variants)
                {
                    double score = 0;
                    if (combined.Contains(variant))
                    {
                        score = variant.Length;
                    }
                    else
                    {
                        var nv = Normalize(variant);
                        var nc = Normalize(combined);
                        if (nv.Length >= 2 && AllCharsIn(nv, nc))
                            score = nv.Length * 0.8;
                    }
                    if (score > bestScore)
                    {
                        bestScore = score;
                        bestName = def.Name;
                        bestCategory = def.Category;
                        bestVariant = variant;
                    }
                }
            }

            if (bestName != null && bestScore > 1)
            {
                var value = ExtractValue(line, bestVariant);
                PerLevelInfo? extra = null;
                if (bestCategory!.StartsWith("plv", StringComparison.Ordinal))
                {
                    extra = ExtractPerLevel(line);
                    if (extra != null)
                    {
                        value = extra.Levels;
                        if (extra.Bonus == 2)
                            bestName = bestName!.Replace("+1", "+2");
                    }
                }

                results.Add(new MatchedAttr
                {
                    Name = bestName!,
                    Category = bestCategory,
                    Value = value,
                    Extra = extra,
                    Raw = new List<string>(line),
                });
            }
        }

        return results;
    }

    private static bool AllCharsIn(string chars, string text)
    {
        foreach (var c in chars)
            if (!text.Contains(c))
                return false;
        return true;
    }

    public static FilterResult CheckFilter(List<MatchedAttr> matched, FilterConfig filter)
    {
        if (!filter.Enabled)
            return new FilterResult { Passed = true, Reason = "filter disabled" };

        // Override rules: if any matches the required count, stop immediately.
        foreach (var rule in filter.OverrideRules)
        {
            var count = CountRule(matched, rule);
            if (count >= (rule.Count > 0 ? rule.Count : 1))
            {
                var res = new FilterResult
                {
                    Passed = true,
                    Reason = $"override: {rule.Name} x{count}",
                };
                res.Hits.Add(new FilterHit { Rule = rule.Name, Override = true });
                return res;
            }
        }

        var mode = string.IsNullOrEmpty(filter.MatchMode) ? "any" : filter.MatchMode;

        if (mode == "per_attr")
        {
            var ruleCounts = new Dictionary<string, int>();
            var hits = new List<FilterHit>();

            foreach (var attr in matched)
            {
                foreach (var rule in filter.Rules)
                {
                    if (attr.Name != rule.Name) continue;
                    if (ValueOk(attr.Value, rule))
                    {
                        ruleCounts[rule.Name] = ruleCounts.GetValueOrDefault(rule.Name) + 1;
                        hits.Add(new FilterHit { Rule = rule.Name, Found = attr.Name, Value = attr.Value });
                        break;
                    }
                }
            }

            foreach (var rule in filter.Rules)
            {
                var need = rule.Count > 0 ? rule.Count : 1;
                var got = ruleCounts.GetValueOrDefault(rule.Name);
                if (got < need)
                    return new FilterResult
                    {
                        Passed = false,
                        Reason = $"'{rule.Name}' matched {got} attrs, need {need}",
                        Hits = hits,
                    };
            }

            return new FilterResult { Passed = true, Reason = $"{hits.Count} attributes matched", Hits = hits };
        }

        var matchedRules = new List<FilterHit>();
        foreach (var rule in filter.Rules)
        {
            foreach (var attr in matched)
            {
                if (attr.Name != rule.Name) continue;
                if (ValueOk(attr.Value, rule))
                    matchedRules.Add(new FilterHit { Rule = rule.Name, Found = attr.Name, Value = attr.Value });
                break;
            }
        }

        if (mode == "any")
        {
            var passed = matchedRules.Count > 0;
            return new FilterResult
            {
                Passed = passed,
                Reason = passed ? $"matched {matchedRules.Count}/{filter.Rules.Count} rules" : "no rules matched",
                Hits = matchedRules,
            };
        }

        // "all"
        var allPassed = matchedRules.Count >= filter.Rules.Count;
        return new FilterResult
        {
            Passed = allPassed,
            Reason = allPassed
                ? $"matched {matchedRules.Count}/{filter.Rules.Count} rules"
                : $"only matched {matchedRules.Count}/{filter.Rules.Count}",
            Hits = matchedRules,
        };
    }

    private static int CountRule(List<MatchedAttr> matched, FilterRule rule)
    {
        var n = 0;
        foreach (var attr in matched)
            if (attr.Name == rule.Name && ValueOk(attr.Value, rule))
                n++;
        return n;
    }

    private static bool ValueOk(int? value, FilterRule rule)
    {
        if (rule.Min != null && value != null && value < rule.Min) return false;
        if (rule.Max != null && value != null && value > rule.Max) return false;
        return true;
    }
}
