using System;
using System.IO.Ports;
using System.Threading;

namespace SealTools.Core;

// Who may send input right now, and the permission slip that proves it.
//
// This is the launcher's test and calibration buttons' way in, and for now it is the ONLY thing it
// does: `PortGate` already decides who drives the game, and the buttons were the one path that never
// asked it. Nineteen sites in MainWindow took the port and wrote to it directly, so a Test Click
// pressed while a tool was mid-gesture landed in the middle of that gesture — and for the pet feeder
// mid-drag that drops the stack of food it is carrying.
//
// It is deliberately the gate plus a name, and not a second rule beside the gate. Two things deciding
// who may drive is worse than one thing deciding it badly, because the two can disagree and neither
// is then authoritative. `PortGate` stays the single owner of the answer; this only gives the UI a
// way to ask it and to hold what it is given.
//
// NOT here, and not later either without a decision: the arbiter does not know what a tool is doing.
// It knows who asked, and whether the game is taken. Pet rows, gem counts and combo timings are the
// tools' business — an arbiter that learns about them has stopped arbitrating and started deciding.
public sealed class InputArbiter
{
    /// <summary>The owner id a test or calibration button claims the game under. Deliberately not a
    /// tool id: a test and a tool have to be able to tell each other apart, and a shared name would
    /// make a stale test look like the tool it is refusing.</summary>
    public const string UiOwner = "test";

    private readonly PortGate _gate;

    public InputArbiter(PortGate gate) => _gate = gate;

    /// <summary>The gate itself, for callers that need to report who holds it without going through
    /// a claim. Read-only use — taking a claim through here would be the second rule this class
    /// exists to avoid.</summary>
    public PortGate Gate => _gate;

    /// <summary>Takes the game for a test, or says why not. The check and the claim are one operation
    /// inside <see cref="PortGate.TryAcquire"/>, so a tool that starts in between loses the race
    /// rather than being interleaved with.</summary>
    public bool TryAcquireUiLease(out string? refusal)
    {
        // A named holder first, because it produces the better sentence: "pet" is a far more useful
        // thing to tell the player than "busy". The plain TryAcquire below is the race guard.
        if (_gate.Owner is { } holder)
        {
            refusal = Busy(holder);
            return false;
        }

        if (!_gate.TryAcquire(UiOwner))
        {
            refusal = Busy(_gate.Owner ?? "another test");
            return false;
        }

        refusal = null;
        return true;
    }

    /// <summary>Hands the game back after a test that acquired it but could not go on — a port that
    /// would not open, say. Without this the claim outlives the attempt and every later test is
    /// refused with "another test is using the game" while no test is.</summary>
    public void ReleaseUiLease() => _gate.Release(UiOwner);

    /// <summary>What a refused test is told. It names the holder and says the one thing that clears
    /// it, because a refusal that does not say how to proceed reads as a broken button rather than a
    /// busy game — which is the failure this repo keeps paying for.</summary>
    public static string Busy(string holder) =>
        $"\"{holder}\" is using the game right now — stop it, then try again.";
}

/// <summary>A test's permission to drive the game, and the port to drive it through. The `using` is
/// the whole contract: the claim ends when the test does, on every path including a throw.
///
/// The port travels with the lease rather than being handed out separately, so "may I write?" and
/// "here is what to write to" cannot get out of step — obtaining the port means obtaining the
/// permission.</summary>
public sealed class InputLease : IDisposable
{
    private readonly InputArbiter _arbiter;
    private int _released;

    public InputLease(InputArbiter arbiter, SerialPort port)
    {
        _arbiter = arbiter;
        Port = port;
    }

    /// <summary>The shared port. Valid while the lease is held — which is the only time it may be
    /// written to, and the reason it is not obtainable any other way from the UI.</summary>
    public SerialPort Port { get; }

    public void Dispose()
    {
        // Exactly once. Two live leases cannot exist — the gate refuses the second — so the only way
        // to release twice is a double Dispose, and without this guard the second one would release
        // whatever claim had been taken since. Both leases carry the same owner name, so that release
        // would succeed: a silent overlap, which is the one outcome the gate exists to prevent.
        if (Interlocked.Exchange(ref _released, 1) == 0) _arbiter.ReleaseUiLease();
    }
}
