using SealTools.Core.Config;

namespace SealTools.Core;

// The composer's routes, and where each one ends. There are TWO move sets for the same routes,
// chosen by gem.move_mode:
//
//   "tuned"   — the raw dx/dy counts in gem.movements: one hand-tuned "D dx dy" per route. Kept as
//               the default; these values are per-PC and deliberately untouched by this class.
//   "arduino" — place the cursor on the route's destination POINT with the Arduino, closed-loop —
//               the same mechanism as the calibrator's Test Click. Self-correcting, so pointer
//               speed/acceleration and any drift mid-run stop mattering, and nothing needs tuning.
//
// This class is the shared definition of the arduino set. A route ends on a calibrated point, so
// the set needs no numbers of its own: it reuses the points the user already calibrated.
public static class GemRoutes
{
    /// <summary>One composer route: <paramref name="Key"/> matches the gem.movements field and the
    /// calibrator's row key; <paramref name="To"/> is a calibrated point name (see
    /// <see cref="Resolve"/>).</summary>
    public sealed record Route(string Key, string From, string To, string Label);

    public static readonly IReadOnlyList<Route> All = new[]
    {
        new Route("radio_N", "N", "Register", "N → Register"),
        new Route("radio_G", "G", "Register", "G → Register"),
        new Route("radio_DG", "DG", "Register", "DG → Register"),
        new Route("register_combine", "Register", "Combine", "Register → Combine"),
        new Route("combine_register", "Combine", "Register", "Combine → Register"),
        new Route("register_slot1", "Register", "Resource1", "Register → Resource1"),
        new Route("slot1_slot2", "Resource1", "Resource2", "Resource1 → Resource2"),
        new Route("slot2_slot3", "Resource2", "Resource3", "Resource2 → Resource3"),
        new Route("slot3_n", "Resource3", "N", "Resource3 → N"),
        new Route("slot3_g", "Resource3", "G", "Resource3 → G"),
        new Route("slot3_dg", "Resource3", "DG", "Resource3 → DG"),
    };

    /// <summary>The calibrated point a route ends on (the arduino set's whole definition), or null
    /// when the key isn't a known route.</summary>
    public static string? Destination(string key)
        => All.FirstOrDefault(r => r.Key == key)?.To;

    /// <summary>Resolves a calibrated point name to its client-relative PHYSICAL offset: the
    /// grade/Register/Combine entries in gem.grade_positions, and Resource1..3 in gem.resource_gems.
    /// Null when that point hasn't been calibrated.</summary>
    public static (int X, int Y)? Resolve(AppConfig cfg, string name)
    {
        if (name.StartsWith("Resource", StringComparison.Ordinal))
        {
            if (!int.TryParse(name.AsSpan("Resource".Length), out var i)) return null;
            if (i < 1 || cfg.Gem.ResourceGems.Count < i) return null;
            var r = cfg.Gem.ResourceGems[i - 1];
            return r is { Count: >= 2 } ? (r[0], r[1]) : null;
        }

        return cfg.Gem.GradePositions.TryGetValue(name, out var p) && p is { Count: >= 2 }
            ? (p[0], p[1])
            : null;
    }
}
