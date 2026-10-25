//! Which serial port to open — and, when one is NAMED in the config, refusing to quietly open a
//! different one.
//!
//! Ported from `v2/SealTools.Core/PortChoice.cs` with `PortChoiceTests.cs` as the oracle. `arduino.port`
//! is documented as an optional override; v2 shipped a build where nothing read it, so a player who
//! named a port got it silently ignored (`PortChoice.cs:10-16`). The direction that matters is the
//! refusal: a named port that is not present is an ERROR, not a fall-through to discovery, because
//! opening a different board than the one named is the failure this exists to prevent.
//!
//! The OS enumeration is the ADAPTER's: this function takes the list it is handed. Enumerating over
//! WMI (`Win32_SerialPort` VID/PID plus the "name mentions Arduino" fallback, `Arduino.cs:25-76`) and
//! proving it needs a machine with and without the board.

/// The port to open and the reason there is none — never both.
///
/// `(None, None)` means "discovery found nothing": the caller writes that message itself, because
/// only it knows the configured VID and PIDs to name.
///
/// `configured` is the merged `arduino.port`; empty/whitespace means no override. `available` is what
/// the adapter enumerated. `discover` is the adapter's VID/PID lookup, called only when no port is
/// named.
pub fn choose(
    configured: Option<&str>,
    available: &[&str],
    discover: impl FnOnce() -> Option<String>,
) -> (Option<String>, Option<String>) {
    if let Some(configured) = configured.map(str::trim).filter(|name| !name.is_empty()) {
        // Case-insensitively: Windows reports COM3, and a hand-written config says com3 as often as
        // not. Port names carry no case significance on any platform this runs on.
        let present = available
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(configured));
        if present {
            return (Some(configured.to_owned()), None);
        }
        return (
            None,
            Some(format!(
                "arduino.port names {configured}, which this machine does not have — so nothing was \
                 opened, rather than opening a different board. Fix it in local.yaml, or clear it to \
                 go back to finding the board by its IDs."
            )),
        );
    }

    (discover(), None)
}
