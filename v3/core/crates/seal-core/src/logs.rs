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
//!   e.g. `PetTool.Log`). Unlike v2 they are rotated by size instead of appended forever (CORE-F12):
//!   a file that reaches [`MAX_FILE_BYTES`] is renamed to `.1` and the older copies shift up to
//!   `.3`, so one long feeder run cannot fill the disk.
//! - The OCR's JSONL timestamp is v2's `yyyyMMdd_HHmmss_fff` (`OcrEngine.ScanOnce`), and the same
//!   stamp names the capture file (`capture_<stamp>.png` in `logs/captures/`).
//! - `error.log` uses `yyyy-MM-dd HH:mm:ss: <exception>` (`App.xaml.cs`), and `holdspace.log` keeps
//!   its own diagnostic shape `HH:mm:ss.fff [svc] <line>` (`LauncherService.HoldDiag`).
//!
//! # Timestamps are LOCAL wall-clock time
//!
//! `std` exposes only UTC, so the local clock is `seal_win::clock::local_now()` — a Win32
//! `GetLocalTime` in the crate where every other Win32 call lives. That keeps this module free of
//! platform calls and the log lines comparable with local in-game events. Non-Windows builds fall
//! back to UTC (this suite is Windows-only by design).

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use seal_domain::decision::{StopKind, StopReason};
use seal_ocr::modes::{Mode, ModeResult};
use seal_tools::tool::ToolId;
use serde_json::{Value, json};

/// The size one log file may reach before it is rotated (CORE-F12). v2 appended forever: a long
/// tuner or pet session left hundreds of megabytes behind, and every status tick paid a reopen.
pub const MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;

/// The rotated copies kept beside the live file (`x.log.1` … `x.log.3`), so one file is bounded at
/// `MAX_FILE_BYTES × (KEEP_ROTATED + 1)`.
pub const KEEP_ROTATED: usize = 3;

/// The file logger for one boot. Cheap to clone behind an `Arc`, and never fails a caller.
pub struct Logs {
    /// `<root>/logs`; `None` disables every write (`CoreState::new` in the dispatcher tests, which
    /// deliberately has no disk).
    dir: Option<PathBuf>,
    /// One open, line-flushed writer per file. The previous shape reopened the file for every line
    /// under the one global lock, so each status tick paid `create_dir_all` + open + write + close
    /// and serialised every other worker behind it (CORE-F12). The map lock still serialises a whole
    /// line at a time (two workers can never interleave mid-line), but the handle stays open.
    writers: Mutex<HashMap<String, OpenLog>>,
    /// When a file reaches this many bytes it is rotated (the tests use a tiny cap).
    max_bytes: u64,
    /// How many rotated copies to keep.
    keep_rotated: usize,
    /// Swallowed write failures, for diagnostics and for the tests that prove we swallow them.
    failures: AtomicU64,
}

/// One file's open writer. The byte count is this module's own, so rotation never has to stat.
struct OpenLog {
    writer: BufWriter<File>,
    bytes: u64,
}

impl Logs {
    /// Open the logger for a config root: files land in `<root>/logs/`. Directories are created on
    /// the first write, so a read-only root is harmless (the writes are swallowed).
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self::with_rotation(root, MAX_FILE_BYTES, KEEP_ROTATED)
    }

    /// [`Self::new`] with explicit rotation limits. The tests use a small cap so a handful of lines
    /// proves the rotation; a future config field can expose the same knobs without a second
    /// implementation (CORE-F12).
    #[must_use]
    pub fn with_rotation(root: impl AsRef<Path>, max_bytes: u64, keep_rotated: usize) -> Self {
        Self {
            dir: Some(root.as_ref().join("logs")),
            writers: Mutex::new(HashMap::new()),
            // A zero cap would rotate before the first line and a zero keep would delete the live
            // file; both are nonsense for a log, so they are floored at one rather than refused.
            max_bytes: max_bytes.max(1),
            keep_rotated: keep_rotated.max(1),
            failures: AtomicU64::new(0),
        }
    }

    /// A logger that writes nothing, for states built without a config root.
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            dir: None,
            writers: Mutex::new(HashMap::new()),
            max_bytes: MAX_FILE_BYTES,
            keep_rotated: KEEP_ROTATED,
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

    /// The line a worker writes when a run actually begins. v2's tools each phrased it in their own
    /// words (`run started — board firmware …`, `Quest Hand-in — flow …`); the supervisor adapter has
    /// only the generic moment, so it writes the generic phrase and the tool's own status lines follow.
    pub fn tool_started(&self, tool: ToolId) {
        self.tool_line(tool, "run started");
    }

    /// The final line, distinguishable by the stop's own reason word: `finished` (done), `stopped`
    /// (the player or the shell asked it to stop) or `failed` (an error, including a tripped guard).
    /// v2 logged that difference in its tools' own logs (`QuestTool.cs`: `DONE — …` vs `cancelled …`);
    /// the reason word carries it here.
    pub fn tool_finished(&self, tool: ToolId, reason: &StopReason) {
        let word = match reason.kind {
            StopKind::Done => "finished",
            StopKind::Cancelled => "stopped",
            StopKind::Error | StopKind::Guard => "failed",
        };
        self.tool_line(tool, &format!("{word} — {}", reason.message));
    }

    /// `holdspace.log`: v2's `HoldDiag` shape, which is deliberately not the generic one (it is a
    /// diagnostic trail written from the service as well as the card).
    /// `arduino_debug.txt`: v2's route/cycle diagnostic. Appended, never truncated, with the local
    /// `HH:mm:ss` prefix v2 used, so a route that did not land can be inspected afterwards.
    pub fn arduino_debug_line(&self, line: &str) {
        self.append("arduino_debug.txt", &format!("{} {line}", line_stamp()));
    }

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

    /// Delete the Tuner capture PNGs directly under `logs/captures` and return
    /// `(deleted, failures)`. Housekeeping never fails a caller: a missing directory is `(0, 0)`,
    /// a directory-read failure is logged and `(0, failures)`; each per-file delete failure is
    /// logged and counted. Only regular files whose extension is `png` are touched — no recursion,
    /// no non-PNG files, no subdirectories, and symlinks/reparse points are skipped by
    /// `DirEntry::file_type` (which does not follow them).
    pub fn cleanup_captures(&self) -> (usize, usize) {
        let Some(dir) = &self.dir else {
            return (0, 0);
        };
        let captures = dir.join("captures");
        let entries = match fs::read_dir(&captures) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return (0, 0),
            Err(error) => {
                self.error_line(&format!("could not read {}: {error}", captures.display()));
                return (0, 1);
            }
        };
        let mut deleted = 0;
        let mut failures = 0;
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    failures += 1;
                    self.error_line(&format!(
                        "could not read an entry under {}: {error}",
                        captures.display()
                    ));
                    continue;
                }
            };
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let is_png = path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("png"));
            if !is_png {
                continue;
            }
            match entry.file_type() {
                Ok(file_type) if file_type.is_file() => {}
                // A symlink/reparse point, a directory named x.png, or an unreadable type: a
                // cleanup is not a recursive delete primitive, so it is reported and skipped.
                Ok(_) => continue,
                Err(error) => {
                    failures += 1;
                    self.error_line(&format!("could not inspect {}: {error}", path.display()));
                    continue;
                }
            }
            match fs::remove_file(&path) {
                Ok(()) => deleted += 1,
                Err(error) => {
                    failures += 1;
                    self.error_line(&format!("could not delete capture {name}: {error}"));
                }
            }
        }
        (deleted, failures)
    }

    /// Writes one scan-evidence file under `logs/reads/` and returns its root-relative path with
    /// forward slashes. Evidence is best-effort: a missing/unwritable directory is swallowed exactly
    /// like every other log write, because a scan must never fail over its evidence.
    pub fn write_read(&self, name: &str, bytes: &[u8]) -> Option<String> {
        let Some(dir) = &self.dir else {
            return None;
        };
        let name = Path::new(name).file_name()?.to_str()?;
        let reads = dir.join("reads");
        if let Err(error) = fs::create_dir_all(&reads) {
            return self.swallow(&error);
        }
        if let Err(error) = fs::write(reads.join(name), bytes) {
            return self.swallow(&error);
        }
        Some(format!("logs/reads/{name}"))
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

    /// Flushes every open writer. Each line is flushed as it is written (v2's `AutoFlush`), so this
    /// is the `PROTOCOL.md` §1 shutdown hook rather than a data-loss window — the map lock makes the
    /// flush see exactly the lines already appended.
    pub fn flush(&self) {
        let mut writers = self
            .writers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut failures = 0u64;
        for open in writers.values_mut() {
            if open.writer.flush().is_err() {
                failures += 1;
            }
        }
        drop(writers);
        if failures > 0 {
            self.failures.fetch_add(failures, Ordering::Relaxed);
            eprintln!("seal-core: {failures} log file(s) could not be flushed");
        }
    }

    fn json_line(&self, file: &str, record: &Value) {
        match serde_json::to_string(record) {
            Ok(line) => self.append(file, &line),
            Err(error) => {
                self.failures.fetch_add(1, Ordering::Relaxed);
                eprintln!("seal-core: log record could not be serialised: {error}");
            }
        }
    }

    /// Appends one whole line, opening (and, when the file has grown past the cap, rotating) it on
    /// the way. The writers map is held for the whole append so two workers can never interleave
    /// mid-line; the handle stays open between lines (CORE-F12).
    fn append(&self, file: &str, line: &str) {
        let Some(dir) = self.dir.as_deref() else {
            return;
        };
        let mut writers = self
            .writers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Take the entry out: a failed write leaves it absent so the next line tries a fresh open,
        // and a rotation needs to replace the handle anyway.
        let mut open = match writers.remove(file) {
            Some(open) => open,
            None => match open_log(dir, file) {
                Ok(open) => open,
                Err(error) => {
                    drop(writers);
                    self.report(&error);
                    return;
                }
            },
        };
        if open.bytes >= self.max_bytes {
            // The handle is CLOSED before the rename: Windows refuses to rename a file that is open
            // without FILE_SHARE_DELETE, so rotating with our own writer still alive would fail
            // every time. The writers map was the only owner, so dropping it here frees the name.
            drop(open);
            match rotate(dir, file, self.keep_rotated) {
                Ok(()) => match open_log(dir, file) {
                    Ok(fresh) => open = fresh,
                    Err(error) => {
                        drop(writers);
                        self.report(&error);
                        return;
                    }
                },
                Err(error) => {
                    // A failed rotation is housekeeping, not the line's problem: reopen the live
                    // file, report it, and keep appending — losing a log line is worse than a full
                    // file.
                    match open_log(dir, file) {
                        Ok(fresh) => {
                            open = fresh;
                            self.report(&error);
                        }
                        Err(open_error) => {
                            drop(writers);
                            self.report(&open_error);
                            return;
                        }
                    }
                }
            }
        }
        match open.write_line(line) {
            Ok(()) => {
                writers.insert(file.to_owned(), open);
            }
            Err(error) => {
                drop(open);
                drop(writers);
                self.report(&error);
            }
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

impl OpenLog {
    /// Writes one terminated line and flushes it, so `PROTOCOL.md` §1's "shutdown flushes the logs"
    /// describes a file that is already durable line by line.
    fn write_line(&mut self, line: &str) -> std::io::Result<()> {
        self.writer.write_all(line.as_bytes())?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()?;
        self.bytes += line.len() as u64 + 1;
        Ok(())
    }
}

/// Opens (creating the directory and the file) one append-mode writer and records its current size,
/// so rotation is a comparison rather than a `stat` per line.
fn open_log(dir: &Path, file: &str) -> std::io::Result<OpenLog> {
    fs::create_dir_all(dir)?;
    let handle = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(file))?;
    let bytes = handle.metadata().map_or(0, |metadata| metadata.len());
    Ok(OpenLog {
        writer: BufWriter::new(handle),
        bytes,
    })
}

/// `x.log` → `x.log.1`, shifting the existing copies up; the oldest is deleted. The caller holds the
/// writers map, so no write can land between the rename and the reopen.
fn rotate(dir: &Path, file: &str, keep: usize) -> std::io::Result<()> {
    let keep = keep.max(1);
    let oldest = rotated_path(dir, file, keep);
    match fs::remove_file(&oldest) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    for index in (1..keep).rev() {
        let from = rotated_path(dir, file, index);
        let to = rotated_path(dir, file, index + 1);
        match fs::rename(&from, &to) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    fs::rename(dir.join(file), rotated_path(dir, file, 1))
}

fn rotated_path(dir: &Path, file: &str, index: usize) -> PathBuf {
    dir.join(format!("{file}.{index}"))
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
            .is_none_or(|rows| rows.len() != 3 || rows.iter().any(std::vec::Vec::is_empty)),
        Mode::Grade => outcome.grade.is_none(),
    }
}

/// v2's OCR stamp (`OcrEngine.ScanOnce`: `yyyyMMdd_HHmmss_fff`), also used in capture file names.
#[must_use]
pub fn scan_stamp() -> String {
    let [year, month, day, hour, minute, second, millis] = seal_win::clock::local_now();
    format!("{year:04}{month:02}{day:02}_{hour:02}{minute:02}{second:02}_{millis:03}")
}

/// v2's bag-scan evidence stamp: `yyyyMMdd_HHmmss`, deliberately the same second-resolution stamp
/// as `scan_page<N>_<stamp>.png` and `scan_<stamp>.txt` in `logs/reads/`.
#[must_use]
pub fn read_stamp() -> String {
    let [year, month, day, hour, minute, second, _] = seal_win::clock::local_now();
    format!("{year:04}{month:02}{day:02}_{hour:02}{minute:02}{second:02}")
}

fn line_stamp() -> String {
    let [year, month, day, hour, minute, second, _] = seal_win::clock::local_now();
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
}

fn hold_stamp() -> String {
    let [_, _, _, hour, minute, second, millis] = seal_win::clock::local_now();
    format!("{hour:02}:{minute:02}:{second:02}.{millis:03}")
}

/// v2's short stamp for diagnostic capture names (`GemComposer`: `HHmmss_fff`), used by the optional
/// `gem.save_empty_captures` crop.
#[must_use]
pub fn short_stamp() -> String {
    let [_, _, _, hour, minute, second, millis] = seal_win::clock::local_now();
    format!("{hour:02}{minute:02}{second:02}_{millis:03}")
}
