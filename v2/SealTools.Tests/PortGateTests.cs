using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using SealTools.Core;
using Xunit;

namespace SealTools.Tests;

// One Arduino, one cursor, two tools that must take turns. The gate exists so that "is it free?" and
// "take it" are a single operation — the interesting tests here are the ones about what happens when
// two callers arrive together, and about a release arriving from someone who does not hold it.
public class PortGateTests
{
    [Fact]
    public void AFreeGateCanBeTakenAndGivenBack()
    {
        var gate = new PortGate();

        Assert.Null(gate.Owner);
        Assert.True(gate.TryAcquire("pet"));
        Assert.True(gate.Release("pet"));
        Assert.Null(gate.Owner);
    }

    [Fact]
    public void TheOwnerIsReadableWhileHeld()
    {
        // Whoever is waiting needs to name who they are waiting FOR, so the owner has to survive the
        // claim rather than being a bool.
        var gate = new PortGate();
        gate.TryAcquire("pet");

        Assert.Equal("pet", gate.Owner);
    }

    [Fact]
    public void ASecondClaimIsRefusedWhileItIsHeld()
    {
        var gate = new PortGate();
        gate.TryAcquire("pet");

        Assert.False(gate.TryAcquire("gem"));
        Assert.Equal("pet", gate.Owner); // and the refusal did not take it
    }

    [Fact]
    public void TheSameOwnerAskingAgainIsAlsoRefused()
    {
        // Not reference-counted on purpose: a claim that is refused leaves nothing to undo, whereas a
        // tool that claimed twice and released once would leave the gate free while it still ran.
        var gate = new PortGate();
        gate.TryAcquire("pet");

        Assert.False(gate.TryAcquire("pet"));
    }

    [Fact]
    public void AReleaseFromSomeoneElseIsIgnored()
    {
        // The dangerous direction. A stale tool finishing late must not free the gate its competitor
        // is waiting on — that is how two tools end up writing at once.
        var gate = new PortGate();
        gate.TryAcquire("pet");

        Assert.False(gate.Release("gem"));
        Assert.Equal("pet", gate.Owner);
        Assert.False(gate.TryAcquire("gem")); // still held, and gem still cannot have it
    }

    [Fact]
    public void ReleasingAFreeGateIsNotARelease()
    {
        var gate = new PortGate();

        Assert.False(gate.Release("pet"));
        Assert.Null(gate.Owner);
    }

    [Fact]
    public void AnOwnerIsRequired()
    {
        var gate = new PortGate();

        Assert.Throws<ArgumentException>(() => gate.TryAcquire(""));
        Assert.Throws<ArgumentException>(() => gate.TryAcquire(null!));
    }

    [Fact]
    public async Task OnlyOneCallerCanWinWhenTheyArriveTogether()
    {
        // The reason this is a gate and not a check-then-act. Thirty tools racing for the port; if the
        // claim were a read followed by a write, more than one would come away believing it had it.
        var gate = new PortGate();
        var winners = new List<string>();
        var start = new TaskCompletionSource();

        var racers = Enumerable.Range(0, 30).Select(i => Task.Run(async () =>
        {
            await start.Task;
            if (gate.TryAcquire($"tool{i}")) lock (winners) winners.Add($"tool{i}");
        })).ToArray();

        start.SetResult();
        await Task.WhenAll(racers);

        Assert.Single(winners);
        Assert.Equal(winners[0], gate.Owner);
    }

    [Fact]
    public async Task TheGateIsFreeAgainAfterTheHolderReleasesIt()
    {
        var gate = new PortGate();
        gate.TryAcquire("pet");
        await Task.Run(() => gate.Release("pet"));

        Assert.True(gate.TryAcquire("gem"));
        Assert.Equal("gem", gate.Owner);
    }
}
