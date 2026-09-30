//! 读（施工 6-4）：照先后，一条头一行、下面是原文；工具调用写成 `→ <工具名> <参数原文>`；往下翻；整页上限、一条就
//! 超过上限的截掉。

use super::*;

use crate::history::page::PAGE;

#[tokio::test]
async fn entries_come_in_order_with_calls_written_out() {
    let done = run_on(sample(), json!({"from": 64, "to": 71})).await;
    assert_eq!(
        text(&done),
        concat!(
            "#64 2026-09-25 16:33 user\n把仓库里的 .editorconfig 装到我的家目录\n\n",
            "#67 2026-09-25 16:33 assistant\n我把它复制过去。\n",
            "→ shell {\"command\":\"cp .editorconfig ~/.editorconfig\"}\n\n",
            "#71 2026-09-25 16:33 tool\nThe call was not run: the user denied it and said \"家目录里已经有一份了，别覆盖\".\n",
        )
    );
    assert!(human(&done).contains("history/read"), "{}", human(&done));
}

#[tokio::test]
async fn the_next_page_starts_after_the_last_shown() {
    let done = run_on(sample(), json!({"limit": 2})).await;
    assert_eq!(seqs(&done), [52, 54]);
    assert!(
        text(&done).ends_with("(Showing entries 52-54. Use from=55 to continue.)\n"),
        "{}",
        text(&done)
    );
    assert!(
        human(&done).contains("\"52\"") && human(&done).contains("\"54\""),
        "{}",
        human(&done)
    );
    let done = run_on(sample(), json!({"limit": 3, "from": 56})).await;
    assert_eq!(seqs(&done), [63, 64, 67]);
    assert!(text(&done).ends_with("(Showing entries 63-67. Use from=68 to continue.)\n"));
    // 读到最后一页，不写往下翻那一句。
    let done = run_on(sample(), json!({"from": 80})).await;
    assert_eq!(seqs(&done), [81, 82]);
    assert!(!text(&done).contains("Showing"));
}

/// 几条人说的话，每条是 `sizes` 里那么多个「字」。
fn long(sizes: &[usize]) -> Vec<Vec<Event>> {
    vec![
        sizes
            .iter()
            .enumerate()
            .map(|(k, size)| {
                let at = format!("2026-09-29T05:{:02}:00.000Z", k);
                let text = "字".repeat(*size);
                event(
                    k as u64 + 1,
                    &at,
                    "message.user",
                    None,
                    alice(),
                    json!({"blocks": [{"type": "text", "text": text}]}),
                )
            })
            .collect(),
    ]
}

#[tokio::test]
async fn a_page_stays_within_its_characters() {
    // 两条加起来放不下：只给第一条，第二条在下一页。
    let done = run_on(long(&[20_000, 20_000]), json!({})).await;
    assert_eq!(seqs(&done), [1]);
    assert!(text(&done).chars().count() < PAGE + 100);
    assert!(text(&done).ends_with("(Showing entries 1-1. Use from=2 to continue.)\n"));
    // 一条就超过的：截到放得下，写明一共多少字。
    let done = run_on(long(&[40_000, 10]), json!({})).await;
    let page = text(&done);
    assert!(
        page.chars().count() <= PAGE + 200,
        "{}",
        page.chars().count()
    );
    let head = "#1 2026-09-29 14:00 user\n";
    let shown = PAGE - head.chars().count() - 1;
    assert!(
        page.contains(&format!(
            "(This entry is cut at {shown} of its 40000 characters.)\n"
        )),
        "{}",
        &page[page.len() - 200..]
    );
    assert!(page.ends_with("(Showing entries 1-1. Use from=2 to continue.)\n"));
    let done = run_on(long(&[40_000, 10]), json!({"from": 2})).await;
    assert_eq!(seqs(&done), [2]);
}

#[tokio::test]
async fn a_page_has_twenty_entries_unless_told_otherwise() {
    let done = run_on(long(&[1; 25]), json!({})).await;
    assert_eq!(seqs(&done), (1..=20).collect::<Vec<_>>());
    assert!(text(&done).ends_with("(Showing entries 1-20. Use from=21 to continue.)\n"));
}
