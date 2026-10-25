//! The ONNX sessions and the model-owned dictionary: the runtime half of the OCR port.
//!
//! What is here is the minimum the spike needed and the port will keep:
//!
//! - [`RecModel`] opens the recogniser once, reads the character dictionary from the model's own
//!   `metadata_props["character"]` (the finding from `v3/docs/records/OCR-RESEARCH.md`: 6,623
//!   entries, 6,625 class slots after the C# library's leading `"blank"` and trailing `" "`), pins
//!   the thread counts, and runs tensors;
//! - [`decode_ctc`] is the C# library's greedy CTC decode, pure and unit-tested — blank at index 0,
//!   repeated labels collapsed, confidence = mean of the selected timesteps' maxima (rounded to five
//!   decimals, like `RecPostprocess.cs`);
//! - [`sha256_hex`] exists only so the dictionary hash can be pinned in a test without a new
//!   dependency (the model files are not committed and are read from `SEAL_OCR_MODELS`).
//!
//! The preprocessing (resize/normalise) and the detector post-processing are NOT here: they are the
//! named spike in [`crate::det`].

use std::fmt;
use std::path::Path;
use std::time::Instant;

use ort::environment::Environment;
use ort::session::Session;
use ort::value::{Tensor, ValueType};

/// What can go wrong while opening a model or running a tensor.
#[derive(Debug)]
pub enum OcrError {
    /// The runtime refused something (`ort`'s own text, kept for the log).
    Ort(String),
    /// The model has no `character` metadata — the dictionary is not where this crate expects it.
    MissingDictionary,
    /// The model files could not be read.
    Io(std::io::Error),
}

impl fmt::Display for OcrError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ort(message) => write!(formatter, "onnx runtime: {message}"),
            Self::MissingDictionary => formatter.write_str(
                "the model has no `character` metadata — the dictionary must come from the model",
            ),
            Self::Io(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for OcrError {}

impl From<std::io::Error> for OcrError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

fn ort_error(error: impl fmt::Display) -> OcrError {
    OcrError::Ort(error.to_string())
}

/// One tensor a session declares: the shape uses `-1` for a dynamic dimension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TensorInfo {
    pub name: String,
    pub dtype: String,
    pub shape: Vec<i64>,
}

/// A session's declared inputs and outputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelIo {
    pub inputs: Vec<TensorInfo>,
    pub outputs: Vec<TensorInfo>,
}

fn describe(session: &Session) -> ModelIo {
    fn tensor_info(outlet: &ort::value::Outlet) -> TensorInfo {
        match outlet.dtype() {
            ValueType::Tensor { ty, shape, .. } => TensorInfo {
                name: outlet.name().to_owned(),
                dtype: format!("{ty:?}"),
                shape: shape.to_vec(),
            },
            other => TensorInfo {
                name: outlet.name().to_owned(),
                dtype: format!("{other:?}"),
                shape: Vec::new(),
            },
        }
    }
    ModelIo {
        inputs: session.inputs().iter().map(tensor_info).collect(),
        outputs: session.outputs().iter().map(tensor_info).collect(),
    }
}

pub(crate) fn open_session(
    path: &Path,
    threads: usize,
    label: &'static str,
) -> Result<(Environment, Session, ModelIo), OcrError> {
    if !path.is_file() {
        return Err(OcrError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("model not found: {}", path.display()),
        )));
    }
    let environment = ort::init().with_name(label).build().map_err(ort_error)?;
    let builder = Session::builder(&environment).map_err(ort_error)?;
    let builder = builder.with_intra_threads(threads).map_err(ort_error)?;
    let builder = builder.with_inter_threads(threads).map_err(ort_error)?;
    let mut builder = builder;
    let session = builder.commit_from_file(path).map_err(ort_error)?;
    let io = describe(&session);
    Ok((environment, session, io))
}

/// The recogniser: one session, its dictionary, and the CTC decode.
///
/// The dictionary is the model's `character` metadata, exactly as the C# library reads it — this is
/// the single most important fact the spike pins, because a file-based dictionary was the plan's
/// wrong assumption.
pub struct RecModel {
    // Kept alive with the session; ORT's environment must outlive its sessions.
    _environment: Environment,
    session: Session,
    input_name: String,
    io: ModelIo,
    dictionary: Vec<String>,
    class_list: Vec<String>,
}

/// One recogniser output: the tensor the model returned, its shape `[N, T, C]`, and how long the
/// call took. `logits` is row-major `[T, C]` (batch 1).
#[derive(Debug, Clone, PartialEq)]
pub struct RecOutput {
    pub shape: Vec<i64>,
    pub logits: Vec<f32>,
    pub elapsed_ms: u128,
}

/// The decoded text and its confidence.
#[derive(Debug, Clone, PartialEq)]
pub struct Decoded {
    pub text: String,
    /// Mean of the selected timesteps' maximum scores, rounded to five decimals like the C#.
    pub confidence: f32,
}

impl RecModel {
    /// Opens the recogniser and reads its dictionary. `threads` is pinned (the parity corpus needs a
    /// deterministic run; 1 is the value the spike uses).
    pub fn load(path: &Path, threads: usize) -> Result<Self, OcrError> {
        let (environment, session, io) = open_session(path, threads, "seal-ocr-rec")?;
        let dictionary: Vec<String> = {
            let metadata = session.metadata().map_err(ort_error)?;
            let characters = metadata
                .custom("character")
                .ok_or(OcrError::MissingDictionary)?;
            characters
                .split('\n')
                .map(str::to_owned)
                .collect::<Vec<String>>()
        };
        let class_list = class_list(&dictionary);
        let input_name = io
            .inputs
            .first()
            .map(|input| input.name.clone())
            .ok_or_else(|| OcrError::Ort("the rec model declares no inputs".to_owned()))?;
        Ok(Self {
            _environment: environment,
            session,
            input_name,
            io,
            dictionary,
            class_list,
        })
    }

    pub fn io(&self) -> &ModelIo {
        &self.io
    }

    pub fn custom_metadata_keys(&self) -> Vec<String> {
        self.session
            .metadata()
            .map(|metadata| metadata.custom_keys().unwrap_or_default())
            .unwrap_or_default()
    }

    /// The model's own dictionary: `metadata_props["character"]` split on newlines.
    pub fn dictionary(&self) -> &[String] {
        &self.dictionary
    }

    /// The class list the CTC decode indexes: `"blank"` + dictionary + `" "` (C# `TextRecognizerBase`).
    pub fn class_list(&self) -> &[String] {
        &self.class_list
    }

    pub fn class_count(&self) -> usize {
        self.class_list.len()
    }

    pub fn dictionary_sha256(&self) -> String {
        sha256_hex(self.dictionary.join("\n").as_bytes())
    }

    /// Runs one recogniser tensor. `data` is row-major `[N, C, H, W]` f32, as `shape` says.
    pub fn infer(&mut self, data: &[f32], shape: &[i64]) -> Result<RecOutput, OcrError> {
        let expected: usize = shape.iter().map(|dim| *dim as usize).product();
        if data.len() != expected {
            return Err(OcrError::Ort(format!(
                "input has {} values but shape {shape:?} needs {expected}",
                data.len()
            )));
        }
        let tensor = Tensor::from_array((shape.to_vec(), data.to_vec())).map_err(ort_error)?;
        let input_name = self.input_name.clone();
        let started = Instant::now();
        let outputs = self
            .session
            .run(ort::inputs![input_name => tensor])
            .map_err(ort_error)?;
        let elapsed_ms = started.elapsed().as_millis();
        let (output_shape, logits) = outputs[0].try_extract_tensor::<f32>().map_err(ort_error)?;
        Ok(RecOutput {
            shape: output_shape.to_vec(),
            logits: logits.to_vec(),
            elapsed_ms,
        })
    }

    /// Greedy CTC decode with this model's class list.
    pub fn decode(&self, output: &RecOutput) -> Decoded {
        let (timesteps, classes) = match output.shape.as_slice() {
            [_, timesteps, classes] => (*timesteps, *classes),
            _ => (0, 0),
        };
        decode_ctc(
            &output.logits,
            timesteps.max(0) as usize,
            classes.max(0) as usize,
            &self.dictionary,
        )
    }
}

/// `"blank"` + the dictionary + `" "`: the class list the C# library builds around the model's
/// metadata string (`TextRecognizerBase` constructor).
pub fn class_list(dictionary: &[String]) -> Vec<String> {
    let mut classes = Vec::with_capacity(dictionary.len() + 2);
    classes.push("blank".to_owned());
    classes.extend(dictionary.iter().cloned());
    classes.push(" ".to_owned());
    classes
}

/// The C# library's greedy CTC decode: per timestep argmax; drop index 0 (blank) and a token equal to
/// the PREVIOUS timestep's argmax; join the remaining class strings; confidence = mean of the kept
/// timesteps' maxima, rounded to five decimals.
pub fn decode_ctc(
    logits: &[f32],
    timesteps: usize,
    classes: usize,
    dictionary: &[String],
) -> Decoded {
    let class_list = class_list(dictionary);
    if classes == 0 || timesteps == 0 || logits.len() < timesteps * classes {
        return Decoded {
            text: String::new(),
            confidence: 0.0,
        };
    }

    let mut text = String::new();
    let mut confidence_sum = 0.0f32;
    let mut kept = 0usize;
    let mut previous: Option<usize> = None;
    for step in 0..timesteps {
        let row = &logits[step * classes..(step + 1) * classes];
        let mut best_index = 0usize;
        let mut best_score = f32::MIN;
        for (index, score) in row.iter().enumerate() {
            if *score > best_score {
                best_score = *score;
                best_index = index;
            }
        }
        let repeated = previous == Some(best_index);
        previous = Some(best_index);
        if best_index == 0 || repeated {
            continue;
        }
        if let Some(character) = class_list.get(best_index) {
            text.push_str(character);
            confidence_sum += best_score;
            kept += 1;
        }
    }

    let confidence = if kept == 0 {
        0.0
    } else {
        // C# `Math.Round(avg, 5)`; the currency of this number is a comparison tolerance, not a score.
        (confidence_sum / kept as f32 * 100_000.0).round() / 100_000.0
    };
    Decoded { text, confidence }
}

/// SHA-256 (FIPS 180-4), hex-lowercase. Exists to pin the dictionary hash without adding a crate.
pub fn sha256_hex(bytes: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut state: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    let mut message = bytes.to_vec();
    let bit_len = (bytes.len() as u64).wrapping_mul(8);
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in message.as_chunks::<64>().0 {
        let mut schedule = [0u32; 64];
        for (index, word) in chunk.as_chunks::<4>().0.iter().enumerate() {
            schedule[index] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for index in 16..64 {
            let s0 = schedule[index - 15].rotate_right(7)
                ^ schedule[index - 15].rotate_right(18)
                ^ (schedule[index - 15] >> 3);
            let s1 = schedule[index - 2].rotate_right(17)
                ^ schedule[index - 2].rotate_right(19)
                ^ (schedule[index - 2] >> 10);
            schedule[index] = schedule[index - 16]
                .wrapping_add(s0)
                .wrapping_add(schedule[index - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ (!e & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(choice)
                .wrapping_add(K[index])
                .wrapping_add(schedule[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        for (slot, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *slot = slot.wrapping_add(value);
        }
    }

    state
        .iter()
        .map(|word| format!("{word:08x}"))
        .collect::<String>()
}
