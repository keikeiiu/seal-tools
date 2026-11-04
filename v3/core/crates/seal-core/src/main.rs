//! The core binary: one JSON request per line in, one response per line out.
//!
//! The transport contract is `v3/docs/spec/PROTOCOL.md` §1 — a spawn from the shell, UTF-8 NDJSON
//! on stdin/stdout, EOF meaning "stop safely". Every outgoing line — responses AND events — goes
//! through ONE writer thread ([`seal_core::events::EventWriter`]), so a tool worker can never
//! interleave a line with a response. The tool layer is
//! [`seal_core::supervisor::Supervisor`] over [`seal_core::registry::ProductionRegistry`]; the
//! production registry is an explicit placeholder until the tools and their effects are ported, so a
//! start is refused by name rather than silently doing nothing.
//!
//! Startup reads the merged config before the loop begins: a core that cannot read its config exits
//! with a stderr line rather than answering `hello` and then failing every command.
//!
//! `--replay <script>` is the same binary's development mode (`PROTOCOL.md` §13): it serves the same
//! stdio protocol from a script with no config, board, window, frame or OCR. Its branch runs BEFORE
//! root discovery/config load below, so no machine-touching startup is reachable in that mode.
//!
//! Shutdown order is load-bearing (OPEN.md §1): the final response is queued FIRST, then the
//! supervisor is shut down (it cancels runs, waits for the workers up to its deadline and emits
//! `core.stopping`), and only then is the writer finished — the supervisor thread is the other
//! `EventBus` holder, and the writer only stops when the last sender is gone. `CORE-F8`: a worker
//! that outlives the supervisor's shutdown budget is logged to `error.log`, the writer is given a
//! short bounded settle, and the process exits non-zero so a wedged run is reported instead of
//! hanging onto the COM port.

use std::backtrace::Backtrace;
use std::panic;
use std::sync::Arc;
use std::time::Duration;

use seal_core::dispatch::{CoreState, handle};
use seal_core::events::EventWriter;
use seal_core::registry::{ProductionRegistry, ToolRegistry};
use seal_core::root::find_root;
use seal_core::supervisor::{ShutdownOutcome, Supervisor};
use seal_proto::framing::decode_line;
use seal_proto::{Error, Request, Response};
use std::io::{BufRead, Read};
use std::path::PathBuf;

/// One NDJSON line may be large because calibration frames are inline base64, but it must not be
/// unbounded: a client bug must not grow the core's buffer without limit (CORE-F9).
const MAX_LINE_BYTES: usize = 64 * 1024 * 1024;

/// How long the writer gets to drain after the supervisor is gone (CORE-F8). A clean shutdown has no
/// senders left and finishes in milliseconds; the budget only bounds the wedged case, where the
/// writer is still waiting for an `EventBus` clone held by a stuck worker and the process must leave
/// anyway.
const WRITER_DRAIN_BUDGET: Duration = Duration::from_secs(2);

#[allow(
    clippy::too_many_lines,
    reason = "main keeps the ordered load → serve → shutdown lifecycle in one place; that order is the contract"
)]
fn main() {
    // Replay wins over `--root` (§13). This branch is deliberately before root discovery and before
    // `CoreState::load`, so replay cannot construct the config store, open the board, measure/capture
    // a window or load OCR models.
    if let Some(replay) = replay_from_args() {
        match replay {
            Ok(path) => match seal_core::replay::run(&path) {
                Ok(()) => return,
                Err(error) => {
                    eprintln!("seal-core: replay: {error}");
                    std::process::exit(1);
                }
            },
            Err(error) => {
                eprintln!("seal-core: {error}");
                std::process::exit(1);
            }
        }
    }

    let Some(root) = root_from_args() else {
        eprintln!(
            "seal-core: no config/defaults.yaml or config/defaults.yaml.example found; \
             pass --root <dir>"
        );
        std::process::exit(1);
    };

    // The config is read before anything else: a core that cannot read it must not answer `hello`
    // and then fail every tool command, so the failure is reported the way `PROTOCOL.md` §1 names —
    // the process exits and the shell shows the captured stderr (`core crash`).
    let mut state = match CoreState::load(root) {
        Ok(state) => state,
        Err(error) => {
            eprintln!("seal-core: cannot load config: {error}");
            std::process::exit(1);
        }
    };

    let (writer, bus) = EventWriter::spawn(Box::new(std::io::stdout()));
    // The dispatcher queues `config.changed` events and the loop flushes them after each response, so
    // the wire order is answer-then-event and a shell never sees its own write as an external change.
    state.attach_events(bus.clone());
    // CORE-F3: an unhandled panic must leave a durable reason, not only stderr. Chain to the default
    // hook so the child stderr the shell captures still gets the panic too.
    let panic_logs = Arc::clone(&state.logs);
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let backtrace = Backtrace::force_capture();
        panic_logs.error_line(&format!("panic: {info}\n{backtrace}"));
        default_hook(info);
    }));
    // The registry reads the merged config through this cell; publishing before the supervisor
    // starts means the first Start already sees the config `hello` reported.
    state.publish_config();
    let pet_state = Arc::new(seal_core::pet_state::ConfigPetStateSink::new(
        state.store.clone(),
        state.revision_handle(),
        bus.clone(),
        Arc::clone(&state.logs),
        state.shared_config.clone(),
    ));
    state.attach_pet_state_sink(
        Arc::clone(&pet_state) as Arc<dyn seal_core::pet_state::PetStateSink>
    );
    let registry: Arc<dyn ToolRegistry> = Arc::new(
        ProductionRegistry::new(
            state.shared_config.clone(),
            state.store.clone(),
            Arc::clone(&state.logs),
        )
        .with_pet_state_sink(pet_state)
        .with_firmware_cache(state.firmware_handle()),
    );
    state.attach_supervisor(Supervisor::start(registry, bus, Arc::clone(&state.logs)));

    let stdin = std::io::stdin();
    let reason = loop {
        let mut raw = Vec::new();
        let read = {
            let input = stdin.lock();
            input
                .take((MAX_LINE_BYTES + 1) as u64)
                .read_until(b'\n', &mut raw)
        };
        let overlong = matches!(read, Ok(bytes) if bytes > MAX_LINE_BYTES);
        match read {
            Ok(0) => break "eof",
            Ok(_) => {}
            Err(error) => {
                // A read failure (not an invalid byte: the bytes are decoded below) is answered and
                // the loop keeps serving; the next read starts at the next line.
                writer.response(Response::err(
                    "",
                    Error::new(
                        "bad_request",
                        &format!("stdin read failed: {error}"),
                        serde_json::json!({}),
                    ),
                ));
                state.flush_events();
                continue;
            }
        }
        if overlong && !raw.ends_with(b"\n") {
            // Discard the remainder of the offending line so the next request starts cleanly.
            let mut rest = Vec::new();
            loop {
                rest.clear();
                match stdin.lock().read_until(b'\n', &mut rest) {
                    Ok(0) => break,
                    Ok(_) if rest.ends_with(b"\n") => break,
                    Ok(_) => {}
                    Err(error) => {
                        writer.response(Response::err(
                            "",
                            Error::new(
                                "bad_request",
                                &format!(
                                    "stdin failed while discarding an over-long line: {error}"
                                ),
                                serde_json::json!({}),
                            ),
                        ));
                        state.flush_events();
                        break;
                    }
                }
            }
        }
        if overlong {
            writer.response(Response::err(
                "",
                Error::new(
                    "bad_request",
                    &format!("request line exceeds {MAX_LINE_BYTES} bytes"),
                    serde_json::json!({}),
                ),
            ));
            state.flush_events();
            continue;
        }
        let Ok(line) = String::from_utf8(raw) else {
            writer.response(Response::err(
                "",
                Error::new(
                    "bad_request",
                    "request line is not valid UTF-8",
                    serde_json::json!({}),
                ),
            ));
            state.flush_events();
            continue;
        };
        if line.trim().is_empty() {
            continue;
        }

        // CORE-F9: decode the envelope first so a malformed request can still echo its string id
        // instead of the shell waiting 10 s for a timeout. Missing/numeric ids are also answered.
        let response = match decode_line::<serde_json::Value>(&line) {
            Ok(value) => {
                let id = value
                    .get("id")
                    .map(|id| id.as_str().map_or_else(|| id.to_string(), str::to_owned))
                    .unwrap_or_default();
                match serde_json::from_value::<Request>(value) {
                    Ok(request) => handle(&mut state, request),
                    Err(error) => Response::err(
                        &id,
                        Error::new(
                            "bad_request",
                            &format!("unparseable request: {error}"),
                            serde_json::json!({}),
                        ),
                    ),
                }
            }
            Err(error) => Response::err(
                "",
                Error::new(
                    "bad_request",
                    &format!("unparseable line: {error}"),
                    serde_json::json!({}),
                ),
            ),
        };
        writer.response(response);
        state.flush_events();

        // `shutdown` answers first, then the supervisor and the writer settle; EOF does the same.
        if state.stopping {
            break "shutdown";
        }
    };

    let shutdown = state.shutdown_tools(reason);
    // `PROTOCOL.md` §1: shutdown flushes the logs. Every line is already durable (the logger flushes
    // each line), so this is the contract hook rather than a data-loss window.
    state.logs.flush();
    // The dispatcher's event sender must be gone before the writer can stop: it is the last sender
    // besides the supervisor, and leaving it attached would hang the process AFTER its final line.
    state.detach_pet_state_sink();
    state.detach_events();
    let writer_finished = writer.finish_within(WRITER_DRAIN_BUDGET);
    if writer_finished && shutdown == ShutdownOutcome::Drained {
        return;
    }
    // A worker outlived the shutdown budget (CORE-F8): the supervisor already wrote it to
    // `error.log`. Exiting non-zero is the only honest signal left — the process must not stay alive
    // holding the COM port, and the shell reports a non-zero exit as a crash rather than a clean
    // stop. `process::exit` is deliberate: a stuck worker is still holding threads, and running the
    // rest of the runtime's teardown would wait on exactly the thread this deadline exists to leave.
    eprintln!("seal-core: shutdown did not finish cleanly — exiting non-zero");
    std::process::exit(1);
}

/// `--replay <script>` (`PROTOCOL.md` §13). `None` means normal mode; `Some(Err(..))` means the flag
/// was given without its value, which is a boot refusal rather than a fall-through to discovery.
fn replay_from_args() -> Option<Result<PathBuf, String>> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--replay" {
            return Some(match args.next() {
                Some(value) => Ok(PathBuf::from(value)),
                None => Err("--replay needs a script path".to_owned()),
            });
        }
    }
    None
}

/// `--root <dir>` overrides discovery, for tests and a future CLI. Otherwise the root is found by
/// walking up from the executable, which is how the shell launches this binary.
fn root_from_args() -> Option<PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--root" {
            let path = PathBuf::from(args.next()?);
            return path.is_dir().then_some(path);
        }
    }

    find_root(std::env::current_exe().ok()?.parent()?)
}
