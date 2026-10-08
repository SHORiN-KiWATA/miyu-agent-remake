//! 空了的通知（施工 C-6，`docs/blueprint/cross-session.md` 第六条，`docs/blueprint/kernel/session.md`「空了的通知」）：
//! 在等的记一条 `peer.idle`、照回报的规矩叫醒她；不在等的拒绝 `unknown_watch`、什么都不记；撤掉订它的那一轮就不等了，
//! 恢复了又等；作废只在到点以后（含正好那一刻）、只记下不叫醒；`gone` 只记下；又订从新的时刻算；「空了」要子代理都报完、
//! 后台命令不算；通知带的那一行怎么截；它开的那一轮重做不了；还能恢复撤销时记在一边、载入以后恢复了接着开。

use super::reports::{CHILD, dispatched, event, last_request, trigger_of};
use super::upward::last_turn;
use super::*;
use crate::event::{ChildReason, EndReason, IdleReason, PeerIdle};

/// 被等的会话：短编号 `899aa000`。
const PEER: &str = "0192f3a0-2222-7abc-8def-5566899aa000";

/// 一小时多少毫秒。
const HOUR: i64 = 3_600_000;

/// 第一轮（3 号）订了 `PEER`，结果是 8 号，说完了，到 11 号。
fn watching() -> Stage {
    let mut s = stage();
    s.model([
        Line::calls("订了。", &[("read", "{}")]),
        Line::says("等它。"),
    ]);
    s.tools([Play::watches(PEER)]);
    s.say("让它跑完告诉我");
    assert_eq!(s.log().len(), 11, "{:#?}", story(&s));
    assert!(matches!(event(&s, 8).body, Body::ToolResult(_)));
    s
}

/// 第 `n` 条是 `peer.idle`，交回它。
fn idle_at(s: &Stage, n: u64) -> &PeerIdle {
    match &event(s, n).body {
        Body::PeerIdle(idle) => idle,
        body => panic!("第 {n} 条应该是 peer.idle：{body:?}，{:#?}", story(s)),
    }
}

/// 订的那条结果的时刻。
fn since(s: &Stage) -> i64 {
    event(s, 8).at.unix_millis()
}

/// 毫秒数的时刻。
fn ms(n: i64) -> Timestamp {
    Timestamp::from_unix_millis(n).unwrap()
}

fn refused() -> Option<&'static Outcome> {
    static REFUSED: Outcome = Outcome::Rejected {
        reason: Reason::UnknownWatch,
        about: Vec::new(),
    };
    Some(&REFUSED)
}

#[test]
fn a_notice_while_idle_is_recorded_and_opens_a_turn() {
    let mut s = watching();
    assert_eq!(s.watching().len(), 1, "订了就在等");
    s.model([Line::says("它跑完了。")]);
    let id = s.peer_idle(PEER, Some("测试全过了。"));
    assert_eq!(
        story(&s)[11..13],
        ["12 peer.idle child", "13 turn.started kernel t13"],
        "不带回合编号，由它开一轮"
    );
    let idle = idle_at(&s, 12);
    assert_eq!(idle.reason, IdleReason::Idle);
    assert_eq!(idle.status.as_deref(), Some("测试全过了。"));
    assert_eq!(idle.session.as_str(), PEER);
    assert_eq!(event(&s, 12).cause, Some(id.clone()));
    assert_eq!(trigger_of(&s, 13), Some(seq(12)));
    assert_eq!(event(&s, 13).cause, Some(id.clone()), "cause 照它的");
    assert_eq!(
        s.outcome(&id),
        Some(&Outcome::Accepted {
            events: seqs(&[12])
        })
    );
    assert!(last_request(&s).contains("12 peer.idle\n"));
    assert!(s.watching().is_empty(), "等到了就不等了");
    let again = s.peer_idle(PEER, None);
    assert_eq!(s.outcome(&again), refused(), "已经收到过的再来拒绝");
}

#[test]
fn a_notice_while_busy_is_heard_at_the_next_step() {
    let mut s = watching();
    s.model([
        Line::calls("我读一下。", &[("read", r#"{"path":"a"}"#)]),
        Line::says("好了。"),
    ]);
    s.tools([Play::done("A").held()]);
    s.say("看看 a");
    let running = s.ran().last().unwrap().0;
    s.peer_idle(PEER, None);
    s.release_tool(running);
    let notice = s
        .log()
        .iter()
        .find(|event| matches!(event.body, Body::PeerIdle(_)))
        .expect("记下了");
    assert_eq!(notice.turn, None, "不带回合编号");
    let request = last_request(&s);
    assert!(
        request.contains(&format!("{} peer.idle\n", notice.seq)),
        "下一步听到了：{request}"
    );
    assert_eq!(s.turns().len(), 2, "结束时不再开");
}

#[test]
fn a_notice_nobody_waits_for_is_refused_and_records_nothing() {
    let mut s = stage();
    s.model([Line::says("好。")]);
    s.say("hi");
    let before = s.log().len();
    let id = s.peer_idle(PEER, Some("做完了"));
    assert_eq!(s.outcome(&id), refused(), "没订过的拒绝");
    let mut s = watching();
    let other = s.peer_idle("0192f3a0-2222-7abc-8def-0c5d77aa0c5d", None);
    assert_eq!(s.outcome(&other), refused(), "订的不是它");
    assert_eq!(s.log().len(), 11, "什么都不记");
    assert_eq!(before, 8);
}

#[test]
fn undoing_the_watching_turn_stops_the_wait_and_restoring_it_brings_it_back() {
    let mut s = watching();
    let first = s.watching();
    s.revert(TurnId::new(seq(3)));
    assert!(s.watching().is_empty(), "撤掉订它的那一轮就不等了");
    let id = s.peer_idle(PEER, None);
    assert_eq!(s.outcome(&id), refused());
    s.unrevert();
    assert_eq!(s.watching(), first, "恢复了照原来的时刻又等");
}

#[test]
fn expiry_counts_from_the_watch_and_only_records() {
    let mut s = watching();
    let due = since(&s) + 12 * HOUR;
    s.watch_ends_at(PEER, IdleReason::Expired, ms(due - 1));
    assert_eq!(s.log().len(), 11, "还没到点：不理");
    s.watch_ends_at(PEER, IdleReason::Expired, ms(due));
    let idle = idle_at(&s, 12);
    assert_eq!(idle.reason, IdleReason::Expired);
    assert_eq!(idle.status, None);
    let notice = event(&s, 12);
    assert_eq!(notice.by, By::Kernel, "作废是内核记的");
    assert_eq!(notice.at, ms(due), "正好那一刻也算");
    assert_eq!(notice.turn, None);
    assert_eq!(notice.cause, event(&s, 3).cause, "cause 是订它的那一轮的");
    assert_eq!(s.log().len(), 12, "只记下，不叫醒");
    assert!(s.watching().is_empty());
    let late = s.peer_idle(PEER, Some("才做完"));
    assert_eq!(s.outcome(&late), refused(), "作废了的不再收");
}

#[test]
fn a_session_that_is_gone_is_only_recorded() {
    let mut s = watching();
    s.watch_ends(PEER, IdleReason::Gone);
    let idle = idle_at(&s, 12);
    assert_eq!(idle.reason, IdleReason::Gone);
    assert_eq!(event(&s, 12).by, By::Kernel);
    assert_eq!(s.log().len(), 12, "只记下，不叫醒");
    assert!(s.watching().is_empty());
    s.watch_ends(PEER, IdleReason::Gone);
    assert_eq!(s.log().len(), 12, "不在等了的不理");
}

#[test]
fn an_idle_reason_from_the_executor_is_ignored() {
    let mut s = watching();
    s.watch_ends(PEER, IdleReason::Idle);
    assert_eq!(s.log().len(), 11, "执行器只交作废、不在了");
    assert_eq!(s.watching().len(), 1);
}

#[test]
fn watching_again_restarts_the_clock() {
    let mut s = watching();
    let first = since(&s);
    s.model([
        Line::calls("再订一次。", &[("read", "{}")]),
        Line::says("好。"),
    ]);
    s.tools([Play::watches(PEER)]);
    s.advance(60);
    s.say("再订一次");
    let [(_, again)] = s.watching().try_into().unwrap();
    assert!(again.unix_millis() > first + 59 * 60_000, "从新的时刻算");
    s.watch_ends_at(PEER, IdleReason::Expired, ms(first + 12 * HOUR));
    assert!(
        !s.log()
            .iter()
            .any(|event| matches!(event.body, Body::PeerIdle(_))),
        "旧的计时到了不算"
    );
    s.watch_ends_at(
        PEER,
        IdleReason::Expired,
        ms(again.unix_millis() + 12 * HOUR),
    );
    assert!(s.watching().is_empty(), "新的到了点才作废");
}

#[test]
fn vacant_waits_for_the_subagents_but_not_for_commands() {
    let mut s = dispatched(stage());
    assert!(!s.vacant(), "子代理还没报");
    s.model([Line::says("看到了。")]);
    s.child_reports(1, CHILD, ChildReason::Done, "CI 修好了。");
    assert_eq!(s.turns().len(), 2, "回报叫醒的那一轮也做完了");
    assert!(s.vacant(), "后台命令还在跑也算空");
    s.model([Line::says("好").held()]);
    s.say("再查");
    assert!(!s.vacant(), "有回合在进行不算空");
}

#[test]
fn a_session_about_to_restart_is_not_vacant() {
    let mut s = stage();
    s.model([Line::says("好").held()]);
    s.say("跑测试");
    s.restarting();
    assert!(s.log().iter().any(|event| matches!(&event.body, Body::TurnEnded(ended) if ended.reason == EndReason::Restarted)));
    assert!(!s.vacant(), "被重启打断的那一轮再起来接着干，这时不算空");
}

#[test]
fn the_last_line_is_the_first_line_of_the_last_words() {
    let mut s = stage();
    assert_eq!(s.last_line(), None, "一轮都没有");
    s.model([Line::says("  \n  CI 修好了。  \n细节在下面。")]);
    s.say("hi");
    assert_eq!(s.last_line().as_deref(), Some("CI 修好了。"));
    s.model([
        Line::calls("我先读一下。", &[("read", r#"{"path":"a"}"#)]),
        Line::says("").thinking("只想了"),
    ]);
    s.tools([Play::done("A")]);
    s.say("看看");
    assert_eq!(
        s.last_line().as_deref(),
        Some("我先读一下。"),
        "最后一条有字的回复"
    );
    s.model([Line::says("").thinking("只想了")]);
    s.say("嗯");
    assert_eq!(s.last_line(), None, "那一轮一个字都没说");
    let exact = "字".repeat(200);
    s.model([Line::says(&exact)]);
    s.say("长一点");
    assert_eq!(s.last_line(), Some(exact), "正好 200 个字不截");
    s.model([Line::says(&"长".repeat(201))]);
    s.say("再长一点");
    assert_eq!(
        s.last_line(),
        Some(format!("{}…", "长".repeat(200))),
        "超了的截到 200 个字、接 …"
    );
}

#[test]
fn the_turn_a_notice_opened_cannot_be_redone() {
    let mut s = watching();
    s.model([Line::says("它跑完了。")]);
    s.peer_idle(PEER, None);
    let redo = s.redo(None);
    assert_eq!(
        s.outcome(&redo),
        Some(&Outcome::Rejected {
            reason: Reason::NotRedoable,
            about: Vec::new(),
        }),
        "不是人说的话开的"
    );
    s.revert(last_turn(&s));
    s.model([Line::says("看到了。")]);
    s.say("刚才那个会话怎么样了");
    assert!(
        last_request(&s).contains("12 peer.idle\n"),
        "别处来的，撤销不带走：{:#?}",
        story(&s)
    );
}

#[test]
fn while_an_undo_can_be_restored_a_notice_waits_even_across_a_reload() {
    let mut s = watching();
    s.model([Line::says("嗯。")]);
    s.say("再来");
    s.revert(TurnId::new(seq(13)));
    s.peer_idle(PEER, Some("好了"));
    assert_eq!(s.turns().len(), 2, "还能恢复撤销：只记下");
    s.crash();
    assert_eq!(s.turns().len(), 2, "载入不因为没听到的通知开轮");
    s.model([Line::says("看到了。")]);
    s.unrevert();
    assert_eq!(s.turns().len(), 3, "记在一边的照日志算回来，恢复了接着开");
    let notice = s
        .log()
        .iter()
        .find(|event| matches!(event.body, Body::PeerIdle(_)))
        .unwrap()
        .seq;
    assert!(last_request(&s).contains(&format!("{notice} peer.idle\n")));
}
