using System;
using System.Collections.Generic;
using System.Linq;

namespace SealTools.Core;

/// <summary>Which serial port to open — and, when one is NAMED in the config, refusing to quietly open a
/// different one.
///
/// `arduino.port` is documented in `local.yaml.example` as an *"optional override; empty =
/// auto-detect"*, and it is merged into the config on every load. Until now it was read by **nothing**:
/// the port was always discovered by VID/PID, so a player who named a port got it silently ignored. The
/// same class of defect as the four `PetConfig` properties no config file could carry — a setting that
/// looks honoured and does nothing.
///
/// The I/O is passed IN rather than done here, because the decision is the part that has to be right and
/// the part that can be checked without a board: a machine with no Arduino, a machine with the wrong
/// one, and a config naming a port that does not exist are all just arguments.</summary>
public static class PortChoice
{
    /// <summary>The port to open and the reason there is none — never both.
    ///
    /// A named port that is NOT present is an ERROR rather than a fall-through to discovery. Opening a
    /// different board than the one named is the failure this method exists to prevent: a player names a
    /// port precisely because they do not want the other one, and a fall-through would defeat that
    /// silently, in the direction of driving the wrong board.
    ///
    /// No port and NO error means "discovery found nothing" — the caller writes that message itself,
    /// because only it knows the configured VID and PIDs to name.</summary>
    public static (string? Port, string? Error) Choose(
        string? configured, IReadOnlyCollection<string> available, Func<string?> discover)
    {
        if (!string.IsNullOrWhiteSpace(configured))
        {
            var named = configured.Trim();
            // Case-insensitively: Windows reports COM3, and a hand-written config says com3 as often as
            // not. Port names carry no case significance on any platform this runs on.
            return available.Contains(named, StringComparer.OrdinalIgnoreCase)
                ? (named, null)
                : (null, $"arduino.port names {named}, which this machine does not have — so nothing was " +
                         "opened, rather than opening a different board. Fix it in local.yaml, or clear " +
                         "it to go back to finding the board by its IDs.");
        }

        return discover() is { } found ? (found, null) : (null, null);
    }
}
