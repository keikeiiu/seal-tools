namespace SealTools.Core;

// Which tools hold a keyboard key down for as long as they run, so that every stop path can let go
// of it without knowing the tool's internals.
//
// Hold Space is the only one today: it writes P to press the spacebar down and U to release it. A
// stop that skips the U leaves the player's keyboard stuck, which is a failure the player cannot
// miss and the tool cannot recover from on its own. The loop that would normally send the release
// is exactly what a stop is interrupting — and the release that matters is the one that still goes
// out when that loop's own write is the thing that threw — so the question lives here, in Core,
// where LauncherService can ask it and the test project can reach it.
public static class HeldKeys
{
    /// <summary>True when stopping this tool must also send the spacebar's release. Null (nothing
    /// running) is false — there is no key to let go of.</summary>
    public static bool NeedsSpaceRelease(string? toolId) => toolId == "holdspace";
}
