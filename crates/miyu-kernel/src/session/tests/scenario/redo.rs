//! 场景：重做最后一轮（`docs/blueprint/kernel/history.md`「重做」，施工 4-7 再补）。撤掉最后一轮、原话再发、开新的一轮，
//! 一批落了盘才回应，回应附撤销和重发的几句；换了话的只换开这一轮的那一句，附件照带，换了附件的换掉，一块都不剩的拒绝；排着接过来的几句一起重发；重做过
//! 的再重做、再撤销都带着全部几句；撤掉压缩的先读回；不是人的话开的、没说过话的、有回合在进行的拒绝；重做以后恢复不了；
//! 载入以后一样。
//!
//! 替身的组装一条事件一行（[`Listing`]）：新的一轮的请求和撤掉的那一轮的一字不差，由 `miyu-assemble` 的真组装器比
//! （`crates/miyu-assemble/tests/redo.rs`）。这里比的是除了序号以外一行不差。

use super::compaction::{compacting, line};
use super::reports::{CHILD, dispatched};
use super::*;
use crate::block::{Image, Text};
use crate::event::{ChildReason, MessageUser};
use crate::id::{ContentHash, MediaType};

/// 一段话写成的一块文字。
fn text(words: &str) -> Block {
    Block::Text(Text {
        text: words.to_string(),
    })
}

/// 一张图。
fn image() -> Block {
    Block::Image(Image {
        blob: ContentHash::of(b"png"),
        media_type: MediaType::parse("image/png").unwrap(),
        width: 1000,
        height: 500,
    })
}

/// 第 `n` 条事件。
fn event(s: &Stage, n: u64) -> &Event {
    &s.log()[usize::try_from(n - 1).unwrap()]
}

/// 第 `n` 条人的话的内容块。
fn said(s: &Stage, n: u64) -> &[Block] {
    match &event(s, n).body {
        Body::MessageUser(MessageUser { blocks }) => blocks,
        body => panic!("第 {n} 条应该是 message.user：{body:?}"),
    }
}

/// 第 `n` 条是一轮的开头，交回它的触发。
fn trigger_of(s: &Stage, n: u64) -> Option<Seq> {
    match &event(s, n).body {
        Body::TurnStarted(started) => started.trigger,
        body => panic!("第 {n} 条应该是 turn.started：{body:?}"),
    }
}

/// 第 `k` 次请求去掉序号，一条一行：新的一轮和撤掉的那一轮比。
fn shape(s: &Stage, k: usize) -> Vec<String> {
    listed_request(&s.requests()[k].1)
        .lines()
        .map(|line| {
            line.split_once(' ')
                .map_or(line, |(_, kind)| kind)
                .to_string()
        })
        .collect()
}

/// 被拒绝，原因码是 `reason`。
fn refused(reason: Reason) -> Outcome {
    Outcome::Rejected { reason }
}

#[test]
fn redo_undoes_the_last_turn_and_says_the_same_again() {
    let mut s = stage();
    s.model([
        Line::says("好"),
        Line::says("又好"),
        Line::says("换个说法的好"),
    ]);
    s.say("hi");
    s.say("再来");
    let id = s.redo(None);
    assert_eq!(
        story(&s)[13..],
        [
            "14 turn.reverted:10 alice",
            "15 message.user alice",
            "16 turn.started kernel t16",
            "17 message.assistant model t16",
            "18 model.called:ok kernel t16",
            "19 turn.ended:completed kernel t16",
        ]
    );
    assert_eq!(
        s.outcome(&id),
        Some(&Outcome::Accepted {
            events: seqs(&[14, 15])
        }),
        "回应附撤销和重发的那一句，新的一轮的开头不附"
    );
    // 原话原样再发：内容、谁说的照原来的，原因是重做的命令，空闲时发的不带回合编号；新的一轮由它开、原因也是它。
    assert_eq!(said(&s, 15), said(&s, 9));
    let (resent, opened) = (event(&s, 15), event(&s, 16));
    assert_eq!(
        (&resent.by, &resent.cause, resent.turn),
        (&alice(), &Some(id.clone()), None)
    );
    assert_eq!(trigger_of(&s, 16), Some(seq(15)));
    assert_eq!(opened.cause, Some(id.clone()));
    for n in [14, 15, 16] {
        assert_eq!(event(&s, n).at, event(&s, 14).at, "同一批，时刻一样");
    }
    // 撤掉的那一轮的事实跟着撤了，新的一轮照同样的比法：请求和撤掉的那一轮的除了序号一行不差。
    assert_eq!(shape(&s, 2), shape(&s, 1));
    // 新的一轮开了，撤销恢复不了。
    let restore = s.unrevert();
    assert_eq!(
        s.outcome(&restore),
        Some(&refused(Reason::NothingToUnrevert))
    );
}

#[test]
fn new_words_replace_only_the_opener_and_the_attachments_stay() {
    let mut s = stage();
    s.model([Line::says("看到了"), Line::says("看到了这张")]);
    s.send(vec![text("看看这张图"), image()]);
    let id = s.redo(Some(vec![text("看这张")]));
    assert_eq!(said(&s, 10), [text("看这张"), image()]);
    assert_eq!(trigger_of(&s, 11), Some(seq(10)));
    assert_eq!(
        s.outcome(&id),
        Some(&Outcome::Accepted {
            events: seqs(&[9, 10])
        })
    );
    // 原样再重做一次：改过的那句原样发，附件照带。
    s.model([Line::says("又看了一遍")]);
    s.redo(None);
    assert_eq!(said(&s, 18), [text("看这张"), image()]);
}

/// 另一张图。
fn photo() -> Block {
    Block::Image(Image {
        blob: ContentHash::of(b"jpg"),
        media_type: MediaType::parse("image/jpeg").unwrap(),
        width: 640,
        height: 480,
    })
}

#[test]
fn new_attachments_replace_the_old_ones_and_the_words_may_stay() {
    let mut s = stage();
    s.model([
        Line::says("看到了"),
        Line::says("换了一张"),
        Line::says("没图了"),
        Line::says("只有图"),
    ]);
    s.send(vec![text("看看这张图"), image()]);
    // 只换附件：字照原来的。
    s.redo_with(None, Some(vec![photo()]));
    assert_eq!(said(&s, 10), [text("看看这张图"), photo()]);
    // 不要附件了：只剩字。
    s.redo_with(None, Some(Vec::new()));
    assert_eq!(said(&s, 18), [text("看看这张图")]);
    // 字换成空的、附件换一张：只剩图。
    s.redo_with(Some(Vec::new()), Some(vec![image()]));
    assert_eq!(said(&s, 26), [image()]);
}

#[test]
fn as_it_was_keeps_the_blocks_in_their_order() {
    // 两样都没换的原样重发：图在前、字在后的那一句不重排成字在前。
    let mut s = stage();
    s.model([Line::says("看到了"), Line::says("又看到了")]);
    s.send(vec![image(), text("这是什么")]);
    s.redo(None);
    assert_eq!(said(&s, 10), [image(), text("这是什么")]);
}

#[test]
fn an_empty_replacement_is_refused() {
    let mut s = stage();
    s.model([Line::says("好")]);
    s.say("hi");
    // 原来只有字：字换成空的，一块都不剩。
    let id = s.redo(Some(Vec::new()));
    assert_eq!(s.outcome(&id), Some(&refused(Reason::EmptyMessage)));
    assert_eq!(s.log().len(), 8, "什么都没记");
    // 原来只有图：不要附件，也一块都不剩；只换字的照收。
    let mut s = stage();
    s.model([Line::says("看到了"), Line::says("配了字")]);
    s.send(vec![image()]);
    let id = s.redo_with(None, Some(Vec::new()));
    assert_eq!(s.outcome(&id), Some(&refused(Reason::EmptyMessage)));
    s.redo(Some(vec![text("配一句")]));
    assert_eq!(said(&s, 10), [text("配一句"), image()]);
}

/// 第一轮说到一半停住，人又说了两句排着；放行以后第一轮说完，排着的第二句开了第二轮（11 号），第一句由它接过去。
fn queued_two() -> Stage {
    let mut s = stage();
    s.model([Line::says("好").held(), Line::says("两句都听到了")]);
    s.say("hi");
    s.say("先读 a");
    s.say("再读 b");
    s.release_model();
    assert_eq!(
        story(&s)[5..],
        [
            "6 message.user alice t3",
            "7 message.user alice t3",
            "8 message.assistant model t3",
            "9 model.called:ok kernel t3",
            "10 turn.ended:completed kernel t3",
            "11 turn.started kernel t11",
            "12 message.assistant model t11",
            "13 model.called:ok kernel t11",
            "14 turn.ended:completed kernel t11",
        ]
    );
    assert_eq!(trigger_of(&s, 11), Some(seq(7)));
    s
}

#[test]
fn the_words_taken_over_from_the_turn_before_are_said_again_in_order() {
    let mut s = queued_two();
    s.model([Line::says("换了一句也都听到了")]);
    let id = s.redo(Some(vec![text("再读 c")]));
    assert_eq!(
        story(&s)[14..18],
        [
            "15 turn.reverted:11 alice",
            "16 message.user alice",
            "17 message.user alice",
            "18 turn.started kernel t18",
        ]
    );
    assert_eq!(said(&s, 16), said(&s, 6), "排着接过来的照原样");
    assert_eq!(said(&s, 17), [text("再读 c")], "开这一轮的那一句换掉");
    assert_eq!(trigger_of(&s, 18), Some(seq(17)));
    assert_eq!(
        s.outcome(&id),
        Some(&Outcome::Accepted {
            events: seqs(&[15, 16, 17])
        })
    );
    assert_eq!(shape(&s, 2).len(), shape(&s, 1).len());
}

#[test]
fn a_redone_turn_takes_all_its_words_along_when_redone_or_undone() {
    let mut s = queued_two();
    s.model([Line::says("又听到了两句"), Line::says("第三次")]);
    s.redo(None);
    // 再重做：两句都再发一次，不是只发开了这一轮的那一句。
    let id = s.redo(None);
    assert_eq!(
        story(&s)[21..25],
        [
            "22 turn.reverted:18 alice",
            "23 message.user alice",
            "24 message.user alice",
            "25 turn.started kernel t25",
        ]
    );
    assert_eq!(
        s.outcome(&id),
        Some(&Outcome::Accepted {
            events: seqs(&[22, 23, 24])
        })
    );
    // 撤掉它：重发的两句一起拿走，请求里一句都不剩。
    s.revert(TurnId::new(seq(25)));
    s.model([Line::says("好")]);
    s.say("算了");
    let last = listed_request(&s.requests().last().unwrap().1);
    for gone in ["23 ", "24 ", "25 "] {
        assert!(!last.lines().any(|line| line.starts_with(gone)), "{last}");
    }
    assert!(
        last.lines().any(|line| line == "10 turn.ended"),
        "上一轮照旧：{last}"
    );
}

#[test]
fn only_a_turn_opened_by_a_person_can_be_redone() {
    // 一轮都没有。
    let mut s = stage();
    let id = s.redo(None);
    assert_eq!(s.outcome(&id), Some(&refused(Reason::NotRedoable)));
    // 都撤掉了。
    s.model([Line::says("好")]);
    s.say("hi");
    s.revert(TurnId::new(seq(3)));
    let id = s.redo(None);
    assert_eq!(s.outcome(&id), Some(&refused(Reason::NotRedoable)));
    // 清空单开的那一轮。
    let mut s = stage();
    s.model([Line::says("好")]);
    s.say("hi");
    s.request_clear();
    let id = s.redo(None);
    assert_eq!(s.outcome(&id), Some(&refused(Reason::NotRedoable)));
    assert!(
        s.read_backs().is_empty(),
        "开头在有效历史里，当场判，不读回"
    );
    // 回报叫醒的那一轮。
    let mut s = dispatched(stage());
    s.model([Line::says("看到了。")]);
    s.child_reports(1, CHILD, ChildReason::Done, "CI 红在 macOS。");
    let before = s.log().len();
    let id = s.redo(None);
    assert_eq!(s.outcome(&id), Some(&refused(Reason::NotRedoable)));
    assert_eq!(s.log().len(), before, "什么都没记");
}

#[test]
fn a_turn_resumed_after_a_restart_cannot_be_redone() {
    let mut s = stage();
    s.model([Line::says("说到一半").held(), Line::says("接着说完")]);
    s.say("hi");
    s.restart();
    let resumed = *s.turns().last().unwrap();
    assert_ne!(resumed, TurnId::new(seq(3)), "接着干开了一轮");
    let id = s.redo(None);
    assert_eq!(s.outcome(&id), Some(&refused(Reason::NotRedoable)));
}

#[test]
fn a_running_turn_is_not_redone() {
    let mut s = stage();
    s.model([Line::says("说到一半").held()]);
    s.say("hi");
    let id = s.redo(None);
    assert_eq!(s.outcome(&id), Some(&refused(Reason::TurnRunning)));
}

#[test]
fn a_compaction_in_the_turn_is_read_back_before_saying_it_again() {
    // 第二轮一开头就过线压了（替代到 8）：开头、触发的那句都在检查点后面，当场判得出；撤它要读回。
    let mut s = compacting(None);
    s.model([Line::says("好。")]);
    s.say("hi");
    line(&mut s, 100);
    s.model([Line::says("S1"), Line::says("嗯。")]);
    s.say("再说一句");
    s.model([Line::says("S2"), Line::says("又嗯。")]);
    let id = s.redo(None);
    assert_eq!(
        s.read_backs(),
        [seq(1)],
        "一次都没有还算数的压缩：从第 1 条读回"
    );
    assert_eq!(
        story(&s)[17..21],
        [
            "18 turn.reverted:10 alice",
            "19 message.user alice",
            "20 turn.started kernel t20",
            "21 model.called:ok kernel t20",
        ],
        "读回来记撤销、重发；新的一轮开头照常到线再压"
    );
    assert_eq!(said(&s, 19), said(&s, 9));
    assert_eq!(
        s.outcome(&id),
        Some(&Outcome::Accepted {
            events: seqs(&[18, 19])
        })
    );
}

#[test]
fn a_turn_whose_opener_was_compacted_away_is_judged_after_reading_back() {
    // 第一轮中途压过，替代到 8：开头、触发的那句都压掉了，读回来才判得出是人开的。
    let mut s = compacting(None);
    line(&mut s, 100);
    s.model([
        Line::calls("", &[("read", "{}")]),
        Line::says("S1"),
        Line::says("看完了。").reports(10),
    ]);
    s.tools([Play::Done("lib.rs".to_string())]);
    s.say("看看");
    s.model([Line::says("又看完了。")]);
    let id = s.redo(None);
    assert_eq!(s.read_backs(), [seq(1)]);
    assert_eq!(
        story(&s)[15..18],
        [
            "16 turn.reverted:3 alice",
            "17 message.user alice",
            "18 turn.started kernel t18",
        ]
    );
    assert_eq!(said(&s, 17), said(&s, 2));
    assert_eq!(
        s.outcome(&id),
        Some(&Outcome::Accepted {
            events: seqs(&[16, 17])
        })
    );
}

#[test]
fn a_compaction_turn_is_refused_after_reading_back() {
    // 替身单开的压缩那一轮由上一轮的结尾触发，那一条压掉了：读回来才判得出不是人开的，照样拒绝，什么都不记。
    let mut s = stage();
    s.model([Line::says("好")]);
    s.say("hi");
    s.compact("S1");
    let before = s.log().len();
    let id = s.redo(None);
    assert_eq!(s.read_backs(), [seq(1)]);
    assert_eq!(s.outcome(&id), Some(&refused(Reason::NotRedoable)));
    assert_eq!(s.log().len(), before, "什么都没记");
    // 有效历史没换：接着说，请求照旧从检查点以后算起。
    s.model([Line::says("接着说")]);
    s.say("接着来");
    let last = listed_request(&s.requests().last().unwrap().1);
    assert!(
        last.starts_with("9 turn.started\n11 turn.ended\n"),
        "{last}"
    );
}

#[test]
fn after_a_reload_it_is_redone_the_same() {
    let mut s = queued_two();
    s.model([Line::says("又听到了两句"), Line::says("第三次")]);
    s.redo(None);
    s.crash();
    let id = s.redo(None);
    assert_eq!(
        s.outcome(&id),
        Some(&Outcome::Accepted {
            events: seqs(&[22, 23, 24])
        }),
        "载入以后照样认出重发的两句"
    );
}

#[test]
fn words_said_during_the_turn_are_not_said_again() {
    // 那一轮中途来的话（8）带着那一轮的编号，撤掉的那一轮的第一次请求没听到过它：只重发开这一轮的那一句。
    let mut s = stage();
    s.model([
        Line::calls("我看看。", &[("read", "{}")]),
        Line::says("看完了，b 也看了。"),
        Line::says("重新看完了。"),
    ]);
    s.tools([Play::done("A").held()]);
    s.say("看看");
    let call = s.ran()[0].0;
    s.say("顺便看看 b");
    s.release_tool(call);
    assert_eq!(story(&s)[7], "8 message.user alice t3");
    let id = s.redo(None);
    assert_eq!(
        s.outcome(&id),
        Some(&Outcome::Accepted {
            events: seqs(&[13, 14])
        })
    );
    assert_eq!(said(&s, 14), said(&s, 2));
}

#[test]
fn a_turn_opened_by_another_session_cannot_be_redone() {
    // 子会话：父会话的交代开的那一轮不是人说的话开的；人切进来说的那一轮能重做，重发的 `by` 照原来的（人），不是发重做
    // 命令的一方。
    let child = || Stage::child(policy, environment("~/src/miyu"), at(0), CHILD);
    let mut s = child();
    s.model([Line::says("查完了"), Line::says("好"), Line::says("又好")]);
    s.say("查一下 CI");
    let id = s.redo(None);
    assert_eq!(s.outcome(&id), Some(&refused(Reason::NotRedoable)));
    s.person_says("我来问一句");
    let asked = s.log().len();
    let id = s.redo(None);
    let resent = u64::try_from(asked).unwrap() + 2;
    assert!(
        matches!(s.outcome(&id), Some(Outcome::Accepted { events }) if events.len() == 2),
        "{:?}",
        s.outcome(&id)
    );
    assert_eq!(event(&s, resent).by, alice());
    assert_eq!(said(&s, resent), [text("我来问一句")]);
}
