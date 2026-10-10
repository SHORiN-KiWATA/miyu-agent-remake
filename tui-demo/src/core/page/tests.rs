//! 按页读：读到哪了、更早的一页去掉已经画过的（蓝图「会话列表」第 5 条「按页读」）。

use serde_json::{Value, json};

use super::{latest_arrived, older_arrived};
use crate::core::Push;
use crate::core::replay::Replay;
use crate::core::serve::Link;

/// 你说的一句。
fn said(seq: u64, text: &str) -> Value {
    json!({"seq": seq, "at": "2026-10-08T08:00:00Z", "kind": "message.user", "by": {"kind": "person"},
        "body": {"blocks": [{"type": "text", "text": text}]}})
}

/// 第 `turn` 轮开始，由 `trigger` 那一句开的。
fn started(seq: u64, turn: u64, trigger: u64) -> Value {
    json!({"seq": seq, "at": "2026-10-08T08:00:01Z", "kind": "turn.started", "turn": turn,
        "by": {"kind": "kernel"}, "body": {"trigger": trigger}})
}

/// 推送里你说的话，照先后。
fn spoken(pushes: &[Push]) -> Vec<u64> {
    pushes
        .iter()
        .filter_map(|p| match p {
            Push::Said { seq, .. } => Some(*seq),
            _ => None,
        })
        .collect()
}

/// 最新一页：第 2 轮的触发消息（4）在切点（6）前面，带在最前面。
fn latest_page() -> Value {
    json!({"events": [said(4, "第 2 句"), started(6, 2, 4), said(8, "第 3 句"), started(9, 3, 8)],
        "first": 6, "last": 9, "more": true,
        "jobs": [{"job": "j1", "what": "command", "title": "开着的服务"}]})
}

fn opened(session: &str) -> Link {
    let mut link = Link::default();
    link.replays.insert(session.to_string(), Replay::default());
    link
}

#[test]
fn the_latest_page_reads_like_a_replay_and_remembers_where_it_stopped() {
    let mut link = opened("s");
    let (pushes, more) = latest_arrived(&mut link, "s", &latest_page());
    assert!(more);
    assert_eq!(spoken(&pushes), [4, 8]);
    // 页里带的跨页任务先记进任务表，排在页里的事件前面。
    assert!(matches!(&pushes[0], Push::JobEarlier(j) if j.job == "j1" && j.title == "开着的服务"));
    // 接着 `subscribe {after: last}`；还在补发中，钟没回到现在。
    assert_eq!(link.seen.get("s"), Some(&9));
    assert!(link.replays.contains_key("s"));
    assert!(!pushes.iter().any(|p| matches!(p, Push::Clock(None))));
}

#[test]
fn the_earlier_page_skips_what_is_already_drawn_and_moves_the_mark() {
    let mut link = opened("s");
    latest_arrived(&mut link, "s", &latest_page());
    link.replays.clear();
    // 更早的一页：第 1 轮整轮，第 2 轮的触发消息（4）也在里面，已经画过。
    let earlier = json!({"events": [said(1, "第 1 句"), started(2, 1, 1), said(4, "第 2 句")],
        "first": 1, "last": 4, "more": false});
    let (pushes, more) = older_arrived(&mut link, "s", &earlier);
    assert!(!more);
    assert_eq!(spoken(&pushes), [1]);
    // 单独读的：钟不回到现在，读到的最后一条不动（只接新的那条线照旧）。
    assert!(!pushes.iter().any(|p| matches!(p, Push::Clock(None))));
    assert_eq!(link.seen.get("s"), Some(&9));
}

#[test]
fn an_empty_session_has_no_earlier_page() {
    let mut link = opened("s");
    let (pushes, more) = latest_arrived(&mut link, "s", &json!({"events": [], "more": true}));
    assert!(pushes.is_empty());
    assert!(!more, "没有事件就没有 first，往前要不了");
    assert!(!link.pages.contains_key("s"));
}

#[test]
fn an_earlier_page_for_a_session_left_behind_gives_nothing() {
    let mut link = opened("s");
    latest_arrived(&mut link, "s", &latest_page());
    link.pages.clear();
    let earlier = json!({"events": [said(1, "第 1 句")], "first": 1, "last": 1, "more": false});
    let (pushes, more) = older_arrived(&mut link, "s", &earlier);
    assert!(pushes.is_empty() && !more);
    assert!(!link.pages.contains_key("s"), "切走了的不再记");
}
