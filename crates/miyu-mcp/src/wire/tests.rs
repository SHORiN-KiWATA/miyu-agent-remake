//! 一行认成哪一种；读一行去掉行尾、太长的认出来。

use serde_json::json;
use tokio::io::BufReader;

use super::*;

#[test]
fn a_line_is_an_answer_a_request_or_a_notice() {
    assert_eq!(
        parse(br#"{"jsonrpc":"2.0","id":3,"result":{"ok":true}}"#),
        Incoming::Answer(3, Ok(json!({"ok": true})))
    );
    assert_eq!(
        parse(br#"{"jsonrpc":"2.0","id":4,"error":{"code":-1,"message":"no"}}"#),
        Incoming::Answer(4, Err(json!({"code": -1, "message": "no"})))
    );
    assert_eq!(
        parse(br#"{"jsonrpc":"2.0","id":"s1","method":"ping"}"#),
        Incoming::Request(json!("s1"), "ping".to_string())
    );
    assert_eq!(
        parse(br#"{"jsonrpc":"2.0","method":"notifications/tools/list_changed"}"#),
        Incoming::Notice("notifications/tools/list_changed".to_string(), json!({}))
    );
}

#[test]
fn what_is_not_understood_says_why() {
    for line in [
        &b"not json"[..],
        br#"[1,2]"#,
        br#"{"jsonrpc":"2.0","id":"x","result":{}}"#,
        br#"{"jsonrpc":"2.0","id":5}"#,
        br#"{"jsonrpc":"2.0"}"#,
    ] {
        assert!(
            matches!(parse(line), Incoming::Bad(_)),
            "{}",
            String::from_utf8_lossy(line)
        );
    }
}

#[test]
fn what_we_write_is_one_line_each() {
    let written = request(7, "tools/call", json!({"text": "a\nb"}));
    assert_eq!(written.iter().filter(|byte| **byte == b'\n').count(), 1);
    assert_eq!(written.last(), Some(&b'\n'));
    let value: serde_json::Value = serde_json::from_slice(&written).expect("JSON");
    assert_eq!(value["id"], 7);
    assert_eq!(value["params"]["text"], "a\nb");
    let refused = answer(&json!("s1"), Err((-32601, "Method not found")));
    let value: serde_json::Value = serde_json::from_slice(&refused).expect("JSON");
    assert_eq!(value["error"]["code"], -32601);
    assert_eq!(value["id"], "s1");
}

#[tokio::test]
async fn lines_come_without_their_ending_and_the_end_is_seen() {
    let mut reader = BufReader::new(&b"{\"a\":1}\r\n{\"b\":2}\n"[..]);
    assert!(matches!(read_line(&mut reader).await, Ok(Read::Line(line)) if line == b"{\"a\":1}"));
    assert!(matches!(read_line(&mut reader).await, Ok(Read::Line(line)) if line == b"{\"b\":2}"));
    assert!(matches!(read_line(&mut reader).await, Ok(Read::Closed)));
}

#[tokio::test]
async fn a_line_past_the_limit_is_too_long() {
    let long = vec![b'x'; LINE_LIMIT + 1];
    let mut reader = BufReader::new(&long[..]);
    assert!(matches!(read_line(&mut reader).await, Ok(Read::TooLong)));
}
