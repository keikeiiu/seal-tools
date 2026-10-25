//! The headless v3 core: root discovery, the request dispatcher (handshake, shutdown, the config
//! commands and the authoring-rule gate every save runs), the event writer, and (later) the Win32,
//! board, OCR and tool subsystems. The shell never links this crate; it talks to the binary over the
//! protocol in `v3/docs/spec/PROTOCOL.md`.

pub mod dispatch;
pub mod events;
pub mod registry;
pub mod root;
pub mod supervisor;
pub mod validate;
