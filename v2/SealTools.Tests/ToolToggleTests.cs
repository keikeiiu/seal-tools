using SealTools.Core;
using Xunit;

namespace SealTools.Tests;

// One press, one decision — and this decision has been wrong three times, each time in a way that cost
// a live debugging session. The three states below are the three bugs.
public class ToolToggleTests
{
    // Nothing engaged: the only case that starts anything.
    [Fact]
    public void AnIdleToolStarts()
    {
        Assert.Equal(ToggleAction.Start, ToolToggle.Decide(isLoaded: false, isStarting: false, isStopping: false));
    }

    [Fact]
    public void ALiveToolStops()
    {
        Assert.Equal(ToggleAction.Stop, ToolToggle.Decide(isLoaded: true, isStarting: false, isStopping: false));
    }

    // Bug 1, 2026-10-02: a press during the ~2 s cold start took the Start branch again and was
    // swallowed, while the key stayed held and nothing on screen said so.
    [Fact]
    public void APressWhileStartingStopsRatherThanStartingAgain()
    {
        Assert.Equal(ToggleAction.Stop, ToolToggle.Decide(isLoaded: false, isStarting: true, isStopping: false));
    }

    // Bug 3, 2026-10-06, and the mirror of bug 1: StopTool clears the current id BEFORE the worker has
    // left its loop, so for that window the tool is neither loaded nor starting — and a press used to
    // START it. The key came back on.
    [Fact]
    public void APressWhileStoppingDoesNotStartItAgain()
    {
        Assert.Equal(ToggleAction.Stop, ToolToggle.Decide(isLoaded: false, isStarting: false, isStopping: true));
    }

    // Stopping outranks loaded, which matters because both are true at the same instant: the id is
    // cleared as the stop begins, but a card can still be holding the old reading for a tick. Either
    // way the answer is Stop, so the combination cannot be the one that starts a tool.
    [Fact]
    public void EveryEngagedCombinationStops()
    {
        foreach (var loaded in new[] { true, false })
            foreach (var starting in new[] { true, false })
                foreach (var stopping in new[] { true, false })
                {
                    var engaged = loaded || starting || stopping;
                    var expected = engaged ? ToggleAction.Stop : ToggleAction.Start;
                    Assert.Equal(expected, ToolToggle.Decide(loaded, starting, stopping));
                }
    }
}
