//! The headless v3 core: root discovery, the request dispatcher (handshake, shutdown and the config
//! commands), and (later) the Win32, board, OCR and tool subsystems. The shell never links this
//! crate; it talks to the binary over the protocol in `v3/docs/spec/PROTOCOL.md`.

pub mod dispatch;
pub mod root;
