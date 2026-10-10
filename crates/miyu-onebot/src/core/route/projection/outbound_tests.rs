//! 投影交给出站链的（施工 O-25 上，`onebot.md` 第一条「群里怎么叫她」第 2、9 条）：她回的那一条是这一轮触发的最后一条，并进来
//! 的换成并进来的最后一条，平台编号、发的人、时刻都对；那之后别人说了几条（发它的人自己补的不算）；群里最后一条是不是她的；
//! 这一轮发出去的（O-25 中照入队的），换了回合就清，晚到的上一轮的不算；桥先算上的，日志推来同一段不重复算。夹具在 `tests.rs`。

use serde_json::json;

use super::tests::{assistant, at, ended, event, joined, qq, said_at, started, take_all};
use super::{Aim, Projection, QUEUED};

/// 第 `turn` 轮入队了的她的一段 `text`（O-25 中）。
fn queued(seq: u64, turn: u64, text: &str) -> miyu_kernel::event::Event {
    let body = json!({"kind": "reply", "text": text, "line": "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91", "turn": turn});
    event(
        seq,
        0,
        QUEUED,
        None,
        json!({"kind": "module", "id": "onebot"}),
        body,
    )
}

/// 第 `turn` 轮发出去的一段 `text`。
fn delivered(seq: u64, turn: u64, text: &str) -> miyu_kernel::event::Event {
    let body = json!({"line": "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91", "turn": turn, "to": [], "msg": format!("m{seq}"), "text": text});
    event(
        seq,
        0,
        "venue.delivered",
        None,
        json!({"kind": "module", "id": "onebot"}),
        body,
    )
}

#[test]
fn she_answers_the_last_trigger_and_counts_the_others_after_it() {
    let mut projection = Projection::new(0);
    let spoken = take_all(
        &mut projection,
        vec![
            said_at(1, 1, 20002),
            said_at(2, 2, 20003),
            started(3, 3, &[1, 2]),
            said_at(4, 4, 20003),
            said_at(5, 5, 20002),
            assistant(6, 3),
            joined(7, 3, &[5]),
            said_at(8, 8, 20003),
            assistant(9, 3),
            ended(10, 3),
            assistant(11, 3),
        ],
    );
    let aim = |msg: &str, user: i64, seconds: i64| Aim {
        msg: Some(msg.to_string()),
        sender: qq(user),
        at: at(seconds),
    };
    assert_eq!(spoken[0].aim, Some(aim("2", 20003, 2)), "照最后一条");
    assert_eq!(spoken[0].others, 1, "发它的人自己补的不算，之前的不算");
    assert_eq!(spoken[1].aim, Some(aim("5", 20002, 5)), "并进来的换上");
    assert_eq!(spoken[1].others, 1);
    assert_eq!(spoken[2].aim, None, "这一轮完了");
    assert_eq!(spoken[2].others, 0);
}

#[test]
fn whether_the_last_one_is_hers() {
    let mut projection = Projection::new(0);
    let spoken = take_all(
        &mut projection,
        vec![
            said_at(1, 1, 20002),
            started(2, 2, &[1]),
            assistant(3, 2),
            delivered(4, 2, "在。"),
            assistant(5, 2),
            said_at(6, 6, 20003),
            assistant(7, 2),
        ],
    );
    let own: Vec<bool> = spoken.iter().map(|speaking| speaking.last_is_own).collect();
    assert_eq!(own, [false, true, false], "照序号比最后一条人说的话和她的");
}

/// 她那一段发出去到平台回执之间进来的人话，在日志里排在 `venue.delivered` 前面，可群里她那一段在前：照她说那一段的序号比，
/// 不照记回执的序号（施工 O-25 上，CI 的 macOS 上撞出来的）。桥重启以后补来的从前那几条（`upto` 以内）照样认，算出来一样。
#[test]
fn a_message_arriving_before_her_receipt_is_still_later_than_hers() {
    for upto in [0, 6] {
        let mut projection = Projection::new(upto);
        let spoken = take_all(
            &mut projection,
            vec![
                said_at(1, 1, 20002),
                started(2, 2, &[1]),
                assistant(3, 2),
                said_at(4, 4, 20003),
                delivered(5, 2, "在。"),
                ended(6, 2),
                started(7, 7, &[4]),
                assistant(8, 7),
            ],
        );
        let own: Vec<bool> = spoken.iter().map(|speaking| speaking.last_is_own).collect();
        assert_eq!(
            own.last(),
            Some(&false),
            "第 4 条在她那一段以后（upto {upto}）"
        );
    }
}

/// 主线发来的（`line` 是主线，这个群的日志里没有那一轮她说的话）照记回执的序号；晚到的这个群的回执不把它往前拉。
#[test]
fn a_main_line_delivery_counts_from_its_receipt() {
    let mut projection = Projection::new(0);
    let spoken = take_all(
        &mut projection,
        vec![
            said_at(1, 1, 20002),
            started(2, 2, &[1]),
            assistant(3, 2),
            delivered(4, 2, "在。"),
            ended(5, 2),
            said_at(6, 6, 20003),
            delivered(7, 40, "主线发的"),
            started(8, 8, &[6]),
            assistant(9, 8),
            said_at(10, 10, 20002),
            delivered(11, 41, "主线又发的"),
            delivered(12, 8, "晚到的回执"),
            assistant(13, 8),
        ],
    );
    let own: Vec<bool> = spoken.iter().map(|speaking| speaking.last_is_own).collect();
    assert_eq!(
        own,
        [false, true, true],
        "主线那一段在第 6 条以后；第 9 条的回执晚到，主线第 11 条那一段还是在第 10 条以后"
    );
}

#[test]
fn what_was_sent_this_turn() {
    let mut projection = Projection::new(0);
    let spoken = take_all(
        &mut projection,
        vec![
            assistant(1, 1),
            queued(2, 1, "一"),
            queued(3, 1, "二"),
            assistant(4, 1),
            queued(5, 6, "三"),
            queued(6, 1, "晚到的"),
            delivered(7, 6, "只有回执的不算"),
            assistant(8, 6),
            assistant(9, 1),
        ],
    );
    let sent: Vec<Vec<String>> = spoken
        .iter()
        .map(|speaking| speaking.sent.clone())
        .collect();
    assert_eq!(
        sent,
        [
            Vec::<String>::new(),
            vec!["一".to_string(), "二".to_string()],
            vec!["三".to_string()],
            Vec::new(),
        ],
        "照入队的算（O-25 中）；换了回合就清；晚到的上一轮的不算；别的回合的不给"
    );
}

/// 桥入队记成了先算上（O-25 中，「施工时定的」第 112 条）：日志推来的同一段照正文认，不重复算；别的段照加；桥算的别的回合的
/// 换了回合就清。
#[test]
fn the_bridge_counts_first_and_the_log_does_not_count_twice() {
    let mut projection = Projection::new(0);
    projection.queued(2, "一");
    projection.queued(2, "二");
    let spoken = take_all(
        &mut projection,
        vec![
            queued(3, 2, "一"),
            queued(4, 2, "二"),
            queued(5, 2, "三"),
            assistant(6, 2),
        ],
    );
    assert_eq!(spoken[0].sent, ["一", "二", "三"]);
    projection.queued(1, "上一轮晚算的");
    projection.queued(9, "四");
    let spoken = take_all(&mut projection, vec![assistant(10, 9)]);
    assert_eq!(spoken[0].sent, ["四"], "换了回合就清，上一轮的不算");
}
