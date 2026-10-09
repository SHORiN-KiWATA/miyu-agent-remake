//! 认帧（施工 O-22，`onebot.md` 第一条「怎么走」第 5 条、「群消息」第 1 条、「撤回」）：群消息带上群号、发的人的名字（群名片，
//! 空白的取昵称）和认出来的段；私聊也带名字；机器人自己发的不认；两种撤回认出撤的人；缺了号的不认。禁言（施工 O-25 中，
//! 「出站队列」第 7 条）：禁的是她的才认，0 秒、`lift_ban` 是解禁；别人、全员、没带秒数、负的、别的种类不认。

use serde_json::{Value, json};

use miyu_onebot::onebot::{Event, Frame, Piece, Posted, Recall, read};

/// 一条群消息事件：群 555 里 `user` 发的第 9 条，`sender` 照给的写。
fn group(user: Value, sender: Value) -> Value {
    json!({
        "time": 1_759_800_000,
        "self_id": 30003,
        "post_type": "message",
        "message_type": "group",
        "message_id": 9,
        "group_id": "555",
        "user_id": user,
        "message": [{"type": "text", "data": {"text": "在"}}, {"type": "at", "data": {"qq": 20003}}],
        "sender": sender,
    })
}

/// 认出来的一件事。
fn event(frame: Value) -> Event {
    match read(frame) {
        Frame::Event(event) => event,
        other => panic!("该认成一件事：{other:?}"),
    }
}

#[test]
fn a_group_message_carries_its_group_sender_name_and_segments() {
    let Event::Group { group: 555, posted } = event(group(
        json!(20002),
        json!({"card": "小林", "nickname": "lin"}),
    )) else {
        panic!("该认成群 555 的消息");
    };
    let Posted {
        bot,
        user,
        message_id,
        time,
        name,
        text,
        segments,
    } = posted;
    assert_eq!(
        (bot, user, message_id, time),
        (30003, 20002, 9, 1_759_800_000)
    );
    assert_eq!(name.as_deref(), Some("小林"));
    assert_eq!(text, "在", "只有字");
    assert_eq!(
        segments.pieces,
        [Piece::Text("在".to_string()), Piece::At(20003)]
    );
    for (sender, wanted) in [
        (json!({"card": " \t", "nickname": "阿杰"}), Some("阿杰")),
        (json!({"nickname": "阿杰"}), Some("阿杰")),
        (json!({"card": "", "nickname": ""}), None),
        (json!(null), None),
    ] {
        let Event::Group { posted, .. } = event(group(json!(20002), sender.clone())) else {
            panic!("该认成群消息");
        };
        assert_eq!(posted.name.as_deref(), wanted, "{sender}");
    }
}

#[test]
fn a_private_message_carries_the_senders_name() {
    let frame = json!({
        "self_id": 30003, "post_type": "message", "message_type": "private", "message_id": 1, "user_id": 10001,
        "message": "在吗", "sender": {"user_id": 10001, "nickname": "主人"},
    });
    let Event::Private(posted) = event(frame) else {
        panic!("该认成私聊");
    };
    assert_eq!(posted.name.as_deref(), Some("主人"));
    assert_eq!(posted.text, "在吗");
}

#[test]
fn the_bots_own_messages_and_messages_without_numbers_are_not_read() {
    assert_eq!(
        read(group(json!(30003), json!({}))),
        Frame::Other("message".to_string())
    );
    assert_eq!(
        read(group(json!("30003"), json!({}))),
        Frame::Other("message".to_string())
    );
    let mut no_group = group(json!(20002), json!({}));
    no_group.as_object_mut().expect("是对象").remove("group_id");
    assert_eq!(read(no_group), Frame::Other("message".to_string()));
    let mut other = group(json!(20002), json!({}));
    other["message_type"] = json!("guild");
    assert_eq!(read(other), Frame::Other("message".to_string()));
}

#[test]
fn recalls_name_who_recalled() {
    let in_group = json!({
        "self_id": 30003, "post_type": "notice", "notice_type": "group_recall", "group_id": 555,
        "user_id": 20002, "operator_id": "40004", "message_id": 1,
    });
    assert_eq!(
        event(in_group.clone()),
        Event::Recalled(Recall {
            bot: 30003,
            group: Some(555),
            user: 20002,
            by: 40004,
            message_id: 1,
        })
    );
    let in_private = json!({
        "self_id": 30003, "post_type": "notice", "notice_type": "friend_recall", "user_id": 10001, "message_id": 2,
    });
    assert_eq!(
        event(in_private),
        Event::Recalled(Recall {
            bot: 30003,
            group: None,
            user: 10001,
            by: 10001,
            message_id: 2,
        })
    );
    let mut no_operator = in_group.clone();
    no_operator
        .as_object_mut()
        .expect("是对象")
        .remove("operator_id");
    assert_eq!(read(no_operator), Frame::Other("notice".to_string()));
    let mut other = in_group;
    other["notice_type"] = json!("group_increase");
    assert_eq!(read(other), Frame::Other("notice".to_string()));
}

/// 群 555 里 `user` 被禁言的通知（施工 O-25 中）：`sub_type`、`duration` 照给的（`null` 的不写这一格）。
fn ban(user: Value, sub_type: &str, duration: Value) -> Value {
    let mut frame = json!({
        "time": 1_759_800_000, "self_id": 30003, "post_type": "notice", "notice_type": "group_ban",
        "sub_type": sub_type, "group_id": "555", "operator_id": 40004, "user_id": user,
    });
    if !duration.is_null() {
        frame["duration"] = duration;
    }
    frame
}

#[test]
fn only_her_own_ban_is_read() {
    let muted = |seconds| Event::Muted {
        bot: 30003,
        group: 555,
        seconds,
    };
    let unmuted = Event::Unmuted {
        bot: 30003,
        group: 555,
    };
    assert_eq!(event(ban(json!(30003), "ban", json!(600))), muted(600));
    assert_eq!(
        event(ban(json!("30003"), "ban", json!("60"))),
        muted(60),
        "号、秒数写成整数的字也认"
    );
    assert_eq!(event(ban(json!(30003), "lift_ban", json!(0))), unmuted);
    assert_eq!(event(ban(json!(30003), "lift_ban", Value::Null)), unmuted);
    assert_eq!(
        event(ban(json!(30003), "ban", json!(0))),
        unmuted,
        "禁 0 秒是解禁"
    );
    for (name, frame) in [
        ("别人", ban(json!(20002), "ban", json!(600))),
        ("全员", ban(json!(0), "ban", json!(600))),
        ("没带秒数", ban(json!(30003), "ban", Value::Null)),
        ("负的", ban(json!(30003), "ban", json!(-1))),
        ("别的种类", ban(json!(30003), "whole_ban", json!(600))),
    ] {
        assert_eq!(read(frame), Frame::Other("notice".to_string()), "{name}");
    }
}
