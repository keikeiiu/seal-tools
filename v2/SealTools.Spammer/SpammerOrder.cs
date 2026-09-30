using System.Collections.Generic;
using System.Linq;

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
}
