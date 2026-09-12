using System.Collections.Generic;
using SealTools.Core.Config;
using SealTools.Tuner;
using Xunit;

namespace SealTools.Tests;

// The filter is what decides the tuner should stop, and two of its behaviours look like bugs to a
// reader who does not know why they are there. Those are pinned first, so a future "cleanup" has to
// argue with a failing test rather than quietly change when the tuner stops.
public class AttrMatcherTests
{
    private static MatchedAttr Attr(string name, int? value) => new() { Name = name, Value = value };

    private static FilterConfig Filter(bool enabled = true, string mode = "per_attr", params FilterRule[] rules) =>
        new() { Enabled = enabled, MatchMode = mode, Rules = new List<FilterRule>(rules) };

    private static FilterRule Rule(string name, int count, int? min = null, int? max = null) =>
        new() { Name = name, Count = count, Min = min, Max = max };

    // DELIBERATE, do not "fix" this into a rejection. 減少傷害 is rare enough that the NAME matching is
    // the signal; a value the OCR failed to read must not disqualify a roll that would otherwise stop
    // the run. Bounds still apply whenever a value was actually read — see the next test.
    [Fact]
    public void AnUnreadableValueStillSatisfiesABoundedRule()
    {
        var filter = Filter(rules: Rule("減少傷害", count: 2, min: 1, max: 1));

        var result = AttrMatcher.CheckFilter(
            new List<MatchedAttr> { Attr("減少傷害", null), Attr("減少傷害", null) }, filter);

        Assert.True(result.Passed);
    }

    // The other half of that decision: when the value IS read, the bounds are enforced.
    [Fact]
    public void AReadValueOutsideTheBoundsDoesNotSatisfyTheRule()
    {
        var filter = Filter(rules: Rule("減少傷害", count: 2, min: 1, max: 1));

        var result = AttrMatcher.CheckFilter(
            new List<MatchedAttr> { Attr("減少傷害", 3), Attr("減少傷害", 3) }, filter);

        Assert.False(result.Passed);
    }

    // With the filter off, CheckFilter reports Passed — so SealTuner has to consult filter.Enabled
    // itself rather than trusting this result. Pinned from this side so the coupling is visible from
    // either file; the tuner side was the "stopped below target grade" bug.
    [Fact]
    public void ADisabledFilterReportsPassed()
    {
        var result = AttrMatcher.CheckFilter(new List<MatchedAttr>(), Filter(enabled: false));

        Assert.True(result.Passed);
        Assert.Equal("filter disabled", result.Reason);
    }

    // The count has to be met by distinct attribute lines, not by one line counted repeatedly.
    [Fact]
    public void TooFewMatchingLinesFailsTheRule()
    {
        var filter = Filter(rules: Rule("減少傷害", count: 2, min: 1, max: 1));

        var result = AttrMatcher.CheckFilter(
            new List<MatchedAttr> { Attr("減少傷害", 1) }, filter);

        Assert.False(result.Passed);
    }

    // An override rule short-circuits the whole filter, which is why the null-value behaviour above
    // reaches the stop condition so directly.
    [Fact]
    public void AnOverrideRulePassesWithoutConsultingTheNormalRules()
    {
        var filter = Filter(mode: "all", rules: Rule("增加傷害", count: 3, min: 1, max: 1));
        filter.OverrideRules = new List<FilterRule> { Rule("減少傷害", count: 2, min: 1, max: 1) };

        var result = AttrMatcher.CheckFilter(
            new List<MatchedAttr> { Attr("減少傷害", null), Attr("減少傷害", null) }, filter);

        Assert.True(result.Passed);
        Assert.StartsWith("override", result.Reason);
    }
}
