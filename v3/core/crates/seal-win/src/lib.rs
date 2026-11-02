//! The Win32 side of the core: finding the game window, measuring it in both DPI spaces, and
//! capturing its client area as a PNG.
//!
//! The core process stays **DPI-unaware** on purpose (`CALIBRATION.md` §2): the screen blit is 1:1
//! physical while Win32 reports logical values to an unaware process, and that asymmetry is what
//! makes the coordinate contract work. So the two physical reads here run under a THREAD-scoped
//! `DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2` context and are restored before returning — the
//! process awareness never changes ([`window::measure`]).
//!
//! What lives here is the part that talks to Windows; the arithmetic it produces is the domain's
//! ([`seal_domain::geometry::DisplayInfo::from_rects`]), and the PNG wrapper is pure
//! ([`pure::encode_png`]) so both are testable without a window. Live behaviour (a real game window,
//! a real blit) is `#[ignore]` tests with what would prove them; the pure halves run everywhere.

pub mod clock;
pub mod error;
pub mod pure;

#[cfg(windows)]
pub mod window;

pub use error::WinError;

#[cfg(windows)]
pub use window::{Captured, Hwnd};

/// The platform's capture layer is missing: non-Windows builds get a named error instead of a panic
/// (the crate is Windows-only by design; see the root `AGENTS.md`).
#[cfg(not(windows))]
pub fn not_windows() -> WinError {
    WinError::Unsupported
}
