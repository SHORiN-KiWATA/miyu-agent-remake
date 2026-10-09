//! 账本里照记下的几条开的回合（施工 O-14 上）：`turn.started` 带 `triggers` 的，最后一条要是 `trigger`、照序号排好不重，
//! 每一条都要是旁听的 `message.user`、没当过触发的；撤掉的回合当过的照样算当过。账本答得出哪条是旁听的、哪条当过。
//!
//! 底子：造会话（1），旁听两条（2、3），人说一句（4），场所里冲她来的一句（5）。

use super::*;

const OVERHEARD: &str =
    r#"{"blocks":[{"type":"text","text":"今天谁值班"}],"venue":{"msg":"8810","ambient":true}}"#;

fn base() -> Ledger {
    let mut ledger = Ledger::default();
    for event in [
        event(1, None, "session.created", CREATED),
        event(2, None, "message.user", OVERHEARD),
        event(3, None, "message.user", OVERHEARD),
        event(4, None, "message.user", SAID),
        event(
            5,
            None,
            "message.user",
            r#"{"blocks":[{"type":"text","text":"@她"}],"venue":{"msg":"8811"}}"#,
        ),
    ] {
        ledger.append(&event).unwrap();
    }
    ledger
}

/// 第 `seq` 条开一轮，`trigger`、`triggers` 照写。
fn started(seq: u64, trigger: u64, triggers: &[u64]) -> Event {
    let triggers: Vec<String> = triggers.iter().map(u64::to_string).collect();
    event(
        seq,
        Some(seq),
        "turn.started",
        &format!(
            r#"{{"trigger":{trigger},"triggers":[{}]}}"#,
            triggers.join(",")
        ),
    )
}

#[test]
fn triggers_are_overheard_in_order_unused_and_end_with_the_trigger() {
    let mut ledger = base();
    assert!(ledger.overheard(Seq::new(2).unwrap()));
    assert!(!ledger.overheard(Seq::new(4).unwrap()), "人说的不是旁听的");
    assert!(
        !ledger.overheard(Seq::new(5).unwrap()),
        "场所里冲她来的也不是"
    );
    refused(
        &mut ledger,
        &started(6, 2, &[2, 3]),
        "trigger should be the last of triggers",
    );
    refused(
        &mut ledger,
        &started(6, 2, &[3, 2]),
        "triggers should be in order, each once",
    );
    refused(
        &mut ledger,
        &started(6, 3, &[3, 3]),
        "triggers should be in order, each once",
    );
    refused(
        &mut ledger,
        &started(6, 4, &[2, 4]),
        "trigger 4 is not an overheard message.user",
    );
    ledger.append(&started(6, 3, &[2, 3])).unwrap();
    assert!(ledger.answered(Seq::new(2).unwrap()));
    assert!(!ledger.answered(Seq::new(4).unwrap()));
    ledger
        .append(&event(
            7,
            Some(6),
            "turn.ended",
            r#"{"reason":"completed"}"#,
        ))
        .unwrap();
    // 撤掉那一轮以后照样算当过。
    ledger
        .append(&event(8, None, "turn.reverted", r#"{"turns":[6]}"#))
        .unwrap();
    refused(
        &mut ledger,
        &started(9, 3, &[3]),
        "trigger 3 has already opened a turn",
    );
    // 人说的照旧能开，`triggers` 不写。
    ledger
        .append(&event(9, Some(9), "turn.started", r#"{"trigger":4}"#))
        .unwrap();
}

/// `turn.joined`（施工 O-14 下）：只在回合里；不能是空的；照 `turn.started` 的规矩查那几条；记下以后算当过触发。
#[test]
fn a_join_happens_in_a_turn_and_uses_its_triggers_up() {
    let mut ledger = base();
    let joined = |seq: u64, turn: Option<u64>, triggers: &str| {
        event(
            seq,
            turn,
            "turn.joined",
            &format!(r#"{{"triggers":[{triggers}]}}"#),
        )
    };
    refused(
        &mut ledger,
        &joined(6, None, "2"),
        "turn.joined happens only in a turn",
    );
    ledger
        .append(&event(6, Some(6), "turn.started", r#"{"trigger":4}"#))
        .unwrap();
    refused(
        &mut ledger,
        &joined(7, Some(6), ""),
        "turn.joined should have triggers",
    );
    refused(
        &mut ledger,
        &joined(7, Some(6), "3,2"),
        "triggers should be in order, each once",
    );
    refused(
        &mut ledger,
        &joined(7, Some(6), "4"),
        "trigger 4 is not an overheard message.user",
    );
    ledger.append(&joined(7, Some(6), "2")).unwrap();
    assert!(ledger.answered(Seq::new(2).unwrap()));
    refused(
        &mut ledger,
        &joined(8, Some(6), "2,3"),
        "trigger 2 has already opened a turn",
    );
}
