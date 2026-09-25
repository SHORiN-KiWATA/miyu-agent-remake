//! 事件外壳的测试：图纸上的两行读写一字不差、认得出种类；字段顺序固定；
//! 可选字段、不认识的字段怎么读；坏的报错说清哪里坏。

use super::*;
use crate::block::{Block, Text};
use crate::id::AccountId;
use crate::origin::Person;
use crate::test_support::rejected;

const USER: &str = r#"{"seq":41,"at":"2026-09-25T07:04:05.123Z","kind":"message.user","by":{"kind":"person","account":"alice"},"cause":"cmd-7f3a","body":{"blocks":[{"type":"text","text":"看看 src 目录"}]}}"#;
const UNKNOWN: &str = r#"{"seq":43,"at":"2026-09-25T07:04:05.456Z","kind":"ext.memory.recalled","turn":42,"by":{"kind":"module","id":"memory"},"body":{"hits":[{"id":"m1","score":0.87}]}}"#;

#[test]
fn lines_from_the_drawing_round_trip() {
    for line in [USER, UNKNOWN] {
        assert_eq!(Event::from_line(line).unwrap().to_line(), line);
    }
}

#[test]
fn a_known_kind_reads_into_its_type() {
    let event = Event::from_line(USER).unwrap();
    assert_eq!(event.seq.get(), 41);
    assert_eq!(event.turn, None);
    assert_eq!(
        event.cause.map(|c| c.to_string()),
        Some("cmd-7f3a".to_string())
    );
    let Body::MessageUser(message) = &event.body else {
        panic!("应该认得出 message.user：{:?}", event.body);
    };
    assert_eq!(message.blocks.len(), 1);
}

#[test]
fn an_unknown_kind_keeps_its_body_byte_for_byte() {
    let line = UNKNOWN.replace(r#""body":{"hits""#, r#""body":{ "hits""#);
    let event = Event::from_line(&line).unwrap();
    let Body::Unknown { kind, body } = &event.body else {
        panic!("ext.memory.recalled 应该当成不认识的：{:?}", event.body);
    };
    assert_eq!(kind.as_str(), "ext.memory.recalled");
    assert!(body.get().starts_with(r#"{ "hits""#), "{}", body.get());
    assert_eq!(event.to_line(), line);
}

/// 从代码里造一条事件写出去，字段顺序照图纸：seq、at、kind、turn、by、cause、body。
#[test]
fn fields_are_written_in_the_drawing_order() {
    let event = Event {
        seq: Seq::new(41).unwrap(),
        at: Timestamp::parse("2026-09-25T07:04:05.123Z").unwrap(),
        turn: None,
        by: By::Person(Person {
            account: AccountId::parse("alice").unwrap(),
        }),
        cause: Some(CommandId::parse("cmd-7f3a").unwrap()),
        body: Body::MessageUser(MessageUser {
            blocks: vec![Block::Text(Text {
                text: "看看 src 目录".to_string(),
            })],
        }),
    };
    assert_eq!(event.to_line(), USER);
}

#[test]
fn optional_fields_missing_or_null_read_as_absent() {
    let with_nulls = USER.replace(r#""cause":"cmd-7f3a","#, r#""turn":null,"cause":null,"#);
    let event = Event::from_line(&with_nulls).unwrap();
    assert_eq!((event.turn, event.cause.clone()), (None, None));
    assert!(
        !event.to_line().contains("null"),
        "没有的字段不写：{}",
        event.to_line()
    );
}

/// 新版本在外壳上加的字段：日志里的原文留着，读进内存时不管（03 第八节第 1 条）。
#[test]
fn new_fields_on_the_envelope_are_ignored() {
    let line = USER.replace(r#""seq":41,"#, r#""seq":41,"shard":7,"#);
    assert_eq!(Event::from_line(&line).unwrap().to_line(), USER);
}

#[test]
fn broken_lines_say_what_is_wrong() {
    let known_body = USER.replace(r#"{"blocks":"#, r#"{"blockz":"#);
    rejected::<Event>(&known_body, "message.user 的 body 读不出来");
    rejected::<Event>(
        &USER.replace("message.user", "Message.user"),
        "事件种类的写法不对",
    );
    rejected::<Event>(&USER.replace("message.user", "message"), "至少两段");
    rejected::<Event>(&USER.replace(r#""seq":41,"#, ""), "missing field `seq`");
    rejected::<Event>(&USER.replace(r#""seq":41"#, r#""seq":0"#), "从 1 开始");
    rejected::<Event>(&USER.replace(".123Z", ".123+08:00"), "时间的写法不对");
    rejected::<Event>(
        &USER.replace(r#""seq":41,"#, r#""seq":41,"seq":42,"#),
        "duplicate field",
    );
    rejected::<Event>(r#"{"seq":1}"#, "missing field");
}
