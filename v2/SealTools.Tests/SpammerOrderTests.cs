using System.Collections.Generic;
using SealTools.Spammer;
using Xunit;

namespace SealTools.Tests;

// The spammer's walk order: which key gets to fire when several are due at once. Pulled out of the
// run loop precisely so it can be pinned here — the loop needs a serial port and a board, and this is
// the half of the priority rule that is a decision rather than a press.
public class SpammerOrderTests
{
    // Insertion order IS the dictionary order the loop sees, so these fixtures say what the tool
    // would actually walk rather than an idealised set.
    private static Dictionary<string, double> Keys(params string[] keys)
    {
        var d = new Dictionary<string, double>();
        foreach (var k in keys) d[k] = 1.0;
        return d;
    }

    [Fact]
    public void NamedKeysComeFirstInTheOrderGiven()
    {
        var order = SpammerOrder.For(Keys("q", "w", "e", "t").Keys, new List<string> { "e", "q" });

        Assert.Equal(new List<string> { "e", "q", "w", "t" }, order);
    }

    // The unnamed ones are the "press when we are able to" half. They must still be walked — a
    // priority list that silently dropped them would turn a filler key off entirely, which is the
    // opposite of what naming an order is for.
    [Fact]
    public void UnnamedKeysFollowInDictionaryOrder()
    {
        var order = SpammerOrder.For(Keys("t", "u", "v", "q").Keys, new List<string> { "q" });

        Assert.Equal(new List<string> { "q", "t", "u", "v" }, order);
    }

    // A typo in the order must not stop the rotation, and must not invent a key either.
    [Fact]
    public void ANameMatchingNoKeyIsSkipped()
    {
        var order = SpammerOrder.For(Keys("q", "w").Keys, new List<string> { "nope", "w" });

        Assert.Equal(new List<string> { "w", "q" }, order);
    }

    // First position wins. Taking the LAST occurrence instead would let a duplicated name silently
    // move a key down the rotation, which is the opposite of what the player wrote.
    [Fact]
    public void ARepeatedNameIsTakenOnceAtItsFirstPosition()
    {
        var order = SpammerOrder.For(Keys("q", "w", "e").Keys, new List<string> { "e", "q", "e" });

        Assert.Equal(new List<string> { "e", "q", "w" }, order);
    }

    // The additive promise: a preset written before any of this existed has no order, and must walk
    // exactly as it did. The loop reads this empty case as "no rule", not as "nothing prioritised" —
    // see the comment at SpammerOrder.For.
    [Fact]
    public void NoOrderGivenWalksInDictionaryOrder()
    {
        var order = SpammerOrder.For(Keys("q", "w", "e").Keys, new List<string>());

        Assert.Equal(new List<string> { "q", "w", "e" }, order);
    }

    // Every key the preset has must appear exactly once, whatever the order is — a duplicate would
    // press the same key twice in one tick, which the preempt rule exists to prevent.
    [Fact]
    public void EveryKeyAppearsExactlyOnce()
    {
        var order = SpammerOrder.For(Keys("q", "w", "e", "t").Keys, new List<string> { "t", "t", "ghost", "q" });

        Assert.Equal(new List<string> { "t", "q", "w", "e" }, order);
        Assert.Equal(4, order.Count);
    }
}
