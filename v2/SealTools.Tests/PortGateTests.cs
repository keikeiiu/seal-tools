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

    // ── the waiting signal: how a HOLDER finds out that someone needs the game ──────────────────

    // A claim can be refused but never interrupted, so without this the holder simply never learns.
    // That is not hypothetical: a pet reload is ~30 s and a spammer run is unbounded, so the feeder
    // would wait for the entire run and eventually run dry behind a tool that would have stepped
    // aside for half a minute.
    [Fact]
    public void WhoIsWaitingIsReadableWhileTheGateIsHeld()
    {
        var gate = new PortGate();
        gate.TryAcquire("spammer");
        gate.AnnounceWaiting("pet");

        Assert.Equal("spammer", gate.Owner);
        Assert.Equal("pet", gate.Waiting);
    }

    [Fact]
    public void NobodyIsWaitingOnAFreshGate()
    {
        Assert.Null(new PortGate().Waiting);
    }

    // Acquiring IS the end of the wait. Cleared here rather than left to each caller, because a mark
    // left up makes the new holder want its own game — it would step aside the moment it looked.
    [Fact]
    public void AcquiringClearsYourOwnWaitingMark()
    {
        var gate = new PortGate();
        gate.AnnounceWaiting("pet");
        Assert.True(gate.TryAcquire("pet"));

        Assert.Null(gate.Waiting);
    }

    // The same shape as a release from a non-holder, and the dangerous direction: a stale withdrawal
    // must not erase a LIVE waiter, or the next holder sees nobody waiting and keeps the game.
    [Fact]
    public void AWithdrawalFromSomeoneElseLeavesTheWaiterAlone()
    {
        var gate = new PortGate();
        gate.AnnounceWaiting("pet");
        gate.WithdrawWaiting("gem");

        Assert.Equal("pet", gate.Waiting);
    }

    [Fact]
    public void TheWaiterCanStandDown()
    {
        var gate = new PortGate();
        gate.AnnounceWaiting("pet");
        gate.WithdrawWaiting("pet");

        Assert.Null(gate.Waiting);
    }

    [Fact]
    public void AnOwnerIsRequiredToWaitToo()
    {
        var gate = new PortGate();

        Assert.Throws<ArgumentException>(() => gate.AnnounceWaiting(""));
        Assert.Throws<ArgumentException>(() => gate.AnnounceWaiting(null!));
    }

    // The deadline is what replaces every hand-written pair-wise rule: one comparison answers "should
    // I step aside?" for any pair of participants. These tests pin the comparison itself, because it
    // is the one place a policy error is invisible — a rule that is wrong in the yielding direction
    // does not crash, it just starves whoever should have won.

    [Fact]
    public void ACallerWithNoWaiterNeverStandsDown()
    {
        var gate = new PortGate();
        gate.TryAcquire("spammer");

        Assert.False(gate.ShouldStandDown("spammer"));
    }

    [Fact]
    public void ACallerWhoIsNotTheHolderNeverStandsDown()
    {
        // Only the holder has a claim to give up. True here would let a bystander tell itself to
        // release someone else's game.
        var gate = new PortGate();
        gate.TryAcquire("pet");

        Assert.False(gate.ShouldStandDown("spammer"));
    }

    [Fact]
    public void AHolderWithNoDeadlineStandsDownForAWaiterThatHasOne()
    {
        // The whole rule in one test: a tool that merely wants the port yields to one that needs it
        // by a time. This is what the spammer does for the pet feeder today, arrived at by comparison
        // rather than by naming the pet.
        var gate = new PortGate();
        gate.TryAcquire("spammer");
        gate.AnnounceWaiting("pet", DateTime.UtcNow.AddMinutes(5));

        Assert.True(gate.ShouldStandDown("spammer"));
    }

    [Fact]
    public void AWaiterWithNoDeadlineNeverPreempts()
    {
        // The trap, pinned: "someone is waiting" is NOT the same question as "someone is waiting with
        // a reason to go first". Moving a caller onto ShouldStandDown before its waiter declares a
        // deadline would stop it yielding at all — a silent starvation, not a crash.
        var gate = new PortGate();
        gate.TryAcquire("spammer");
        gate.AnnounceWaiting("pet");

        Assert.False(gate.ShouldStandDown("spammer"));
    }

    [Fact]
    public void AHoldersOwnSoonerDeadlineDoesNotStandDown()
    {
        var gate = new PortGate();
        gate.TryAcquire("pet", DateTime.UtcNow.AddMinutes(1));
        gate.AnnounceWaiting("spammer", DateTime.UtcNow.AddMinutes(10));

        Assert.False(gate.ShouldStandDown("pet"));
    }

    [Fact]
    public void ALaterDeadlineStandsDownForASoonerOne()
    {
        var gate = new PortGate();
        gate.TryAcquire("pet", DateTime.UtcNow.AddMinutes(10));
        gate.AnnounceWaiting("spammer", DateTime.UtcNow.AddMinutes(1));

        Assert.True(gate.ShouldStandDown("pet"));
    }

    [Fact]
    public void EqualDeadlinesDoNotStandDown()
    {
        // PLAN-SCHEDULING §8 defers the tie-break because it needs three participants to matter.
        // Pinned so the current answer is a decision rather than an accident of which way `<` fell.
        var at = DateTime.UtcNow.AddMinutes(5);
        var gate = new PortGate();
        gate.TryAcquire("pet", at);
        gate.AnnounceWaiting("spammer", at);

        Assert.False(gate.ShouldStandDown("pet"));
    }

    [Fact]
    public void ADepartedWaiterCannotLeaveItsUrgencyBehind()
    {
        // The deadline belongs to the claim, not to the name. If WithdrawWaiting left it set, the next
        // holder would read the urgency of a waiter that had already given up.
        var gate = new PortGate();
        gate.TryAcquire("spammer");
        gate.AnnounceWaiting("pet", DateTime.UtcNow.AddMinutes(1));
        gate.WithdrawWaiting("pet");

        Assert.False(gate.ShouldStandDown("spammer"));
    }

    [Fact]
    public void ANewHolderDoesNotInheritTheLastOnesDeadline()
    {
        var gate = new PortGate();
        gate.TryAcquire("pet", DateTime.UtcNow.AddMinutes(1));
        gate.Release("pet");

        // The spammer takes the game with no reason to want it by any time.
        gate.TryAcquire("spammer");
        gate.AnnounceWaiting("watcher", DateTime.UtcNow.AddMinutes(30));

        // A stale deadline from the pet would have read as "mine is sooner" and refused to yield.
        Assert.True(gate.ShouldStandDown("spammer"));
    }

    [Fact]
    public void AcquiringEndsTheWaitAndItsDeadline()
    {
        // The existing rule — acquiring takes the mark down — extended to the deadline that now
        // travels with it, or the new holder would carry a reason that was never its own.
        var gate = new PortGate();
        gate.AnnounceWaiting("pet", DateTime.UtcNow.AddMinutes(1));
        gate.TryAcquire("pet");

        Assert.Null(gate.Waiting);
        Assert.False(gate.ShouldStandDown("pet"));
    }

    [Fact]
    public void TheDeadlineDefaultsToNoneAndChangesNothing()
    {
        // Every caller before this existed passes no deadline, and none of them may behave differently
        // now. The old calls are still the old calls.
        var gate = new PortGate();
        gate.TryAcquire("spammer");
        gate.AnnounceWaiting("pet");

        Assert.Equal("spammer", gate.Owner);
        Assert.Equal("pet", gate.Waiting);
        Assert.False(gate.ShouldStandDown("spammer"));
    }
}
