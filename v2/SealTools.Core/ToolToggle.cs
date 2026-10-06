namespace SealTools.Core;

/// <summary>What pressing a tool's toggle should do.</summary>
public enum ToggleAction
{
    Start,
    Stop,
}

/// <summary>What one press of a tool toggle means, given what that tool is doing right now.
///
/// THREE separate bugs have landed on this single question, which is why it is a function with tests
/// rather than an `if` inside a click handler:
///
///  - it asked only "is the tool loaded?", so a press during the ~2 s cold start re-entered Start and
///    was swallowed while the key stayed held — the Hold Space bug of 2026-10-02;
///  - it then asked "loaded OR starting?", which fixed that and left the mirror image: **stopping**.
///    `StopTool` clears the current id BEFORE the worker has left its loop, so the button reads idle
///    while the tool is still winding down, and the next press starts it again (found 2026-10-06);
///  - both of those were "is it running?", a question with a **window** in it — and each fix closed one
///    window and revealed another.
///
/// So the question this asks is not "is it running?" but **"is it engaged at all?"** — loaded, on its
/// way in, or on its way out — and that has no window: every one of those three states means a press
/// should stop or cancel, and none of them means start.</summary>
public static class ToolToggle
{
    public static ToggleAction Decide(bool isLoaded, bool isStarting, bool isStopping)
        => isLoaded || isStarting || isStopping ? ToggleAction.Stop : ToggleAction.Start;
}
