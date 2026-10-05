using System.Collections.Generic;
using System.Linq;
using SealTools.Core.Config;

namespace SealTools.Spammer;

// The spammer's walk order, pulled out of the run loop so it can be tested without a serial port:
// the loop that uses it needs a board, and this is the half of the priority rule that is a DECISION
// rather than a press. What the order MEANS at run time is SkillSpammer's — see the preempt comment
// there for why a higher key holds the lower ones but does not delete them.
public static class SpammerOrder
{
    /// <summary>The keys to walk, highest precedence first: those the preset NAMES, in the order it
    /// gives them, followed by every other key in dictionary order.
    ///
    /// A name that matches no key in the preset is SKIPPED — a typo in the order must not stop the
    /// rotation, and must not add a phantom key either. A name repeated is taken once, at its first
    /// position.
    ///
    /// An empty <paramref name="priority"/> returns the keys in dictionary order, which is exactly
    /// what the loop did before any of this existed. The CALLER decides what that means, because an
    /// empty list cannot distinguish "no rule was asked for" from "no key outranks another" — and
    /// those are different: the first keeps the old send-everything-due behaviour, the second would
    /// need preemption turned on with nothing to give precedence to.</summary>
    public static List<string> For(IEnumerable<string> keys, IEnumerable<string> priority)
    {
        var all = keys.ToList();
        var order = new List<string>();
        foreach (var k in priority)
            if (all.Contains(k) && !order.Contains(k)) order.Add(k);
        foreach (var k in all)
            if (!order.Contains(k)) order.Add(k);
        return order;
    }

    /// <summary>Each name respelled to the form the preset actually stores, when the two differ only by
    /// the leading `*`; anything else comes back untouched.
    ///
    /// This exists because the rows editor writes every key as `*name` — the fast tap — so saving a
    /// preset that a hand-edited config spelled bare RENAMES its keys. `priority` and `combos` reference
    /// keys BY NAME, so without this the editor prunes those references against names that had merely
    /// changed shape: the ordering and the combos vanish on the first save, silently.
    ///
    /// One way only, and only where it is unambiguous. A name matching NEITHER spelling is left alone,
    /// so a genuine typo is still pruned by the normal rule rather than being rewritten into some key
    /// that happens to exist — "correcting" it would be the silent version of the same bug.</summary>
    public static List<string> Respell(IEnumerable<string> names, IEnumerable<string> presetKeys)
    {
        var keys = new HashSet<string>(presetKeys);
        var result = new List<string>();
        foreach (var n in names)
        {
            if (keys.Contains(n)) { result.Add(n); continue; }
            var other = n.StartsWith('*') ? n[1..] : "*" + n;
            result.Add(keys.Contains(other) ? other : n);
        }
        return result;
    }

    /// <summary>The keys the loop may press ON THEIR OWN: the whole preset minus everything a combo
    /// owns, in <see cref="For"/>'s precedence order.
    ///
    /// The subtraction is what makes a combo a UNIT. Left in the walk, the second key of a pair would
    /// fire on its own whenever the first was on cooldown — the effect never triggers, and nothing
    /// says so. A key named in a combo is therefore cast only as part of it.</summary>
    public static List<string> Singles(IEnumerable<string> keys, IEnumerable<string> priority,
                                       IEnumerable<SpammerCombo> combos)
    {
        var owned = new HashSet<string>();
        foreach (var c in combos)
            foreach (var k in c.Keys) owned.Add(k);
        return For(keys, priority).Where(k => !owned.Contains(k)).ToList();
    }

    /// <summary>The combos that can actually run, in the order given: at least two keys, and every
    /// one of them a key this preset has.
    ///
    /// Everything else is DROPPED rather than run, and that includes a combo naming a key the preset
    /// does not have — a typo. Half a combo is worse than no combo: it fires the first skill, spends
    /// its cooldown, and leaves the effect un-triggered with nothing to show for it.</summary>
    public static List<SpammerCombo> Runnable(IEnumerable<string> keys, IEnumerable<SpammerCombo> combos)
    {
        var all = new HashSet<string>(keys);
        return combos.Where(c => c.Keys.Count >= 2 && c.Keys.All(all.Contains)).ToList();
    }
}
