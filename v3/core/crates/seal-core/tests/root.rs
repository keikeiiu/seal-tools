//! Root discovery, the way v2 does it: walk up for `config/defaults.yaml`, or the shipped
//! template `config/defaults.yaml.example` in an install that has taken an update.

mod common;

use common::{TestDir, marker};
use seal_core::root::find_root;
use std::fs;

#[test]
fn finds_the_root_from_a_nested_directory() {
    let root = TestDir::new();
    marker(root.path(), "defaults.yaml");
    let nested = root.path().join("a/b/c");
    fs::create_dir_all(&nested).unwrap();

    assert_eq!(find_root(&nested).unwrap(), root.path());
}

#[test]
fn a_shipped_template_also_marks_the_root() {
    let root = TestDir::new();
    marker(root.path(), "defaults.yaml.example");

    assert_eq!(find_root(root.path()).unwrap(), root.path());
}

#[test]
fn the_nearest_marker_wins() {
    let outer = TestDir::new();
    marker(outer.path(), "defaults.yaml");
    let inner = outer.path().join("inner");
    marker(&inner, "defaults.yaml");
    let leaf = inner.join("x/y");
    fs::create_dir_all(&leaf).unwrap();

    assert_eq!(find_root(&leaf).unwrap(), inner);
}

#[test]
fn a_directory_under_the_checkout_finds_the_committed_v3_root() {
    // `v3/config/defaults.yaml` is a real, committed root marker (the dev/install tree), so ANY
    // directory under this checkout now legitimately walks up to `<repo>/v3`; the old "no marker
    // anywhere" fixture cannot be built inside the repository any more. The pure walk-past-an-inner-
    // marker case is covered by `the_nearest_marker_wins`.
    let lonely = TestDir::new();
    let leaf = lonely.path().join("nothing/here");
    fs::create_dir_all(&leaf).unwrap();

    let root = find_root(&leaf).expect("the checkout has a committed root marker");
    assert!(
        root.ends_with("v3"),
        "the committed dev root is <repo>/v3, got {}",
        root.display()
    );
}
