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

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
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

/// The element count a static shape declares, or `None` when a dimension is not a length.
///
/// A negative or overflowing dimension is refused rather than turned into a bogus expected length;
/// `i64` is ONNX's shape type and `usize` is what the buffers index with.
pub(crate) fn tensor_len(shape: &[i64]) -> Option<usize> {
    shape.iter().try_fold(1usize, |length, dimension| {
        usize::try_from(*dimension).ok()?.checked_mul(length)
    })
}

/// The process-wide ONNX Runtime environment: ORT allows exactly one per process and the det and rec
/// sessions share it, so it is built once and never dropped.
///
/// The build's outcome is cached, not just its success: a failure is returned as an [`OcrError`] and
/// a later call repeats it, because a second initialisation attempt could not succeed either.
fn environment() -> Result<&'static Environment, OcrError> {
    static ENVIRONMENT: OnceLock<Result<Environment, String>> = OnceLock::new();
    ENVIRONMENT
        .get_or_init(|| {
            ort::init()
                .with_name("seal-ocr")
                .build()
                .map_err(|error| error.to_string())
        })
        .as_ref()
        .map_err(|message| {
            OcrError::Ort(format!(
                "the ONNX Runtime environment failed to initialise: {message}"
            ))
        })
}

/// One loaded ONNX session, shared by every pipeline over the same file (OCR-F9): the core builds a
/// pipeline for the dispatcher, one for the tuner source and one for the pet source, and each used to
/// load its own ~25 MB copy of the model set plus ORT arenas. The session needs `&mut` to run, hence
/// the mutex; one copy also keeps three concurrent readers from oversubscribing the CPU.
pub(crate) struct SharedSession {
    session: Mutex<Session>,
    pub(crate) io: ModelIo,
}

impl SharedSession {
    pub(crate) fn lock(&self) -> MutexGuard<'_, Session> {
        self.session
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// The cache key: the model file (canonicalized when possible, so `./models/x` and `models/x` are one
/// entry) and the pinned thread counts, which are part of the session's identity.
type SessionKey = (PathBuf, usize);

static SESSIONS: OnceLock<Mutex<HashMap<SessionKey, Arc<SharedSession>>>> = OnceLock::new();

/// The one loader: the file is opened on the first call and every later call for the same path and
/// thread counts returns the already-loaded session (OCR-F9).
pub(crate) fn shared_session(path: &Path, threads: usize) -> Result<Arc<SharedSession>, OcrError> {
    let key = (
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf()),
        threads,
    );
    let cache = SESSIONS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut cache = cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(existing) = cache.get(&key) {
        return Ok(Arc::clone(existing));
    }
    let (session, io) = open_session(path, threads)?;
    let shared = Arc::new(SharedSession {
        session: Mutex::new(session),
        io,
    });
    cache.insert(key, Arc::clone(&shared));
    Ok(shared)
}

fn open_session(path: &Path, threads: usize) -> Result<(Session, ModelIo), OcrError> {
    if !path.is_file() {
        return Err(OcrError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("model not found: {}", path.display()),
        )));
    }
    let builder = Session::builder(environment()?).map_err(ort_error)?;
    let builder = builder.with_intra_threads(threads).map_err(ort_error)?;
    let builder = builder.with_inter_threads(threads).map_err(ort_error)?;
    let mut builder = builder;
    let session = builder.commit_from_file(path).map_err(ort_error)?;
    let io = describe(&session);
    Ok((session, io))
}

/// The recogniser: the shared session, its dictionary, and the CTC decode.
///
/// The dictionary is the model's `character` metadata, exactly as the C# library reads it — this is
/// the single most important fact the spike pins, because a file-based dictionary was the plan's
/// wrong assumption.
pub struct RecModel {
    shared: Arc<SharedSession>,
    input_name: String,
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
    ///
    /// # Errors
    ///
    /// Returns [`OcrError::Io`] when the model file is not readable, [`OcrError::Ort`] when the
    /// runtime rejects the session, and [`OcrError::MissingDictionary`] when the model carries no
    /// `character` metadata.
    pub fn load(path: &Path, threads: usize) -> Result<Self, OcrError> {
        let shared = shared_session(path, threads)?;
        let dictionary: Vec<String> = {
            // The session is shared with the detector and other recognisers, so the metadata read
            // takes the same lock inference does; it is a one-off cost at load.
            let session = shared.lock();
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
        let input_name = shared
            .io
            .inputs
            .first()
            .map(|input| input.name.clone())
            .ok_or_else(|| OcrError::Ort("the rec model declares no inputs".to_owned()))?;
        Ok(Self {
            shared,
            input_name,
            dictionary,
            class_list,
        })
    }

    /// Whether both recognisers use the same loaded session (OCR-F9; the pipeline's sharing check).
    #[must_use]
    pub(crate) fn shares_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.shared, &other.shared)
    }

    #[must_use]
    pub fn io(&self) -> &ModelIo {
        &self.shared.io
    }

    #[must_use]
    pub fn custom_metadata_keys(&self) -> Vec<String> {
        let session = self.shared.lock();
        session
            .metadata()
            .map(|metadata| metadata.custom_keys().unwrap_or_default())
            .unwrap_or_default()
    }

    /// The model's own dictionary: `metadata_props["character"]` split on newlines.
    #[must_use]
    pub fn dictionary(&self) -> &[String] {
        &self.dictionary
    }

    /// The class list the CTC decode indexes: `"blank"` + dictionary + `" "` (C# `TextRecognizerBase`).
    #[must_use]
    pub fn class_list(&self) -> &[String] {
        &self.class_list
    }

    #[must_use]
    pub fn class_count(&self) -> usize {
        self.class_list.len()
    }

    #[must_use]
    pub fn dictionary_sha256(&self) -> String {
        sha256_hex(self.dictionary.join("\n").as_bytes())
    }

    /// Runs one recogniser tensor. `data` is row-major `[N, C, H, W]` f32, as `shape` says.
    ///
    /// # Errors
    ///
    /// Returns [`OcrError::Ort`] when `shape` has a negative or overflowing dimension, when `data`
    /// does not match `shape`, or when the runtime rejects the tensor or the inference.
    pub fn infer(&mut self, data: &[f32], shape: &[i64]) -> Result<RecOutput, OcrError> {
        let Some(expected) = tensor_len(shape) else {
            return Err(OcrError::Ort(format!(
                "input shape {shape:?} has a negative or overflowing dimension"
            )));
        };
        if data.len() != expected {
            return Err(OcrError::Ort(format!(
                "input has {} values but shape {shape:?} needs {expected}",
                data.len()
            )));
        }
        let tensor = Tensor::from_array((shape.to_vec(), data.to_vec())).map_err(ort_error)?;
        let input_name = self.input_name.clone();
        let started = Instant::now();
        let mut session = self.shared.lock();
        let outputs = session
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
    #[must_use]
    pub fn decode(&self, output: &RecOutput) -> Decoded {
        let (timesteps, classes) = match output.shape.as_slice() {
            [_, timesteps, classes] => (*timesteps, *classes),
            _ => (0, 0),
        };
        decode_ctc(
            &output.logits,
            usize::try_from(timesteps.max(0)).unwrap_or(0),
            usize::try_from(classes.max(0)).unwrap_or(0),
            &self.dictionary,
        )
    }
}

/// `"blank"` + the dictionary + `" "`: the class list the C# library builds around the model's
/// metadata string (`TextRecognizerBase` constructor).
#[must_use]
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
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    reason = "`kept` counts CTC timesteps (the model's T axis, hundreds at most) and is far below f32's 2^24 exact-integer range"
)]
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

/// The SHA-256 round constants (FIPS 180-4 §4.2.2), in order.
const SHA256_K: [u32; 64] = [
    0x428a_2f98,
    0x7137_4491,
    0xb5c0_fbcf,
    0xe9b5_dba5,
    0x3956_c25b,
    0x59f1_11f1,
    0x923f_82a4,
    0xab1c_5ed5,
    0xd807_aa98,
    0x1283_5b01,
    0x2431_85be,
    0x550c_7dc3,
    0x72be_5d74,
    0x80de_b1fe,
    0x9bdc_06a7,
    0xc19b_f174,
    0xe49b_69c1,
    0xefbe_4786,
    0x0fc1_9dc6,
    0x240c_a1cc,
    0x2de9_2c6f,
    0x4a74_84aa,
    0x5cb0_a9dc,
    0x76f9_88da,
    0x983e_5152,
    0xa831_c66d,
    0xb003_27c8,
    0xbf59_7fc7,
    0xc6e0_0bf3,
    0xd5a7_9147,
    0x06ca_6351,
    0x1429_2967,
    0x27b7_0a85,
    0x2e1b_2138,
    0x4d2c_6dfc,
    0x5338_0d13,
    0x650a_7354,
    0x766a_0abb,
    0x81c2_c92e,
    0x9272_2c85,
    0xa2bf_e8a1,
    0xa81a_664b,
    0xc24b_8b70,
    0xc76c_51a3,
    0xd192_e819,
    0xd699_0624,
    0xf40e_3585,
    0x106a_a070,
    0x19a4_c116,
    0x1e37_6c08,
    0x2748_774c,
    0x34b0_bcb5,
    0x391c_0cb3,
    0x4ed8_aa4a,
    0x5b9c_ca4f,
    0x682e_6ff3,
    0x748f_82ee,
    0x78a5_636f,
    0x84c8_7814,
    0x8cc7_0208,
    0x90be_fffa,
    0xa450_6ceb,
    0xbef9_a3f7,
    0xc671_78f2,
];

/// The SHA-256 initial hash state (FIPS 180-4 §5.3.3).
const SHA256_INITIAL_STATE: [u32; 8] = [
    0x6a09_e667,
    0xbb67_ae85,
    0x3c6e_f372,
    0xa54f_f53a,
    0x510e_527f,
    0x9b05_688c,
    0x1f83_d9ab,
    0x5be0_cd19,
];

/// SHA-256 (FIPS 180-4), hex-lowercase. Exists to pin the dictionary hash without adding a crate.
#[must_use]
#[allow(
    clippy::many_single_char_names,
    reason = "`a`..`h` are the state variable names FIPS 180-4 defines; renaming them would obscure the algorithm"
)]
pub fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut state = SHA256_INITIAL_STATE;

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
                .wrapping_add(SHA256_K[index])
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

    let mut hex = String::with_capacity(state.len() * 8);
    for word in state {
        let _ = write!(hex, "{word:08x}");
    }
    hex
}
