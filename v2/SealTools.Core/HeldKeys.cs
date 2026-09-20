namespace SealTools.Core;

// Which tools can leave something HELD DOWN when they stop, so that every stop path can let go of it
// without knowing the tool's internals.
//
// Two today, and both hold something the player cannot miss:
//
//   holdspace  writes P to press the spacebar down and U to release it.
//   pet        writes L to press the left mouse button down and l to release it, for the food drag —
//              and a left button left down follows the player's REAL cursor and drops whatever it is
//              over on the next press.
//
// A stop that skips the release leaves the player's input stuck. The loop that would normally send it
// is exactly what a stop is interrupting — and the release that matters is the one that still goes out
// when that loop's own write is the thing that threw — so the question lives here, in Core, where
// LauncherService can ask it and the test project can reach it.
public static class HeldKeys
{
    /// <summary>True when stopping this tool must also let go of whatever it can leave held. Null
    /// (nothing running) is false — there is nothing to let go of.</summary>
    public static bool NeedsReleaseOnStop(string? toolId) => toolId is "holdspace" or "pet";
}
