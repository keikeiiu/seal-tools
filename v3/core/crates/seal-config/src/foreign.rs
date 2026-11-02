//! Foreign top-level blocks: what a save must not destroy.
//!
//! `local.yaml` is the player's own file, and a save re-serialises everything the application owns.
//! Any top-level block it does NOT own is carried through the save as raw text — comments included,
//! quoting and list style intact — so a hand-added block, or one a future release added that this
//! build does not know about, survives. That is the rule `CONFIG.md` states and the same
//! silent-deletion shape this file has paid for before.
//!
//! The port follows `ConfigLoader.ForeignBlocks` literally: walk the lines, find top-level keys
//! (column zero, not a comment, bare `name:`), keep the ones not in [`crate::OWNED_LOCAL_BLOCKS`]
//! together with the `#` lines immediately above them, and stop a block at the next top-level key.
//! A blank line ends the walk-back over comments, which is what keeps the file's own header out.

use crate::OWNED_LOCAL_BLOCKS;

/// True when the application owns and rewrites this top-level block.
#[must_use]
pub fn is_owned_block(key: &str) -> bool {
    OWNED_LOCAL_BLOCKS.contains(&key)
}

/// The top-level blocks in `text` that the application does NOT own, as raw text ready to append
/// after a save. Empty when there is nothing foreign, which is the normal case.
#[must_use]
pub fn foreign_blocks(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }

    // Normalised like v2 does before splitting, so a CRLF hand-edit is carried through as plain
    // lines rather than smuggling a stray `\r` into an otherwise LF file.
    let normalised = text.replace("\r\n", "\n");
    let lines: Vec<&str> = normalised.split('\n').collect();
    let mut kept: Vec<String> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let Some(key) = try_top_level_key(lines[i]) else {
            i += 1;
            continue;
        };
        if is_owned_block(key) {
            i += 1;
            continue;
        }

        // Walk back over the comment lines that introduce the block, so a hand-written explanation
        // travels with it. A blank line stops the walk, which keeps the file's own header out.
        let mut from = i;
        while from > 0 && lines[from - 1].starts_with('#') {
            from -= 1;
        }

        let mut to = i + 1;
        while to < lines.len() && try_top_level_key(lines[to]).is_none() {
            to += 1;
        }

        // Do not steal the NEXT block's leading comment lines. v2's literal forward walk sweeps
        // everything up to the next top-level key and its walk-back then starts at the next key, so
        // a comment between two foreign blocks is emitted twice; here it is carried once, with the
        // block it introduces. A comment after a blank line stays with the block above, because the
        // walk-back below stops at the blank.
        let mut end = to;
        while end > i + 1 && lines[end - 1].starts_with('#') {
            end -= 1;
        }

        kept.push(
            lines[from..end]
                .join("\n")
                .trim_end_matches('\n')
                .to_owned(),
        );
        i = to;
    }

    if kept.is_empty() {
        String::new()
    } else {
        format!("\n{}\n", kept.join("\n"))
    }
}

/// True when the line opens a top-level block: column zero, not a comment, and a bare `name:` —
/// which is how the writer emits every block it owns. `key` is the text before the first colon.
fn try_top_level_key(line: &str) -> Option<&str> {
    let first = line.chars().next()?;
    if first == '#' || first.is_whitespace() {
        return None;
    }

    let colon = line.find(':')?;
    if colon == 0 {
        return None;
    }

    let key = &line[..colon];
    if key.chars().all(|c| c.is_alphanumeric() || c == '_') {
        Some(key)
    } else {
        None
    }
}
