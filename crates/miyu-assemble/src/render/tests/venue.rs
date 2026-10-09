//! 场所的消息和桥记的事件（施工 O-13 上）：这一步先不进上下文，O-13 下渲染成群聊近况。

use super::*;

/// 场所里旁听的消息、桥记的场所事件这一步先不进上下文（施工 O-13 上；O-13 下渲染成群聊近况）。不旁听的场所消息照常。
#[test]
fn ambient_messages_and_venue_events_stay_out_for_now() {
    let bridge = r#"{"kind":"module","id":"onebot"}"#;
    let mut log = Log::new();
    log.detached(
        r#"{"kind":"person","account":"alice"}"#,
        "message.user",
        &format!(
            r#"{{"blocks":[{}],"venue":{{"msg":"8810","ambient":true}}}}"#,
            text_json("今天谁值班")
        ),
    );
    log.detached(
        bridge,
        "venue.recalled",
        r#"{"msg":"8810","by":"qq:20017"}"#,
    );
    let called = log.send(&format!(
        r#"[{}],"venue":{{"msg":"8811","mentions_me":true}}"#,
        text_json("你来")
    ));
    log.start(called);
    log.detached(
        bridge,
        "venue.delivered",
        r#"{"line":"01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91","turn":4,"to":["qq:10001"],"msg":"8812","text":"我来"}"#,
    );
    assert_eq!(rendered(&log), ["user: 你来"]);
}
