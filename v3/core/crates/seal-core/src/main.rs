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
//! supervisor is shut down (it cancels runs, waits for the workers and emits `core.stopping`), and
//! only then is the writer finished — the supervisor thread is the other `EventBus` holder, and the
//! writer only stops when the last sender is gone.

use std::backtrace::Backtrace;
use std::panic;
use std::sync::Arc;

use seal_core::dispatch::{CoreState, handle};
use seal_core::events::EventWriter;
use seal_core::registry::{ProductionRegistry, ToolRegistry};
use seal_core::root::find_root;
use seal_core::supervisor::Supervisor;
use seal_proto::framing::decode_line;
use seal_proto::{Error, Request, Response};
use std::io::BufRead;
use std::path::PathBuf;

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
        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) | Err(_) => break "eof",
            Ok(_) => {}
        }
        if line.trim().is_empty() {
            continue;
        }

        let response = match decode_line::<Request>(&line) {
            Ok(request) => handle(&mut state, request),
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

    state.shutdown_tools(reason);
    // `PROTOCOL.md` §1: shutdown flushes the logs. Every line is already durable (the logger opens,
    // appends and closes per line), so this is the contract hook rather than a data-loss window.
    state.logs.flush();
    // The dispatcher's event sender must be gone before the writer can stop: it is the last sender
    // besides the supervisor, and leaving it attached would hang the process AFTER its final line.
    state.detach_pet_state_sink();
    state.detach_events();
    writer.finish();
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
