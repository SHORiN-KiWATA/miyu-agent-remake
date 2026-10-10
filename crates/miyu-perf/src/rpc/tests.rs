//! 请求的编号整个进程里不重复：核心照编号认重发，连接换了也照认（施工 V-1 试跑时撞过：每个连接从 1 数起，造会话交回了
//! 上一次的、说话不开轮）。

use super::*;

#[test]
fn ids_never_repeat() {
    let ids: Vec<String> = (0..1000).map(|_| next_id()).collect();
    let mut unique = ids.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), ids.len());
    assert!(ids.iter().all(|id| id.starts_with("perf-")));
}
