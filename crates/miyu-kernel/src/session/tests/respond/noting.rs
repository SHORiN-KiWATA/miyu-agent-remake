//! 记几块事实，不开回合（施工 O-14 补，`note.rs`）：空闲时不带回合编号、不开回合；正在跑一轮时带这一轮的回合编号，不打断、
//! 不另开；空的拒。

use super::super::executor::{call_tools, done, ran};
use super::*;

/// 编号是 `n` 的命令：桥记一块 `failed` 类的事实。
fn note(n: u64) -> Input {
    Input::Command(Received {
        id: id(n),
        by: bridge(),
        at: at(n % 60),
        command: Command::Note {
            facts: vec![ContextInjected {
                kind: FactKind::parse("failed").expect("合写法"),
                text: "<upload-failed/>\n".to_string(),
                refs: Vec::new(),
            }],
        },
    })
}

#[test]
fn a_note_while_idle_opens_no_turn_and_carries_no_turn() {
    let mut logged = Logged::new();
    let actions = logged.handle(note(3));
    let events = appended_events(&actions);
    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!(events[0].body.kind(), "context.injected");
    assert_eq!(
        (
            events[0].by.clone(),
            events[0].turn,
            events[0].cause.clone()
        ),
        (bridge(), None, Some(id(3)))
    );
    logged.handle(stored(logged.last()));
    assert_eq!(
        outcome(&logged.handle(note(3))),
        Some(&Outcome::Accepted {
            events: vec![events[0].seq]
        }),
        "同一个编号只记一次"
    );
    assert!(
        !logged
            .log
            .iter()
            .any(|event| event.body.kind() == "turn.started"),
        "不开回合"
    );
    let noted = events[0].seq.get();
    let (_, request) = logged.open(5, "hi");
    let trigger = logged
        .log
        .iter()
        .find(|event| event.body.kind() == "message.user")
        .expect("发了一句")
        .seq
        .get();
    let fact = request.find(&format!("{noted} context.injected\n"));
    let said = request.find(&format!("{trigger} message.user\n"));
    assert!(
        fact.is_some() && fact < said,
        "下一轮开头看到，排在触发前面：{request}"
    );
}

#[test]
fn a_note_during_a_turn_belongs_to_it_and_neither_interrupts_nor_opens_another() {
    let mut logged = Logged::new();
    let seen = logged.ask(4, "hi");
    let turn = logged
        .log
        .iter()
        .rev()
        .find(|event| event.body.kind() == "turn.started")
        .expect("开了一轮")
        .seq;
    let actions = call_tools(&mut logged.session, seen, &[("read", "{}")]);
    logged.log.extend(appended_events(&actions));
    let running = ran(&logged.allowing(stored(logged.last())))[0];
    let actions = logged.handle(note(9));
    let events = appended_events(&actions);
    assert_eq!(
        events[0].turn.map(|turn| turn.started()),
        Some(turn),
        "带这一轮的回合编号"
    );
    assert!(
        !actions.iter().any(|action| matches!(
            action,
            Action::CancelTool { .. } | Action::StopTool { .. } | Action::CallModel { .. }
        )),
        "不打断、不叫醒：{actions:?}"
    );
    let noted = events[0].seq.get();
    logged.handle(stored(noted));
    logged.handle(done(running, "ok"));
    let next = calls(&logged.handle(stored(logged.last())));
    assert!(
        next[0].1.contains(&format!("{noted} context.injected\n")),
        "下一次请求就看到：{}",
        next[0].1
    );
    logged.say(next[0].0.get(), "嗯");
    let started = logged
        .log
        .iter()
        .filter(|event| event.body.kind() == "turn.started")
        .count();
    assert_eq!(started, 1, "这一轮说完就结束，不为它另开一轮");
}

#[test]
fn an_empty_note_is_refused() {
    let mut logged = Logged::new();
    let actions = logged.handle(Input::Command(Received {
        id: id(3),
        by: bridge(),
        at: at(3),
        command: Command::Note { facts: Vec::new() },
    }));
    assert!(matches!(outcome(&actions), Some(Outcome::Rejected { .. })));
    assert!(appended_events(&actions).is_empty());
}
