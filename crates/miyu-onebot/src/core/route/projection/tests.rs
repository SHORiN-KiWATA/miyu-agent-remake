//! 从日志投影一个群（施工 O-23，`onebot.md` 第一条「群里怎么叫她」第 1、2 条）：发的人、是不是主人；开过的回合去掉主人、
//! 自己人开的；主线这一轮回的人并进 `turn.joined` 的；她的回复一轮一笔、回的人取并集；她发过的编号；限流的提示；补来的她的
//! 话不交出来；重的、更早的不收。

use miyu_chat::Reply;
use miyu_kernel::event::Event;
use miyu_kernel::id::ExternalId;
use miyu_kernel::time::Timestamp;
use serde_json::{Value, json};

use super::{Projection, Speaking};

/// 一条主线的会话编号。
const LINE: &str = "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91";

/// 第 `seconds` 秒的时刻。
fn at(seconds: i64) -> Timestamp {
    Timestamp::from_unix_millis(1_760_000_000_000 + seconds * 1000).expect("在范围里")
}

/// 平台上的人 `qq:<号>`。
fn qq(user: i64) -> ExternalId {
    ExternalId::parse(&format!("qq:{user}")).expect("合写法")
}

/// 照日志的写法读一条事件：序号 `seq`、第 `seconds` 秒、种类 `kind`，回合编号、`by`、`body` 照给的。
fn event(seq: u64, seconds: i64, kind: &str, turn: Option<u64>, by: Value, body: Value) -> Event {
    let mut line = json!({"seq": seq, "at": at(seconds), "kind": kind, "by": by, "body": body});
    if let Some(turn) = turn {
        line["turn"] = json!(turn);
    }
    serde_json::from_value(line).expect("读得成事件")
}

/// 号是 `user` 的人在群里说的一句；`owner` 的带 `account`。
fn user(seq: u64, user: i64, owner: bool) -> Event {
    let mut by = json!({"kind": "external", "venue": "qq:group:5", "id": format!("qq:{user}")});
    if owner {
        by["account"] = json!("admin");
    }
    let body = json!({"blocks": [{"type": "text", "text": "嗨"}], "venue": {"msg": seq.to_string(), "ambient": true}});
    event(seq, 0, "message.user", None, by, body)
}

/// 第 `seconds` 秒开的一轮，由那几条触发；没有的是回报这类。
fn started(seq: u64, seconds: i64, triggers: &[u64]) -> Event {
    let mut body = json!({});
    if let Some(last) = triggers.last() {
        body = json!({"trigger": last, "triggers": triggers});
    }
    event(
        seq,
        seconds,
        "turn.started",
        Some(seq),
        json!({"kind": "kernel"}),
        body,
    )
}

/// 并进第 `turn` 轮的那几条。
fn joined(seq: u64, turn: u64, triggers: &[u64]) -> Event {
    let body = json!({"triggers": triggers});
    event(
        seq,
        0,
        "turn.joined",
        Some(turn),
        json!({"kind": "kernel"}),
        body,
    )
}

/// 第 `turn` 轮完了。
fn ended(seq: u64, turn: u64) -> Event {
    let body = json!({"reason": "completed"});
    event(
        seq,
        0,
        "turn.ended",
        Some(turn),
        json!({"kind": "kernel"}),
        body,
    )
}

/// 她在第 `turn` 轮说的一段。
fn assistant(seq: u64, turn: u64) -> Event {
    let by = json!({"kind": "model", "endpoint": "deepseek", "model": "deepseek-v4"});
    let body = json!({"blocks": [{"type": "text", "text": "在。"}], "seen": 1});
    event(seq, 0, "message.assistant", Some(turn), by, body)
}

/// 第 `seconds` 秒发出去的一段：第 `turn` 轮，回 `to`，平台编号 `msg`。
fn delivered(seq: u64, seconds: i64, turn: u64, to: &[i64], msg: &str) -> Event {
    let to: Vec<String> = to.iter().map(|user| format!("qq:{user}")).collect();
    let body = json!({"line": LINE, "turn": turn, "to": to, "msg": msg, "text": "在。"});
    let by = json!({"kind": "module", "id": "onebot"});
    event(seq, seconds, "venue.delivered", None, by, body)
}

/// 第 `seconds` 秒记的一条桥的事件：种类 `kind`，`body` 照给的。
fn ext(seq: u64, seconds: i64, kind: &str, body: Value) -> Event {
    let by = json!({"kind": "module", "id": "onebot"});
    event(seq, seconds, kind, None, by, body)
}

/// 照先后收一串事件，交回她新说的话。
fn take_all(projection: &mut Projection, events: Vec<Event>) -> Vec<Speaking> {
    events
        .iter()
        .filter_map(|event| projection.take(event))
        .collect()
}

#[test]
fn who_said_it_and_whether_the_owner_did() {
    let mut projection = Projection::new(0);
    take_all(
        &mut projection,
        vec![user(1, 10001, true), user(2, 20002, false)],
    );
    assert!(projection.owner(1), "带 account 的是主人");
    assert!(!projection.owner(2));
    assert!(!projection.owner(9), "没收过的不是");
    assert!(projection.knows(1) && projection.knows(2));
    assert!(!projection.knows(3), "回合、别的事件不算人说的话");
}

#[test]
fn turns_only_owners_or_trusted_started_are_left_out() {
    let mut projection = Projection::new(0);
    take_all(
        &mut projection,
        vec![
            user(1, 10001, true),
            user(2, 20002, false),
            user(3, 20003, false),
            started(4, 40, &[1]),
            started(5, 50, &[2]),
            started(6, 60, &[3]),
            started(7, 70, &[]),
            started(8, 80, &[1, 2]),
            started(9, 90, &[1, 3]),
        ],
    );
    let trusted = ["qq:20003".to_string()];
    assert_eq!(
        projection.turns(&trusted),
        [at(50), at(70), at(80)],
        "主人、自己人开的不算；没有触发的照算；有一个别人的就算"
    );
    assert_eq!(
        projection.turns(&[]),
        [at(50), at(60), at(70), at(80), at(90)],
        "不是自己人了就算"
    );
}

#[test]
fn the_running_turn_answers_its_triggers_and_whoever_joined() {
    let mut projection = Projection::new(0);
    let spoken = take_all(
        &mut projection,
        vec![
            user(1, 10001, true),
            user(2, 20002, false),
            started(3, 0, &[1]),
            joined(4, 3, &[2]),
            joined(5, 99, &[1]),
            joined(6, 3, &[1]),
            assistant(7, 3),
            ended(8, 3),
            assistant(9, 3),
        ],
    );
    assert_eq!(
        spoken,
        [
            Speaking {
                turn: 3,
                to: vec![qq(10001), qq(20002)],
            },
            Speaking {
                turn: 3,
                to: Vec::new(),
            },
        ],
        "并进来的不重；别的回合的不算；完了以后的回的人是空的"
    );
}

#[test]
fn her_replies_are_one_per_turn_and_her_ids_are_kept() {
    let mut projection = Projection::new(0);
    take_all(
        &mut projection,
        vec![
            delivered(1, 10, 3, &[20002], "m1"),
            delivered(2, 11, 3, &[20003, 20002], "m2"),
            delivered(3, 30, 8, &[], "m3"),
        ],
    );
    assert_eq!(
        projection.replies(),
        [
            Reply {
                at: at(10),
                to: vec![qq(20002), qq(20003)],
            },
            Reply {
                at: at(30),
                to: Vec::new(),
            },
        ],
        "同一轮的几条并成一笔：时刻取第一条，回的人取并集"
    );
    assert!(projection.mine("m1") && projection.mine("m2") && projection.mine("m3"));
    assert!(!projection.mine("m4"));
}

#[test]
fn notices_are_the_rate_limit_notices_only() {
    let mut projection = Projection::new(0);
    let queued = "ext.onebot.venues.queued";
    take_all(
        &mut projection,
        vec![
            ext(
                1,
                10,
                queued,
                json!({"kind": "notice", "reason": "rate_limited"}),
            ),
            ext(
                2,
                20,
                queued,
                json!({"kind": "reply", "reason": "rate_limited"}),
            ),
            ext(3, 30, queued, json!({"kind": "notice", "reason": "asleep"})),
            ext(
                4,
                40,
                "ext.onebot.chat.decided",
                json!({"kind": "notice", "reason": "rate_limited"}),
            ),
            ext(
                5,
                50,
                queued,
                json!({"kind": "notice", "reason": "rate_limited"}),
            ),
        ],
    );
    assert_eq!(projection.notices(), [at(10), at(50)]);
}

#[test]
fn history_is_not_spoken_and_old_or_repeated_events_are_skipped() {
    let mut projection = Projection::new(5);
    let spoken = take_all(
        &mut projection,
        vec![
            user(1, 20002, false),
            started(2, 0, &[1]),
            assistant(5, 2),
            user(1, 10001, true),
            assistant(6, 2),
            assistant(6, 2),
            assistant(4, 2),
        ],
    );
    assert_eq!(
        spoken,
        [Speaking {
            turn: 2,
            to: vec![qq(20002)],
        }],
        "不大于 upto 的是从前的（正好是 upto 的也是）；重的、比收过的早的不收"
    );
    assert!(!projection.owner(1), "重的不收");
    assert_eq!(projection.last(), 6);
    projection.caught_up(10);
    projection.caught_up(7);
    let spoken = take_all(&mut projection, vec![assistant(10, 2), assistant(11, 2)]);
    assert_eq!(spoken.len(), 1, "掉队再补来的不发，upto 只往后挪");
    assert_eq!(projection.last(), 11);
}
