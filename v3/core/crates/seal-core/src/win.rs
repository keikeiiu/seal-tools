//! The `window.find`, `window.measure` and `capture` plumbing: merged config → `seal-win` → the JSON
//! shapes `PROTOCOL.md` §3/§4 define.
//!
//! Everything Win32-shaped lives in `seal-win`; this module only translates. The frame cache is here
//! because the protocol retains the last four frames by `frame_id` (§8) so `calibration.save` can
//! reference the frame the calibrator just took instead of re-sending megabytes.

use base64::Engine as _;
use seal_domain::geometry::Rect;
use seal_win::WinError;
use seal_win::window::{self, Hwnd};
use serde_json::{Value, json};

/// Why a window/capture command could not produce its result, in the terms the dispatcher maps to
/// protocol error codes. The player-facing sentence is the dispatcher's (`PROTOCOL.md` §6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandError {
    /// The merged config has no usable `window.title`.
    MissingTitle,
    /// No top-level window's title contains the configured title.
    NotFound { title: String },
    /// The game is not the foreground window; a capture would photograph whatever covers it.
    NotForeground,
    /// The requested capture region does not fit inside the client area.
    RegionOutside,
    /// A malformed argument (not a config or Win32 failure).
    BadRequest(String),
    /// A Win32 call failed in a way with no more specific answer.
    Internal(String),
}

impl CommandError {
    /// The player-facing sentence for this failure.
    ///
    /// The protocol mapping (`dispatch::win_error_response`) uses this for its message and a tool's
    /// read loop uses it directly, so the wording lives in one place instead of growing a second
    /// copy in the tools.
    #[must_use]
    pub fn line(&self) -> String {
        match self {
            Self::MissingTitle => "window.title is not configured".to_owned(),
            Self::NotFound { .. } => "the game window was not found".to_owned(),
            Self::NotForeground => "capture refused because another window is in front".to_owned(),
            Self::RegionOutside => {
                "the capture region does not fit inside the game client area".to_owned()
            }
            Self::BadRequest(message) | Self::Internal(message) => message.clone(),
        }
    }
}

impl From<WinError> for CommandError {
    fn from(error: WinError) -> Self {
        match error {
            WinError::WindowNotFound => Self::NotFound {
                title: String::new(),
            },
            WinError::NotForeground => Self::NotForeground,
            WinError::RegionOutsideClient => Self::RegionOutside,
            WinError::EmptyCapture => Self::Internal("the capture produced no pixels".to_owned()),
            WinError::Unsupported => {
                Self::Internal("capture is not available on this build".to_owned())
            }
            WinError::Win32 { call, code } => {
                Self::Internal(format!("{call} failed with Win32 error {code}"))
            }
        }
    }
}

/// The configured window title, or a typed refusal. An empty title is a config error rather than
/// "match every window" (`seal-win`'s `title_matches` makes the same choice).
///
/// # Errors
///
/// Returns the command error when `window.title` is missing or empty.
pub fn configured_title(config: &Value) -> Result<String, CommandError> {
    config
        .get("window")
        .and_then(|window| window.get("title"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(str::to_owned)
        .ok_or(CommandError::MissingTitle)
}

/// `window.find`'s result. "Not found" is a RESULT here, not an error: the calibrator shows
/// found/not-found long before it measures (`PROTOCOL.md` §4).
///
/// # Errors
///
/// Returns the command error when the title is empty; a missing window is an ordinary result.
pub fn find(title: &str) -> Result<Value, CommandError> {
    match window::find_window(title) {
        Some(hwnd) => Ok(json!({
            "found": true,
            "title": window::title_text(hwnd).unwrap_or_else(|| title.to_owned()),
        })),
        None => Ok(json!({"found": false})),
    }
}

/// `window.measure`'s result: the `DisplayInfo` object, serialised from the domain type so the wire
/// shape has one source (`PROTOCOL.md` §3).
///
/// # Errors
///
/// Returns the command error when the window cannot be found or measured.
pub fn measure(title: &str) -> Result<Value, CommandError> {
    let hwnd = locate(title)?;
    let display = window::measure(hwnd).map_err(CommandError::from)?;
    Ok(json!({ "display": serialised(&display)? }))
}

/// `capture`'s result: the frame (without its id — the cache assigns it) and the fresh `DisplayInfo`.
///
/// # Errors
///
/// Returns the command error when the window cannot be found, measured or captured.
pub fn capture(title: &str, region: Option<Rect>) -> Result<(Value, Value), CommandError> {
    let hwnd = locate(title)?;
    let display = window::measure(hwnd).map_err(CommandError::from)?;
    let captured = window::capture_client_region(hwnd, region).map_err(CommandError::from)?;
    let data = base64::engine::general_purpose::STANDARD.encode(&captured.png);
    let frame = json!({
        "format": "png",
        "width": captured.width,
        "height": captured.height,
        "origin": {"x": captured.origin.x, "y": captured.origin.y},
        "data": data,
    });
    Ok((frame, serialised(&display)?))
}

fn locate(title: &str) -> Result<Hwnd, CommandError> {
    window::find_window(title).ok_or_else(|| CommandError::NotFound {
        title: title.to_owned(),
    })
}

fn serialised<T: serde::Serialize>(value: &T) -> Result<Value, CommandError> {
    serde_json::to_value(value).map_err(|error| {
        CommandError::Internal(format!("the display info could not be serialised: {error}"))
    })
}

/// The `region` argument of `capture`: absent → the whole client, otherwise `{x,y,w,h}` integers. A
/// malformed region is a caller bug and is refused before any Win32 call.
///
/// # Errors
///
/// Returns the command error when `region` is not a `{x,y,w,h}` object of integers.
pub fn parse_region(args: &Value) -> Result<Option<Rect>, CommandError> {
    let Some(region) = args.get("region") else {
        return Ok(None);
    };
    if region.is_null() {
        return Ok(None);
    }
    let number = |name: &str| -> Result<i32, CommandError> {
        region
            .get(name)
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok())
            .ok_or_else(|| {
                CommandError::BadRequest(format!("capture region needs an integer {name:?}"))
            })
    };
    let rect = Rect::new(number("x")?, number("y")?, number("w")?, number("h")?);
    Ok(Some(rect))
}

/// The last four frames, newest last, keyed by an id the core invents (`PROTOCOL.md` §8). A fifth
/// frame pushes the oldest out; `calibration.save` referencing an evicted id answers `bad_request`.
#[derive(Debug, Default)]
pub struct FrameCache {
    entries: Vec<(String, Value)>,
    next: u64,
}

impl FrameCache {
    pub const CAPACITY: usize = 4;

    /// Store a frame, assign its `frame_id`, and return the id.
    pub fn insert(&mut self, mut frame: Value) -> String {
        self.next += 1;
        let id = format!("f{}", self.next);
        if let Some(object) = frame.as_object_mut() {
            object.insert("frame_id".to_owned(), Value::String(id.clone()));
        }
        self.entries.push((id.clone(), frame));
        while self.entries.len() > Self::CAPACITY {
            self.entries.remove(0);
        }
        id
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Value> {
        self.entries
            .iter()
            .find(|(entry_id, _)| entry_id == id)
            .map(|(_, frame)| frame)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_or_blank_title_is_a_typed_refusal() {
        assert_eq!(
            configured_title(&json!({})),
            Err(CommandError::MissingTitle)
        );
        assert_eq!(
            configured_title(&json!({"window": {"title": "   "}})),
            Err(CommandError::MissingTitle)
        );
        assert_eq!(
            configured_title(&json!({"window": {"title": "Seal Online"}})).expect("a title"),
            "Seal Online"
        );
    }

    #[test]
    fn regions_parse_and_malformed_regions_are_refused() {
        assert_eq!(parse_region(&json!({})).expect("absent"), None);
        assert_eq!(
            parse_region(&json!({"region": {"x": 1, "y": 2, "w": 3, "h": 4}})).expect("fits"),
            Some(Rect::new(1, 2, 3, 4))
        );
        assert!(matches!(
            parse_region(&json!({"region": {"x": 1}})),
            Err(CommandError::BadRequest(_))
        ));
    }

    #[test]
    fn the_frame_cache_keeps_four_and_ids_them() {
        let mut cache = FrameCache::default();
        for index in 0..6 {
            let id = cache.insert(json!({"format": "png", "n": index}));
            assert_eq!(id, format!("f{}", index + 1));
        }
        assert_eq!(cache.len(), FrameCache::CAPACITY);
        assert!(cache.get("f1").is_none(), "the oldest was evicted");
        assert!(cache.get("f2").is_none(), "the oldest was evicted");
        let newest = cache.get("f6").expect("the newest is kept");
        assert_eq!(newest["frame_id"], "f6");
    }
}
