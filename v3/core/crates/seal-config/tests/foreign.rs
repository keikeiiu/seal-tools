//! Foreign-block preservation, the owned-block list, and the constant header.
//!
//! Ported from v2's `ATopLevelBlockTheAppDoesNotOwnSurvivesASave` and `EveryLocalOverridesBlockIsIn`
//! the owned list, plus the byte-level rules `ConfigLoader.ForeignBlocks` implements: comments ride
//! with their block, quoting is preserved because the text is carried rather than re-emitted, and a
//! blank line ends the comment walk so the file's own header stays out.
//!
//! These are pure tests of the extractor; the writer tests then prove a save actually appends what
//! this returns.

mod common;

use seal_config::foreign::{foreign_blocks, is_owned_block};
use seal_config::{LOCAL_HEADER, OWNED_LOCAL_BLOCKS};

#[test]
fn the_owned_block_list_is_the_one_v2_wrote() {
    // Ported verbatim from ConfigLoader.OwnedLocalBlocks. A block added to the local file without
    // being added here would be treated as foreign — preserved by accident and never updated by any
    // save — so the list is asserted literally rather than by length.
    assert_eq!(
        OWNED_LOCAL_BLOCKS,
        &[
            "calibration",
            "tuner",
            "gem",
            "arduino",
            "ui",
            "spammer",
            "buy_sell",
            "pet",
            "tooltip",
            "quest",
            "settings",
        ]
    );
}

#[test]
fn owned_names_are_recognised_and_other_names_are_foreign() {
    for owned in OWNED_LOCAL_BLOCKS {
        assert!(is_owned_block(owned), "{owned} should be owned");
    }
    for foreign in ["window", "game", "my_notes", "Settings", "ui2", ""] {
        assert!(!is_owned_block(foreign), "{foreign} should be foreign");
    }
}

#[test]
fn an_empty_file_has_no_foreign_blocks() {
    assert_eq!(foreign_blocks(""), "");
}

#[test]
fn a_file_of_owned_blocks_only_keeps_nothing() {
    let text = "\
calibration:
  dpi_scale: 1.5
tuner:
  ocr:
    region: {left: 0, top: 0, width: 300, height: 320}
settings:
  legacy_folded: true
";
    assert_eq!(foreign_blocks(text), "");
}

#[test]
fn a_foreign_block_keeps_its_comment_its_quoting_and_its_list_style() {
    let text = "\
ui:
  left: 111
# My own notes about this machine, which nothing should delete.
my_notes:
  monitor: \"left\"
  notes:
  - and a list
";
    let extracted = foreign_blocks(text);

    // The block, verbatim — not parsed and re-emitted: a re-serialisation would drop the quotes and
    // any other styling the author chose.
    assert_eq!(
        extracted,
        "\n# My own notes about this machine, which nothing should delete.\nmy_notes:\n  monitor: \"left\"\n  notes:\n  - and a list\n"
    );
}

#[test]
fn the_comment_walk_back_stops_at_a_blank_line_so_the_header_stays_out() {
    let text = "\
# ── the file's own header ──
# second header line

my_notes:
  monitor: left
";
    assert_eq!(extracted_block(text), "my_notes:\n  monitor: left");
}

#[test]
fn every_foreign_block_travels_in_order() {
    let text = "\
# first
first_block:
  a: 1
# second
second_block:
  b: 2
";
    assert_eq!(
        foreign_blocks(text),
        "\n# first\nfirst_block:\n  a: 1\n# second\nsecond_block:\n  b: 2\n"
    );
}

#[test]
fn indented_keys_lists_and_bad_names_do_not_open_a_block() {
    // None of these is a top-level key: they are content of the previous block, or not a block at
    // all. If one were mistaken for a key the walk would split a foreign block in half.
    let text = "\
ui:
  left: 1
  nested:
    deeper: 2
  - a list item: with a colon
my notes: not a key
";
    // Only the owned `ui` block was found, so there is nothing foreign.
    assert_eq!(foreign_blocks(text), "");
}

#[test]
fn crlf_is_normalised_to_lf() {
    let text = "ui:\r\n  left: 1\r\nmy_notes:\r\n  monitor: left\r\n";
    assert_eq!(foreign_blocks(text), "\nmy_notes:\n  monitor: left\n");
}

#[test]
fn a_file_that_is_only_a_header_keeps_nothing() {
    assert_eq!(foreign_blocks("# a header\n#\n\n"), "");
}

#[test]
fn the_header_is_the_ported_text() {
    // Starts a comment, not content; says the things that have already cost data; names the
    // documents that explain the rest; and ends with a blank line before the serialised body.
    assert!(LOCAL_HEADER.starts_with('#'));
    assert!(LOCAL_HEADER.ends_with("\n\n"));
    assert!(LOCAL_HEADER.contains("REWRITES this whole file"));
    assert!(LOCAL_HEADER.contains("docs/spec/CONFIG.md"));
    assert!(LOCAL_HEADER.contains("docs/spec/CALIBRATION.md"));
    assert!(LOCAL_HEADER.contains("The `settings:` block: the PORTABLE half"));
    assert!(LOCAL_HEADER.contains("defaults.yaml is the template"));
    assert!(LOCAL_HEADER.contains("do not commit it or share it"));
}

#[test]
fn the_header_says_an_unmodelled_owned_key_is_kept_not_dropped() {
    // Deliberate divergence from v2's LocalHeader, and the only rewording it has. v2 rebuilt every
    // owned block from a typed projection, so a key it did not model was silently DROPPED — four
    // recorded losses. The v3 config is opaque JSON and the writer merges the values it was given,
    // so no key disappears inside an owned block; a header that still promised the drop would be a
    // false warning in the player's own file.
    assert!(LOCAL_HEADER.contains("KEPT"));
    assert!(!LOCAL_HEADER.contains("DROPPED"));
    // The half that IS still true is kept and must stay said: comments cannot survive a save.
    assert!(LOCAL_HEADER.contains("Comments are still lost everywhere"));
    assert!(LOCAL_HEADER.contains("Keep a copy if you edit"));
}

/// The extracted text without the leading/trailing newlines the writer adds.
fn extracted_block(text: &str) -> String {
    foreign_blocks(text).trim_matches('\n').to_owned()
}
