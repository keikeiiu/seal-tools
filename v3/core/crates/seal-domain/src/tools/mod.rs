//! The tools' pure decision halves: the part of each tool that turns a READ plus a CONFIG into what
//! to do next, with no I/O.
//!
//! `PLAN-V3-CORE.md` §6.4: "each tool's decision half becomes a pure function returning a `Decision`
//! (`Click`, `Key`, `Wait`, `Stop(Reason)`) that carries the message to show; the loop only performs
//! effects." That split is why this crate exists: in v2 the interesting half of every tool lived in a
//! class that needed a serial port to construct, so the tests could not reach it — the nine-defect
//! entry in `PROGRESS.md` (63) is the list of decisions made where no test could look.
//!
//! Each module defines the small READ structs its decisions consume, and documents for each field
//! where the core's reader will produce it. Those structs are the contract the reader port (the
//! Win32/OCR/serial side) implements against.
//!
//! The tools do NOT all return one [`Decision`](crate::decision::Decision) at a time. Where a decision
//! is really an ordered plan — a spammer tick's presses, a gem movement set, a quest flow's steps —
//! the module defines a small plan type instead; the caller executes it in order and reports back
//! what failed. Returning one effect per call would move the ordering rules back into the loop.

pub mod spammer;
pub mod tuner;
