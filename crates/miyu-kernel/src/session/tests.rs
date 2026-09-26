//! 会话的测试。这一份是命令这一层：造会话；发消息；空消息；同一个编号落盘前后再来；
//! 拒绝过的再来；落盘到一半；落盘超出追加过的；只记最近 1024 个。开回合、发请求在
//! [`turn`]；收回复、结束回合在 [`reply`]；调工具在 [`tools`]；打断在 [`interrupt`]；排队的消息在
//! [`queue`]；切权限级别在 [`permission`]；确认在 [`approval`]；提问在 [`question`]；载入和崩溃在
//! [`load`]；有计划的重启在 [`restart`]；随机一串输入在 [`random`]；执行器的替身在 [`executor`]。
//!
//! 空闲时发的第一条消息会开一个回合，所以它后面紧跟着三条：`turn.started` 和两块事实。

mod approval;
mod executor;
mod interrupt;
mod load;
mod permission;
mod question;
mod queue;
mod random;
mod reply;
mod restart;
mod tools;
mod turn;

use std::collections::BTreeMap;

use super::recent::CAPACITY;
use super::*;
use crate::assemble::Assembler;
use crate::block::{Block, Text};
use crate::event::ContextInjected;
use crate::facts::FactTemplates;
use crate::request::{Message, Request};
use crate::time::UtcOffset;
use crate::tool::{Access, ToolRule, ToolTextSources, ToolTexts};

const CREATED: &str = r#"{"owner":"alice","venue":"local","policy":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","permission":{"level":"workspace","read_only":false}}"#;

fn id(n: u64) -> CommandId {
    CommandId::parse(&format!("cmd-{n}")).unwrap()
}

fn alice() -> By {
    serde_json::from_str(r#"{"kind":"person","account":"alice"}"#).unwrap()
}

/// 07:00 过 `second` 秒，UTC。东九区是 16:00 那一个小时。
fn at(second: u64) -> Timestamp {
    Timestamp::parse(&format!("2026-09-25T07:00:{second:02}.000Z")).unwrap()
}

fn seq(n: u64) -> Seq {
    Seq::new(n).unwrap()
}

fn seqs(numbers: &[u64]) -> Vec<Seq> {
    numbers.iter().map(|&n| seq(n)).collect()
}

/// 编号是 `n` 的命令：alice 发一条消息，内容是 `words`；`words` 是空的就一块内容都没有。
fn send(n: u64, words: &str) -> Input {
    message(n, words, false)
}

/// 同上，急着插话。
fn urgent(n: u64, words: &str) -> Input {
    message(n, words, true)
}

/// 编号是 `n` 的命令：alice 打断正在进行的回合，排着队的接着发。
fn interrupt(n: u64) -> Input {
    stop_with(n, at(n % 60), Queued::Send)
}

/// 编号是 `n` 的命令：alice 打断正在进行的回合，排着队的退回来。
fn take_back(n: u64) -> Input {
    stop_with(n, at(n % 60), Queued::Return)
}

/// alice 在 `at` 打断，排着队的照 `queued` 办。
fn stop_with(n: u64, at: Timestamp, queued: Queued) -> Input {
    Input::Command(Received {
        id: id(n),
        by: alice(),
        at,
        command: Command::Interrupt { queued },
    })
}

fn message(n: u64, words: &str, urgent: bool) -> Input {
    let blocks = if words.is_empty() {
        Vec::new()
    } else {
        vec![Block::Text(Text {
            text: words.to_string(),
        })]
    };
    Input::Command(Received {
        id: id(n),
        by: alice(),
        at: at(n % 60),
        command: Command::Send { blocks, urgent },
    })
}

fn stored(upto: u64) -> Input {
    Input::Stored { upto: seq(upto) }
}

fn accepted_reply(n: u64, events: &[u64]) -> Action {
    Action::Reply {
        id: id(n),
        outcome: Outcome::Accepted {
            events: seqs(events),
        },
    }
}

/// 替身的组装：有效历史里每条事件一条 user 消息，写着序号和种类。测的是会话什么时候、
/// 拿哪一段历史组装，和怎么组装无关。历史只往后加，请求也只往后加。
struct Listing;

impl Assembler for Listing {
    fn assemble(&self, history: &History) -> Request {
        let messages = history
            .events()
            .iter()
            .map(|event| Message::User {
                blocks: vec![Block::Text(Text {
                    text: format!("{} {}", event.seq, event.body.kind()),
                })],
            })
            .collect();
        Request {
            tools: Vec::new(),
            system: "listing".to_string(),
            messages,
            stable: 0,
        }
    }
}

/// 这几条事件，一条一行：序号和种类。
fn listing(events: &[Event]) -> String {
    events
        .iter()
        .map(|event| format!("{} {}\n", event.seq, event.body.kind()))
        .collect()
}

/// 替身的组装出来的请求，照 [`listing`] 的样子一条一行。
fn listed_request(request: &Request) -> String {
    request
        .messages
        .iter()
        .map(|message| match message {
            Message::User { blocks } => match blocks.as_slice() {
                [Block::Text(text)] => format!("{}\n", text.text),
                other => panic!("替身的组装一条消息只有一块字：{other:?}"),
            },
            other => panic!("替身的组装只出 user 消息：{other:?}"),
        })
        .collect()
}

/// 替身的组装把这几条列出来的样子：序号和种类，一条一行。
fn listed(events: &[(u64, &str)]) -> String {
    events
        .iter()
        .map(|(seq, kind)| format!("{seq} {kind}\n"))
        .collect()
}

/// 空闲时的第一条消息是 2 号，它开的回合是 3 号。
fn turn3() -> TurnId {
    TurnId::new(seq(3))
}

/// 回合开始的挂接点跑完了，交回这几块注入。
fn hooks_done(turn: TurnId, injected: Vec<Injection>) -> Input {
    Input::TurnStartHooksDone {
        at: at(30),
        turn,
        injected,
    }
}

/// 叫跑回合开始的挂接点的那几个回合。
fn hooks(actions: &[Action]) -> Vec<TurnId> {
    actions
        .iter()
        .filter_map(|action| match action {
            Action::RunTurnStartHooks { turn } => Some(*turn),
            _ => None,
        })
        .collect()
}

/// 请求模型的那几次：看到了第几条为止，和替身的组装列出来的历史。
fn calls(actions: &[Action]) -> Vec<(Seq, String)> {
    actions
        .iter()
        .filter_map(|action| match action {
            Action::CallModel { seen, request } => Some((*seen, listed_request(request))),
            _ => None,
        })
        .collect()
}

fn fact_of(event: &Event) -> &ContextInjected {
    match &event.body {
        Body::ContextInjected(fact) => fact,
        body => panic!("应该是一块事实：{body:?}"),
    }
}

/// 替身的策略：模板短，一眼认得出是哪个字段；工具面上四件工具，读、写、跑命令、问人；步数不限。
fn policy() -> Policy {
    let rule = |access: Access, parameters: &str| ToolRule {
        access,
        parameters: serde_json::from_str(parameters).unwrap(),
    };
    Policy {
        assembler: Box::new(Listing),
        facts: FactTemplates::new(
            r#"<e t="{time}" z="{timezone}" d="{cwd}"/>"#,
            r#"<p l="{level}"/>"#,
        )
        .unwrap(),
        tools: BTreeMap::from([
            (
                "read".to_string(),
                rule(
                    Access::Read,
                    r#"{"type":"object","properties":{"path":{"type":"string"},"limit":{"type":"integer"}}}"#,
                ),
            ),
            (
                "write".to_string(),
                rule(
                    Access::Write,
                    r#"{"type":"object","properties":{"path":{"type":"string"},"text":{"type":"string"}}}"#,
                ),
            ),
            (
                "shell".to_string(),
                rule(Access::Execute, r#"{"type":"object"}"#),
            ),
            (
                "ask_user".to_string(),
                rule(Access::Read, r#"{"type":"object"}"#),
            ),
        ]),
        step_limit: None,
        tool_texts: ToolTexts::new(ToolTextSources {
            unknown: "no tool {name}",
            not_an_object: "bad args {name}",
            cancelled_before: "cancelled before",
            cancelled_running: "cancelled running",
            skipped: "skipped",
            read_only: "read only",
            denied: "denied",
            denied_with_reason: "denied: {reason}",
            unattended: "unattended",
            question_interrupted: "question interrupted",
            question_voided: "question voided",
            question_unattended: "question unattended",
            restarted: "restarted",
        })
        .unwrap(),
        attended: true,
        resumes: 3,
    }
}

/// 东九区，工作目录是 `cwd`。
fn environment(cwd: &str) -> Environment {
    Environment {
        offset: UtcOffset::from_minutes(540).unwrap(),
        cwd: cwd.to_string(),
    }
}

/// 一个造好、第 1 条已经落了盘的会话，在 `~/src/miyu`。造会话的命令编号是 0。
fn session() -> Session {
    session_with(policy())
}

/// 同上，策略换成 `policy`。
fn session_with(policy: Policy) -> Session {
    let created: SessionCreated = serde_json::from_str(CREATED).unwrap();
    let (mut session, _) = Session::create(
        id(0),
        alice(),
        at(0),
        created,
        policy,
        environment("~/src/miyu"),
    );
    session.handle(stored(1));
    session
}

/// 追加动作里的事件，照先后。
fn appended_events(actions: &[Action]) -> Vec<Event> {
    actions
        .iter()
        .filter_map(|action| match action {
            Action::Append(events) => Some(events.iter().cloned()),
            _ => None,
        })
        .flatten()
        .collect()
}

/// 追加动作里的事件的序号。
fn appended(actions: &[Action]) -> Vec<Seq> {
    appended_events(actions)
        .iter()
        .map(|event| event.seq)
        .collect()
}

/// 动作里的回应。
fn replies(actions: &[Action]) -> Vec<&Action> {
    actions
        .iter()
        .filter(|action| matches!(action, Action::Reply { .. }))
        .collect()
}

#[test]
fn creating_a_session_appends_session_created_and_replies_once_stored() {
    let created: SessionCreated = serde_json::from_str(CREATED).unwrap();
    let (mut session, actions) = Session::create(
        id(0),
        alice(),
        at(0),
        created,
        policy(),
        environment("~/src/miyu"),
    );
    let [Action::Append(events)] = actions.as_slice() else {
        panic!("造会话应该只追加一条：{actions:?}");
    };
    let [event] = events.as_slice() else {
        panic!("造会话应该只追加一条：{events:?}");
    };
    assert_eq!(event.seq, seq(1));
    assert_eq!(event.cause, Some(id(0)));
    assert_eq!(event.turn, None);
    assert!(matches!(event.body, Body::SessionCreated(_)));
    let actions = session.handle(stored(1));
    assert_eq!(
        actions,
        [Action::Push(vec![event.clone()]), accepted_reply(0, &[1])]
    );
}

#[test]
fn a_message_is_appended_and_answered_once_stored() {
    let mut session = session();
    let actions = session.handle(send(1, "看看 src 目录"));
    let [Action::Append(events)] = actions.as_slice() else {
        panic!("发消息应该只追加，不回应：{actions:?}");
    };
    let message = &events[0];
    assert_eq!(message.seq, seq(2));
    assert_eq!(message.at, at(1));
    assert_eq!(message.by, alice());
    assert_eq!(message.cause, Some(id(1)));
    assert_eq!(message.turn, None);
    assert!(matches!(&message.body, Body::MessageUser(message) if message.blocks.len() == 1));
    // 回应只附消息本身：回合是内核接着开的。
    let actions = session.handle(stored(2));
    assert_eq!(
        actions,
        [Action::Push(vec![message.clone()]), accepted_reply(1, &[2])]
    );
}

#[test]
fn an_empty_message_is_rejected_on_the_spot() {
    let mut session = session();
    let actions = session.handle(send(1, ""));
    assert_eq!(
        actions,
        [Action::Reply {
            id: id(1),
            outcome: Outcome::Rejected {
                reason: Reason::EmptyMessage
            },
        }]
    );
    assert_eq!(Reason::EmptyMessage.code(), "empty_message");
    // 什么都没追加，也没开回合：下一条消息还是 2 号，接着开回合。
    assert_eq!(
        appended(&session.handle(send(2, "hi"))),
        seqs(&[2, 3, 4, 5])
    );
}

#[test]
fn a_command_sent_again_before_it_is_stored_is_answered_twice_after() {
    let mut session = session();
    session.handle(send(1, "hi"));
    assert!(session.handle(send(1, "hi")).is_empty());
    let actions = session.handle(stored(2));
    assert_eq!(
        replies(&actions),
        [&accepted_reply(1, &[2]), &accepted_reply(1, &[2])]
    );
}

#[test]
fn a_command_sent_again_after_it_is_stored_is_answered_on_the_spot() {
    let mut session = session();
    session.handle(send(1, "hi"));
    session.handle(stored(2));
    assert_eq!(session.handle(send(1, "hi")), [accepted_reply(1, &[2])]);
}

#[test]
fn a_rejected_command_is_judged_again() {
    let mut session = session();
    let rejected = |actions: Vec<Action>| {
        matches!(
            actions.as_slice(),
            [Action::Reply {
                outcome: Outcome::Rejected { .. },
                ..
            }]
        )
    };
    assert!(rejected(session.handle(send(1, ""))));
    assert!(rejected(session.handle(send(1, ""))));
    assert_eq!(
        appended(&session.handle(send(1, "这回有字了"))).first(),
        Some(&seq(2))
    );
}

#[test]
fn a_partial_store_answers_only_the_commands_fully_stored() {
    let mut session = session();
    session.handle(send(1, "one"));
    // 回合正在开，第二条是中途来的，排在回合开头那几条后面。
    assert_eq!(appended(&session.handle(send(2, "two"))), seqs(&[6]));
    let first = session.handle(stored(2));
    assert!(
        matches!(first.as_slice(), [Action::Push(events), reply] if events.len() == 1 && *reply == accepted_reply(1, &[2]))
    );
    assert!(replies(&session.handle(stored(5))).is_empty());
    let last = session.handle(stored(6));
    assert!(
        matches!(last.as_slice(), [Action::Push(events), reply] if events.len() == 1 && *reply == accepted_reply(2, &[6]))
    );
}

#[test]
fn a_store_beyond_what_was_appended_counts_only_what_was_appended() {
    let mut session = session();
    session.handle(send(1, "one"));
    let actions = session.handle(stored(100));
    assert_eq!(
        replies(&actions),
        [&accepted_reply(1, &[2])],
        "追加过的都落了盘"
    );
    // 之后追加的，要等它自己落了盘。
    session.handle(send(2, "two"));
    assert!(session.handle(send(2, "two")).is_empty());
    assert!(
        session.handle(stored(5)).is_empty(),
        "不比上一次往后的，什么都不做"
    );
    assert_eq!(replies(&session.handle(stored(6))).len(), 2);
}

#[test]
fn only_the_latest_commands_are_remembered() {
    let mut session = session();
    for n in 1..=CAPACITY as u64 + 1 {
        session.handle(send(n, "hi"));
    }
    // 第 1 个命令的消息是 2 号，后面跟着回合开头的三条；第 n 个（n 大于 1）是 n + 4 号。
    let last = CAPACITY as u64 + 5;
    session.handle(stored(last));
    // 第 2 个还记得，照上一次回应；第 1 个已经忘了，当新命令。
    assert_eq!(session.handle(send(2, "hi")), [accepted_reply(2, &[6])]);
    assert_eq!(appended(&session.handle(send(1, "hi"))), seqs(&[last + 1]));
}
