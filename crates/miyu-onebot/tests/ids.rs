//! 编号怎么拼（施工 O-8 补，`onebot.md` 第一条「怎么走」第 7、8 条，照 `chat.md` 第七条第 1 条）：场所、平台上的人经群聊
//! 内核拼，样子不变；命令编号带上平台给的时刻；事件的 `time` 是整数、写成整数的字符串都认，没带、读不出的是 0。

use serde_json::{Value, json};

use miyu_chat::{Venue, VenueKind};
use miyu_onebot::onebot::{Frame, PLATFORM, command_id, person, private_venue, read};

#[test]
fn venue_and_person_are_made_by_the_chat_kernel() {
    let venue = private_venue(10001).expect("号是整数，拼得出");
    assert_eq!(venue.as_str(), "qq:private:10001");
    let made = Venue::new(PLATFORM, VenueKind::Private, "10001").expect("拼得出");
    assert_eq!(&venue, made.id(), "和群聊内核拼的是同一个");
    let parsed = Venue::parse(&venue).expect("群聊内核解得回");
    assert_eq!(
        (parsed.platform(), parsed.kind(), parsed.number()),
        ("qq", VenueKind::Private, "10001")
    );

    let who = person(10001).expect("号是整数，拼得出");
    assert_eq!(who.as_str(), "qq:10001");
    assert_eq!(
        miyu_chat::parse_person(&who),
        Some(("qq", "10001")),
        "群聊内核解得回"
    );

    // 号照十进制写，负的也照写（照说不会有）。
    assert_eq!(private_venue(-5).expect("拼得出").as_str(), "qq:private:-5");
    assert_eq!(
        person(i64::MAX).expect("拼得出").as_str(),
        "qq:9223372036854775807"
    );
}

#[test]
fn the_command_id_carries_the_platforms_time() {
    assert_eq!(
        command_id(30003, 501, 1_759_800_000),
        "qq:30003:501:1759800000"
    );
    assert_ne!(
        command_id(30003, 7, 1_759_800_000),
        command_id(30003, 7, 1_759_886_400),
        "同一个消息编号、时刻不同，是两条"
    );
    assert_eq!(command_id(30003, 7, 0), "qq:30003:7:0");
}

/// 一条私聊事件，`time` 照给的写；`None` 是不带。
fn private(time: Option<Value>) -> Value {
    let mut frame = json!({
        "self_id": 30003,
        "post_type": "message",
        "message_type": "private",
        "message_id": 7,
        "user_id": 10001,
        "message": "在吗",
    });
    if let Some(time) = time {
        frame["time"] = time;
    }
    frame
}

/// 认出来的私聊的时刻。
fn time_of(frame: Value) -> i64 {
    match read(frame) {
        Frame::Private(private) => private.time,
        other => panic!("该认成私聊：{other:?}"),
    }
}

#[test]
fn time_is_a_number_or_a_number_written_as_text() {
    assert_eq!(time_of(private(Some(json!(1_759_800_000)))), 1_759_800_000);
    assert_eq!(
        time_of(private(Some(json!("1759800000")))),
        1_759_800_000,
        "写成字符串的也认，和号一样"
    );
}

#[test]
fn time_missing_or_unreadable_is_zero_and_the_message_is_still_read() {
    assert_eq!(time_of(private(None)), 0, "没带");
    for unreadable in [json!(null), json!("昨天"), json!(1.5), json!([1])] {
        assert_eq!(
            time_of(private(Some(unreadable.clone()))),
            0,
            "{unreadable}"
        );
    }
}
