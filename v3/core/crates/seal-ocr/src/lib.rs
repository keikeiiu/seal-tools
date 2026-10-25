//! The OCR runtime: the ONNX sessions, the model-owned character dictionary, and the pure
//! post-processing around them.
//!
//! The plan this implements changed after [`v3/docs/records/OCR-RESEARCH.md`](../../../docs/records/OCR-RESEARCH.md)
//! measured the real payload: the character dictionary is the recogniser model's
//! `metadata_props["character"]` string (6,623 entries), NOT a file inside RapidOCRSharpOnnx, and
//! the constants to port are the C# library's PP-OCRv4 defaults (mean/std 0.485/0.456/0.406,
//! box_thresh 0.6, unclip_ratio 1.5), not the Python package's (which differ). v2's recorded corpus
//! is the oracle for text and post-processing; its captures are debug drawings, so end-to-end
//! parity needs a raw-crop recording pass.
//!
//! The split this crate keeps: ONNX sessions and tensor plumbing here; the pure text/dictionary
//! work stays in `seal-domain` (`ocr::text_cleaner`, `ocr::matcher`, `ocr::panel`). Model files are
//! not committed — tests read a directory from `SEAL_OCR_MODELS` and skip with a clear message when
//! it is unset, so the normal suite is green on a machine without them.
//!
//! # Runtime spike (task-28)
//!
//! - **Dictionary:** [`model::RecModel::load`] reads `metadata_props["character"]` from the
//!   recogniser ONNX (never a file); `dictionary_sha256` pins it in tests.
//! - **Threads:** sessions are opened with intra/inter = 1 in the spike so two runs are comparable;
//!   the parity runner keeps that pin (the production tools may raise it, but corpus numbers must
//!   come from a pinned run).
//! - **Detector:** [`det`] loads the model and records its shapes only — its OpenCV-shaped
//!   post-processing is the named spike documented there.
//! - **Measured (this machine, 2026-10-25):** rec input `[-1,3,-1,-1]`, rec output
//!   `[-1,-1,6625]` (the class axis equals 6,623 dictionary entries + blank + space), det input
//!   `[-1,3,-1,-1]`, det output `[-1,1,-1,-1]`; rec session load 173 ms, one 320-wide synthetic
//!   inference 17 ms on CPU with intra/inter threads = 1; two runs bit-identical.
//! - **Shipping:** `ort`'s `download-binaries` feature fetched the DirectML static distribution in
//!   this build: the runtime is linked statically and the copied payload DLL is `DirectML.dll`
//!   (18,527,776 B). `copy-dylibs` is what puts it beside the artifacts; a release zip must include
//!   it. A clean build needs the binary download step — which fails in this sandbox (see the task
//!   report), so gate builds need either network or vendored ORT artifacts.

pub mod det;
pub mod model;
