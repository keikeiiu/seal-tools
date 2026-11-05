//! The `window.find`, `window.measure` and `capture` plumbing: merged config → `seal-win` → the JSON
//! shapes `PROTOCOL.md` §3/§4 define.
//!
//! Everything Win32-shaped lives in `seal-win`; this module only translates. The frame cache is here
//! because the protocol retains the last twelve frames by `frame_id` (§8) so `calibration.save` can
//! reference the frame the calibrator just took instead of re-sending megabytes.

use std::collections::VecDeque;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use base64::Engine as _;
use seal_domain::geometry::Rect;
use seal_win::WinError;
use seal_win::window::{self, Captured, Hwnd};
use serde_json::{Value, json};

/// How long one capture waits for the game window to come to the front before it reports the
/// existing `game_not_foreground` refusal (`PROTOCOL.md` §6).
///
/// The shell's collapse on Start (or its re-show after an earlier capture) can leave the shell
/// foreground for a moment, so a single refusal is not yet evidence that the game is covered for
/// good. The wait spends that moment instead of consuming the read; a game that is already in front
/// never pays it.
const FOREGROUND_WAIT: Duration = Duration::from_millis(1_000);

/// How often the foreground wait retries while the game is not in front.
const FOREGROUND_POLL: Duration = Duration::from_millis(50);

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
/// The capture stays foreground-gated exactly as `seal_win::window::capture_client_region` defines
/// it: a covered game is never photographed. When that gate first refuses, the core allows itself at
/// most `FOREGROUND_WAIT` to get the game back in front — one `SetForegroundWindow` nudge (the
/// shell's own Start/collapse/re-show can be the foreground thief) and then the same gated capture on
/// a short poll — before it reports the existing refusal. So a momentary foreground loss does not
/// consume the player's read, while a genuinely covered game still fails with the unchanged
/// `game_not_foreground` line and the tool stops as before.
///
/// The wait itself is silent and adds no protocol message: a successful wait returns the ordinary
/// frame, and on a timeout the player gets exactly the clear, existing refusal the shell already
/// renders (its text is pinned by `protocol/fixtures/response.error.game_not_foreground.json`). A
/// live "waiting for the game window" line would need a shell/tool surface, which this change does
/// not touch.
///
/// # Errors
///
/// Returns the command error when the window cannot be found, measured or captured. A capture that
/// never sees the game in front returns [`CommandError::NotForeground`] once the deadline expires.
pub fn capture(title: &str, region: Option<Rect>) -> Result<(Value, Value), CommandError> {
    capture_with(title, region, false)
}

/// A capture with the calibration exemption: when `allow_covered` is true the foreground gate is
/// skipped. The shell hides itself first, but Windows can still report the hidden shell as the
/// foreground window; only the calibration commands pass this, and the returned frame is shown to
/// the player before anything is saved. Tool scans always use the gated [`capture`].
///
/// # Errors
///
/// Same as [`capture`], except that a not-foreground window is captured instead of refused.
pub fn capture_with(
    title: &str,
    region: Option<Rect>,
    allow_covered: bool,
) -> Result<(Value, Value), CommandError> {
    let hwnd = locate(title)?;
    let display = window::measure(hwnd).map_err(CommandError::from)?;
    let captured = if allow_covered {
        window::capture_client_region_ungated(hwnd, region).map_err(CommandError::from)?
    } else {
        let started = Instant::now();
        capture_waiting(
            FOREGROUND_WAIT,
            || window::capture_client_region(hwnd, region),
            || {
                // Best effort: the answer says only whether Windows accepted the request, and a
                // refusal is exactly what the deadline below handles, so it is deliberately
                // discarded.
                let _ = window::activate(hwnd);
            },
            || started.elapsed(),
            std::thread::sleep,
        )?
    };
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

/// The bounded foreground wait around one foreground-gated capture attempt.
///
/// `attempt` is a full `capture_client_region` call, so the pixel-integrity check runs on every try
/// and a refusal never reads pixels. On the first [`WinError::NotForeground`] the helper calls
/// `nudge` once — the core's `SetForegroundWindow` attempt — and retries immediately, which is the
/// app-own-window path where the shell holds the foreground; after that it retries after each
/// `sleep(FOREGROUND_POLL)` until `elapsed()` reaches `wait`. Any other failure is returned unchanged
/// and an exhausted deadline returns [`CommandError::NotForeground`], so the player-facing line and
/// the tool's stop behaviour are exactly the ones the protocol already defines.
///
/// `elapsed` and `sleep` are injected so the tests drive a fake clock instead of waiting; the
/// production caller passes the real clock and `thread::sleep`.
fn capture_waiting<A, N, E, S>(
    wait: Duration,
    mut attempt: A,
    mut nudge: N,
    mut elapsed: E,
    mut sleep: S,
) -> Result<Captured, CommandError>
where
    A: FnMut() -> Result<Captured, WinError>,
    N: FnMut(),
    E: FnMut() -> Duration,
    S: FnMut(Duration),
{
    let mut nudged = false;
    loop {
        match attempt() {
            Ok(captured) => return Ok(captured),
            Err(WinError::NotForeground) => {
                if !nudged {
                    nudged = true;
                    nudge();
                    continue;
                }
                if elapsed() >= wait {
                    return Err(CommandError::NotForeground);
                }
                sleep(FOREGROUND_POLL);
            }
            Err(error) => return Err(error.into()),
        }
    }
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

/// The `allow_covered` argument of `capture`: absent/false keeps the foreground gate. Only the
/// shell's calibration capture sends true, after it has hidden itself.
///
/// # Errors
///
/// Returns a bad request when the argument is present but not a boolean.
pub fn parse_allow_covered(args: &Value) -> Result<bool, CommandError> {
    match args.get("allow_covered") {
        None => Ok(false),
        Some(Value::Bool(allow)) => Ok(*allow),
        Some(_) => Err(CommandError::BadRequest(
            "allow_covered must be a boolean".to_owned(),
        )),
    }
}

/// One cached capture: the frame metadata without the multi-megabyte base64 payload, the decoded PNG
/// bytes, and the RGBA decoding computed at most once (CORE-F11). The base64 string exists only when
/// a protocol response needs it.
#[derive(Debug)]
pub struct CachedFrame {
    meta: Value,
    png: Vec<u8>,
    rgba: OnceLock<(usize, usize, Vec<u8>)>,
}

impl CachedFrame {
    fn from_value(mut frame: Value) -> Self {
        let png = frame
            .get("data")
            .and_then(Value::as_str)
            .and_then(|data| base64::engine::general_purpose::STANDARD.decode(data).ok())
            .unwrap_or_default();
        if let Some(object) = frame.as_object_mut() {
            object.remove("data");
        }
        Self {
            meta: frame,
            png,
            rgba: OnceLock::new(),
        }
    }

    /// The protocol frame shape; `data` is re-encoded only here.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let mut value = self.meta.clone();
        if let Some(object) = value.as_object_mut() {
            object.insert(
                "data".to_owned(),
                Value::String(base64::engine::general_purpose::STANDARD.encode(&self.png)),
            );
        }
        value
    }

    /// The decoded RGBA image, decoded at most once per cached frame.
    ///
    /// # Errors
    ///
    /// Returns the PNG error sentence if the cached payload is not a decodable image.
    pub fn rgba(&self) -> Result<(usize, usize, &[u8]), String> {
        if let Some(decoded) = self.rgba.get() {
            return Ok((decoded.0, decoded.1, &decoded.2));
        }
        let (width, height, rgba) = seal_win::pure::decode_png(&self.png)
            .map_err(|error| format!("the frame PNG could not be decoded: {error}"))?;
        let width = usize::try_from(width).unwrap_or(usize::MAX);
        let height = usize::try_from(height).unwrap_or(usize::MAX);
        let _ = self.rgba.set((width, height, rgba));
        let decoded = self.rgba.get().ok_or("the frame decode vanished")?;
        Ok((decoded.0, decoded.1, &decoded.2))
    }

    /// The metadata object (`width`, `height`, `origin`, `format`, `frame_id`).
    #[must_use]
    pub fn meta(&self) -> &Value {
        &self.meta
    }

    /// The PNG bytes as captured.
    #[must_use]
    pub fn png(&self) -> &[u8] {
        &self.png
    }
}

/// The last twelve frames, newest last, keyed by an id the core invents (`PROTOCOL.md` §8). The
/// thirteenth pushes the oldest out; `calibration.save` referencing an evicted id answers `bad_request`.
#[derive(Debug, Default)]
pub struct FrameCache {
    entries: VecDeque<(String, CachedFrame)>,
    next: u64,
}

impl FrameCache {
    /// Twelve live frames cover a whole calibration pass (Buy/Sell bag + shop, Gem composer and the
    /// Pet panel/bag screens) with retry margin; the old four-frame cache forced a capture before
    /// every save. The cache holds PNG bytes and decodes RGBA lazily, so this stays bounded.
    pub const CAPACITY: usize = 12;

    /// Store a frame, assign its `frame_id`, and return the id. The base64 payload is decoded here
    /// once, so later scans reuse the bytes/RGBA instead of decoding per command (CORE-F11).
    pub fn insert(&mut self, mut frame: Value) -> String {
        self.next += 1;
        let id = format!("f{}", self.next);
        if let Some(object) = frame.as_object_mut() {
            object.insert("frame_id".to_owned(), Value::String(id.clone()));
        }
        self.entries
            .push_back((id.clone(), CachedFrame::from_value(frame)));
        while self.entries.len() > Self::CAPACITY {
            self.entries.pop_front();
        }
        id
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&CachedFrame> {
        self.entries
            .iter()
            .find(|(entry_id, _)| entry_id == id)
            .map(|(_, frame)| frame)
    }

    /// The protocol-shaped frame; this is the ONLY path that base64-encodes the PNG again.
    #[must_use]
    pub fn get_value(&self, id: &str) -> Option<Value> {
        self.get(id).map(CachedFrame::to_value)
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
    fn allow_covered_is_absent_false_or_refused_when_malformed() {
        assert!(!parse_allow_covered(&json!({})).expect("absent"));
        assert!(parse_allow_covered(&json!({"allow_covered": true})).expect("bool"));
        assert!(matches!(
            parse_allow_covered(&json!({"allow_covered": "yes"})),
            Err(CommandError::BadRequest(_))
        ));
    }

    #[test]
    fn the_frame_cache_keeps_capacity_and_ids_them() {
        let mut cache = FrameCache::default();
        for index in 0..14 {
            let id = cache.insert(json!({"format": "png", "n": index}));
            assert_eq!(id, format!("f{}", index + 1));
        }
        assert_eq!(cache.len(), FrameCache::CAPACITY);
        assert!(cache.get("f1").is_none(), "the oldest was evicted");
        assert!(cache.get("f2").is_none(), "the oldest was evicted");
        assert!(cache.get("f3").is_some(), "the capacity boundary is kept");
        let newest = cache.get("f14").expect("the newest is kept");
        assert_eq!(newest.to_value()["frame_id"], "f14");
    }

    #[test]
    fn a_cached_frame_decodes_its_png_once_and_re_encodes_only_for_the_protocol() {
        let bgra = [
            10_u8, 20, 30, 255, 40, 50, 60, 255, 70, 80, 90, 255, 100, 110, 120, 255,
        ];
        let png = seal_win::pure::encode_png(2, 2, &bgra).expect("a synthetic PNG encodes");
        let data = base64::engine::general_purpose::STANDARD.encode(&png);
        let mut cache = FrameCache::default();
        let id = cache.insert(json!({
            "format": "png",
            "width": 2,
            "height": 2,
            "origin": {"x": 0, "y": 0},
            "data": data,
        }));

        let frame = cache.get(&id).expect("cached");
        let (width, height, rgba) = frame.rgba().expect("the PNG decodes");
        assert_eq!((width, height), (2, 2));
        assert_eq!(&rgba[0..4], &[30, 20, 10, 255], "RGBA, not BGRA");
        let again = frame.rgba().expect("the cached decode is reused");
        assert_eq!(again.2, rgba, "the same RGBA is returned");
        assert_eq!(frame.png(), png.as_slice());
        assert_eq!(frame.to_value()["data"], data, "the protocol gets base64");
        assert!(
            frame.meta().get("data").is_none(),
            "the cache does not retain the base64 payload"
        );
    }

    // ── the bounded foreground wait ─────────────────────────────────────────────────────────
    //
    // The seam is driven with fake window state: `attempt` is the gated capture (refused while the
    // fake foreground flag is false), `nudge` is the activation call, and the clock is advanced by
    // the fake sleep instead of a real one. None of these tests touches Win32 or waits.

    use std::cell::Cell;

    fn captured_stub() -> Captured {
        Captured {
            width: 2,
            height: 2,
            origin: seal_domain::geometry::Point::new(0, 0),
            png: vec![0x89, b'P', b'N', b'G'],
        }
    }

    #[test]
    fn a_momentary_refusal_is_retried_after_the_activation_nudge() {
        // The app-own-window path: the shell is foreground, the first gated capture refuses, the
        // nudge hands the foreground back, and the very next attempt captures.
        let foreground = Cell::new(false);
        let attempts = Cell::new(0usize);
        let successful_reads = Cell::new(0usize);
        let nudges = Cell::new(0usize);
        let clock = Cell::new(Duration::ZERO);

        let result = capture_waiting(
            FOREGROUND_WAIT,
            || {
                attempts.set(attempts.get() + 1);
                if foreground.get() {
                    successful_reads.set(successful_reads.get() + 1);
                    Ok(captured_stub())
                } else {
                    Err(WinError::NotForeground)
                }
            },
            || {
                nudges.set(nudges.get() + 1);
                foreground.set(true);
            },
            || clock.get(),
            |wait| clock.set(clock.get() + wait),
        );

        assert_eq!(result, Ok(captured_stub()));
        assert_eq!(
            nudges.get(),
            1,
            "one activation attempt for the first refusal"
        );
        assert_eq!(attempts.get(), 2, "one refused attempt, then the capture");
        assert_eq!(
            successful_reads.get(),
            1,
            "the read is consumed exactly once"
        );
        assert!(
            clock.get() < FOREGROUND_WAIT,
            "the nudge path must not wait for the whole deadline"
        );
    }

    #[test]
    fn a_foreground_window_that_arrives_later_is_still_captured_within_the_deadline() {
        // The nudge is denied (another app is in front), but the game comes forward while the wait
        // polls — e.g. the player clicks back in — and the read is still accepted.
        let foreground = Cell::new(false);
        let attempts = Cell::new(0usize);
        let clock = Cell::new(Duration::ZERO);
        let deadline = Duration::from_millis(500);

        let result = capture_waiting(
            deadline,
            || {
                attempts.set(attempts.get() + 1);
                if foreground.get() {
                    Ok(captured_stub())
                } else {
                    Err(WinError::NotForeground)
                }
            },
            || {},
            || clock.get(),
            |wait| {
                clock.set(clock.get() + wait);
                if clock.get() >= Duration::from_millis(100) {
                    foreground.set(true);
                }
            },
        );

        assert_eq!(result, Ok(captured_stub()));
        assert!(
            clock.get() < deadline,
            "the capture must happen within the bounded wait"
        );
        assert!(
            attempts.get() >= 3,
            "the polls between the refusal and the foreground change are retries"
        );
    }

    #[test]
    fn a_window_that_never_comes_forward_refuses_after_the_deadline() {
        // Pixel integrity: no attempt ever succeeds while the game is covered, and the deadline
        // returns the existing refusal rather than a new error shape.
        let attempts = Cell::new(0usize);
        let successful_reads = Cell::new(0usize);
        let clock = Cell::new(Duration::ZERO);
        let deadline = Duration::from_millis(200);

        let result = capture_waiting(
            deadline,
            || {
                attempts.set(attempts.get() + 1);
                Err(WinError::NotForeground)
            },
            || {},
            || clock.get(),
            |wait| clock.set(clock.get() + wait),
        );

        assert_eq!(result, Err(CommandError::NotForeground));
        assert_eq!(successful_reads.get(), 0, "a covered game is never read");
        assert!(
            clock.get() >= deadline,
            "the wait is bounded by the deadline, not unbounded"
        );
        let polls = deadline.as_millis() as usize / FOREGROUND_POLL.as_millis() as usize;
        assert!(
            attempts.get() <= polls + 2,
            "the retries are bounded: {} attempts",
            attempts.get()
        );
    }

    #[test]
    fn a_successful_first_attempt_consumes_exactly_one_capture() {
        // No retry is invented for a read that was never refused: one attempt, one read, no nudge
        // and no sleep, so a normal capture cannot be counted twice.
        let attempts = Cell::new(0usize);
        let nudges = Cell::new(0usize);
        let sleeps = Cell::new(0usize);

        let result = capture_waiting(
            FOREGROUND_WAIT,
            || {
                attempts.set(attempts.get() + 1);
                Ok(captured_stub())
            },
            || nudges.set(nudges.get() + 1),
            || Duration::ZERO,
            |_| sleeps.set(sleeps.get() + 1),
        );

        assert_eq!(result, Ok(captured_stub()));
        assert_eq!(attempts.get(), 1);
        assert_eq!(nudges.get(), 0);
        assert_eq!(sleeps.get(), 0);
    }

    #[test]
    fn a_failure_other_than_the_foreground_gate_is_not_retried() {
        // Only `NotForeground` is the recoverable refusal; a missing window or a GDI failure keeps
        // its old immediate answer.
        let attempts = Cell::new(0usize);

        let result = capture_waiting(
            FOREGROUND_WAIT,
            || {
                attempts.set(attempts.get() + 1);
                Err(WinError::WindowNotFound)
            },
            || {},
            || Duration::ZERO,
            |_| {},
        );

        assert_eq!(
            result,
            Err(CommandError::NotFound {
                title: String::new()
            })
        );
        assert_eq!(attempts.get(), 1);
    }
}
