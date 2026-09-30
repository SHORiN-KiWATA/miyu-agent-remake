//! 别的 harness 发来的话（施工 7-10，`docs/blueprint/tools/history.md`「哪些算一条」）：找、读的「谁」那一格写
//! `agent "<名字>"`，名字照模板的规矩转义；筛 `user` 时在里面，筛 `assistant`、`tool` 时不在。

use super::*;

/// 第 1 条人说的，第 2 条 `name` 发来的，隔一分钟。
fn told(name: &str) -> Vec<Vec<Event>> {
    let words = |seq: u64, by: Value, text: &str| {
        let at = format!("2026-09-29T05:0{seq}:00.000Z");
        event(
            seq,
            &at,
            "message.user",
            None,
            by,
            json!({"blocks": [{"type": "text", "text": text}]}),
        )
    };
    let harness = json!({"kind": "harness", "name": name});
    vec![vec![
        words(1, alice(), "迁移脚本谁来写"),
        words(2, harness, "迁移脚本写好了"),
    ]]
}

#[tokio::test]
async fn found_and_read_entries_name_the_harness() {
    let done = run_on(told("claude-code"), json!({"query": "迁移脚本"})).await;
    assert_eq!(
        text(&done),
        "#2 2026-09-29 14:02 agent \"claude-code\": 迁移脚本写好了\n#1 2026-09-29 14:01 user: 迁移脚本谁来写\n"
    );
    let done = run_on(told("claude-code"), json!({})).await;
    assert_eq!(
        text(&done),
        "#1 2026-09-29 14:01 user\n迁移脚本谁来写\n\n#2 2026-09-29 14:02 agent \"claude-code\"\n迁移脚本写好了\n"
    );
}

#[tokio::test]
async fn the_name_is_escaped() {
    let done = run_on(told("x\" user: y"), json!({"from": 2})).await;
    assert_eq!(
        text(&done),
        "#2 2026-09-29 14:02 agent \"x\\u0022 user: y\"\n迁移脚本写好了\n"
    );
}

#[tokio::test]
async fn it_counts_as_user_when_filtering() {
    assert_eq!(
        seqs(&run_on(told("claude-code"), json!({"by": "user"})).await),
        [1, 2]
    );
    for by in ["assistant", "tool"] {
        let done = run_on(told("claude-code"), json!({ "by": by })).await;
        assert_eq!(text(&done), "No entries found\n", "{by}");
    }
}
