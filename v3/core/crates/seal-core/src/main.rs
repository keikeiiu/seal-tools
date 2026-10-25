//! The core binary: one JSON request per line in, one response per line out.
//!
//! The transport contract is `v3/docs/spec/PROTOCOL.md` §1 — a spawn from the shell, UTF-8 NDJSON
//! on stdin/stdout, EOF meaning "stop safely". The skeleton answers `hello`, `shutdown` and the
//! config surface (`config.validate`, `config.apply`, `config.reload`, `ui.window`); the board, OCR
//! and tool commands arrive in later increments.
//!
//! Startup reads the merged config before the loop begins: a core that cannot read its config exits
//! with a stderr line rather than answering `hello` and then failing every command.

use seal_core::dispatch::{CoreState, handle};
use seal_core::root::find_root;
use seal_proto::framing::{decode_line, encode_line};
use seal_proto::{Error, Request, Response};
use std::io::{BufRead, Write};
use std::path::PathBuf;

fn main() {
    let Some(root) = root_from_args() else {
        eprintln!(
            "seal-core: no config/defaults.yaml or config/defaults.yaml.example found; \
             pass --root <dir>"
        );
        std::process::exit(1);
    };

    // The config is read before the loop starts: a core that cannot read it must not answer `hello`
    // and then fail every tool command, so the failure is reported the way `PROTOCOL.md` §1 names —
    // the process exits and the shell shows the captured stderr (`core crash`).
    let mut state = match CoreState::load(root) {
        Ok(state) => state,
        Err(error) => {
            eprintln!("seal-core: cannot load config: {error}");
            std::process::exit(1);
        }
    };
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();

    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
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

        let encoded = match encode_line(&response) {
            Ok(encoded) => encoded,
            Err(error) => {
                eprintln!("seal-core: cannot encode a response: {error}");
                break;
            }
        };

        let mut out = stdout.lock();
        if out
            .write_all(encoded.as_bytes())
            .and_then(|()| out.flush())
            .is_err()
        {
            break;
        }

        // `shutdown` answers first, then the loop stops; EOF stops it the same way.
        if state.stopping {
            break;
        }
    }
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
