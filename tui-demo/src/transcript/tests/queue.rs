//! 回合进行中发的话（蓝图 `tui.md`「运行状态行和排队的消息」第 5 条，`kernel/session.md`「排队的消息」第 1 条）：
//! 核心下一次请求就带上它，不等下一轮。哪一次请求带上了，看那次请求推来的字里的 `seen`：够着它的序号，前面那段
//! 时间线收起，这句话进正文，接着的步另起一段。

use super::super::{Kind, Transcript};
use super::apply;
use crate::core::{Block, Push};

/// 一次请求想了一下：先报看到了第几条为止，再是一块思考。
fn thought(seen: u64, text: &str) -> Vec<Push> {
    vec![
        Push::Heard(seen),
        Push::BlockStart {
            index: 0,
            block: Block::Reasoning,
        },
        Push::Delta {
            index: 0,
            text: text.into(),
        },
        Push::BlockEnd(0),
    ]
}

/// 看得见的一条：种类、字；时间线那一段是不是收起了、有几步。
type Shown = (Kind, String, Option<(bool, usize)>);

/// 看得见的几条。
fn shown(t: &Transcript) -> Vec<Shown> {
    t.entries
        .iter()
        .filter(|e| !e.hidden && !e.queued)
        .map(|e| {
            let seg = e.segment.as_ref().map(|s| (s.finished, s.steps.len()));
            (e.kind.clone(), e.text.clone(), seg)
        })
        .collect()
}

/// 第一句开了第 2 轮，想了第一步。
fn started() -> Transcript {
    let mut t = Transcript::default();
    t.user("先跑一下".into(), Vec::new());
    apply(
        &mut t,
        vec![Push::UserMessage(1), Push::TurnStarted(2, Some(1))],
    );
    apply(&mut t, thought(1, "第一步"));
    t
}

#[test]
fn a_message_heard_between_steps_folds_the_steps_and_starts_a_new_segment() {
    // 2026-09-29 项目主人：步与步之间发的话，前面的时间线没收起，那句话也不见了。
    let mut t = started();
    t.user("顺便看看 README".into(), Vec::new());
    apply(&mut t, vec![Push::UserMessage(5)]);
    apply(&mut t, thought(5, "第二步"));
    assert_eq!(
        shown(&t),
        [
            (Kind::User, "先跑一下".to_string(), None),
            (Kind::Steps, String::new(), Some((true, 1))),
            (Kind::User, "顺便看看 README".to_string(), None),
            (Kind::Steps, String::new(), Some((false, 1))),
        ],
        "前面那段收起，这句进正文，接着的步另起一段"
    );
    let heard = t
        .entries
        .iter()
        .find(|e| e.text == "顺便看看 README")
        .unwrap();
    assert_eq!(heard.turn, Some(2), "归到在跑的这一轮");
}

#[test]
fn a_request_sent_before_the_message_leaves_it_queued() {
    let mut t = started();
    t.user("顺便看看 README".into(), Vec::new());
    apply(&mut t, vec![Push::UserMessage(5)]);
    // 这次请求在这句落盘之前就发出去了：看到第 4 条为止。
    apply(&mut t, thought(4, "第二步"));
    assert_eq!(
        shown(&t),
        [
            (Kind::User, "先跑一下".to_string(), None),
            (Kind::Steps, String::new(), Some((false, 2))),
        ],
        "还排着：新的一步接在原来那一段里，不因为排着一句话另起一段"
    );
    assert!(
        t.entries
            .iter()
            .any(|e| e.queued && e.text == "顺便看看 README")
    );
}

#[test]
fn messages_heard_together_join_into_one_block() {
    let mut t = started();
    t.user("第一句补充".into(), Vec::new());
    t.user("第二句补充".into(), Vec::new());
    apply(&mut t, vec![Push::UserMessage(5), Push::UserMessage(6)]);
    apply(&mut t, thought(6, "第二步"));
    let users: Vec<String> = shown(&t)
        .into_iter()
        .filter(|(k, _, _)| *k == Kind::User)
        .map(|(_, text, _)| text)
        .collect();
    assert_eq!(
        users,
        ["先跑一下", "第一句补充\n第二句补充"],
        "一起听到的拼成一段"
    );
}

#[test]
fn what_she_said_before_hearing_it_stays_above_it() {
    // 这句排着时，在路上的那次请求先说了话、调了工具；下一次请求才带上它。
    let mut t = started();
    t.user("补充一句".into(), Vec::new());
    apply(&mut t, vec![Push::UserMessage(5)]);
    let mut pushes = vec![
        Push::Heard(4),
        Push::BlockStart {
            index: 0,
            block: Block::Text,
        },
        Push::Delta {
            index: 0,
            text: "我先看看".into(),
        },
        Push::BlockEnd(0),
        Push::BlockStart {
            index: 1,
            block: Block::ToolCall("shell".into()),
        },
        Push::BlockEnd(1),
        Push::Calls(vec!["c1".into()]),
    ];
    pushes.extend(thought(5, "听到了"));
    apply(&mut t, pushes);
    // 工具的结果晚到：还落在那一步上（拿走排着的那条以后，记着的下标跟着挪）。
    apply(
        &mut t,
        vec![Push::ToolResult {
            call_id: "c1".into(),
            status: crate::core::ToolStatus::Ok,
            text: "a.txt".into(),
            said: None,
        }],
    );
    let kinds: Vec<(Kind, String)> = shown(&t)
        .into_iter()
        .map(|(k, text, _)| (k, text))
        .collect();
    assert_eq!(
        kinds,
        [
            (Kind::User, "先跑一下".to_string()),
            (Kind::Steps, String::new()),
            (Kind::Reply, "我先看看".to_string()),
            (Kind::Steps, String::new()),
            (Kind::User, "补充一句".to_string()),
            (Kind::Steps, String::new()),
        ],
        "听到它以前说的、做的在它上面"
    );
    let shell = t
        .entries
        .iter()
        .filter_map(|e| e.segment.as_ref())
        .nth(1)
        .unwrap();
    assert!(
        matches!(&shell.steps[0].kind, crate::transcript::StepKind::Tool { output, .. } if output == "a.txt"),
        "结果落在调它的那一步"
    );
}
