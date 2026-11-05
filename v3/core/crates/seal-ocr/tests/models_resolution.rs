//! The OCR models-directory resolver (`PROTOCOL.md` §4 release blocker, `CUTOVER.md` finding 6):
//! env wins or fails, then `<config root>/models`, then `<exe dir>/models`; a candidate counts only
//! when both REQUIRED models (det + rec) are present. The orientation classifier is OPTIONAL: nothing
//! loads it, and requiring a never-used file made a working install report `ocr_unavailable`
//! (OCR-F10). Pure over injected paths, so no machine and no models are needed.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use seal_ocr::pipeline::{OPTIONAL_MODELS, REQUIRED_MODELS, resolve_models_dir};

struct Scratch {
    root: PathBuf,
    counter: AtomicU64,
}

impl Scratch {
    fn new(name: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "seal-ocr-models-{name}-{}-{id}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("the scratch root creates");
        Self {
            root,
            counter: AtomicU64::new(0),
        }
    }

    fn dir(&self, name: &str) -> PathBuf {
        let id = self.counter.fetch_add(1, Ordering::Relaxed);
        let path = self.root.join(format!("{name}-{id}"));
        std::fs::create_dir_all(&path).expect("the scratch dir creates");
        path
    }

    /// A `models`-shaped directory holding exactly `names` as empty files.
    fn models(parent: &Path, names: &[&str]) -> PathBuf {
        let path = parent.join("models");
        std::fs::create_dir_all(&path).expect("the models dir creates");
        for name in names {
            std::fs::write(path.join(name), b"").expect("the model placeholder writes");
        }
        path
    }

    /// A parent directory whose `models` holds both required files.
    fn valid_parent(&self, name: &str) -> PathBuf {
        let parent = self.dir(name);
        let _ = Self::models(&parent, &REQUIRED_MODELS);
        parent
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn a_set_environment_wins_over_root_and_exe() {
    let scratch = Scratch::new("env-wins");
    let env = scratch.valid_parent("env");
    let root = scratch.valid_parent("root");
    let exe = scratch.valid_parent("exe");

    let resolved = resolve_models_dir(Some(&env.join("models")), Some(&root), Some(&exe))
        .expect("a valid env directory wins");
    assert_eq!(resolved, env.join("models"));
}

#[test]
fn a_set_environment_that_is_not_a_directory_is_an_error_not_a_fallback() {
    let scratch = Scratch::new("env-file");
    let root = scratch.valid_parent("root");
    let env_file = scratch.dir("env");
    let env_path = env_file.join("models");
    std::fs::write(&env_path, b"not a directory").expect("the env path writes");

    let error = resolve_models_dir(Some(&env_path), Some(&root), None)
        .expect_err("a set env var is never ignored in favour of the root");
    assert_eq!(error.reason, "models_dir_missing");
    assert!(
        error.message.contains("not a directory") && error.message.contains("SEAL_OCR_MODELS"),
        "{}",
        error.message
    );
}

#[test]
fn a_set_environment_missing_the_recogniser_is_a_loud_error_not_a_fallback() {
    let scratch = Scratch::new("env-incomplete");
    let root = scratch.valid_parent("root");
    let env_parent = scratch.dir("env");
    // The detector alone: the recogniser is what owns the character dictionary, so this is broken.
    let env = Scratch::models(&env_parent, &[REQUIRED_MODELS[0]]);

    let error = resolve_models_dir(Some(&env), Some(&root), None)
        .expect_err("the env var wins or fails; the valid root must not be used");
    assert_eq!(error.reason, "models_dir_missing");
    assert!(
        error.message.contains(REQUIRED_MODELS[1]) && error.message.contains("never ignored"),
        "the line names the missing file and the rule: {}",
        error.message
    );
}

#[test]
fn a_models_directory_without_the_unused_classifier_resolves() {
    let scratch = Scratch::new("no-classifier");
    let parent = scratch.dir("env");
    let models = Scratch::models(&parent, &REQUIRED_MODELS);

    let resolved = resolve_models_dir(Some(&models), None, None)
        .expect("the orientation classifier is never loaded, so its file is optional");
    assert_eq!(resolved, models);
}

#[test]
fn the_classifier_is_the_optional_model_file() {
    assert_eq!(
        OPTIONAL_MODELS,
        ["ch_ppocr_mobile_v2.0_cls_infer.onnx"],
        "the classifier is the only model the pipeline does not load"
    );
}

#[test]
fn an_unset_environment_resolves_the_config_roots_models() {
    let scratch = Scratch::new("root");
    let root = scratch.valid_parent("root");
    let exe = scratch.dir("exe");

    let resolved = resolve_models_dir(None, Some(&root), Some(&exe))
        .expect("the config root's models directory is the first fallback");
    assert_eq!(resolved, root.join("models"));
}

#[test]
fn an_unset_environment_falls_back_to_the_exe_directory() {
    let scratch = Scratch::new("exe");
    let root = scratch.dir("root");
    let exe = scratch.valid_parent("exe");

    let resolved = resolve_models_dir(None, Some(&root), Some(&exe))
        .expect("the exe directory's models is the second fallback");
    assert_eq!(resolved, exe.join("models"));
}

#[test]
fn when_nothing_works_the_line_names_every_attempted_directory() {
    let scratch = Scratch::new("none");
    let root = scratch.dir("root");
    let exe = scratch.dir("exe");

    let error =
        resolve_models_dir(None, Some(&root), Some(&exe)).expect_err("no directory has the models");
    assert_eq!(error.reason, "models_env_unset");
    assert!(
        error.message.contains("SEAL_OCR_MODELS is not set"),
        "{}",
        error.message
    );
    assert!(
        error
            .message
            .contains(&root.join("models").display().to_string()),
        "the root attempt is named: {}",
        error.message
    );
    assert!(
        error
            .message
            .contains(&exe.join("models").display().to_string()),
        "the exe attempt is named: {}",
        error.message
    );
}

#[test]
fn a_directory_without_the_recogniser_does_not_count() {
    let scratch = Scratch::new("one");
    let root = scratch.dir("root");
    // Detector plus the optional classifier is still not a working install.
    let partial = [REQUIRED_MODELS[0], OPTIONAL_MODELS[0]];
    let _ = Scratch::models(&root, &partial);
    let exe = scratch.dir("exe");

    let error = resolve_models_dir(None, Some(&root), Some(&exe))
        .expect_err("a missing recogniser is a broken install, not a candidate");
    assert!(
        error.message.contains(REQUIRED_MODELS[1]),
        "the missing file is named: {}",
        error.message
    );
    assert!(
        error.message.contains("missing"),
        "the attempt says why: {}",
        error.message
    );
}
