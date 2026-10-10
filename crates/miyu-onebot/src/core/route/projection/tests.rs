//! 从日志投影一个群（施工 O-23，`onebot.md` 第一条「群里怎么叫她」第 1、2 条）：发的人、是不是终端管理员；开过的回合去掉终端管理员、
//! 白名单成员开的；主线这一轮回的人并进 `turn.joined` 的；她的回复一轮一笔、回的人取并集；她发过的编号；限流的提示；补来的她的
//! 话不交出来；重的、更早的不收。判过要回的（O-23 下）：判断的结论是回的那几条，到收了它们的那一轮完了为止。交给出站链的
//! （O-25 上）在 `outbound_tests.rs`，夹具在这里。

use miyu_chat::{Conditions, Hit, Kind, Pending, Reply, Status};
use miyu_kernel::event::Event;
use miyu_kernel::id::{ExternalId, Seq};
use miyu_kernel::time::Timestamp;
use serde_json::{Value, json};

use super::{DECIDED, Projection, Speaking};

/// 一条主线的会话编号。
const LINE: &str = "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91";

/// 第 `seconds` 秒的时刻。
pub(super) fn at(seconds: i64) -> Timestamp {
    Timestamp::from_unix_millis(1_760_000_000_000 + seconds * 1000).expect("在范围里")
}

/// 平台上的人 `qq:<号>`。
pub(super) fn qq(user: i64) -> ExternalId {
    ExternalId::parse(&format!("qq:{user}")).expect("合写法")
}

/// 照日志的写法读一条事件：序号 `seq`、第 `seconds` 秒、种类 `kind`，回合编号、`by`、`body` 照给的。
pub(super) fn event(
    seq: u64,
    seconds: i64,
    kind: &str,
    turn: Option<u64>,
    by: Value,
    body: Value,
) -> Event {
    let mut line = json!({"seq": seq, "at": at(seconds), "kind": kind, "by": by, "body": body});
    if let Some(turn) = turn {
        line["turn"] = json!(turn);
    }
    serde_json::from_value(line).expect("读得成事件")
}

/// 号是 `user` 的人在群里说的一句；`admin` 的带 `account`。
fn user(seq: u64, user: i64, admin: bool) -> Event {
    let mut by = json!({"kind": "external", "venue": "qq:group:5", "id": format!("qq:{user}")});
    if admin {
        by["account"] = json!("admin");
    }
    let body = json!({"blocks": [{"type": "text", "text": "嗨"}], "venue": {"msg": seq.to_string(), "ambient": true}});
    event(seq, 0, "message.user", None, by, body)
}

/// 号是 `user` 的人第 `seconds` 秒在群里说的一句。
pub(super) fn said_at(seq: u64, seconds: i64, user: i64) -> Event {
    let by = json!({"kind": "external", "venue": "qq:group:5", "id": format!("qq:{user}")});
    let body = json!({"blocks": [{"type": "text", "text": "嗨"}], "venue": {"msg": seq.to_string(), "ambient": true}});
    event(seq, seconds, "message.user", None, by, body)
}

/// 一笔判断：判的是 `msgs`，结论 `outcome`，条件只有冲她来（另带一个认不出的种类，读的时候不要）。
fn decided(seq: u64, msgs: &[u64], outcome: &str) -> Event {
    let conditions = json!([{"kind": "direct", "bonus": 0.3}, {"kind": "inherited", "bonus": 1.0}]);
    let body = json!({"msgs": msgs, "standing": "member", "inbound": "pass", "conditions": conditions, "outcome": outcome});
    ext(seq, 0, DECIDED, body)
}

/// 判过要回的一笔：最后一条是 `msg`、前面是 `absorbed`，小林第 `seconds` 秒说的，条件只有冲她来。
fn committed(msg: u64, absorbed: &[u64], seconds: i64) -> Pending {
    let seq = |n: u64| Seq::new(n).expect("不是 0");
    Pending {
        msg: seq(msg),
        absorbed: absorbed.iter().map(|n| seq(*n)).collect(),
        sender: qq(20002),
        at: at(seconds),
        status: Status::Committed,
        conditions: Conditions {
            hits: vec![Hit {
                kind: Kind::Direct,
                bonus: 0.3,
            }],
        },
    }
}

/// 投影里判过要回、她还没回完的。
fn pendings(projection: &Projection) -> Vec<Pending> {
    projection.committed().cloned().collect()
}

/// 第 `seconds` 秒开的一轮，由那几条触发；没有的是回报这类。
pub(super) fn started(seq: u64, seconds: i64, triggers: &[u64]) -> Event {
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
pub(super) fn joined(seq: u64, turn: u64, triggers: &[u64]) -> Event {
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
pub(super) fn ended(seq: u64, turn: u64) -> Event {
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
pub(super) fn assistant(seq: u64, turn: u64) -> Event {
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

/// 照先后收一串事件，交回她新说的话（补来的不算）。
pub(super) fn take_all(projection: &mut Projection, events: Vec<Event>) -> Vec<Speaking> {
    events
        .iter()
        .filter_map(|event| projection.take(event))
        .filter(|speaking| !speaking.replayed)
        .collect()
}

/// 她新说的话的回合编号和这一轮回的人。
fn turns_to(spoken: &[Speaking]) -> Vec<(u64, Vec<ExternalId>)> {
    spoken
        .iter()
        .map(|speaking| (speaking.turn, speaking.to.clone()))
        .collect()
}

#[test]
fn who_said_it_and_whether_the_admin_did() {
    let mut projection = Projection::new(0);
    take_all(
        &mut projection,
        vec![user(1, 10001, true), user(2, 20002, false)],
    );
    assert!(projection.admin(1), "带 account 的是终端管理员");
    assert!(!projection.admin(2));
    assert!(!projection.admin(9), "没收过的不是");
    assert!(projection.knows(1) && projection.knows(2));
    assert!(!projection.knows(3), "回合、别的事件不算人说的话");
}

#[test]
fn turns_only_admins_or_whitelisted_started_are_left_out() {
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
    let whitelist = ["qq:20003".to_string()];
    assert_eq!(
        projection.turns(&whitelist),
        [at(50), at(70), at(80)],
        "终端管理员、白名单成员开的不算；没有触发的照算；有一个别人的就算"
    );
    assert_eq!(
        projection.turns(&[]),
        [at(50), at(60), at(70), at(80), at(90)],
        "不是白名单成员了就算"
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
        turns_to(&spoken),
        [(3, vec![qq(10001), qq(20002)]), (3, Vec::new()),],
        "并进来的不重；别的回合的不算；完了以后的回的人是空的"
    );
}

#[test]
fn a_turn_opened_for_what_joined_answers_whoever_joined() {
    // 并进第 3 轮的那一条，第 3 轮没再请求就结束了：核心接着开第 6 轮，`trigger` 指向那条 `turn.joined`、没有 `triggers`。
    let mut projection = Projection::new(0);
    let opened = event(
        6,
        5,
        "turn.started",
        Some(6),
        json!({"kind": "kernel"}),
        json!({"trigger": 4}),
    );
    let reported = event(
        8,
        9,
        "turn.started",
        Some(8),
        json!({"kind": "kernel"}),
        json!({"trigger": 7}),
    );
    let spoken = take_all(
        &mut projection,
        vec![
            user(1, 20003, false),
            user(2, 20002, false),
            started(3, 0, &[1]),
            joined(4, 3, &[2]),
            ended(5, 3),
            opened,
            assistant(7, 6),
            reported,
            assistant(9, 8),
        ],
    );
    assert_eq!(
        turns_to(&spoken),
        [(6, vec![qq(20002)]), (8, Vec::new()),],
        "照那条 turn.joined 找回触发的人；指向别的（回报这类）的照旧是空的"
    );
    assert_eq!(
        projection.turns(&["qq:20002".to_string()]),
        [at(0), at(9)],
        "触发的人照找回的算：全是白名单成员的那一轮不计限流"
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
        turns_to(&spoken),
        [(2, vec![qq(20002)])],
        "不大于 upto 的是从前的（正好是 upto 的也是）；重的、比收过的早的不收"
    );
    assert!(!projection.admin(1), "重的不收");
    assert_eq!(projection.last(), 6);
    projection.caught_up(10);
    projection.caught_up(7);
    let spoken = take_all(&mut projection, vec![assistant(10, 2), assistant(11, 2)]);
    assert_eq!(spoken.len(), 1, "掉队再补来的不发，upto 只往后挪");
    assert_eq!(projection.last(), 11);
}

#[test]
fn what_she_said_before_upto_comes_out_marked_as_replayed() {
    let mut projection = Projection::new(3);
    let taken: Vec<Option<bool>> = [
        user(1, 20002, false),
        started(2, 0, &[1]),
        assistant(3, 2),
        assistant(4, 2),
    ]
    .iter()
    .map(|event| projection.take(event).map(|speaking| speaking.replayed))
    .collect();
    assert_eq!(
        taken,
        [None, None, Some(true), Some(false)],
        "补来的照样交出这一刻群里的样子，标上补来的（O-32：期限以内、没入队的补发）"
    );
}

#[test]
fn what_she_was_asked_to_answer_stays_until_that_turn_ends() {
    let mut projection = Projection::new(0);
    take_all(
        &mut projection,
        vec![
            said_at(1, 3, 20002),
            said_at(2, 4, 20002),
            said_at(3, 5, 20002),
            decided(4, &[1, 2], "reply"),
            decided(5, &[3], "record"),
        ],
    );
    assert_eq!(projection.at(2), Some(at(4)));
    assert_eq!(projection.at(9), None);
    assert_eq!(
        pendings(&projection),
        [committed(2, &[1], 4)],
        "结论是回的才算；发的人、时刻照最后一条，认不出的条件不要"
    );
    // 开了收它们的那一轮：还没回完，照旧算；接过去的那一条并进同一轮。
    take_all(
        &mut projection,
        vec![
            started(6, 6, &[1, 2]),
            said_at(7, 7, 20002),
            decided(8, &[7], "reply"),
            joined(9, 6, &[7]),
            said_at(10, 8, 20002),
            decided(11, &[10], "reply"),
        ],
    );
    assert_eq!(
        pendings(&projection),
        [
            committed(2, &[1], 4),
            committed(7, &[], 7),
            committed(10, &[], 8)
        ]
    );
    // 别的一轮（回报这类）完了不算；收了它们的那一轮完了，回完了。还没进哪一轮的照旧算。
    take_all(&mut projection, vec![started(12, 9, &[]), ended(13, 12)]);
    assert_eq!(pendings(&projection).len(), 3);
    take_all(&mut projection, vec![ended(14, 6)]);
    assert_eq!(pendings(&projection), [committed(10, &[], 8)]);
    // 判断里判的那几条一条都读不出、最后一条没收过的，不算。
    take_all(
        &mut projection,
        vec![decided(15, &[], "reply"), decided(16, &[99], "reply")],
    );
    assert_eq!(pendings(&projection), [committed(10, &[], 8)]);
}
