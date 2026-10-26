//! Per-run FILE logs — the G8 gap from the porting plan and one of v2's load-bearing behaviours.
//!
//! v2's launcher is a `WinExe`, so every `Console.WriteLine` in a tool goes nowhere; each tool wrote
//! its own `logs/<tool>.log`, the OCR wrote `logs/ocr_log.jsonl` / `logs/ocr_errors.jsonl` (plus
//! optional capture PNGs), and unhandled exceptions landed in `logs/error.log`. `log` EVENTS in v3
//! only reach a watching shell — a run nobody watched left nothing to read. This module is the file
//! half, opened once at boot from the config root and shared by the dispatcher (and, from increment
//! 2, the supervisor's workers).
//!
//! # The v2 rule this module obeys everywhere
//!
//! > Logging must never be the reason a command or a run fails.
//!
//! Every write is best-effort: a missing/unwritable `logs/` directory, a full disk or a locked file
//! increments [`Logs::write_failures`] and is otherwise swallowed. No caller ever sees an error from
//! this module, and the protocol has no command for it — the files are a core responsibility.
//!
//! # Shape (v2's, copied deliberately)
//!
//! - Text lines are prefixed with `yyyy-MM-dd HH:mm:ss ` (`v2/SealTools.Core/FileLogger.cs` callers,
//!   e.g. `PetTool.Log`), appended and never truncated.
//! - The OCR's JSONL timestamp is v2's `yyyyMMdd_HHmmss_fff` (`OcrEngine.ScanOnce`), and the same
//!   stamp names the capture file (`capture_<stamp>.png` in `logs/captures/`).
//! - `error.log` uses `yyyy-MM-dd HH:mm:ss: <exception>` (`App.xaml.cs`), and `holdspace.log` keeps
//!   its own diagnostic shape `HH:mm:ss.fff [svc] <line>` (`LauncherService.HoldDiag`).
//!
//! # Timestamps are LOCAL wall-clock time
//!
//! `std` exposes only UTC and this crate takes no time dependency (no Cargo edits in this task), so
//! the local clock comes from one Win32 call (`GetLocalTime`). Normally every Win32 call lives in
//! `seal-win`; that crate is out of this change's scope and the logger cannot be useful with UTC
//! timestamps next to local in-game events, so the smallest possible call is declared here instead.
//! Non-Windows builds fall back to UTC (this suite is Windows-only by design).

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use seal_ocr::modes::{Mode, ModeResult};
use seal_tools::tool::ToolId;
use serde_json::{Value, json};

/// The file logger for one boot. Cheap to clone behind an `Arc`, and never fails a caller.
pub struct Logs {
    /// `<root>/logs`; `None` disables every write (`CoreState::new` in the dispatcher tests, which
    /// deliberately has no disk).
    dir: Option<PathBuf>,
    /// One lock for all files: a line is written whole before another thread's line starts, so two
    /// workers can never interleave mid-line (`PROTOCOL` §1's one-writer rule applies to files too).
    write_lock: Mutex<()>,
    /// Swallowed write failures, for diagnostics and for the tests that prove we swallow them.
    failures: AtomicU64,
}

impl Logs {
    /// Open the logger for a config root: files land in `<root>/logs/`. Directories are created on
    /// the first write, so a read-only root is harmless (the writes are swallowed).
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            dir: Some(root.as_ref().join("logs")),
            write_lock: Mutex::new(()),
            failures: AtomicU64::new(0),
        }
    }

    /// A logger that writes nothing, for states built without a config root.
    pub fn disabled() -> Self {
        Self {
            dir: None,
            write_lock: Mutex::new(()),
            failures: AtomicU64::new(0),
        }
    }

    /// How many writes have been swallowed. Not part of the protocol; exposed so tests can prove a
    /// failed write is counted rather than propagated.
    pub fn write_failures(&self) -> u64 {
        self.failures.load(Ordering::Relaxed)
    }

    /// `<tool>.log`: one timestamped line in the tool's own words. Increment 2 wires the supervisor
    /// here; the method exists now because the file's shape is the logger's contract.
    pub fn tool_line(&self, tool: ToolId, line: &str) {
        self.append(&format!("{tool}.log"), &format!("{} {line}", line_stamp()));
    }

    /// `holdspace.log`: v2's `HoldDiag` shape, which is deliberately not the generic one (it is a
    /// diagnostic trail written from the service as well as the card).
    pub fn holdspace_line(&self, line: &str) {
        self.append("holdspace.log", &format!("{} [svc] {line}", hold_stamp()));
    }

    /// `error.log`: v2's unhandled-exception shape. Increment 2 wires internal errors and opening
    /// failures here.
    pub fn error_line(&self, line: &str) {
        self.append("error.log", &format!("{}: {line}", line_stamp()));
    }

    /// `ocr_log.jsonl`: one compact JSON object per completed scan (v2 wrote one per `ScanOnce`).
    pub fn ocr_line(&self, record: &Value) {
        self.json_line("ocr_log.jsonl", record);
    }

    /// `ocr_errors.jsonl`: a completed scan that did not produce the thing its mode asked for.
    pub fn ocr_error(&self, record: &Value) {
        self.json_line("ocr_errors.jsonl", record);
    }

    /// Writes one PNG under `logs/captures/` and returns its path relative to the config root, with
    /// forward slashes (the protocol's `capture_path` shape, e.g. `logs/captures/capture_….png`).
    /// The caller decides whether captures are enabled (the `tuner.save_captures` flag).
    pub fn capture(&self, name: &str, png: &[u8]) -> Option<String> {
        let Some(dir) = &self.dir else {
            return None;
        };
        // Only a file name, never a path: a name with separators must not escape logs/captures.
        let name = Path::new(name).file_name()?.to_str()?;
        let captures = dir.join("captures");
        if let Err(error) = fs::create_dir_all(&captures) {
            return self.swallow(&error);
        }
        if let Err(error) = fs::write(captures.join(name), png) {
            return self.swallow(&error);
        }
        Some(format!("logs/captures/{name}"))
    }

    /// The whole OCR log line for one completed scan: writes `ocr_log.jsonl`, writes
    /// `ocr_errors.jsonl` when the scan is unconfirmed, and returns the protocol result unchanged
    /// (its five fields; `capture_path` is the relative path or `null`).
    ///
    /// v2 retried an unconfirmed scan (`tuner.ocr_retries`) and forced a capture on the last attempt.
    /// v3's pipeline scans once per `ocr.scan`, so there is no retry count to reach: this records the
    /// single completed scan whose requested target is missing, and the forced-retry capture remains a
    /// recorded follow-up (reported with increment 1).
    pub fn record_scan(
        &self,
        mode: Mode,
        outcome: &ModeResult,
        frame_png: Option<&[u8]>,
        save_captures: bool,
    ) -> Value {
        let timestamp = scan_stamp();
        let capture_path = if save_captures {
            frame_png.and_then(|png| self.capture(&format!("capture_{timestamp}.png"), png))
        } else {
            None
        };
        let result = json!({
            "lines": outcome.lines,
            "attributes": outcome.attributes,
            "grade": outcome.grade,
            "remaining": outcome.remaining,
            "capture_path": capture_path.clone().map_or(Value::Null, Value::String),
        });

        let mut record = result.clone();
        if let Some(object) = record.as_object_mut() {
            // v2's record carried the stamp; the protocol result has no timestamp field, so the file
            // record is the result PLUS the stamp rather than a second shape.
            object.insert("timestamp".to_owned(), Value::String(timestamp));
        }
        self.ocr_line(&record);
        if scan_unconfirmed(mode, outcome) {
            self.ocr_error(&record);
        }
        result
    }

    /// Every line is opened, appended and closed immediately (v2's `FileLogger` used `AutoFlush`),
    /// so there is nothing buffered to lose. Kept because `PROTOCOL.md` §1 says shutdown flushes the
    /// logs, and a future buffered writer must keep that contract.
    pub fn flush(&self) {}

    fn json_line(&self, file: &str, record: &Value) {
        match serde_json::to_string(record) {
            Ok(line) => self.append(file, &line),
            Err(error) => {
                self.failures.fetch_add(1, Ordering::Relaxed);
                eprintln!("seal-core: log record could not be serialised: {error}");
            }
        }
    }

    fn append(&self, file: &str, line: &str) {
        let Some(dir) = &self.dir else {
            return;
        };
        let _guard = self
            .write_lock
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Err(error) = fs::create_dir_all(dir) {
            self.report(&error);
            return;
        }
        match OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join(file))
        {
            Ok(mut handle) => {
                if let Err(error) = handle
                    .write_all(line.as_bytes())
                    .and_then(|()| handle.write_all(b"\n"))
                {
                    self.report(&error);
                }
            }
            Err(error) => self.report(&error),
        }
    }

    /// A failed write is counted and reported on stderr (the shell already captures stderr), never
    /// returned: the caller's command does not fail because its log line did not land.
    fn report(&self, error: &std::io::Error) {
        self.failures.fetch_add(1, Ordering::Relaxed);
        eprintln!("seal-core: log write failed (swallowed): {error}");
    }

    fn swallow<T>(&self, error: &std::io::Error) -> Option<T> {
        self.report(error);
        None
    }
}

/// Whether a completed scan produced what its mode asked for. v2's `IsConfirmed` was written for its
/// single full scan; v3 splits the read into modes, so the equivalent test is per mode's own target.
fn scan_unconfirmed(mode: Mode, outcome: &ModeResult) -> bool {
    match mode {
        Mode::Lines => outcome.lines.is_empty(),
        Mode::Count => outcome.remaining.is_none(),
        Mode::Attributes => outcome
            .attributes
            .as_ref()
            .is_none_or(|rows| rows.len() != 3 || rows.iter().any(|row| row.is_empty())),
        Mode::Grade => outcome.grade.is_none(),
    }
}

/// v2's OCR stamp (`OcrEngine.ScanOnce`: `yyyyMMdd_HHmmss_fff`), also used in capture file names.
pub fn scan_stamp() -> String {
    let [year, month, day, hour, minute, second, millis] = now_parts();
    format!("{year:04}{month:02}{day:02}_{hour:02}{minute:02}{second:02}_{millis:03}")
}

fn line_stamp() -> String {
    let [year, month, day, hour, minute, second, _] = now_parts();
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
}

fn hold_stamp() -> String {
    let [_, _, _, hour, minute, second, millis] = now_parts();
    format!("{hour:02}:{minute:02}:{second:02}.{millis:03}")
}

/// `[year, month, day, hour, minute, second, milliseconds]`, local time.
#[cfg(windows)]
fn now_parts() -> [u16; 7] {
    #[repr(C)]
    struct WinSystemTime {
        year: u16,
        month: u16,
        day_of_week: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        milliseconds: u16,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        #[link_name = "GetLocalTime"]
        fn get_local_time(system_time: *mut WinSystemTime);
    }

    let mut time = WinSystemTime {
        year: 0,
        month: 0,
        day_of_week: 0,
        day: 0,
        hour: 0,
        minute: 0,
        second: 0,
        milliseconds: 0,
    };
    // SAFETY: `GetLocalTime` writes one `SYSTEMTIME` (eight `u16`s, `repr(C)`) into the pointer and
    // reads nothing else; `time` is a live, correctly shaped stack value for the call.
    unsafe { get_local_time(&mut time) };
    [
        time.year,
        time.month,
        time.day,
        time.hour,
        time.minute,
        time.second,
        time.milliseconds,
    ]
}

/// UTC fallback for non-Windows builds (this project is Windows-only; the fallback keeps the crate
/// buildable elsewhere without a time dependency).
#[cfg(not(windows))]
fn now_parts() -> [u16; 7] {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let seconds_of_day = u32::try_from(secs % 86_400).unwrap_or(0);
    let (year, month, day) = civil_from_days(days);
    [
        u16::try_from(year).unwrap_or(1970),
        u16::try_from(month).unwrap_or(1),
        u16::try_from(day).unwrap_or(1),
        u16::try_from(seconds_of_day / 3600).unwrap_or(0),
        u16::try_from((seconds_of_day % 3600) / 60).unwrap_or(0),
        u16::try_from(seconds_of_day % 60).unwrap_or(0),
        u16::try_from(now.subsec_millis()).unwrap_or(0),
    ]
}

/// Howard Hinnant's days-to-civil conversion, for the non-Windows fallback only.
#[cfg(not(windows))]
fn civil_from_days(days: i64) -> (i32, i32, i32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = year + i64::from(month <= 2);
    (
        i32::try_from(year).unwrap_or(1970),
        i32::try_from(month).unwrap_or(1),
        i32::try_from(day).unwrap_or(1),
    )
}
