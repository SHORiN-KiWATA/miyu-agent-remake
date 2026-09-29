//! 找（施工 6-4）：每个词都要、不分大小写、中文不分词；新的在前；一条一行，摘第一处命中前后一段，空白收成一个空格；
//! 往前翻。

use super::*;

/// 几句人说的话，第 `k` 句是第 `k + 1` 条，隔一分钟一句。
fn said(texts: &[&str]) -> Vec<Vec<Event>> {
    let events = texts
        .iter()
        .enumerate()
        .map(|(k, text)| {
            let at = format!("2026-09-29T05:{:02}:00.000Z", k);
            event(
                k as u64 + 1,
                &at,
                "message.user",
                None,
                alice(),
                json!({"blocks": [{"type": "text", "text": text}]}),
            )
        })
        .collect();
    vec![events]
}

#[tokio::test]
async fn every_word_must_be_there_in_any_case() {
    let log = || {
        said(&[
            "数据库那张表改成按会话分区",
            "Partition the TABLE by session",
            "the table stays",
            "别再用一张大表",
        ])
    };
    let found = |query: &str| {
        let log = log();
        let query = query.to_string();
        async move { seqs(&run_on(log, json!({ "query": query })).await) }
    };
    // 新的在前。
    assert_eq!(found("table").await, [3, 2]);
    assert_eq!(found("table session").await, [2]);
    assert_eq!(found("PARTITION table").await, [2]);
    // 中文不分词：照原样比。
    assert_eq!(found("分区").await, [1]);
    assert_eq!(found("大表").await, [4]);
    assert_eq!(found("表").await, [4, 1]);
    assert_eq!(found("table 大表").await, Vec::<u64>::new());
}

#[tokio::test]
async fn a_line_shows_the_first_hit_with_its_surroundings() {
    let long = format!("{}needle{}", "a ".repeat(200), " b".repeat(200));
    let done = run_on(
        said(&["short one\n  with   gaps", &long]),
        json!({"query": "needle"}),
    )
    .await;
    let line = text(&done);
    let line = line.strip_suffix('\n').expect("一行");
    let (head, snippet) = line.split_once(": ").expect("头和摘的一段");
    assert_eq!(head, "#2 2026-09-29 14:01 user");
    assert!(
        snippet.starts_with('…') && snippet.ends_with('…'),
        "{snippet}"
    );
    let inner: String = snippet.trim_matches('…').to_string();
    assert_eq!(inner.chars().count(), 200);
    assert!(inner.contains("needle"));
    // 命中大约在中间。
    let at = inner.find("needle").unwrap();
    assert!((80..=110).contains(&at), "{at}");
    // 短的整条给，空白收成一个空格。
    let done = run_on(
        said(&["short one\n  with   gaps"]),
        json!({"query": "WITH"}),
    )
    .await;
    assert_eq!(
        text(&done),
        "#1 2026-09-29 14:00 user: short one with gaps\n"
    );
}

#[tokio::test]
async fn older_results_are_one_page_further() {
    let texts: Vec<String> = (1..=5).map(|k| format!("note {k}")).collect();
    let texts: Vec<&str> = texts.iter().map(String::as_str).collect();
    let done = run_on(said(&texts), json!({"query": "note", "limit": 2})).await;
    assert_eq!(
        text(&done),
        "#5 2026-09-29 14:04 user: note 5\n#4 2026-09-29 14:03 user: note 4\n(Showing 2 of 5 results. Use to=3 to see older ones.)\n"
    );
    assert!(
        human(&done).contains("history/found") && human(&done).contains("\"5\""),
        "{}",
        human(&done)
    );
    let done = run_on(said(&texts), json!({"query": "note", "limit": 2, "to": 3})).await;
    assert_eq!(seqs(&done), [3, 2]);
    assert!(text(&done).ends_with("(Showing 2 of 3 results. Use to=1 to see older ones.)\n"));
    let done = run_on(said(&texts), json!({"query": "note", "to": 1})).await;
    assert_eq!(text(&done), "#1 2026-09-29 14:00 user: note 1\n");
}

#[tokio::test]
async fn nothing_found_is_not_an_error() {
    let done = run_on(said(&["hello"]), json!({"query": "bye"})).await;
    assert!(!done.error);
    assert_eq!(text(&done), "No entries found\n");
    assert!(human(&done).contains("history/none"), "{}", human(&done));
}
