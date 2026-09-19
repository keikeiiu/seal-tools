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

    /// <summary>The tool id that holds the gate, or null when it is free. Readable while held —
    /// callers show it, so "who is the pet feeder waiting for?" has an answer.</summary>
    public string? Owner
    {
        get { lock (_sync) return _owner; }
    }

    /// <summary>Claims the gate for <paramref name="owner"/>. False when anyone else holds it,
    /// including the same owner asking twice: a claim that is refused leaves nothing to undo, and
    /// reference-counting claims is how a tool ends up holding the gate while it is free.</summary>
    public bool TryAcquire(string owner)
    {
        if (string.IsNullOrEmpty(owner)) throw new ArgumentException("owner required", nameof(owner));

        lock (_sync)
        {
            if (_owner != null) return false;
            _owner = owner;
            return true;
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
            return true;
        }
    }
}
