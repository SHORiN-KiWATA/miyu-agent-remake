//! 照记下的几条开一轮（施工 O-14 上，`docs/blueprint/chat.md` 第七条第 3 条第 1 项）：闲着时拿旁听记下的几条当触发开一轮，
//! `triggers` 排好去重、`trigger` 是最后一条，桥带来的事实接在内核的事实后面；空的、不是旁听的、当过触发的拒，后两种带上是
//! 哪几条；载入以后照样认得当过触发的。正在跑一轮的并进去（施工 O-14 下，[`joining`]）。

mod joining;

use super::load::{Logged, load};
use super::*;
use crate::event::{TurnStarted, VenueMessage};

use crate::id::{FactKind, ModuleId};
use crate::origin::Module;

/// 桥。
fn bridge() -> By {
    By::Module(Module {
        id: ModuleId::parse("onebot").unwrap(),
    })
}

/// 编号是 `n` 的命令：旁听的一句。
fn overheard(n: u64) -> Input {
    Input::Command(Received {
        id: id(n),
        by: alice(),
        at: at(n % 60),
        command: Command::Send {
            blocks: vec![Block::Text(Text {
                text: format!("第 {n} 句"),
            })],
            urgent: false,
            venue: Some(VenueMessage {
                msg: format!("88{n:02}"),
                ambient: true,
                ..VenueMessage::default()
            }),
        },
    })
}

/// 编号是 `n` 的命令：桥照 `to` 开一轮，带一块 `judge` 类的事实。
fn respond(n: u64, to: &[u64]) -> Input {
    Input::Command(Received {
        id: id(n),
        by: bridge(),
        at: at(n % 60),
        command: Command::Respond {
            to: to.iter().map(|&n| seq(n)).collect(),
            facts: vec![ContextInjected {
                kind: FactKind::parse("judge").unwrap(),
                text: "<reply-to who=\"小林\"/>\n".to_string(),
                refs: Vec::new(),
            }],
        },
    })
}

/// 落了盘的会话，旁听记下了 2、3 两条。
fn overheard_twice() -> Logged {
    let mut logged = Logged::new();
    logged.handle(overheard(1));
    logged.handle(overheard(2));
    logged.handle(stored(3));
    logged
}

fn outcome(actions: &[Action]) -> Option<&Outcome> {
    actions.iter().find_map(|action| match action {
        Action::Reply { outcome, .. } => Some(outcome),
        _ => None,
    })
}

#[test]
fn responding_opens_a_turn_on_what_was_overheard() {
    let mut logged = overheard_twice();
    let actions = logged.handle(respond(5, &[3, 2, 3]));
    let events = appended_events(&actions);
    assert_eq!(
        events[0].body,
        Body::TurnStarted(TurnStarted {
            trigger: Some(seq(3)),
            triggers: seqs(&[2, 3]),
            cwd: Some("~/src/miyu".to_string()),
            dirs: Vec::new(),
        }),
        "排好、去重，trigger 是最后一条"
    );
    assert_eq!(events[0].turn, Some(TurnId::new(seq(4))));
    assert_eq!(events[0].cause, Some(id(5)));
    let last = events.last().unwrap();
    assert_eq!(last.by, bridge(), "桥的事实接在内核的后面");
    assert_eq!(fact_of(last).text, "<reply-to who=\"小林\"/>\n");
    assert_eq!(last.turn, Some(TurnId::new(seq(4))));
    assert!(
        events[1..events.len() - 1]
            .iter()
            .all(|event| event.by == By::Kernel && matches!(event.body, Body::ContextInjected(_))),
        "中间是内核的事实"
    );
    assert!(replies(&actions).is_empty(), "落了盘才回应");
    // 内核的事实落了盘、桥的还没有：不往下走。
    let half = logged.handle(stored(events[events.len() - 2].seq.get()));
    assert!(
        !half
            .iter()
            .any(|action| matches!(action, Action::RunTurnStartHooks { .. })),
        "{half:?}"
    );
    let stored_actions = logged.handle(stored(logged.last()));
    assert!(
        stored_actions
            .iter()
            .any(|action| matches!(action, Action::RunTurnStartHooks { .. })),
        "{stored_actions:?}"
    );
    assert_eq!(
        outcome(&stored_actions),
        Some(&Outcome::Accepted {
            events: events.iter().map(|event| event.seq).collect()
        })
    );
}

#[test]
fn nothing_to_respond_to_is_refused() {
    let mut logged = overheard_twice();
    let actions = logged.handle(respond(5, &[]));
    assert_eq!(actions, [rejected(id(5), Reason::EmptyMessage)]);
}

#[test]
fn what_was_not_overheard_or_already_answered_is_refused_with_which() {
    let mut logged = overheard_twice();
    // 1 是造会话那一条，40 没有：都不是旁听的。
    let actions = logged.handle(respond(5, &[2, 40, 1]));
    assert_eq!(
        actions,
        [rejected_about(id(5), Reason::NotAmbient, seqs(&[1, 40]))]
    );
    let seen = logged.open_respond(6, &[2]);
    logged.say(seen, "在");
    let actions = logged.handle(respond(9, &[3, 2]));
    assert_eq!(
        actions,
        [rejected_about(id(9), Reason::AlreadyAnswered, seqs(&[2]))]
    );
    // 载入以后照样认得。
    let (mut loaded, _) = load(logged.log.clone());
    assert_eq!(
        loaded.handle(respond(10, &[2])),
        [rejected_about(id(10), Reason::AlreadyAnswered, seqs(&[2]))]
    );
}

impl Logged {
    /// 桥照 `to` 开一轮，全落了盘，挂接点跑完，请求交给了执行器：返回这次请求的 `seen`。
    fn open_respond(&mut self, n: u64, to: &[u64]) -> u64 {
        self.handle(respond(n, to));
        self.handle(stored(self.last()));
        let turn = self
            .log
            .iter()
            .rev()
            .find(|event| matches!(event.body, Body::TurnStarted(_)))
            .map(|event| TurnId::new(event.seq))
            .unwrap();
        let actions = self.handle(hooks_done(turn, Vec::new()));
        calls(&actions).remove(0).0.get()
    }
}
