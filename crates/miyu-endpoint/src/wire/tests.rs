//! 认请求：读不懂的、不是请求的、通知、`id` 不是字符串的；读一行：去掉行尾、太长的。

use super::*;

fn bad(line: &str) -> (Value, Refusal) {
    match parse(line.as_bytes()) {
        Incoming::Bad(id, refusal) => (id, refusal),
        other => panic!("「{line}」应该回拒绝：{other:?}"),
    }
}

#[test]
fn only_json_rpc_requests_with_string_ids_count() {
    assert_eq!(bad("not json"), (Value::Null, Refusal::PARSE));
    assert_eq!(bad("[1, 2]"), (Value::Null, Refusal::INVALID));
    assert_eq!(
        bad(r#"{"jsonrpc":"2.0","id":5,"method":"hello"}"#),
        (json!(5), Refusal::INVALID),
        "数字的 id 照原样带回去，但不算请求"
    );
    assert_eq!(
        bad(r#"{"jsonrpc":"2.0","id":"c1"}"#),
        (json!("c1"), Refusal::INVALID),
        "没有方法"
    );
    assert_eq!(
        bad(r#"{"id":"c1","method":"hello"}"#),
        (json!("c1"), Refusal::INVALID),
        "不是 2.0"
    );
    assert_eq!(
        bad(r#"{"jsonrpc":"2.0","id":"c1","method":"hello","params":7}"#),
        (json!("c1"), Refusal::INVALID),
        "参数只能是对象或者数组"
    );
    assert_eq!(
        bad(r#"{"jsonrpc":"2.0","id":"","method":"hello"}"#),
        (json!(""), Refusal::INVALID),
        "空的不合命令编号的写法"
    );
    assert!(matches!(
        parse(br#"{"jsonrpc":"2.0","method":"hello"}"#),
        Incoming::Notification
    ));
    let Incoming::Request(request) = parse(br#"{"jsonrpc":"2.0","id":"c1","method":"hello"}"#)
    else {
        panic!("应该是请求");
    };
    assert_eq!(request.id.as_str(), "c1");
    assert_eq!(request.method, "hello");
    assert_eq!(request.params, json!({}), "没写参数的是空对象");
}

/// 对核心发出去的请求的回应（施工 O-2 上）：有 `result` 或 `error`、没有方法的；`id` 照原样交出去。
#[test]
fn a_response_to_the_core_is_read_as_one() {
    let Incoming::Response(ok) =
        parse(br#"{"jsonrpc":"2.0","id":"core-3","result":{"blocks":[]}}"#)
    else {
        panic!("应该是回应");
    };
    assert_eq!(ok.id, "core-3");
    assert_eq!(ok.outcome, Ok(json!({"blocks": []})));
    let Incoming::Response(failed) =
        parse(br#"{"jsonrpc":"2.0","id":"core-4","error":{"code":-32000,"message":"boom"}}"#)
    else {
        panic!("应该是回应");
    };
    assert_eq!(
        failed.outcome,
        Err(json!({"code": -32000, "message": "boom"}))
    );
    assert_eq!(
        bad(r#"{"jsonrpc":"2.0","id":5,"result":{}}"#),
        (json!(5), Refusal::INVALID),
        "回应的 id 也得是字符串"
    );
    assert_eq!(
        bad(r#"{"id":"core-5","result":{}}"#),
        (json!("core-5"), Refusal::INVALID),
        "不是 2.0"
    );
}

#[test]
fn a_reply_is_one_line() {
    let id = CommandId::parse("c1").expect("合写法");
    let line = result(&id, json!({"events": [2]}));
    assert!(!line.contains('\n'));
    assert_eq!(
        serde_json::from_str::<Value>(&line).expect("是 JSON"),
        json!({"jsonrpc": "2.0", "id": "c1", "result": {"events": [2]}})
    );
    let line = error(json!("c1"), Refusal::NOT_FOUND, Locale::Zh);
    assert!(!line.contains('\n'));
    assert_eq!(
        serde_json::from_str::<Value>(&line).expect("是 JSON"),
        json!({
            "jsonrpc": "2.0",
            "id": "c1",
            "error": {"code": -32010, "message": "会话不存在。", "data": {"reason": "session_not_found"}},
        })
    );
}

#[tokio::test]
async fn lines_lose_their_ends_and_long_ones_are_refused() {
    let data = b"one\r\ntwo\nlast".to_vec();
    let mut reader = tokio::io::BufReader::new(&data[..]);
    for want in ["one", "two", "last"] {
        match read_line(&mut reader).await.expect("读得了") {
            Read::Line(line) => assert_eq!(line, want.as_bytes()),
            other => panic!("应该是一行：{other:?}"),
        }
    }
    assert!(matches!(
        read_line(&mut reader).await.expect("读得了"),
        Read::Closed
    ));
    let long = vec![b'x'; LINE_LIMIT + 10];
    let mut reader = tokio::io::BufReader::new(&long[..]);
    assert!(matches!(
        read_line(&mut reader).await.expect("读得了"),
        Read::TooLong
    ));
}
