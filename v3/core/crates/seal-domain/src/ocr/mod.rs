//! The OCR-side pure logic: text cleanup, sign fixing, attribute matching, and the pet-panel parser.
//!
//! Everything here is a port of v2 code that READ the screen, split from the part that TALKED to the
//! screen: `TextCleaner.cs`, `AttrMatcher.cs` and `PetPanel.cs` are already pure functions over
//! strings, which is why they can move into the domain while `seal-ocr` still needs `ort` and the
//! three model files.
//!
//! The crate has no regular-expression engine (that would be a new dependency, and the workspace
//! builds offline). Two places in v2 used one: [`TextCleaner`]'s word-boundary and configured-regex
//! fixes, and [`matcher`]'s simple numeric scans. Both are ported as small, explicit matchers; the
//! configured one accepts the shape the committed dictionary uses and REFUSES anything else by name,
//! so an unsupported pattern is a loud load failure rather than a silently mis-cleaned reading.

pub mod matcher;
pub mod panel;
mod text_cleaner;

pub use matcher::{AttrMatcher, FilterHit, FilterResult, MatchedAttr, PerLevelInfo, fix_sign};
pub use panel::PetPanel;
pub use text_cleaner::{TextCleaner, TextFixError};
