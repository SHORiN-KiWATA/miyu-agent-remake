//! 订阅补来的（施工 O-32）：场所照 `session.created`、机器人号照最近一条人话的命令编号；补来的只是序号不大于 `upto` 的；期限
//! 以内的她的话才记；这一回合入队过的段不再发，留下的算进去。

use serde_json::{Value, json};

use super::{Backlog, Said};
use crate::onebot::To;

/// 第 `seconds` 秒的时刻，写成日志里的样子。
fn at(seconds: i64) -> String {
    format!("2026-10-11T00:00:{seconds:02}.000Z")
}

/// 第 `seconds` 秒的毫秒数。
fn millis(seconds: i64) -> i64 {
    miyu_kernel::time::Timestamp::parse(&at(seconds))
        .expect("合写法")
        .unix_millis()
}

/// 序号 `seq`、第 `seconds` 秒的一条事件：种类 `kind`、`body`，另带的格照 `more`。
fn event(seq: u64, seconds: i64, kind: &str, body: Value, more: Value) -> Value {
    let mut event = json!({"seq": seq, "at": at(seconds), "kind": kind, "body": body});
    if let (Some(event), Some(more)) = (event.as_object_mut(), more.as_object()) {
        event.extend(more.clone());
    }
    event
}

/// 入队了她在回合 `turn` 的一段 `text`。
fn queued(seq: u64, turn: u64, text: &str) -> Value {
    let body = json!({"kind": "reply", "text": text, "line": "l", "turn": turn});
    event(seq, 0, "ext.onebot.venues.queued", body, json!({}))
}

/// 一条机器人号 `bot` 收进来的人话。
fn user(seq: u64, bot: i64) -> Value {
    let cause = json!({"cause": format!("qq:{bot}:7:1759800000")});
    event(seq, 0, "message.user", json!({}), cause)
}

/// 她在回合 `turn`、第 `seconds` 秒说的一句。
fn assistant(seq: u64, seconds: i64, turn: u64) -> Value {
    event(
        seq,
        seconds,
        "message.assistant",
        json!({}),
        json!({"turn": turn}),
    )
}

#[test]
fn the_peer_comes_from_the_venue_and_the_last_message() {
    let mut backlog = Backlog::new(9, 0);
    assert_eq!(backlog.peer(), None);
    let created = json!({"venue": "qq:group:668"});
    backlog.take(&event(1, 0, "session.created", created, json!({})));
    assert_eq!(backlog.peer(), None, "还不知道哪个号");
    backlog.take(&user(2, 30003));
    backlog.take(&user(3, 30004));
    // 命令编号不是 `qq:<号>:…` 的（自己编的）不改。
    let made = event(
        4,
        0,
        "message.user",
        json!({}),
        json!({"cause": "onebot-1"}),
    );
    backlog.take(&made);
    let (bot, to, venue) = backlog.peer().expect("认得出");
    assert_eq!(
        (bot, to, venue.as_str()),
        (30004, To::Group(668), "qq:group:668")
    );

    let mut private = Backlog::new(9, 0);
    let created = json!({"venue": "qq:private:20003"});
    private.take(&event(1, 0, "session.created", created, json!({})));
    private.take(&user(2, 30003));
    assert_eq!(
        private.peer().map(|(bot, to, _)| (bot, to)),
        Some((30003, To::Private(20003)))
    );
    let mut other = Backlog::new(9, 0);
    let created = json!({"venue": "tg:group:1"});
    other.take(&event(1, 0, "session.created", created, json!({})));
    other.take(&user(2, 30003));
    assert_eq!(other.peer(), None, "别的平台的不认");
}

#[test]
fn only_events_up_to_upto_are_replayed() {
    let backlog = Backlog::new(5, 0);
    assert!(backlog.replayed(&assistant(5, 0, 1)), "正好是 upto 的也是");
    assert!(!backlog.replayed(&assistant(6, 0, 1)));
    assert!(
        !backlog.replayed(&json!({"kind": "message.delta"})),
        "瞬时的不是"
    );
}

#[test]
fn only_words_said_within_the_deadline_are_kept() {
    let mut backlog = Backlog::new(9, millis(10));
    backlog.said(&assistant(1, 10, 2), "正好压线".to_string(), None);
    backlog.said(&assistant(2, 11, 2), "还在期限里".to_string(), None);
    let mut turnless = assistant(3, 12, 2);
    turnless["turn"] = Value::Null;
    backlog.said(&turnless, "没有回合编号".to_string(), None);
    let mut timeless = assistant(4, 12, 2);
    timeless["at"] = json!("不是时刻");
    backlog.said(&timeless, "时刻读不出".to_string(), None);
    assert_eq!(
        backlog.take_said(),
        [Said {
            turn: 2,
            text: "还在期限里".to_string(),
            speaking: None,
        }]
    );
    assert!(backlog.take_said().is_empty(), "取走了");
}

#[test]
fn pieces_queued_in_the_same_turn_are_not_sent_again() {
    let mut backlog = Backlog::new(9, 0);
    backlog.take(&queued(1, 2, "第一段"));
    backlog.take(&queued(2, 3, "别的回合的"));
    let notice = json!({"kind": "notice", "reason": "rate_limited", "text": "第二段"});
    backlog.take(&event(3, 0, "ext.onebot.venues.queued", notice, json!({})));
    assert_eq!(backlog.sent(2), ["第一段"], "提示不算");
    let pieces = ["第一段", "第二段", "别的回合的"]
        .map(String::from)
        .to_vec();
    assert_eq!(
        backlog.unsent(2, pieces.clone()),
        ["第二段", "别的回合的"],
        "只照这一回合的比"
    );
    assert_eq!(backlog.sent(2), ["第一段", "第二段", "别的回合的"]);
    assert!(
        backlog.unsent(2, pieces).is_empty(),
        "补发了的算进去：同一回合后面一样的不再发"
    );
    assert_eq!(backlog.unsent(4, vec!["新的".to_string()]), ["新的"]);
}
