//! The errors the Win32 layer reports. Player-facing text is the caller's job (the core maps these
//! to protocol error codes and its own lines); this type keeps the Win32 code and a message for the
//! log, because "the capture failed" without a code is undebuggable.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WinError {
    /// The game window is not found (by the configured title), or it has no client area.
    WindowNotFound,
    /// The game window is not the foreground window — capture would photograph whatever is in front.
    NotForeground,
    /// A Win32 call failed; carries the API name and `GetLastError` code.
    Win32 { call: &'static str, code: u32 },
    /// The requested region does not fit inside the client area.
    RegionOutsideClient,
    /// A capture produced no pixels (screen locked, desktop switching, driver refusing).
    EmptyCapture,
    /// The capture format is not available on this build (non-Windows, or a future backend).
    Unsupported,
}

impl WinError {
    #[must_use]
    pub fn win32(call: &'static str, code: u32) -> Self {
        Self::Win32 { call, code }
    }
}

impl fmt::Display for WinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WindowNotFound => f.write_str("the game window was not found"),
            Self::NotForeground => {
                f.write_str("capture refused because another window is in front")
            }
            Self::Win32 { call, code } => write!(f, "{call} failed with Win32 error {code}"),
            Self::RegionOutsideClient => {
                f.write_str("the capture region does not fit inside the game client area")
            }
            Self::EmptyCapture => f.write_str("the capture produced no pixels"),
            Self::Unsupported => f.write_str("capture is not available on this build"),
        }
    }
}

impl std::error::Error for WinError {}
