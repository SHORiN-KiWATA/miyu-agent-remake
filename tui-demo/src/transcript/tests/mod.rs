//! 会话状态的测试。

use super::{Kind, StepKind, Tally, ToolState, Transcript};
use crate::config::Config;
use crate::core::{Block, Push, Update};

mod beat;
mod cache;
mod compaction;
mod done;
mod folds;
mod queue;
mod waiting;

fn apply(t: &mut Transcript, pushes: Vec<Push>) {
    let texts = Config::builtin().unwrap().text;
    for p in pushes {
        t.update(Update::Push(p), &texts);
    }
}

#[test]
fn thinking_goes_into_the_timeline_and_speaking_folds_it() {
    let mut t = Transcript::default();
    apply(
        &mut t,
        vec![
            Push::TurnStarted(1, None),
            Push::BlockStart {
                index: 0,
                block: Block::Reasoning,
            },
            Push::Delta {
                index: 0,
                text: "想".into(),
            },
            Push::BlockEnd(0),
        ],
    );
    let segment = t.entries[0].segment.as_ref().unwrap();
    assert!(segment.expanded(true), "进行中的那一段展开着");
    assert!(!t.busy(), "想完就停表");
    apply(
        &mut t,
        vec![
            Push::BlockStart {
                index: 1,
                block: Block::Text,
            },
            Push::Delta {
                index: 1,
                text: "你好".into(),
            },
            Push::TurnEnded(crate::core::EndReason::Completed),
        ],
    );
    let kinds: Vec<_> = t.entries.iter().map(|e| e.kind.clone()).collect();
    assert_eq!(kinds, vec![Kind::Steps, Kind::Reply, Kind::Done]);
    assert_eq!(t.entries[1].text, "你好");
    let segment = t.entries[0].segment.as_ref().unwrap();
    assert!(!segment.expanded(true), "开口说话就收起");
    match &segment.steps[0].kind {
        StepKind::Thought { text } => assert_eq!(text, "想"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_tool_goes_from_preparing_to_its_result() {
    let mut t = Transcript::default();
    let tool = |t: &Transcript| t.entries[0].segment.as_ref().unwrap().steps[0].clone();
    apply(
        &mut t,
        vec![
            Push::TurnStarted(1, None),
            Push::BlockStart {
                index: 0,
                block: Block::ToolCall("shell".into()),
            },
            Push::Delta {
                index: 0,
                text: "{\"command\":\"ls\",".into(),
            },
        ],
    );
    assert!(matches!(
        tool(&t).kind,
        StepKind::Tool {
            state: ToolState::Preparing,
            ..
        }
    ));
    apply(
        &mut t,
        vec![
            Push::Delta {
                index: 0,
                text: "\"description\":\"列目录\"}".into(),
            },
            Push::BlockEnd(0),
            Push::Calls(vec!["c1".into()]),
        ],
    );
    assert_eq!(tool(&t).arg("description"), Some("列目录"));
    assert!(tool(&t).busy());
    apply(
        &mut t,
        vec![Push::ToolResult {
            call_id: "c1".into(),
            status: crate::core::ToolStatus::Error,
            text: "boom".into(),
            said: None,
        }],
    );
    assert!(tool(&t).failed() && !tool(&t).busy());
}

#[test]
fn the_tally_names_a_lone_command_by_its_title() {
    let mut t = Transcript::default();
    apply(
        &mut t,
        vec![
            Push::TurnStarted(1, None),
            Push::BlockStart {
                index: 0,
                block: Block::ToolCall("shell".into()),
            },
            Push::Delta {
                index: 0,
                text: "{\"command\":\"ls\",\"description\":\"列目录\"}".into(),
            },
            Push::BlockEnd(0),
        ],
    );
    let kind_of = |name: &str| (name == "shell").then_some(crate::config::ToolKind::Command);
    let tally = Tally::count(t.entries[0].segment.as_ref().unwrap(), kind_of);
    assert_eq!(tally.only_command.as_deref(), Some("列目录"));
    assert_eq!((tally.commands, tally.thoughts), (1, 0));
}

#[test]
fn thoughts_do_not_stop_a_lone_command_from_naming_the_segment() {
    let mut t = Transcript::default();
    apply(
        &mut t,
        vec![
            Push::TurnStarted(1, None),
            Push::BlockStart {
                index: 0,
                block: Block::Reasoning,
            },
            Push::Delta {
                index: 0,
                text: "先看看目录".into(),
            },
            Push::BlockStart {
                index: 1,
                block: Block::ToolCall("shell".into()),
            },
            Push::Delta {
                index: 1,
                text: "{\"command\":\"ls\",\"description\":\"列目录\"}".into(),
            },
            Push::BlockEnd(1),
        ],
    );
    let kind_of = |name: &str| (name == "shell").then_some(crate::config::ToolKind::Command);
    let tally = Tally::count(t.entries[0].segment.as_ref().unwrap(), kind_of);
    assert_eq!((tally.commands, tally.thoughts), (1, 1));
    assert_eq!(tally.only_command.as_deref(), Some("列目录"));
}

#[test]
fn an_interrupted_turn_stops_every_spinner() {
    let mut t = Transcript::default();
    apply(
        &mut t,
        vec![
            Push::TurnStarted(1, None),
            Push::BlockStart {
                index: 0,
                block: Block::ToolCall("read".into()),
            },
            Push::TurnEnded(crate::core::EndReason::Interrupted),
        ],
    );
    assert!(!t.busy());
}

#[test]
fn an_error_turn_says_why() {
    let mut t = Transcript::default();
    apply(
        &mut t,
        vec![
            Push::TurnStarted(1, None),
            Push::CallFailed {
                class: "auth".into(),
                message: "no key ".into(),
            },
            Push::TurnEnded(crate::core::EndReason::Error),
        ],
    );
    assert_eq!(t.entries[0].kind, Kind::Error);
    assert!(t.entries[0].text.contains("认证失败：no key"));
}

#[test]
fn a_reverted_turn_hides_and_comes_back() {
    let mut t = Transcript::default();
    t.user("你好".into(), Vec::new());
    apply(
        &mut t,
        vec![
            Push::TurnStarted(7, None),
            Push::BlockStart {
                index: 0,
                block: Block::Text,
            },
            Push::Delta {
                index: 0,
                text: "嗨".into(),
            },
            Push::TurnEnded(crate::core::EndReason::Completed),
            Push::Reverted(vec![7]),
        ],
    );
    assert!(t.entries.iter().all(|e| e.hidden), "{:?}", t.entries);
    apply(&mut t, vec![Push::Unreverted(vec![7])]);
    assert!(t.entries.iter().all(|e| !e.hidden));
}

#[test]
fn undo_is_one_line_with_the_full_prompt_and_restore_removes_it() {
    let mut t = Transcript::default();
    let texts = Config::builtin().unwrap().text;
    t.user("第一行\n第二行".into(), Vec::new());
    apply(
        &mut t,
        vec![
            Push::TurnStarted(7, None),
            Push::TurnEnded(crate::core::EndReason::Completed),
            Push::Reverted(vec![7]),
        ],
    );
    let report = crate::core::Report {
        turns: 1,
        said: Some("第一行".into()),
        restored: 2,
        ..Default::default()
    };
    t.update(
        Update::Undone {
            restore: false,
            report,
        },
        &texts,
    );
    let undo = t.entries.last().unwrap();
    assert_eq!(undo.kind, Kind::Undo);
    assert_eq!(undo.text, "第一行\n第二行", "全文照你说的那句");
    apply(&mut t, vec![Push::Unreverted(vec![7])]);
    t.update(
        Update::Undone {
            restore: true,
            report: Default::default(),
        },
        &texts,
    );
    assert!(
        t.entries.iter().all(|e| e.kind != Kind::Undo),
        "恢复以后那一行去掉"
    );
    assert!(t.entries.iter().all(|e| !e.hidden));
}

#[test]
fn undo_counts_skip_zero() {
    let texts = Config::builtin().unwrap().text;
    let report = crate::core::Report {
        turns: 1,
        restored: 2,
        commands: 1,
        ..Default::default()
    };
    assert_eq!(
        super::words::undo_counts(&report, &texts).as_deref(),
        Some("改回 2 个文件 · 1 条命令的改动撤不回")
    );
    assert_eq!(super::words::undo_counts(&Default::default(), &texts), None);
}

#[test]
fn shift_tab_cycles_workspace_full_read_only() {
    let mut t = Transcript::default();
    let order = Config::builtin().unwrap().layout.level_cycle;
    let mut seen = Vec::new();
    for _ in 0..4 {
        t.next_level(&order);
        seen.push(t.level);
    }
    use crate::core::Level;
    assert_eq!(
        seen,
        vec![Level::Full, Level::ReadOnly, Level::Workspace, Level::Full]
    );
}

#[test]
fn the_next_block_closes_the_ones_before_it() {
    // OpenAI 兼容接口的驱动要等整个回复收完才报 `end`：思考之后开始写一个很长的参数，思考得先停表。
    let mut t = Transcript::default();
    apply(
        &mut t,
        vec![
            Push::TurnStarted(1, None),
            Push::BlockStart {
                index: 0,
                block: Block::Reasoning,
            },
            Push::Delta {
                index: 0,
                text: "想".into(),
            },
            Push::BlockStart {
                index: 1,
                block: Block::ToolCall("shell".into()),
            },
            Push::Delta {
                index: 1,
                text: "{\"command\":\"ls\"}".into(),
            },
            Push::BlockStart {
                index: 2,
                block: Block::ToolCall("write".into()),
            },
        ],
    );
    let steps = &t.entries[0].segment.as_ref().unwrap().steps;
    assert!(!steps[0].busy(), "思考停表了");
    assert!(
        matches!(
            steps[1].kind,
            StepKind::Tool {
                state: ToolState::Running,
                ..
            }
        ),
        "前一件工具的参数算写完了：{:?}",
        steps[1].kind
    );
    assert_eq!(steps[1].arg("command"), Some("ls"));
    assert!(
        matches!(
            steps[2].kind,
            StepKind::Tool {
                state: ToolState::Preparing,
                ..
            }
        ),
        "在写的这一件还在准备"
    );
}

#[test]
fn what_you_said_keeps_the_level_it_was_sent_with() {
    let mut t = Transcript::default();
    t.user("你好".into(), Vec::new());
    let order = Config::builtin().unwrap().layout.level_cycle;
    t.next_level(&order);
    t.user("再来".into(), Vec::new());
    let levels: Vec<_> = t.entries.iter().map(|e| e.level).collect();
    use crate::core::Level;
    assert_eq!(levels, vec![Some(Level::Workspace), Some(Level::Full)]);
}

#[test]
fn messages_sent_while_running_queue_until_their_turn() {
    let mut t = Transcript::default();
    t.user("第一句".into(), Vec::new());
    apply(
        &mut t,
        vec![Push::UserMessage(1), Push::TurnStarted(2, Some(1))],
    );
    t.user("排着的".into(), Vec::new());
    assert!(t.entries[1].queued, "在回答时发的先排着");
    apply(
        &mut t,
        vec![
            Push::UserMessage(5),
            Push::TurnEnded(crate::core::EndReason::Completed),
            Push::TurnStarted(7, Some(5)),
        ],
    );
    let queued = t.entries.iter().find(|e| e.text == "排着的").unwrap();
    assert!(!queued.queued, "开了它那一轮就进正文");
    assert_eq!(queued.turn, Some(7));
}

#[test]
fn withdrawn_messages_leave_the_body_and_come_back() {
    let mut t = Transcript::default();
    t.user("在跑的那句".into(), Vec::new());
    t.user("排着的一".into(), Vec::new());
    t.user("排着的二".into(), Vec::new());
    apply(
        &mut t,
        vec![
            Push::UserMessage(5),
            Push::TurnStarted(6, Some(5)),
            Push::UserMessage(8),
            Push::UserMessage(9),
            Push::Withdrawn(vec![8, 9]),
        ],
    );
    assert_eq!(
        t.take_returned(),
        vec![
            ("排着的一".to_string(), Vec::new()),
            ("排着的二".to_string(), Vec::new())
        ]
    );
    let users: Vec<_> = t
        .entries
        .iter()
        .filter(|e| e.kind == Kind::User)
        .map(|e| e.turn)
        .collect();
    assert_eq!(
        users,
        vec![Some(6)],
        "只剩在跑的那句，照 trigger 归到第 6 轮"
    );
}

#[test]
fn a_turn_started_by_the_last_queued_one_brings_all_of_them_in_order() {
    let mut t = Transcript::default();
    t.user("开头".into(), Vec::new());
    apply(
        &mut t,
        vec![Push::UserMessage(1), Push::TurnStarted(2, Some(1))],
    );
    for (i, text) in ["排一", "排二", "排三"].iter().enumerate() {
        t.user((*text).into(), Vec::new());
        apply(&mut t, vec![Push::UserMessage(10 + i as u64)]);
    }
    apply(
        &mut t,
        vec![
            Push::BlockStart {
                index: 0,
                block: Block::Text,
            },
            Push::Delta {
                index: 0,
                text: "上一轮的回答".into(),
            },
            Push::TurnEnded(crate::core::EndReason::Interrupted),
            Push::TurnStarted(20, Some(12)),
        ],
    );
    let users: Vec<_> = t
        .entries
        .iter()
        .filter(|e| e.kind == Kind::User && !e.queued)
        .map(|e| e.text.as_str())
        .collect();
    assert_eq!(
        users,
        vec!["开头", "排一\n排二\n排三"],
        "排着的三条拼成一条进正文"
    );
    assert_eq!(
        t.entries.last().map(|e| e.text.as_str()),
        Some("排一\n排二\n排三"),
        "挪到正文末尾，在上一轮的回答后面"
    );
}

#[test]
fn a_known_refusal_is_one_short_line() {
    let config = Config::builtin().unwrap();
    let mut t = Transcript::default();
    let refusal = |reason: Option<&str>, message: &str| Update::Refused {
        reason: reason.map(str::to_string),
        message: message.to_string(),
    };
    t.update(
        refusal(
            Some("nothing_to_unrevert"),
            "没有能恢复的撤销：没撤过，或者撤了以后又开过一轮、压缩过。",
        ),
        &config.text,
    );
    assert_eq!(t.entries.last().unwrap().text, "没有能恢复的撤销");
    // 认不得的原因码照核心的原话写。
    t.update(refusal(Some("something_new"), "新的原因。"), &config.text);
    assert!(t.entries.last().unwrap().text.ends_with("新的原因。"));
}
