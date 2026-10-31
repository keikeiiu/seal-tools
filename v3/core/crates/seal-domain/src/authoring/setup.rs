//! Setup's client-size rule: the refusal `PLAN-V3-CORE.md` §4.1 names, which v2 does not have.
//!
//! v2's Save Setup parses each box, falls back to `0` when it cannot, and writes `{0,0}` into the
//! stored calibration. PROGRESS.md entry 63 lists the consequence as a known-not-fixed defect:
//! *"Save Setup accepts a blank client size, writing `{0,0}` — which later makes the tool warn
//! 'measured on a 0×0 client, recalibrate', a false warning the UI itself created."*
//!
//! So the rule is not a new opinion; it is the defect's own entry, moved into the domain where a
//! test can hold it. The refusal has to tell the player what to do instead, because the alternative
//! (silently falling back to 1.0 scale / 0×0) is what created the false warning.

use std::fmt;

/// A validated reference client size, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientSize {
    pub width: i32,
    pub height: i32,
}

/// Which box the refusal is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientSizeField {
    Width,
    Height,
}

impl ClientSizeField {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Width => "width",
            Self::Height => "height",
        }
    }
}

/// Why the client size cannot be saved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientSizeError {
    /// The box is empty or whitespace.
    Blank { field: ClientSizeField },
    /// The box holds something that is not a whole number.
    NotANumber {
        field: ClientSizeField,
        input: String,
    },
    /// Zero or negative — the exact value v2 wrote and later warned about.
    NotPositive { field: ClientSizeField, value: i32 },
}

impl ClientSizeError {
    #[must_use]
    pub fn field(&self) -> &'static str {
        match self {
            Self::Blank { field }
            | Self::NotANumber { field, .. }
            | Self::NotPositive { field, .. } => field.as_str(),
        }
    }

    /// The line the Setup tab shows. Every variant says what to do instead of only what is wrong:
    /// the values are measurable, and the button that measures them is on the same card.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::Blank { field } => format!(
                "The client {} is blank. Press Detect with the game open, or type the client width \
                 and height in physical pixels.",
                field.as_str()
            ),
            Self::NotANumber { field, input } => format!(
                "\"{input}\" is not a number. Type the client {} in physical pixels, or press Detect.",
                field.as_str()
            ),
            Self::NotPositive { field, value } => format!(
                "The client {} must be greater than 0 (it is {value}). A 0×0 size is what makes a \
                 later calibration warn that it was measured on a 0×0 client. Press Detect, or type \
                 the real size.",
                field.as_str()
            ),
        }
    }
}

impl fmt::Display for ClientSizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message())
    }
}

impl std::error::Error for ClientSizeError {}

/// Parses the two Setup boxes, or refuses. The first bad box is reported; the player fixes one thing
/// at a time and the message names which.
///
/// # Errors
///
/// Returns [`ClientSizeError`] for the first box that is blank, not a number, or not positive — see
/// [`ClientSizeError::message`] for the player-facing wording.
pub fn validate_client_size(width: &str, height: &str) -> Result<ClientSize, ClientSizeError> {
    Ok(ClientSize {
        width: parse_side(ClientSizeField::Width, width)?,
        height: parse_side(ClientSizeField::Height, height)?,
    })
}

fn parse_side(field: ClientSizeField, text: &str) -> Result<i32, ClientSizeError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(ClientSizeError::Blank { field });
    }

    let value: i32 = trimmed.parse().map_err(|_| ClientSizeError::NotANumber {
        field,
        input: trimmed.to_owned(),
    })?;

    if value <= 0 {
        return Err(ClientSizeError::NotPositive { field, value });
    }

    Ok(value)
}
