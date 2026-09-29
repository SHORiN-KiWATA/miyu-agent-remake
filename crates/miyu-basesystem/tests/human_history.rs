//! `history` 交回的给人看的说法（施工 6-4）：找到几条、读了第几到第几条、没有找到、读不了记录；中文、英文两份字里
//! 都有，换得出字，显示名也有。

mod support;

use miyu_kernel::event::{Event, Said};

use support::{Site, check, human, readable, said};

/// 人说的第 `seq` 条。
fn message(seq: u64, text: &str) -> Event {
    Event::from_line(
        &serde_json::json!({
            "seq": seq, "at": "2026-09-29T05:00:00.000Z", "kind": "message.user",
            "by": {"kind": "person", "account": "alice"},
            "body": {"blocks": [{"type": "text", "text": text}]},
        })
        .to_string(),
    )
    .expect("手写的事件读得懂")
}

#[tokio::test]
async fn every_history_outcome_says_something_people_can_read() {
    let site = Site::new();
    let log = || vec![message(2, "按会话分区"), message(3, "别再用一张大表")];
    let mut checked: Vec<Said> = Vec::new();
    let run = |args: serde_json::Value| site.done_with_log("history", args, log());
    check(
        &mut checked,
        human(run(serde_json::json!({"query": "表"})).await),
        said("history/found").with("count", "1"),
    );
    check(
        &mut checked,
        human(run(serde_json::json!({})).await),
        said("history/read").with("from", "2").with("to", "3"),
    );
    check(
        &mut checked,
        human(run(serde_json::json!({"query": "索引"})).await),
        said("history/none"),
    );
    check(
        &mut checked,
        human(site.done("history", serde_json::json!({})).await),
        said("history/no-log").with("error", "this call has no log"),
    );
    readable(&checked, &["history"]);
}
