//! 换思考强度（施工 8-18，`configure.rs`，`docs/blueprint/models.md`「怎么走」第十一条第 3 条）：会话给一个模型记一格，换了的
//! 记一条、落了盘才回应，`null` 清掉；和记着的一样的、清掉本来就没有的不记；和换模型一起来的只写变了的几格、一条事件；
//! 回合开始连同引用交出；载入照整份日志拼回来，撤掉的回合里换的也算；只换思考强度不算换模型（熔断照旧）。

use std::collections::BTreeMap;

use super::executor::*;
use super::load::{Logged, load};
use super::revert::revert;
use super::*;
use crate::event::{Effort, PolicyChanged};

/// 编号是 `n` 的命令：alice 换模型 `model`（没有的不换）、给 `effort` 的模型记那一档（`None` 清掉）。
fn configure(n: u64, model: Option<&str>, effort: Option<(&str, Option<&str>)>) -> Input {
    Input::Command(Received {
        id: id(n),
        by: alice(),
        at: at(n % 60),
        command: Command::Configure {
            model: model.map(str::to_string),
            effort: effort.map(|(model, level)| cell(model, level)),
        },
    })
}

/// 一格：模型 `model` 记 `level`。
fn cell(model: &str, level: Option<&str>) -> Effort {
    Effort {
        model: model.to_string(),
        level: level.map(str::to_string),
    }
}

/// 换的那一条的 `body`。
fn body(model: Option<&str>, effort: Option<(&str, Option<&str>)>) -> Body {
    Body::PolicyChanged(PolicyChanged {
        model: model.map(str::to_string),
        effort: effort.map(|(model, level)| cell(model, level)),
        ..PolicyChanged::default()
    })
}

/// 会话这时记着的每一格。
fn efforts(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(model, level)| ((*model).to_string(), (*level).to_string()))
        .collect()
}

/// 叫跑回合开始的挂接点时交出去的思考强度。
fn handed(actions: &[Action]) -> Vec<BTreeMap<String, String>> {
    actions
        .iter()
        .filter_map(|action| match action {
            Action::RunTurnStartHooks { efforts, .. } => Some(efforts.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn an_effort_is_recorded_for_one_model_and_null_clears_it() {
    let mut session = session();
    let actions = session.handle(configure(2, None, Some(("a/m", Some("high")))));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[2]));
    assert!(replies(&actions).is_empty(), "落了盘才回应");
    assert_eq!(events[0].body, body(None, Some(("a/m", Some("high")))));
    assert_eq!(
        (&events[0].by, &events[0].cause, events[0].turn),
        (&alice(), &Some(id(2)), None)
    );
    assert_eq!(
        replies(&session.handle(stored(2))),
        [&accepted_reply(2, &[2])]
    );
    assert_eq!(session.efforts(), &efforts(&[("a/m", "high")]));
    assert_eq!(session.reference(), None, "只换思考强度，引用不动");
    // 另一个模型各记各的。
    session.handle(configure(3, None, Some(("b/n", Some("off")))));
    session.handle(stored(3));
    assert_eq!(
        session.efforts(),
        &efforts(&[("a/m", "high"), ("b/n", "off")])
    );
    // 和记着的一样：当场回应，什么都不记。
    assert_eq!(
        session.handle(configure(4, None, Some(("a/m", Some("high"))))),
        [accepted_reply(4, &[])]
    );
    // 清掉：记一条，`level` 是 `null`。
    let actions = session.handle(configure(5, None, Some(("a/m", None))));
    assert_eq!(
        appended_events(&actions)[0].body,
        body(None, Some(("a/m", None)))
    );
    session.handle(stored(4));
    assert_eq!(session.efforts(), &efforts(&[("b/n", "off")]));
    // 清掉本来就没有的：不记。
    assert_eq!(
        session.handle(configure(6, None, Some(("a/m", None)))),
        [accepted_reply(6, &[])]
    );
}

#[test]
fn with_a_model_change_only_what_changed_is_written_in_one_event() {
    let mut session = session();
    let actions = session.handle(configure(2, Some("@free"), Some(("a/m", Some("high")))));
    assert_eq!(appended(&actions), seqs(&[2]), "一条");
    assert_eq!(
        appended_events(&actions)[0].body,
        body(Some("@free"), Some(("a/m", Some("high"))))
    );
    session.handle(stored(2));
    // 模型一样、强度换了：只写强度。
    let actions = session.handle(configure(3, Some("@free"), Some(("a/m", Some("low")))));
    assert_eq!(
        appended_events(&actions)[0].body,
        body(None, Some(("a/m", Some("low"))))
    );
    session.handle(stored(3));
    // 强度一样、模型换了：只写模型。
    let actions = session.handle(configure(4, Some("b/n"), Some(("a/m", Some("low")))));
    assert_eq!(appended_events(&actions)[0].body, body(Some("b/n"), None));
    session.handle(stored(4));
    // 两样都一样：什么都不记。
    assert_eq!(
        session.handle(configure(5, Some("b/n"), Some(("a/m", Some("low"))))),
        [accepted_reply(5, &[])]
    );
    assert_eq!(
        (session.reference(), session.efforts()),
        (Some("b/n"), &efforts(&[("a/m", "low")]))
    );
}

#[test]
fn the_efforts_are_handed_over_when_a_turn_starts_and_do_not_count_as_a_model_change() {
    let mut session = session();
    session.handle(configure(2, None, Some(("a/m", Some("max")))));
    session.handle(stored(2));
    assert!(
        session.reference.after_change(seq(1)),
        "只换思考强度，熔断不当换了模型"
    );
    let sent = session.handle(send(3, "hi"));
    let last = appended(&sent).last().expect("开了一轮").get();
    let actions = session.handle(stored(last));
    assert_eq!(handed(&actions), [efforts(&[("a/m", "max")])]);
    // 没记过的会话交出去的是空的。
    let mut plain = super::session();
    plain.handle(send(2, "hi"));
    assert_eq!(handed(&plain.handle(stored(5))), [BTreeMap::new()]);
}

#[test]
fn a_change_during_a_turn_carries_the_turn() {
    let mut session = asking();
    let actions = session.handle(configure(9, None, Some(("a/m", Some("low")))));
    assert_eq!(appended_events(&actions)[0].turn, Some(turn3()));
    assert!(!session.idle(), "回合照常");
}

#[test]
fn loading_counts_every_effort_change_even_in_undone_turns() {
    let mut logged = Logged::new();
    let seen = logged.ask(1, "hi");
    logged.handle(configure(20, None, Some(("a/m", Some("high")))));
    logged.handle(configure(21, None, Some(("b/n", Some("off")))));
    logged.handle(configure(22, None, Some(("b/n", None))));
    logged.say(seen, "好");
    let last = logged.last();
    logged.handle(revert(23, 3));
    logged.handle(stored(last + 1));
    assert_eq!(
        logged.session.efforts(),
        &efforts(&[("a/m", "high")]),
        "撤掉的回合里换的也算"
    );
    let (mut loaded, _) = load(logged.log.clone());
    assert_eq!(
        loaded.efforts(),
        &efforts(&[("a/m", "high")]),
        "载入照日志拼回来"
    );
    assert_eq!(
        loaded.handle(configure(30, None, Some(("a/m", Some("high"))))),
        [accepted_reply(30, &[])]
    );
}
