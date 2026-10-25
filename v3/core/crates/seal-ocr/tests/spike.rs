//! The runtime spike: the model-owned dictionary, one deterministic inference, the det shapes, and
//! the DLL shipping layout. Everything that needs the model files skips cleanly when
//! `SEAL_OCR_MODELS` is unset — the normal suite stays green on a machine without them.
//!
//! Run with the models on this machine:
//!
//! ```powershell
//! $env:SEAL_OCR_MODELS = "C:\Git_repo\seal-tools\v2\models"
//! .\cargo.cmd test -p seal-ocr --offline --test spike -- --nocapture
//! Remove-Item Env:SEAL_OCR_MODELS
//! ```

use std::path::PathBuf;

use seal_ocr::det::DetModel;
use seal_ocr::model::{Decoded, RecModel, decode_ctc, sha256_hex};

const DICTIONARY_ENTRIES: usize = 6_623;
const CLASS_SLOTS: usize = 6_625;
const DICTIONARY_SHA256: &str = "28b2362ad4ab2dc38769aa72feb535e3a9ddb3fd2a7585a05920e6393b1dc7f7";

/// The directory named by `SEAL_OCR_MODELS`, or `None` with the skip reason printed.
fn models_dir() -> Option<PathBuf> {
    match std::env::var_os("SEAL_OCR_MODELS") {
        Some(path) => {
            let path = PathBuf::from(path);
            if path.is_dir() {
                Some(path)
            } else {
                panic!(
                    "SEAL_OCR_MODELS is set but is not a directory: {}",
                    path.display()
                );
            }
        }
        None => {
            eprintln!(
                "skipping: set SEAL_OCR_MODELS to the directory holding the three PP-OCRv4 .onnx files"
            );
            None
        }
    }
}

fn rec_path(dir: &std::path::Path) -> PathBuf {
    dir.join("ch_PP-OCRv4_rec_infer.onnx")
}

fn det_path(dir: &std::path::Path) -> PathBuf {
    dir.join("ch_PP-OCRv4_det_infer.onnx")
}

// ── pure CTC decode (no model needed; red-first unit tests) ──────────────────────────────────

#[test]
fn ctc_decode_skips_blank_and_collapses_repeats() {
    // Small dictionary: class list is ["blank", "a", "b", " "] (4 classes).
    let dictionary: Vec<String> = vec!["a".to_owned(), "b".to_owned()];
    // 5 timesteps x 4 classes; argmax sequence: a, a, blank, b, space.
    let rows = [
        [0.1f32, 0.9, 0.0, 0.0],
        [0.0f32, 0.8, 0.2, 0.0],
        [0.9f32, 0.1, 0.0, 0.0],
        [0.0f32, 0.0, 0.7, 0.2],
        [0.0f32, 0.0, 0.1, 0.8],
    ];
    let mut logits = Vec::new();
    for row in rows {
        logits.extend_from_slice(&row);
    }

    let decoded: Decoded = decode_ctc(&logits, 5, 4, &dictionary);

    assert_eq!(decoded.text, "ab ");
    // Selected timesteps are 0 (a), 3 (b), 4 (space): the mean of their maxima.
    let expected = (0.9 + 0.7 + 0.8) / 3.0;
    assert!((decoded.confidence - expected).abs() < 1e-6, "{decoded:?}");
}

#[test]
fn ctc_decode_of_all_blank_is_empty_text() {
    let dictionary: Vec<String> = vec!["a".to_owned(), "b".to_owned()];
    let mut logits = Vec::new();
    for _ in 0..3 {
        logits.extend_from_slice(&[0.9f32, 0.1, 0.0, 0.0]);
    }

    let decoded = decode_ctc(&logits, 3, 4, &dictionary);

    assert_eq!(decoded.text, "");
    assert_eq!(decoded.confidence, 0.0);
}

#[test]
fn the_character_slots_are_blank_plus_dictionary_plus_space() {
    let dictionary: Vec<String> = vec!["a".to_owned(), "b".to_owned()];

    let classes = seal_ocr::model::class_list(&dictionary);

    assert_eq!(
        classes.iter().map(String::as_str).collect::<Vec<_>>(),
        vec!["blank", "a", "b", " "]
    );
}

#[test]
fn sha256_matches_the_known_answer_vector() {
    // FIPS 180-4 test vector for "abc".
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

// ── rec model: dictionary from metadata + one deterministic inference ───────────────────────

#[test]
fn the_rec_model_exposes_the_model_owned_dictionary() {
    let Some(dir) = models_dir() else {
        return;
    };

    let model = RecModel::load(&rec_path(&dir), 1).expect("the rec model loads");

    // The dictionary is the model's metadata, not a file beside it.
    let custom_keys = model.custom_metadata_keys();
    assert!(
        custom_keys.iter().any(|key| key == "character"),
        "the dictionary is the model's `character` metadata: {custom_keys:?}"
    );
    assert_eq!(model.dictionary().len(), DICTIONARY_ENTRIES);
    assert_eq!(model.class_count(), CLASS_SLOTS);
    assert_eq!(model.dictionary_sha256(), DICTIONARY_SHA256);
    assert_eq!(model.class_list()[0], "blank");
    assert_eq!(model.class_list().last().map(String::as_str), Some(" "));

    // The shapes the preprocessing must match. The rec model declares the batch/width dynamic and
    // either a fixed 48 input height or a dynamic one; the documented height is a preprocessing
    // contract in both cases.
    let io = model.io();
    let input = &io.inputs[0];
    assert_eq!(input.dtype, "Float32");
    assert_eq!(input.shape.len(), 4, "rec input is [N,3,48,W]: {input:?}");
    assert_eq!(input.shape[1], 3);
    assert!(
        input.shape[2] == 48 || input.shape[2] == -1,
        "rec input height must be the documented 48 (or dynamic): {:?}",
        input.shape
    );
    let output = &io.outputs[0];
    assert_eq!(output.shape.len(), 3, "rec output is [N,T,C]: {output:?}");

    println!("rec inputs:  {:?}", io.inputs);
    println!("rec outputs: {:?}", io.outputs);
    println!(
        "dictionary: {} entries, sha256 {}",
        model.dictionary().len(),
        model.dictionary_sha256()
    );
}

#[test]
fn one_inference_is_deterministic_and_reports_the_models_class_count() {
    let Some(dir) = models_dir() else {
        return;
    };

    let load_started = std::time::Instant::now();
    let mut model = RecModel::load(&rec_path(&dir), 1).expect("the rec model loads");
    let load_ms = load_started.elapsed().as_millis();
    let class_count = model.class_count();

    // A deterministic synthetic image at the documented height: [1,3,48,320], a gradient in the
    // first channel so the tensor is not all zeros.
    let (height, width) = (48usize, 320usize);
    let mut input = vec![0.0f32; 3 * height * width];
    for y in 0..height {
        for x in 0..width {
            input[y * width + x] = (x as f32) / (width as f32);
        }
    }
    let shape = vec![1i64, 3, height as i64, width as i64];

    let first = model.infer(&input, &shape).expect("the first run works");
    let second = model.infer(&input, &shape).expect("the second run works");

    assert_eq!(
        first.shape.len(),
        3,
        "rec output is [N,T,C]: {:?}",
        first.shape
    );
    assert_eq!(first.shape[0], 1);
    assert_eq!(
        first.shape[2] as usize, class_count,
        "the model's class axis must equal the decorated dictionary's slot count"
    );
    assert_eq!(
        &first.logits, &second.logits,
        "two runs with pinned threads must be bit-identical"
    );

    let decoded_first = model.decode(&first);
    let decoded_second = model.decode(&second);
    assert_eq!(decoded_first.text, decoded_second.text);
    // The decode result is only proof of plumbing; a synthetic image is not expected to read text.
    assert!(
        decoded_first.text.chars().all(|character| {
            model
                .class_list()
                .iter()
                .any(|slot| slot == "blank" || slot == &character.to_string())
        }),
        "every decoded character comes from the model's own dictionary"
    );

    println!(
        "rec load: {load_ms} ms; rec output: {:?}, decoded {:?} ({} chars), first run {} ms",
        first.shape,
        decoded_first.text,
        decoded_first.text.chars().count(),
        first.elapsed_ms
    );
}

// ── det model: shapes only; the OpenCV-shaped post-processing is the named spike ─────────────

#[test]
fn the_det_model_loads_and_reports_its_shapes() {
    let Some(dir) = models_dir() else {
        return;
    };

    let load_started = std::time::Instant::now();
    let model = DetModel::load(&det_path(&dir), 1).expect("the det model loads");
    let load_ms = load_started.elapsed().as_millis();
    let io = model.io();

    let input = &io.inputs[0];
    assert_eq!(input.dtype, "Float32");
    assert_eq!(input.shape.len(), 4, "det input is [N,3,H,W]: {input:?}");
    assert_eq!(input.shape[1], 3, "det input has 3 channels");
    let output = &io.outputs[0];
    assert_eq!(output.shape.len(), 4, "det output is [N,1,H,W]: {output:?}");

    println!("det inputs:  {:?}", io.inputs);
    println!("det outputs: {:?}", io.outputs);
    println!("det load: {load_ms} ms");
}

// ── shipping: the DLL copy-dylibs puts beside the build ─────────────────────────────────────

#[test]
fn the_onnxruntime_dll_travels_with_the_build() {
    // With `download-binaries` + `copy-dylibs`, ort copies its distribution payload beside the build
    // artifacts. For the DirectML static distribution that is `DirectML.dll` (the ORT runtime itself
    // is linked into the binary); other distributions ship `onnxruntime.dll`. Pin whichever this
    // build actually links against and its size, because a release zip that misses it fails at
    // process start.
    let exe = std::env::current_exe().expect("the test binary has a path");
    let mut found: Option<PathBuf> = None;
    for name in ["onnxruntime.dll", "DirectML.dll"] {
        if let Some(candidate) = exe
            .ancestors()
            .map(|dir| dir.join(name))
            .find(|candidate| candidate.is_file())
        {
            found = Some(candidate);
            break;
        }
    }
    let found = found.expect(
        "copy-dylibs puts the ort distribution payload (onnxruntime.dll or DirectML.dll) beside the build",
    );
    let size = std::fs::metadata(&found)
        .expect("the DLL is readable")
        .len();

    assert!(
        size > 10_000_000,
        "the runtime payload is the real runtime, not a stub: {size} bytes"
    );
    println!(
        "ort distribution payload: {} ({} bytes) at {}",
        found.file_name().unwrap_or_default().to_string_lossy(),
        size,
        found.display()
    );
}
