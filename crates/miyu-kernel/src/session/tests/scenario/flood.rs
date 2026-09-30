//! 别的会话发来的话防刷屏（施工 C-2，`docs/blueprint/cross-session.md` 第五条第 1 到 3 款、第 5 款）：同一个发话方 600 秒
//! 里第 6 句拒（含正好 600 秒前的那一刻），窗口过了又收；一字不差的拒，不占限速的数；没听到的第 51 句拒，听到以后又收；
//! 被拒的什么都不记；重启以后数照日志算回来；只管别的会话发来的，父子之间的留言不受「5 句」管。
//!
//! 大多用没人看着的一次性会话：话只记下、不开轮，数只看防刷屏。

use super::peers::PEER;
use super::reports::{CHILD, dispatched};
use super::*;

/// 另一个发话方。
const OTHER: &str = "0192f3a0-2222-7abc-8def-5566778899aa";

/// 没人看着的一次性会话。
fn unwatched() -> Stage {
    Stage::oneshot(policy, environment("~/src/miyu"), at(0))
}

/// 这个命令被拒了，原因是 `reason`。
fn refused(s: &Stage, id: &CommandId, reason: Reason) {
    assert_eq!(s.outcome(id), Some(&Outcome::Rejected { reason }), "{id}");
}

/// 这个命令收下了。
fn taken(s: &Stage, id: &CommandId) {
    assert!(
        matches!(s.outcome(id), Some(Outcome::Accepted { .. })),
        "{id}：{:?}",
        s.outcome(id)
    );
}

/// `from` 连着说 `n` 句不一样的，交回第一句的时刻。
fn burst(s: &mut Stage, from: &str, n: usize) -> Timestamp {
    for k in 0..n {
        let id = s.peer_says(from, &format!("第 {k} 句"));
        taken(s, &id);
    }
    s.log()
        .iter()
        .find(|event| matches!(&event.by, By::Session(session) if session.id.as_str() == from))
        .map(|event| event.at)
        .expect("说过")
}

/// `start` 往后 `ms` 毫秒。
fn after(start: Timestamp, ms: i64) -> Timestamp {
    Timestamp::from_unix_millis(start.unix_millis() + ms).unwrap()
}

#[test]
fn the_sixth_message_in_the_window_is_refused_and_nothing_is_recorded() {
    let mut s = unwatched();
    burst(&mut s, PEER, 5);
    let before = s.log().len();
    let sixth = s.peer_says(PEER, "第 5 句");
    refused(&s, &sixth, Reason::TooManyMessages);
    assert_eq!(s.log().len(), before, "被拒的什么都不记");
    let other = s.peer_says(OTHER, "我这边也好了。");
    taken(&s, &other);
}

#[test]
fn the_window_counts_the_very_moment_six_hundred_seconds_back() {
    let mut s = unwatched();
    let first = burst(&mut s, PEER, 5);
    let edge = s.peer_says_at(PEER, "正好 600 秒", after(first, 600_000));
    refused(&s, &edge, Reason::TooManyMessages);
    let past = s.peer_says_at(PEER, "过了 600 秒", after(first, 600_001));
    taken(&s, &past);
}

#[test]
fn an_identical_message_is_refused_and_takes_no_slot() {
    let mut s = unwatched();
    let first = s.peer_says(PEER, "迁移写完了。");
    taken(&s, &first);
    let same = s.peer_says(PEER, "迁移写完了。");
    refused(&s, &same, Reason::DuplicateMessage);
    burst(&mut s, PEER, 4);
    let sixth = s.peer_says(PEER, "迁移写完了。");
    refused(&s, &sixth, Reason::DuplicateMessage);
    let other = s.peer_says(OTHER, "迁移写完了。");
    taken(&s, &other);
}

#[test]
fn an_identical_message_is_taken_again_after_the_window() {
    let mut s = unwatched();
    let first = s.peer_says(PEER, "迁移写完了。");
    let at = s.log().last().unwrap().at;
    taken(&s, &first);
    let edge = s.peer_says_at(PEER, "迁移写完了。", after(at, 600_000));
    refused(&s, &edge, Reason::DuplicateMessage);
    let past = s.peer_says_at(PEER, "迁移写完了。", after(at, 600_001));
    taken(&s, &past);
}

#[test]
fn the_fifty_first_unheard_message_is_refused_until_she_hears_them() {
    let mut s = unwatched();
    for k in 0..10u64 {
        burst(
            &mut s,
            &format!("0192f3a0-3333-7abc-8def-0000000000{k:02}"),
            5,
        );
    }
    let full = s.peer_says(OTHER, "还有一句。");
    refused(&s, &full, Reason::InboxFull);
    s.watched(true);
    s.model([Line::says("看到了。"), Line::says("又一句。")]);
    s.say("刚才谁说话了");
    let heard = s.peer_says(OTHER, "还有一句。");
    taken(&s, &heard);
}

#[test]
fn the_counts_come_back_from_the_log_after_a_restart() {
    let mut s = unwatched();
    burst(&mut s, PEER, 5);
    s.crash();
    let sixth = s.peer_says(PEER, "第 5 句");
    refused(&s, &sixth, Reason::TooManyMessages);
    let same = s.peer_says(PEER, "第 0 句");
    refused(&s, &same, Reason::DuplicateMessage);
}

#[test]
fn the_unheard_count_comes_back_from_the_log_after_a_restart() {
    let mut s = unwatched();
    for k in 0..10u64 {
        burst(
            &mut s,
            &format!("0192f3a0-3333-7abc-8def-0000000000{k:02}"),
            5,
        );
    }
    s.crash();
    let full = s.peer_says(OTHER, "还有一句。");
    refused(&s, &full, Reason::InboxFull);
}

#[test]
fn a_subagent_is_not_held_to_five_messages() {
    let mut s = dispatched(stage());
    s.model([Line::says("好").held(), Line::says("收到。")]);
    s.say("接着查");
    for k in 0..7 {
        let id = s.child_says(CHILD, &format!("进度 {k}"));
        taken(&s, &id);
    }
    let same = s.child_says(CHILD, "进度 0");
    taken(&s, &same);
}
