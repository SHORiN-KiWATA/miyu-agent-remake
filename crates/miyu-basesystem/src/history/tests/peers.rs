//! 别的会话发来的话（施工 C-2，`docs/blueprint/tools/history.md`「哪些算一条」，`cross-session.md` 第二条第 4 款）：找、读的
//! 「谁」那一格写 `session <短编号>`；筛 `user` 时在里面，筛 `assistant`、`tool` 时不在。父会话的话照旧是 `user`。

use super::*;

/// 发话的会话：短编号 `22334455`。
const PEER: &str = "0192f3a0-1111-7abc-8def-001122334455";
/// 父会话。
const PARENT: &str = "01a0d75d-2180-7a3c-9e41-5b7d2c8f6a10";

/// 第 1 条造会话（子会话，父会话是 `PARENT`），第 2 条父会话说的，第 3 条别的会话发来的，隔一分钟。
fn told() -> Vec<Vec<Event>> {
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
    let created = event(
        1,
        "2026-09-29T05:00:00.000Z",
        "session.created",
        None,
        alice(),
        json!({"owner": "alice", "venue": "local", "policy": "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855", "permission": {"level": "workspace", "read_only": false}, "parent": PARENT, "depth": 1}),
    );
    let session = |id: &str| json!({"kind": "session", "id": id});
    vec![vec![
        created,
        words(2, session(PARENT), "迁移脚本谁来写"),
        words(3, session(PEER), "迁移脚本写好了"),
    ]]
}

#[tokio::test]
async fn found_and_read_entries_name_the_session() {
    let done = run_on(told(), json!({"query": "迁移脚本"})).await;
    assert_eq!(
        text(&done),
        "#3 2026-09-29 14:03 session 22334455: 迁移脚本写好了\n#2 2026-09-29 14:02 user: 迁移脚本谁来写\n"
    );
    let done = run_on(told(), json!({})).await;
    assert_eq!(
        text(&done),
        "#2 2026-09-29 14:02 user\n迁移脚本谁来写\n\n#3 2026-09-29 14:03 session 22334455\n迁移脚本写好了\n"
    );
}

#[tokio::test]
async fn it_counts_as_user_when_filtering() {
    assert_eq!(seqs(&run_on(told(), json!({"by": "user"})).await), [2, 3]);
    for by in ["assistant", "tool"] {
        let done = run_on(told(), json!({ "by": by })).await;
        assert_eq!(text(&done), "No entries found\n", "{by}");
    }
}
