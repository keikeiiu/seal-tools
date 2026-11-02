//! `CalibrationCheck`: whether the display environment a calibration was measured in still matches the
//! one in front of it, and WHICH fields moved.
//!
//! Pure on purpose — it takes a stored calibration and a live measurement and compares them, so the
//! rule is testable without a window. That is v2's own reasoning for the split (`CalibrationCheck.cs:7-17`):
//! the launcher is `net8.0-windows` and stays out of the test project.
//!
//! This is the answer to the three things that silently invalidate every stored coordinate at once: a
//! different monitor scale (the physical-pixel geometry stops meaning what it meant), a different
//! client size (every client-relative point lands somewhere else), and a different screen (the
//! calibration was measured on a display that is no longer there).
//!
//! # What is NOT here
//!
//! The LIVE side: measuring [`DisplayInfo`] from Win32 (v2's `Dpi.Measure`/`WindowFinder`), which is
//! `seal-win`'s job. The stored side is the config model ([`CalibrationInfo`]). The "nothing stored
//! yet" state is presented separately by the Setup surface and is deliberately NOT a difference: an
//! install that has never been calibrated has nothing to be wrong about.
//!
//! Oracle: `v2/SealTools.Tests/CalibrationCheckTests.cs`, all seven cases translated in
//! `tests/calibration.rs`; the extra rules v2 relies on without testing (a stored list that is not a
//! pair is ignored, the comparison direction, and the tolerance boundary) are pinned there too and
//! marked as such.

use crate::config::CalibrationInfo;
use crate::geometry::DisplayInfo;

/// How far the stored scale may differ and still be the same scale.
///
/// MEASURED reasoning in v2 (`CalibrationCheck.cs:27-29`): the stored scale is rounded to four places
/// on save, so compare with a tolerance rather than for exact equality — a float that came back as
/// 1.49999996 is the same scale, not a mismatch. The comparison is `>`, so a difference of exactly
/// 0.001 is NOT a mismatch.
const SCALE_TOLERANCE: f64 = 0.001;

/// The fields that differ, described for a reader — "scale 1.5 → 1.25" — or empty when nothing does.
///
/// A field the stored calibration does not carry is not a difference (`CalibrationCheck.cs:22`): an
/// install that has never been calibrated has nothing to be wrong about. The order is v2's: scale,
/// client size, screen.
#[must_use]
pub fn differences(stored: &CalibrationInfo, live: &DisplayInfo) -> Vec<String> {
    let mut diffs = Vec::new();

    if let Some(scale) = stored.dpi_scale
        && (scale - live.scale).abs() > SCALE_TOLERANCE
    {
        diffs.push(format!(
            "scale {} → {}",
            describe_scale(scale),
            describe_scale(live.scale)
        ));
    }

    // `stored.ClientSize is { Count: 2 }` — a stored list that is not a pair carries no claim about the
    // client size, so it cannot disagree with anything.
    if let Some(client) = &stored.client_size
        && client.len() == 2
        && (client[0] != live.physical_client.w || client[1] != live.physical_client.h)
    {
        diffs.push(format!(
            "client {}×{} → {}×{}",
            client[0], client[1], live.physical_client.w, live.physical_client.h
        ));
    }

    if let Some(screen) = &stored.screen
        && screen.len() == 2
        && (screen[0] != live.screen.w || screen[1] != live.screen.h)
    {
        diffs.push(format!(
            "screen {}×{} → {}×{}",
            screen[0], screen[1], live.screen.w, live.screen.h
        ));
    }

    diffs
}

/// C#'s `{scale:0.###}`: at most three decimals, trailing zeros trimmed. v2's messages are part of what
/// the player reads, so the shape is reproduced rather than a `Debug` print.
fn describe_scale(scale: f64) -> String {
    let fixed = format!("{scale:.3}");
    let trimmed = fixed.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() || trimmed == "-" {
        "0".to_owned()
    } else {
        trimmed.to_owned()
    }
}
