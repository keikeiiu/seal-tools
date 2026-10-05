using System.Collections.Generic;
using SealTools.Core.Config;
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

    // ── combos: the keys a pair owns, and which pairs can actually run ──────────────────────────

    private static SpammerCombo Combo(params string[] keys) => new() { Keys = new List<string>(keys), Gap = 0.8 };

    // THE LOCKED PAIR, and the reason Singles exists: a key a combo names must not also be walkable
    // on its own. Left in, "2" would fire whenever "1" happened to be on cooldown — the combo effect
    // never triggers, and nothing anywhere says so.
    [Fact]
    public void KeysOwnedByAComboAreNotPressedOnTheirOwn()
    {
        var singles = SpammerOrder.Singles(
            Keys("1", "2", "3", "4", "q").Keys,
            new List<string> { "1", "2", "3", "4" },
            new List<SpammerCombo> { Combo("1", "2"), Combo("3", "4") });

        Assert.Equal(new List<string> { "q" }, singles);
    }

    // A preset with no combos must lose nothing to this: every key stands on its own, exactly as
    // before combos existed.
    [Fact]
    public void WithNoCombosEveryKeyStandsAlone()
    {
        var singles = SpammerOrder.Singles(
            Keys("q", "w", "e").Keys, new List<string> { "e" }, new List<SpammerCombo>());

        Assert.Equal(new List<string> { "e", "q", "w" }, singles);
    }

    // A key the combo does NOT name keeps its place in the precedence order.
    [Fact]
    public void UnnamedKeysKeepTheirPrecedence()
    {
        var singles = SpammerOrder.Singles(
            Keys("q", "w", "e", "1", "2").Keys,
            new List<string> { "e", "q" },
            new List<SpammerCombo> { Combo("1", "2") });

        Assert.Equal(new List<string> { "e", "q", "w" }, singles);
    }

    // HALF A COMBO IS WORSE THAN NONE: it would fire the first skill, spend its cooldown, and leave
    // the effect un-triggered. So a one-key combo is dropped rather than run.
    [Fact]
    public void AComboNeedsAtLeastTwoKeys()
    {
        var runnable = SpammerOrder.Runnable(Keys("1", "2").Keys, new List<SpammerCombo> { Combo("1") });

        Assert.Empty(runnable);
    }

    // A typo in a combo must not produce a broken pair — it produces no pair at all.
    [Fact]
    public void AComboNamingAKeyThePresetLacksIsDropped()
    {
        var runnable = SpammerOrder.Runnable(Keys("1", "2").Keys, new List<SpammerCombo> { Combo("1", "nope") });

        Assert.Empty(runnable);
    }

    // ── respelling a reference to match the keys it names ───────────────────────────────────────

    // THE BUG THIS EXISTS FOR: the rows editor writes every key as '*name', so saving a preset that a
    // hand-edited config spelled bare RENAMES its keys — and `priority` and `combos` name keys, so the
    // references were pruned against names that had merely changed shape. The ordering and the combos
    // disappeared on the first save, silently.
    [Fact]
    public void AReferenceFollowsItsKeyWhenOnlyTheStarDiffers()
    {
        var respelled = SpammerOrder.Respell(
            new List<string> { "F4", "F5" },
            new List<string> { "*F4", "*F5" });

        Assert.Equal(new List<string> { "*F4", "*F5" }, respelled);
    }

    // The other direction, so the helper is a match rather than a rule about stars.
    [Fact]
    public void AStarredReferenceFollowsABareKey()
    {
        var respelled = SpammerOrder.Respell(
            new List<string> { "*F4" },
            new List<string> { "F4" });

        Assert.Equal(new List<string> { "F4" }, respelled);
    }

    // A name that matches NEITHER spelling must be left alone. "Correcting" it would rewrite a typo
    // into some key that happens to exist — the silent version of the same bug the helper prevents.
    [Fact]
    public void ANameThatMatchesNothingIsLeftAlone()
    {
        var respelled = SpammerOrder.Respell(
            new List<string> { "ghost", "F4" },
            new List<string> { "*F4", "*F5" });

        Assert.Equal(new List<string> { "ghost", "*F4" }, respelled);
    }

    // Already consistent, which is the common case: nothing moves.
    [Fact]
    public void ANameAlreadySpelledRightIsUnchanged()
    {
        var respelled = SpammerOrder.Respell(
            new List<string> { "*1", "*2" },
            new List<string> { "*1", "*2" });

        Assert.Equal(new List<string> { "*1", "*2" }, respelled);
    }

    // The order the combos are listed in is the order they are tried, so it must survive.
    [Fact]
    public void RunnableCombosKeepTheirListedOrder()
    {
        var runnable = SpammerOrder.Runnable(
            Keys("1", "2", "3", "4").Keys,
            new List<SpammerCombo> { Combo("3", "4"), Combo("1", "2") });

        Assert.Equal(2, runnable.Count);
        Assert.Equal(new List<string> { "3", "4" }, runnable[0].Keys);
        Assert.Equal(new List<string> { "1", "2" }, runnable[1].Keys);
    }
}
