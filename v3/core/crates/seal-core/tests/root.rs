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
fn no_marker_means_no_root() {
    let lonely = TestDir::new();
    let leaf = lonely.path().join("nothing/here");
    fs::create_dir_all(&leaf).unwrap();

    assert!(find_root(&leaf).is_none());
}
