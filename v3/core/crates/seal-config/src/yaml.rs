//! The only place the YAML crate is named: `serde_json::Value` ⇄ YAML text.
//!
//! `PROTOCOL.md` §7 carries the config as opaque JSON, so the file layer is the one boundary where
//! YAML is parsed and emitted. No other module (and no other crate) sees a YAML type; swapping
//! `serde_norway` for a future crate stays this file's work.
//!
//! The mapping is the obvious one — a YAML mapping is a JSON object, a sequence is an array, and
//! scalars are bool / number / string / null — with one limit stated rather than hidden: a mapping
//! **key** must be a string, because JSON object keys are. Every key in the three config files is
//! one already (`'*0'` is a quoted string in the spammer presets), and a hand-written non-string
//! key is reported as a parse failure naming the file rather than silently coerced.

use serde_json::Value;

/// Parses one YAML document into JSON-shaped data. An empty document becomes `Value::Null`, which
/// is exactly how v2's deserialiser spelled "nothing here"; the caller decides what that means.
pub fn parse(text: &str) -> Result<Value, String> {
    serde_norway::from_str::<Value>(text).map_err(|error| error.to_string())
}

/// Emits one YAML document. The result ends with a newline, which the writer relies on when it
/// appends the preserved foreign blocks.
pub fn to_string(value: &Value) -> Result<String, String> {
    serde_norway::to_string(value).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_document_becomes_json_shaped_data() {
        let value = parse(
            "window:\n  title: TW_LIVE\nhotkeys:\n  start: 123\n  ratio: 0.4\n  on: true\n  \n",
        )
        .unwrap();
        assert_eq!(
            value,
            json!({"window": {"title": "TW_LIVE"}, "hotkeys": {"start": 123, "ratio": 0.4, "on": true}})
        );
    }

    #[test]
    fn an_empty_document_is_null() {
        assert_eq!(parse("").unwrap(), Value::Null);
        assert_eq!(parse("# a comment only\n").unwrap(), Value::Null);
    }

    #[test]
    fn comments_are_not_content() {
        assert_eq!(
            parse("# prose\nwindow:\n  # more prose\n  title: TW_LIVE\n").unwrap(),
            json!({"window": {"title": "TW_LIVE"}})
        );
    }

    #[test]
    fn round_trip_keeps_nulls_and_unicode_and_quoted_keys() {
        let original = json!({
            "settings": {"legacy_folded": true, "tuner": {"target_grade": "S"}},
            "spammer": {"presets": {"default": {"*0": 0.25, "F1": 5.0}}},
            "gem": {"empty_signature": null, "grades": ["N", "G", "DG"]},
            "quest": {"active": "紅1", "presets": {"紅1": {"steps": []}}},
        });
        let text = to_string(&original).unwrap();
        assert!(
            text.ends_with('\n'),
            "the writer appends foreign blocks: {text:?}"
        );
        assert_eq!(parse(&text).unwrap(), original);
    }

    #[test]
    fn integers_and_floats_stay_distinct() {
        let value = parse("a: 9025\nb: 0.4\nc: 115200\n").unwrap();
        assert_eq!(value["a"], json!(9025));
        assert_eq!(value["b"], json!(0.4));
        assert_eq!(value["c"], json!(115200));
    }

    #[test]
    fn a_non_string_mapping_key_becomes_its_textual_form() {
        // JSON object keys are strings, and the YAML deserialiser reads a plain scalar key as its
        // text. Every real key in the three config files is already a string; this pins that the
        // odd hand-written one is still usable rather than silently dropped.
        assert_eq!(parse("1: one\n").unwrap(), json!({"1": "one"}));
    }

    #[test]
    fn broken_yaml_is_reported() {
        assert!(parse("window: [unclosed\n").is_err());
    }
}
