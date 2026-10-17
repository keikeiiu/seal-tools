using System;
using System.Collections.Generic;
using SealTools.Core.Config;

namespace SealTools.Core;

/// <summary>Whether the display environment a machine was calibrated in still matches the one in front of
/// it, and WHICH fields moved.
///
/// Pure on purpose: it takes a stored calibration and a live measurement and compares them, so the rule
/// is testable without a window — the launcher is net8.0-windows and stays out of the test project, which
/// is the reason this is not a method on the Setup tab.
///
/// This is the answer to the three things that silently invalidate every stored coordinate at once: a
/// different monitor scale (the physical-pixel geometry stops meaning what it meant), a different client
/// size (every client-relative point lands somewhere else), and a different screen (the calibration was
/// measured on a display that is no longer there).</summary>
public static class CalibrationCheck
{
    /// <summary>The fields that differ, described for a reader — "scale 1.5 → 1.25" — or empty when
    /// nothing does. A field the stored calibration does not carry is not a difference: an install that
    /// has never been calibrated has nothing to be wrong about, and the Setup tab says so separately.</summary>
    public static IReadOnlyList<string> Differences(CalibrationInfo stored, DisplayInfo live)
    {
        var diffs = new List<string>();

        // The stored scale is rounded to 4 places on save, so compare with a tolerance rather than for
        // exact equality — a float that came back as 1.49999996 is the same scale, not a mismatch.
        if (stored.DpiScale is { } scale && Math.Abs(scale - live.Scale) > 0.001)
            diffs.Add($"scale {scale:0.###} → {live.Scale:0.###}");

        if (stored.ClientSize is { Count: 2 } client &&
            (client[0] != live.PhysicalClient.Width || client[1] != live.PhysicalClient.Height))
        {
            diffs.Add($"client {client[0]}×{client[1]} → {live.PhysicalClient.Width}×{live.PhysicalClient.Height}");
        }

        if (stored.Screen is { Count: 2 } screen &&
            (screen[0] != live.ScreenWidth || screen[1] != live.ScreenHeight))
        {
            diffs.Add($"screen {screen[0]}×{screen[1]} → {live.ScreenWidth}×{live.ScreenHeight}");
        }

        return diffs;
    }
}
