//! 模型调用的记录：看到的在它自己之前。回顾的两条（施工 3-8 四补）：不带回合编号，有回合在进行时也收；`session.recapped`
//! 照到的在它之前；回顾的 `model.called` 不算她听到了排着的话，那几句照样撤得回。施工 3-8 四补从 `tests.rs` 分出来。
//! 换模型（施工 8-10）：`replaced` 只和 `model` 一起出现。

use super::*;

/// 换模型的 `session.policy_changed`（施工 8-10）：`replaced` 只和 `model` 一起出现；只有 `model` 的、两格都有的收，不带回合
/// 编号的、带着在跑的回合的都收。
#[test]
fn replaced_comes_only_with_a_model() {
    let mut ledger = after(5);
    refused(
        &mut ledger,
        &event(6, None, "session.policy_changed", r#"{"replaced":"a/m"}"#),
        "replaced comes only with model",
    );
    ledger
        .append(&event(
            6,
            None,
            "session.policy_changed",
            r#"{"model":"@free"}"#,
        ))
        .unwrap();
    let body = r#"{"model":"a/m","replaced":"@free"}"#;
    ledger
        .append(&event(7, Some(3), "session.policy_changed", body))
        .unwrap();
}

/// 模型调用的记录，看到的在它自己之前（03 第三节「模型调用怎么写」）。
#[test]
fn a_model_call_saw_what_came_before_it() {
    let called = |seen: u64| format!(r#"{{"seen":{seen},"messages":1,"result":"ok"}}"#);
    let mut ledger = after(5);
    refused(
        &mut ledger,
        &event(6, Some(3), "model.called", &called(6)),
        "seen 6 should come before this event",
    );
    ledger
        .append(&event(6, Some(3), "model.called", &called(4)))
        .unwrap();
}

/// 回顾的 `model.called`：看到第 `seen` 条。
fn recap_called(seen: u64) -> String {
    format!(r#"{{"seen":{seen},"messages":1,"result":"ok","purpose":"recap"}}"#)
}

#[test]
fn a_recap_line_comes_after_what_it_covers() {
    let mut ledger = after(2);
    for upto in [3, 4] {
        refused(
            &mut ledger,
            &event(
                3,
                None,
                "session.recapped",
                &format!(r#"{{"text":"x","upto":{upto}}}"#),
            ),
            &format!("upto {upto} should come before this event"),
        );
    }
    ledger
        .append(&event(3, None, "model.called", &recap_called(2)))
        .unwrap();
    ledger
        .append(&event(
            4,
            None,
            "session.recapped",
            r#"{"text":"x","upto":2}"#,
        ))
        .unwrap();
}

/// 回合里排着一句，回顾照到了它：她还是没听到，打断时照样撤得回。主请求记下的 `model.called` 就算听到了。
#[test]
fn a_recap_does_not_hear_what_is_queued() {
    let mut ledger = after(3);
    ledger
        .append(&event(4, Some(3), "message.user", SAID))
        .unwrap();
    ledger
        .append(&event(5, None, "model.called", &recap_called(4)))
        .unwrap();
    let withdrawn = event(6, Some(3), "message.withdrawn", r#"{"messages":[4]}"#);
    ledger.clone().append(&withdrawn).unwrap();
    let body = r#"{"seen":4,"messages":1,"result":"ok"}"#;
    ledger
        .append(&event(6, Some(3), "model.called", body))
        .unwrap();
    refused(
        &mut ledger,
        &event(7, Some(3), "message.withdrawn", r#"{"messages":[4]}"#),
        "event 4 is not a queued message of the running turn",
    );
}
