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
