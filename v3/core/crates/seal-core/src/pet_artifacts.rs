//! Increment 6 of PLAN-PORTABLE-SETUP: inline base64 crops move to `config/artifacts/*.png`.
//!
//! The migration extracts the four inline crop groups (`pet.close_button_png`,
//! `pet.food_icon_png`, `pet.slots[].pet_slot_empty_png`, `pet.queue[].png`) into small PNG files
//! and leaves an `artifacts/<name>.png` REFERENCE in the config. This module is the reader side: at
//! plan time a reference is loaded from disk and turned back into the base64 the existing decoders
//! take, so the wire, the run and the tools never see the file layout. A legacy inline value (or a
//! data-URI-prefixed one) passes through untouched, which is what keeps an unmigrated or hand-edited
//! config working.
//!
//! A reference whose file is missing is left as the reference string: the existing decode refusal
//! will name the crop, and preserving the reference is better than silently dropping the mark.

use std::path::Path;

use base64::Engine as _;
use serde_json::Value;

/// The prefix a stored reference carries (the registry's `artifacts/` domain).
pub const REFERENCE_PREFIX: &str = "artifacts/";

/// Turn every `artifacts/...` crop reference in `config` into the inline base64 the decoders take.
/// Values that are not references are left exactly as they are.
pub(crate) fn resolve_config(config: &mut Value, root: &Path) {
    for path in ["/pet/close_button_png", "/pet/food_icon_png"] {
        resolve_at(config, path, root);
    }
    for index in 0.. {
        let path = format!("/pet/slots/{index}/pet_slot_empty_png");
        if config.pointer(&path).is_none() {
            break;
        }
        resolve_at(config, &path, root);
    }
    for index in 0.. {
        let path = format!("/pet/queue/{index}/png");
        if config.pointer(&path).is_none() {
            break;
        }
        resolve_at(config, &path, root);
    }
}

fn resolve_at(config: &mut Value, pointer: &str, root: &Path) {
    let Some(value) = config.pointer(pointer).and_then(Value::as_str) else {
        return;
    };
    let Some(inline) = load_reference(root, value) else {
        return;
    };
    if let Some(slot) = config.pointer_mut(pointer) {
        *slot = Value::String(inline);
    }
}

/// True when a reference's remainder is a relative path inside `artifacts/`. Absolute paths, drive
/// letters and UNC prefixes are rejected before any `Path::join`, or Windows can resolve the value
/// outside the config root (CORE-F15).
fn safe_relative(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('\\')
        && !name.contains(':')
        && !name.starts_with('/')
        && !name
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
}

/// The inline base64 for an `artifacts/...` reference, or `None` when `value` is not a reference (or
/// its file is missing, in which case the caller keeps the reference).
pub(crate) fn load_reference(root: &Path, value: &str) -> Option<String> {
    let name = value.strip_prefix(REFERENCE_PREFIX)?;
    if !safe_relative(name) {
        return None;
    }
    let direct = root.join(REFERENCE_PREFIX).join(name);
    let bytes = std::fs::read(&direct)
        .or_else(|_| std::fs::read(root.join("config").join(REFERENCE_PREFIX).join(name)))
        .ok()?;
    Some(base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// Write one PNG into `config/artifacts/` and return the reference string. Used by the migration and
/// by the save paths; the name is a bare file name, never a path.
pub(crate) fn write_reference(
    root: &Path,
    name: &str,
    bytes: &[u8],
) -> Result<String, std::io::Error> {
    if !safe_relative(name) || name.contains('/') {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "an artifact name is a bare file name",
        ));
    }
    let directory = root.join(REFERENCE_PREFIX);
    std::fs::create_dir_all(&directory)?;
    let path = directory.join(name);
    let temp = path.with_extension("tmp");
    std::fs::write(&temp, bytes)?;
    std::fs::rename(&temp, &path)?;
    Ok(format!("{REFERENCE_PREFIX}{name}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;

    fn scratch() -> PathBuf {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("target")
            .join(format!("pet-artifacts-{}-{id}", std::process::id()));
        fs::create_dir_all(path.join("artifacts")).unwrap();
        path
    }

    #[test]
    fn an_artifact_reference_is_loaded_back_into_base64() {
        let root = scratch();
        let png = [0x89, b'P', b'N', b'G'];
        fs::write(root.join("artifacts/pet_close.png"), png).unwrap();
        let mut config = json!({"pet": {"close_button_png": "artifacts/pet_close.png"}});
        resolve_config(&mut config, &root);
        let expected = base64::engine::general_purpose::STANDARD.encode(png);
        assert_eq!(config["pet"]["close_button_png"], expected);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn an_inline_value_passes_through_untouched() {
        let root = scratch();
        let mut config = json!({"pet": {"food_icon_png": "aGVsbG8="}});
        resolve_config(&mut config, &root);
        assert_eq!(config["pet"]["food_icon_png"], "aGVsbG8=");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn a_missing_reference_is_kept_so_the_decode_refusal_still_names_it() {
        let root = scratch();
        let mut config = json!({"pet": {"queue": [{"png": "artifacts/missing.png"}]}});
        resolve_config(&mut config, &root);
        assert_eq!(config["pet"]["queue"][0]["png"], "artifacts/missing.png");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn absolute_drive_and_unc_references_are_refused() {
        let root = scratch();
        for value in [
            "artifacts/C:/Windows/System32/x.png",
            "artifacts//server/share/x.png",
            "artifacts/c:relative.png",
            "artifacts/../../escape.png",
            "artifacts/a//b.png",
        ] {
            let mut config = json!({"pet": {"food_icon_png": value}});
            resolve_config(&mut config, &root);
            assert_eq!(
                config["pet"]["food_icon_png"], value,
                "{value} must be left as the refusal reference"
            );
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn writing_a_reference_is_atomic_and_refuses_a_path() {
        let root = scratch();
        let reference = write_reference(&root, "pet_food.png", &[1, 2, 3]).unwrap();
        assert_eq!(reference, "artifacts/pet_food.png");
        assert!(!root.join("artifacts/pet_food.tmp").exists());
        assert!(write_reference(&root, "../escape.png", &[1]).is_err());
        let _ = fs::remove_dir_all(root);
    }
}
