//! 撤销能撤掉压缩（`docs/blueprint/kernel/history.md`「撤掉压缩」，施工 6-9）：撤的几轮里有还算数的压缩的，先读回
//! 日志，读回的时候拒绝命令、不算空闲；读回的对不上的不理；读回来了照它重建有效历史，回到前一个检查点，一次都没有的
//! 从头；改回的文件照读回的那一段算。恢复不读磁盘，压缩跟着回来；一次撤掉几次压缩；载入时认出哪次还算数。
//!
//! 用执行器替身跑的，压缩是替身单开的一轮（[`Stage::compact`]）：替代到原来的最后一条。

use super::super::load::Logged;
use super::super::load::load;
use super::super::restore::{changed, edited, steps_of};
use super::*;
use crate::event::{EndReason, RestoredFile, TurnEnded, TurnStarted};
use crate::id::ContentHash;
use crate::testkit::{Line, Stage};

/// 一个替身，照 [`policy`] 造的会话，在 `~/src/miyu`。
fn stage() -> Stage {
    Stage::new(policy, environment("~/src/miyu"), at(0))
}

/// 说完两轮（3 号、10 号），替身单开一轮压缩（14 号，压到 13），再说一轮（18 号）。
fn compacted_once() -> Stage {
    let mut s = stage();
    s.model([Line::says("好"), Line::says("又好"), Line::says("接着好")]);
    s.say("hi");
    s.say("再来");
    s.compact("S1");
    s.say("接着来");
    assert_eq!(s.turns(), turns(&[3, 10, 14, 18]));
    s
}

/// 最近一次请求，替身的组装一条一行。
fn last_request(s: &Stage) -> String {
    listed_request(&s.requests().last().unwrap().1)
}

/// 日志里第 `n` 条。
fn at_seq(s: &Stage, n: u64) -> &Event {
    &s.log()[usize::try_from(n - 1).unwrap()]
}

#[test]
fn undoing_the_compaction_turn_reads_back_and_the_request_is_as_before() {
    let mut s = compacted_once();
    let before = listed_request(&s.requests()[1].1);
    let id = s.revert(TurnId::new(seq(14)));
    assert_eq!(
        s.read_backs(),
        [seq(1)],
        "一次都没有还算数的压缩：从第 1 条读回"
    );
    let reverted = s.log().last().unwrap();
    assert_eq!(
        reverted.body,
        Body::TurnReverted(TurnReverted {
            turns: turns(&[14, 18])
        })
    );
    assert_eq!(
        s.outcome(&id),
        Some(&Outcome::Accepted {
            events: vec![reverted.seq]
        })
    );
    assert!(s.recalls().is_empty(), "没有检查点，不取回原文");
    s.model([Line::says("好")]);
    s.say("再说");
    let after = last_request(&s);
    assert!(
        after.starts_with(&before),
        "请求回到压缩前的样子，接着那时的往下长：\n{before}\n{after}"
    );
    assert!(!after.contains("context.compacted"), "{after}");
    assert!(after.lines().any(|line| line == "13 turn.ended"), "{after}");
}

#[test]
fn undoing_a_turn_before_the_compaction_takes_it_along() {
    let mut s = compacted_once();
    s.revert(TurnId::new(seq(3)));
    assert_eq!(s.read_backs(), [seq(1)]);
    assert_eq!(
        s.log().last().unwrap().body,
        Body::TurnReverted(TurnReverted {
            turns: turns(&[3, 10, 14, 18])
        })
    );
    s.model([Line::says("好")]);
    s.say("从头来");
    let after = last_request(&s);
    assert!(after.starts_with("1 session.created\n"), "{after}");
    for gone in ["2 message.user", "3 turn.started", "9 message.user"] {
        assert!(
            !after.lines().any(|line| line == gone),
            "撤掉的不在请求里：{after}"
        );
    }
}

#[test]
fn a_turn_after_the_compaction_is_undone_without_reading_back() {
    let mut s = compacted_once();
    s.revert(TurnId::new(seq(18)));
    assert!(s.read_backs().is_empty(), "撤不到压缩的，照以前在内存里撤");
    s.model([Line::says("好")]);
    s.say("换个说法");
    assert!(
        !last_request(&s)
            .lines()
            .any(|line| line == "2 message.user")
    );
}

#[test]
fn restoring_brings_the_compaction_back_without_reading_or_asking_the_model() {
    let mut s = compacted_once();
    let before = last_request(&s);
    let requests = s.requests().len();
    s.revert(TurnId::new(seq(14)));
    s.unrevert();
    assert_eq!(s.read_backs().len(), 1, "恢复不读磁盘");
    assert_eq!(s.requests().len(), requests, "恢复不请求模型");
    s.model([Line::says("好")]);
    s.say("接着说");
    let after = last_request(&s);
    assert!(
        after.starts_with(&before),
        "恢复以后和撤之前一样：\n{after}"
    );
    assert!(
        !after.lines().any(|line| line == "2 message.user"),
        "压缩跟着回来了：{after}"
    );
    assert_eq!(s.model_calls().last().unwrap().first_difference, None);
}

#[test]
fn undoing_back_to_the_first_of_two_compactions_reads_from_after_it() {
    let mut s = compacted_once();
    let before = last_request(&s);
    s.compact("S2");
    let second = *s.turns().last().unwrap();
    s.revert(second);
    assert_eq!(
        s.read_backs(),
        [seq(14)],
        "前一次还算数的压缩替代到 13，从 14 读回"
    );
    s.model([Line::says("好")]);
    s.say("接着说");
    let after = last_request(&s);
    assert!(after.starts_with(&before), "回到第一次压缩：\n{after}");
    assert!(
        !after.lines().any(|line| line == "2 message.user"),
        "{after}"
    );
}

#[test]
fn one_undo_takes_two_compactions_and_restoring_brings_both_back() {
    let mut s = compacted_once();
    s.compact("S2");
    s.model([Line::says("好")]);
    s.say("第五轮");
    let before = last_request(&s);
    s.revert(TurnId::new(seq(10)));
    assert_eq!(s.read_backs(), [seq(1)]);
    assert_eq!(
        s.log().last().unwrap().body,
        Body::TurnReverted(TurnReverted {
            turns: s.turns().into_iter().skip(1).collect()
        })
    );
    s.unrevert();
    s.model([Line::says("好")]);
    s.say("接着");
    assert!(last_request(&s).starts_with(&before));
}

#[test]
fn after_a_reload_the_compaction_that_counts_is_known_and_can_be_restored() {
    let mut s = compacted_once();
    let before = last_request(&s);
    s.revert(TurnId::new(seq(14)));
    s.crash();
    let requests = s.requests().len();
    s.unrevert();
    assert_eq!(s.requests().len(), requests);
    s.model([Line::says("好")]);
    s.say("接着说");
    assert!(
        last_request(&s).starts_with(&before),
        "载入以后照样能恢复撤掉的压缩"
    );
    // 撤掉以后崩了：载入认得压缩不算了，从头。
    let mut s = compacted_once();
    s.revert(TurnId::new(seq(14)));
    s.crash();
    s.model([Line::says("好")]);
    s.say("再说");
    assert!(last_request(&s).starts_with("1 session.created\n2 message.user\n"));
}

/// 在一段日志后面追加替身单开的一轮压缩，替代到原来的最后一条：`turn.started`、`context.compacted`、`turn.ended`。
fn compact(log: &mut Vec<Event>) {
    compact_rereading(log, Vec::new());
}

/// 同 [`compact`]，检查点里重读过这几个文件。
fn compact_rereading(log: &mut Vec<Event>, restored: Vec<RestoredFile>) {
    let upto = log.last().unwrap().seq;
    let turn = TurnId::new(upto.next());
    let event = |seq: Seq, body: Body| Event {
        seq,
        at: at(54),
        turn: Some(turn),
        by: By::Kernel,
        cause: None,
        body,
    };
    log.push(event(
        turn.started(),
        Body::TurnStarted(TurnStarted {
            trigger: Some(upto),
            cwd: None,
            dirs: Vec::new(),
        }),
    ));
    log.push(event(
        upto.next().next(),
        Body::ContextCompacted(ContextCompacted {
            upto,
            summary: "S".to_string(),
            trigger: None,
            instructions: None,
            notes: String::new(),
            restored,
            refills: None,
        }),
    ));
    log.push(event(
        upto.next().next().next(),
        Body::TurnEnded(TurnEnded {
            reason: EndReason::Completed,
        }),
    ));
}

/// 读回第 `from` 条起的这几条。
fn read_back(from: u64, events: Vec<Event>) -> Input {
    Input::ReadBack {
        at: at(58),
        from: seq(from),
        events,
    }
}

#[test]
fn while_reading_back_commands_are_refused_and_what_does_not_fit_is_ignored() {
    let (logged, _) = edited();
    let mut log = logged.log;
    compact(&mut log);
    let (mut session, _) = load(log.clone());
    let actions = session.handle(revert(20, 3));
    assert_eq!(actions, [Action::ReadBack { from: seq(1) }]);
    assert!(!session.idle(), "读回的时候不算空闲");
    assert_eq!(
        session.handle(send(21, "等等")),
        [rejected(id(21), Reason::Restoring)]
    );
    assert_eq!(
        session.handle(revert(20, 3)),
        [rejected(id(20), Reason::Restoring)],
        "撤销自己的编号这时还没记下"
    );
    let mut short = log.clone();
    short.pop();
    assert_eq!(
        session.handle(read_back(1, short)),
        [],
        "没连到最后一条的不理"
    );
    assert_eq!(
        session.handle(read_back(2, log[1..].to_vec())),
        [],
        "起点不对的不理"
    );
    let mut gap = log.clone();
    gap.remove(4);
    gap.push(log.last().unwrap().clone());
    assert_eq!(session.handle(read_back(1, gap)), [], "中间断了的不理");
    // 对得上的：撤掉回合 3 和压缩那一轮，改过的文件照读回的那一段改回。
    let actions = session.handle(read_back(1, log.clone()));
    let events = appended_events(&actions);
    assert_eq!(
        events.iter().map(|event| &event.body).collect::<Vec<_>>(),
        [&Body::TurnReverted(TurnReverted {
            turns: turns(&[3, logged_compaction_turn(&log)])
        })]
    );
    assert_eq!(events[0].at, at(58), "记在读回来的那一刻");
    assert_eq!(steps_of(&actions).len(), 1, "回合 3 改过的那个文件改回去");
    assert_eq!(
        session.handle(read_back(1, log)),
        [],
        "读回过了，再来的是过时的"
    );
}

/// 日志里最后一轮（替身单开的压缩那一轮）的编号。
fn logged_compaction_turn(log: &[Event]) -> u64 {
    log.iter()
        .rev()
        .find(|event| matches!(event.body, Body::TurnStarted(_)))
        .unwrap()
        .seq
        .get()
}

#[test]
fn a_compaction_without_its_turn_does_not_load() {
    let s = compacted_once();
    let mut log = s.log().to_vec();
    let compaction = at_seq(&s, 15).clone();
    assert!(matches!(compaction.body, Body::ContextCompacted(_)));
    log[14].turn = None;
    let error = Session::load(log, at(55), policy(), environment("~/src/miyu")).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("context.compacted happens only in a turn"),
        "{error}"
    );
}

/// 撤掉压缩回到的检查点重读过文件、撤掉的那一轮又改过文件：先记撤销，紧跟着取回原文，再改回文件。
#[test]
fn back_to_a_checkpoint_with_files_the_recall_comes_right_after_the_undo() {
    let (logged, _) = edited();
    let mut log = logged.log;
    let blob = ContentHash::of(b"fn a() {}\n");
    compact_rereading(
        &mut log,
        vec![RestoredFile {
            path: "a.rs".to_string(),
            blob: blob.clone(),
            tokens: 3,
        }],
    );
    // 压完再改一次文件，再单开一轮压一次。
    let (session, _) = load(log.clone());
    let mut logged = Logged { session, log };
    let seen = logged.ask(30, "再改一下");
    logged.tools(seen, &[("write", r#"{"file_path":"a.txt"}"#)]);
    let reply = logged
        .log
        .iter()
        .rev()
        .find(|event| matches!(event.body, Body::MessageAssistant(_)))
        .unwrap()
        .seq;
    let actions = logged.handle(changed(call(reply.get(), 1)));
    assert_eq!(appended(&actions).len(), 1, "改了文件：{actions:?}");
    let (seen, _) = calls(&logged.handle(stored(logged.last()))).remove(0);
    logged.say(seen.get(), "改好了");
    let turn = logged
        .log
        .iter()
        .rev()
        .find(|event| matches!(event.body, Body::TurnStarted(_)))
        .unwrap()
        .seq
        .get();
    let mut log = logged.log;
    compact(&mut log);
    let (mut session, first) = load(log.clone());
    assert!(
        first.is_empty(),
        "最后一次压缩没重读过文件，载入不取：{first:?}"
    );
    let from = match session.handle(revert(40, turn)).as_slice() {
        [Action::ReadBack { from }] => *from,
        other => panic!("先读回：{other:?}"),
    };
    let events: Vec<Event> = log.into_iter().filter(|event| event.seq >= from).collect();
    let actions = session.handle(Input::ReadBack {
        at: at(58),
        from,
        events,
    });
    assert!(
        matches!(
            actions.as_slice(),
            [Action::Append(_), Action::Recall { blobs }, Action::Restore { .. }] if *blobs == [blob.clone()]
        ),
        "{actions:?}"
    );
}
