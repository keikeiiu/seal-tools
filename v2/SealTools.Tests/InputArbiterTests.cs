using System;
using System.IO.Ports;
using SealTools.Core;
using Xunit;

namespace SealTools.Tests;

// The UI's test buttons are the one input path that never asked the gate, and the whole fix is that
// they now ask it and are refused while something else is driving. These tests pin the refusal and
// the giving-back, which are the two halves that fail silently if they are wrong: a lease that is
// never released refuses every later test, and a refusal that does not name the holder reads as a
// broken button.
//
// They live here rather than against the launcher because the launcher is net8.0-windows and is not
// referenced by this project — which is why the decision was put in Core rather than in the service.
public class InputArbiterTests
{
    private static InputLease Lease(InputArbiter arbiter)
    {
        Assert.True(arbiter.TryAcquireUiLease(out var refusal), refusal);
        return new InputLease(arbiter, new SerialPort());
    }

    [Fact]
    public void AFreeGameCanBeTakenForATest()
    {
        var gate = new PortGate();
        var arbiter = new InputArbiter(gate);

        Assert.True(arbiter.TryAcquireUiLease(out var refusal));
        Assert.Null(refusal);
        Assert.Equal(InputArbiter.UiOwner, gate.Owner);
    }

    [Fact]
    public void ATestIsRefusedWhileAToolHoldsTheGameAndIsToldWhich()
    {
        var gate = new PortGate();
        var arbiter = new InputArbiter(gate);
        gate.TryAcquire("pet");

        Assert.False(arbiter.TryAcquireUiLease(out var refusal));

        // The name matters more than the refusal: "pet" is actionable, "busy" is not, and a refusal a
        // player cannot act on is the broken-button failure this repo keeps paying for.
        Assert.NotNull(refusal);
        Assert.Contains("pet", refusal);
        Assert.Contains("stop it", refusal);
    }

    [Fact]
    public void ARefusedTestLeavesNothingBehind()
    {
        // A refused claim must not have taken the game on its way to refusing — otherwise the refusal
        // itself is what locks everyone else out.
        var gate = new PortGate();
        var arbiter = new InputArbiter(gate);
        gate.TryAcquire("spammer");

        Assert.False(arbiter.TryAcquireUiLease(out _));
        Assert.Equal("spammer", gate.Owner);
    }

    [Fact]
    public void DisposingTheLeaseGivesTheGameBack()
    {
        var gate = new PortGate();
        var arbiter = new InputArbiter(gate);

        var lease = Lease(arbiter);
        Assert.Equal(InputArbiter.UiOwner, gate.Owner);

        lease.Dispose();
        Assert.Null(gate.Owner);
    }

    [Fact]
    public void AToolThatStartsAfterTheLeaseEndsCanTakeTheGame()
    {
        // The point of giving it back: the next tool is not permanently locked out by a test that ran
        // once. This is the failure mode when a lease is never released.
        var gate = new PortGate();
        var arbiter = new InputArbiter(gate);

        Lease(arbiter).Dispose();

        Assert.True(gate.TryAcquire("buy"));
    }

    [Fact]
    public void ASecondTestIsRefusedWhileTheFirstHoldsTheGame()
    {
        var gate = new PortGate();
        var arbiter = new InputArbiter(gate);

        var first = Lease(arbiter);

        Assert.False(arbiter.TryAcquireUiLease(out var refusal));
        Assert.NotNull(refusal);

        first.Dispose();
        Assert.True(arbiter.TryAcquireUiLease(out _));
    }

    [Fact]
    public void AStaleDoubleDisposeCannotReleaseAClaimTakenSince()
    {
        // Both leases carry the same owner name, so a second Dispose would release whatever claim had
        // been taken in between — a silent overlap rather than a redundant call. That is the one
        // outcome the gate exists to prevent, and it is why Dispose is guarded by an exchange.
        var gate = new PortGate();
        var arbiter = new InputArbiter(gate);

        var first = Lease(arbiter);
        first.Dispose();

        var second = Lease(arbiter);
        first.Dispose();   // stale

        Assert.Equal(InputArbiter.UiOwner, gate.Owner);

        second.Dispose();
        Assert.Null(gate.Owner);
    }

    [Fact]
    public void GivingUpAfterAcquiringDoesNotStrandTheClaim()
    {
        // The path a failed port open takes: the claim is taken, then handed back. If it were not,
        // every later test would be refused with "another test is using the game" and no test would
        // be — a refusal that is true about nothing.
        var gate = new PortGate();
        var arbiter = new InputArbiter(gate);

        Assert.True(arbiter.TryAcquireUiLease(out _));
        arbiter.ReleaseUiLease();

        Assert.Null(gate.Owner);
    }

    [Fact]
    public void ReleasingOnBehalfOfAnotherOwnerIsIgnored()
    {
        // The gate refuses a release from a non-holder, and the arbiter must not be a way around
        // that — a stale UI release must not free a tool's claim.
        var gate = new PortGate();
        var arbiter = new InputArbiter(gate);
        gate.TryAcquire("pet");

        arbiter.ReleaseUiLease();

        Assert.Equal("pet", gate.Owner);
    }
}
