//! The pure domain: configuration model, geometry, the decisions, and the authoring rules.
//!
//! `#![forbid(unsafe_code)]`, no I/O, no Win32. This crate is where `PLAN-V3-CORE.md` §2's rule
//! lives: the domain never knows the screen exists. Its port source is v2's `SealTools.Core`
//! (config model, geometry, matchers, arbitration) plus the rules the launcher currently keeps
//! inline (Quest reference integrity, preset naming, variant switching, and the rest).

#![forbid(unsafe_code)]

pub mod authoring;
pub mod config;
pub mod decision;
pub mod geometry;
pub mod ocr;
