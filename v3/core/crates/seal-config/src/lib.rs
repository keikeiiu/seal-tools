//! The three-layer config file, ported from v2's `ConfigLoader`.
//!
//! `defaults.yaml.example` is the base, `local.yaml` is the per-machine overlay (with the one-time
//! fold marker), `attributes.yaml` is the read-only OCR dictionary. The writer keeps the constant
//! header and every foreign top-level block verbatim, and writes atomically — those are the
//! measured rules from `v2/docs/spec/CONFIG.md` and `PLAN-V3-CORE.md` §6.1.
//!
//! The YAML crate is `serde_norway`, a maintained fork of the deprecated `serde_yaml`; it is used
//! behind this crate alone, so swapping it stays one file's work.
