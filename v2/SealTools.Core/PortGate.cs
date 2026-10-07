using System;

namespace SealTools.Core;

// Who is driving the game right now.
//
// There is one Arduino and one cursor, so two tools must never write at the same time. Today that is
// enforced by there only ever being one tool — pressing Start stops whatever was running. The pet
// feeder breaks that assumption: it has to keep its schedule while other tools come and go, so the
// rule becomes "at most one NON-PET tool, plus the pet feeder, which waits".
//
// The gate is the half of that rule that can be checked without a UI: a tool claims it before it
// starts writing and releases it when it stops. Checking "is anything running?" and then acting on
// the answer is a race — the answer can change between the two — so the check and the claim have to
// be one operation. That is all this is.
//
// Deliberately NOT a lock and not a queue. Serial writes stay direct and unlocked, and the gate is
// the guarantee rather than a wrapper around each write: it makes the two tools take turns, which is
// the only thing that is safe. Two tools INTERLEAVING on the cursor is still wrong, which is why the
// pet feeder defers for the whole reload instead of sharing it. A gate that made interleaving look
// safe would be worse than no gate.
public sealed class PortGate
{
    private readonly object _sync = new();
    private string? _owner;
    private DateTime? _ownerDeadline;
    private string? _waiting;
    private DateTime? _waitingDeadline;

    /// <summary>The tool id that holds the gate, or null when it is free. Readable while held —
    /// callers show it, so "who is the pet feeder waiting for?" has an answer.</summary>
    public string? Owner
    {
        get { lock (_sync) return _owner; }
    }

    /// <summary>Who is waiting for the gate, or null when nobody is.
    ///
    /// This exists because a claim can only be REFUSED, never interrupted. A tool that holds the game
    /// never finds out that someone else wants it, so a waiter with a short job is at the mercy of a
    /// holder with a long one — and that is not hypothetical: a pet reload is ~30 s, and a spammer run
    /// is unbounded, so without this the feeder waits for the whole run and eventually runs dry with
    /// the game held by a tool that would have been happy to step aside for half a minute.
    ///
    /// The holder reads this and decides. The gate does not break a claim on anyone's behalf.</summary>
    public string? Waiting
    {
        get { lock (_sync) return _waiting; }
    }

    /// <summary>Claims the gate for <paramref name="owner"/>. False when anyone else holds it,
    /// including the same owner asking twice: a claim that is refused leaves nothing to undo, and
    /// reference-counting claims is how a tool ends up holding the gate while it is free.
    ///
    /// <paramref name="deadline"/> is why the holder wants the game, when it has a reason to want it
    /// by a time. Null means "no deadline" — the ordinary case, and what every caller had before this
    /// existed. It changes nothing on its own; it is only read by <see cref="ShouldStandDown"/>.</summary>
    public bool TryAcquire(string owner, DateTime? deadline = null)
    {
        if (string.IsNullOrEmpty(owner)) throw new ArgumentException("owner required", nameof(owner));

        lock (_sync)
        {
            if (_owner != null) return false;
            _owner = owner;
            _ownerDeadline = deadline;
            // Acquiring ENDS the wait, so the mark comes down here rather than relying on every caller
            // to remember — a mark left up makes the new holder want its own game, and it would step
            // aside the moment it looked.
            if (_waiting == owner) { _waiting = null; _waitingDeadline = null; }
            return true;
        }
    }

    /// <summary>A waiter says so BEFORE it starts polling. Without the announcement the holder cannot
    /// know to stand down, which is the whole point of the field above.
    ///
    /// <paramref name="deadline"/> is the same idea as the one on <see cref="TryAcquire"/>: a waiter
    /// that has a time it must run by can outrank a holder that merely wants the port. A waiter that
    /// names none can never outrank anyone — see <see cref="ShouldStandDown"/>.</summary>
    public void AnnounceWaiting(string owner, DateTime? deadline = null)
    {
        if (string.IsNullOrEmpty(owner)) throw new ArgumentException("owner required", nameof(owner));
        lock (_sync)
        {
            _waiting = owner;
            _waitingDeadline = deadline;
        }
    }

    /// <summary>The waiter stands down — it gave up, or the run was cancelled. Only clears its OWN
    /// mark, so a stale withdrawal cannot erase a live waiter and leave the next holder with no reason
    /// to step aside.</summary>
    public void WithdrawWaiting(string owner)
    {
        lock (_sync)
        {
            if (_waiting != owner) return;
            _waiting = null;
            _waitingDeadline = null;
        }
    }

    /// <summary>The one question a holder asks instead of comparing strings: does whoever is waiting
    /// have a SOONER deadline than mine? A holder with no deadline always stands down for a waiter
    /// that has one; a holder with a sooner deadline than the waiter's does not.
    ///
    /// This replaces every hand-written pair-wise rule. Today the spammer yields to anyone waiting and
    /// the only waiter is the pet, so "anything without a deadline yields to anything with one" is
    /// already what the code does — it just looks like a special case because there has only ever been
    /// one case.
    ///
    /// **A waiter that declared no deadline never preempts, so this returns false for it.** That makes
    /// it a behaviour change to move a caller onto this method *before the waiter declares a
    /// deadline* — it would stop yielding entirely. The two move together.
    ///
    /// False for a caller that does not hold the gate: only the holder has anything to stand down
    /// from, and answering otherwise would let a bystander release someone else's claim.
    ///
    /// Equal deadlines do NOT stand down. That is the deferred tie-break from PLAN-SCHEDULING §8 —
    /// it only matters with three participants, so it is pinned here as a decision rather than left
    /// to whichever way `&lt;` happened to fall.</summary>
    public bool ShouldStandDown(string owner)
    {
        lock (_sync)
        {
            if (_owner != owner) return false;
            if (_waiting == null || _waitingDeadline == null) return false;
            if (_ownerDeadline == null) return true;
            return _waitingDeadline < _ownerDeadline;
        }
    }

    /// <summary>Releases the gate, but only for the tool that holds it. A release from anyone else is
    /// ignored — it is either a stale tool finishing late or a bug, and in both cases freeing the gate
    /// for the real owner's competitor is the one outcome that must not happen. Returns whether the
    /// gate was actually released.</summary>
    public bool Release(string owner)
    {
        lock (_sync)
        {
            if (_owner != owner) return false;
            _owner = null;
            // The deadline belongs to the claim, not to the name — leaving it behind would have the
            // next holder ask "how urgent was the last one?" and stand down for a claim that is gone.
            _ownerDeadline = null;
            return true;
        }
    }
}
