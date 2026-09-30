//! 人切权限级别（施工 2-7 补，`docs/blueprint/tools/history.md`「哪些算一条」）：带 `permission` 的 `session.policy_changed`
//! 算一条，「谁」是 `user`，原文写切成了哪一级；找、读都有；筛 `user` 时在，筛 `assistant`、`tool` 时不在。撤销不改现在的
//! 权限，撤掉的回合里切的照样列；只换了策略快照的不算。

use super::*;

/// 一条人切权限级别的事件：第 `seq` 条，在第 `turn` 轮里（空闲时切的没有），切成 `permission`。
fn switch(seq: u64, turn: Option<u64>, permission: Value) -> Event {
    let at = format!("2026-09-29T05:0{seq}:00.000Z");
    event(
        seq,
        &at,
        "session.policy_changed",
        turn,
        alice(),
        json!({ "permission": permission }),
    )
}

/// 一段日志：第 3 轮里切到完全放开，这一轮撤掉了；再空闲时开只读；最后一条只换了策略快照。
fn undone() -> Vec<Vec<Event>> {
    let at = |seq: u64| format!("2026-09-29T05:0{seq}:00.000Z");
    let kernel = json!({"kind": "kernel"});
    vec![vec![
        event(
            2,
            &at(2),
            "message.user",
            None,
            alice(),
            json!({"blocks": [{"type": "text", "text": "放开一点"}]}),
        ),
        event(
            3,
            &at(3),
            "turn.started",
            Some(3),
            kernel.clone(),
            json!({"trigger": 2}),
        ),
        switch(4, Some(3), json!({"level": "full", "read_only": false})),
        event(
            5,
            &at(5),
            "message.assistant",
            Some(3),
            model(),
            json!({"blocks": [{"type": "text", "text": "能写了。"}], "seen": 4}),
        ),
        event(
            6,
            &at(6),
            "turn.ended",
            Some(3),
            kernel,
            json!({"reason": "completed"}),
        ),
        event(
            7,
            &at(7),
            "turn.reverted",
            None,
            alice(),
            json!({"turns": [3]}),
        ),
        switch(8, None, json!({"level": "full", "read_only": true})),
        event(
            9,
            &at(9),
            "session.policy_changed",
            None,
            alice(),
            json!({"policy": "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"}),
        ),
    ]]
}

#[tokio::test]
async fn found_and_read_switches_are_by_the_user() {
    let done = run_on(sample(), json!({"query": "permission level"})).await;
    assert_eq!(
        text(&done),
        concat!(
            "#63 2026-09-25 16:32 user: Changed the permission level to workspace\n",
            "#52 2026-09-25 16:05 user: Changed the permission level to read_only\n",
        )
    );
    let done = run_on(sample(), json!({"from": 52, "to": 53})).await;
    assert_eq!(
        text(&done),
        "#52 2026-09-25 16:05 user\nChanged the permission level to read_only\n"
    );
}

#[tokio::test]
async fn switches_count_as_user_when_filtering() {
    let user = seqs(&run_on(sample(), json!({"by": "user", "to": 63})).await);
    assert_eq!(user, [52, 55, 63]);
    for by in ["assistant", "tool"] {
        let done = run_on(sample(), json!({ "by": by, "query": "permission" })).await;
        assert_eq!(text(&done), "No entries found\n", "{by}");
    }
}

#[tokio::test]
async fn a_switch_in_an_undone_turn_is_still_listed() {
    let done = run_on(undone(), json!({})).await;
    assert_eq!(
        text(&done),
        concat!(
            "#4 2026-09-29 14:04 user\nChanged the permission level to full\n\n",
            "#8 2026-09-29 14:08 user\nChanged the permission level to read_only\n",
        ),
        "撤掉的那一轮里人说的、她答的都不算；切的照样列，只读开着写只读；只换了策略快照的不算"
    );
}
