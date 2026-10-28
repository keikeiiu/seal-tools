//! The headless v3 core: root discovery, the request dispatcher (handshake, shutdown, the config
//! commands and the authoring-rule gate every save runs), the event writer, and (later) the Win32,
//! board, OCR and tool subsystems. The shell never links this crate; it talks to the binary over the
//! protocol in `v3/docs/spec/PROTOCOL.md`.

pub mod artifacts;
pub mod board;
pub mod calibration;
pub mod dispatch;
pub mod effects;
pub mod events;
pub mod gem;
pub mod holdspace;
pub mod logs;
pub mod pet;
pub mod pet_state;
pub mod pointer;
pub mod quest;
pub mod registry;
pub mod replay;
pub mod root;
pub mod shop;
pub mod spammer;
pub mod startup;
pub mod supervisor;
pub mod tuner;
pub mod validate;
pub mod win;
