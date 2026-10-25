//! Line framing, as `v3/docs/spec/PROTOCOL.md` §1 defines it: one JSON object per line.

use seal_proto::framing::{decode_line, encode_line};
use seal_proto::Request;
use serde_json::json;

#[test]
fn a_message_encodes_to_exactly_one_line() {
    let line = encode_line(&Request::new("1", "hello", json!({"protocol": 1}))).unwrap();

    assert!(line.ends_with('\n'), "a framed message ends with one newline");
    assert_eq!(
        line.matches('\n').count(),
        1,
        "a message must not leak a second newline"
    );
    assert_eq!(
        line.trim_end(),
        r#"{"v":1,"id":"1","cmd":"hello","args":{"protocol":1}}"#
    );
}

#[test]
fn decode_accepts_a_line_whatever_its_line_ending() {
    let request: Request =
        decode_line("{\"v\":1,\"id\":\"1\",\"cmd\":\"hello\",\"args\":{}}\r\n").unwrap();

    assert_eq!(request.cmd, "hello");
}

#[test]
fn a_malformed_line_is_an_error_not_a_panic() {
    assert!(decode_line::<Request>("not json at all").is_err());
}
