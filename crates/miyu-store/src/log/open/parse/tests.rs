//! 分块读和一行行读交回的一样：照先后、行号对、读不成的停在第一处。

use super::{SPLIT_AT, lines};

/// 第 `seq` 条的一行原文：alice 说的一句（同 `log/tests.rs` 的写法）。
fn line(seq: u64) -> String {
    format!(
        r#"{{"seq":{seq},"at":"2026-09-25T07:00:00.000Z","kind":"message.user","by":{{"kind":"person","account":"alice"}},"body":{{"blocks":[{{"type":"text","text":"第 {seq} 句"}}]}}}}"#
    )
}

#[test]
fn many_lines_come_back_in_order_with_their_numbers() {
    let texts: Vec<String> = (1..=SPLIT_AT as u64 * 3).map(line).collect();
    let bytes: Vec<&[u8]> = texts.iter().map(String::as_bytes).collect();
    let parsed = lines(&bytes);
    assert_eq!(parsed.len(), texts.len());
    for (k, one) in parsed.iter().enumerate() {
        let (number, event) = one
            .as_ref()
            .unwrap_or_else(|(n, why)| panic!("第 {n} 行：{why}"));
        assert_eq!(*number, k + 1);
        assert_eq!(event.seq.get(), k as u64 + 1);
    }
}

#[test]
fn reading_stops_at_the_first_broken_line() {
    let mut texts: Vec<String> = (1..=SPLIT_AT as u64 * 3).map(line).collect();
    let broken = SPLIT_AT * 2 + 7;
    texts[broken - 1] = "{not json".to_string();
    texts[broken + 100] = "{also not json".to_string();
    let bytes: Vec<&[u8]> = texts.iter().map(String::as_bytes).collect();
    let parsed = lines(&bytes);
    assert_eq!(parsed.len(), broken, "第一处读不成的后面的不交");
    let (number, why) = parsed.last().expect("有").as_ref().expect_err("读不成");
    assert_eq!(*number, broken);
    assert!(why.starts_with("not readable"), "{why}");
    assert!(parsed[..broken - 1].iter().all(Result::is_ok));
}
