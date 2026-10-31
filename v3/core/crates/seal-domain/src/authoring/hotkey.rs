//! Hotkey names: the launcher's `ParseVk` / `VkName` (v2.16), ported as the rule for what the Hotkeys
//! tab may save.
//!
//! A hotkey is a Win32 VIRTUAL-KEY code; the tab shows it as a NAME and the player edits the name.
//! Both halves have to agree or a setting changes itself the first time the tab is saved, so
//! [`parse_virtual_key`] and [`virtual_key_name`] are tested as inverses.
//!
//! This is a DIFFERENT question from "can the board press this key":
//! [`crate::decision::Key`] stops at F12 (the firmware's table) while hotkeys are read with
//! `GetAsyncKeyState` and go to F24. Two answers, two types, on purpose.

use crate::config::HotkeysConfig;
use std::fmt;

/// The four fields the Hotkeys tab edits, as text. Named so a refusal can say WHICH box is wrong —
/// v2's single "one of the names isn't a key" over four boxes is a hunt.
#[derive(Debug, Clone, Copy)]
pub struct HotkeyNames<'a> {
    pub start: &'a str,
    pub quit: &'a str,
    pub advance_grade: &'a str,
    pub pause: &'a str,
}

/// One name that is not a key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotkeyNameError {
    /// The text as the player typed it (not trimmed: the message quotes what they see).
    pub input: String,
}

impl HotkeyNameError {
    #[must_use]
    pub fn message(&self) -> String {
        format!(
            "\"{}\" is not a key. Use F1–24, Esc, CapsLock, Space, Tab, Enter, or a single \
             letter/digit.",
            self.input
        )
    }
}

impl fmt::Display for HotkeyNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message())
    }
}

impl std::error::Error for HotkeyNameError {}

/// A bad name together with the config field it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HotkeyError {
    Invalid {
        /// `start`, `quit`, `advance_grade` or `pause`.
        field: &'static str,
        input: String,
    },
}

impl HotkeyError {
    #[must_use]
    pub fn field(&self) -> &'static str {
        match self {
            Self::Invalid { field, .. } => field,
        }
    }

    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::Invalid { input, .. } => HotkeyNameError {
                input: input.clone(),
            }
            .message(),
        }
    }
}

impl fmt::Display for HotkeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message())
    }
}

impl std::error::Error for HotkeyError {}

/// Parses one virtual-key NAME to its VK code (`MainWindow.ParseVk`).
///
/// Accepted, exactly as v2 accepts them:
/// - `F1`–`F24`, with a case-insensitive `F` and a C#-parsed number (so `F01` means F1);
/// - `Esc`/`Escape`, `CapsLock`/`Caps`, `Space`, `Tab`, `Enter`/`Return`, `Backspace`;
/// - any single non-ASCII-agnostic character, upper-cased — which is v2's fallback and is why a bare
///   `F` parses as the letter F rather than as a nameless function key.
///
/// A name that is none of those is refused. v2 returned `0` there and the Save compared against 0;
/// the error type makes "not a key" unrepresentable as a valid value.
///
/// # Errors
///
/// Returns [`HotkeyNameError`] when the name is not one of the accepted spellings, carrying the text
/// as the player typed it so the refusal can quote it.
pub fn parse_virtual_key(name: &str) -> Result<u32, HotkeyNameError> {
    let trimmed = name.trim();

    if let Some(number) = trimmed.strip_prefix(['F', 'f'])
        && let Ok(function) = number.trim().parse::<u32>()
        && (1..=24).contains(&function)
    {
        return Ok(0x70 + (function - 1));
    }

    match trimmed.to_uppercase().as_str() {
        "ESC" | "ESCAPE" => return Ok(0x1B),
        "CAPSLOCK" | "CAPS" => return Ok(0x14),
        "SPACE" => return Ok(0x20),
        "TAB" => return Ok(0x09),
        "ENTER" | "RETURN" => return Ok(0x0D),
        "BACKSPACE" => return Ok(0x08),
        _ => {}
    }

    let mut characters = trimmed.chars();
    if let (Some(character), None) = (characters.next(), characters.next()) {
        // C# `char.ToUpperInvariant`: one character in, one code out. Rust's `to_uppercase` may expand
        // (ß → SS), so take its first character — the simple mapping is what a virtual-key code can
        // hold, and a non-ASCII character is not a real key anyway.
        let upper = character.to_uppercase().next().unwrap_or(character);
        let vk = upper as u32;
        if vk != 0 {
            return Ok(vk);
        }
    }

    Err(HotkeyNameError {
        input: name.to_owned(),
    })
}

/// The name the Hotkeys tab shows for a VK code (`MainWindow.VkName`), or its hex code when there is
/// no name.
#[must_use]
pub fn virtual_key_name(vk: u32) -> String {
    if (0x70..=0x87).contains(&vk) {
        return format!("F{}", vk - 0x70 + 1);
    }

    match vk {
        0x1B => "Esc".to_owned(),
        0x14 => "CapsLock".to_owned(),
        0x20 => "Space".to_owned(),
        0x09 => "Tab".to_owned(),
        0x0D => "Enter".to_owned(),
        0x08 => "Backspace".to_owned(),
        0x21..=0x7E => char::from_u32(vk)
            .map_or_else(|| format!("0x{vk:X}"), |character| character.to_string()),
        _ => format!("0x{vk:X}"),
    }
}

/// Parses all four names, or refuses the set and names EVERY bad box.
///
/// v2's Save refused the whole set and wrote nothing when any parsed to 0; the port keeps the refusal
/// (a partial write is worse) and reports each bad field, because a single message over four boxes
/// makes the player hunt.
///
/// # Errors
///
/// Returns every [`HotkeyError`] the set produced — one per box that would not parse — so the tab can
/// mark each bad field rather than only the first.
pub fn parse_hotkeys(names: HotkeyNames<'_>) -> Result<HotkeysConfig, Vec<HotkeyError>> {
    let fields = [
        ("start", names.start),
        ("quit", names.quit),
        ("advance_grade", names.advance_grade),
        ("pause", names.pause),
    ];

    let mut codes = [0i32; 4];
    let mut errors = Vec::new();
    for (index, (field, input)) in fields.iter().enumerate() {
        match parse_virtual_key(input) {
            Ok(code) => {
                // A VK code is one keyboard character or F1-F24: `parse_virtual_key` maps a `char`
                // (at most 0x10FFFF) or 0x70..=0x87, so it always fits the config's `i32` field.
                #[allow(
                    clippy::cast_possible_wrap,
                    reason = "a virtual-key code is one keyboard character or F1-F24, so it always fits i32"
                )]
                let code = code as i32;
                codes[index] = code;
            }
            Err(_) => errors.push(HotkeyError::Invalid {
                field,
                input: (*input).to_owned(),
            }),
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(HotkeysConfig {
        start: codes[0],
        quit: codes[1],
        advance_grade: codes[2],
        pause: codes[3],
    })
}
