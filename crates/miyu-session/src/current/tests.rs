//! 累计的用量和计数（施工 9-6 上）：口径同用量汇总的一行；没发出去的不算请求；有用量没金额的算一次「没金额」；压缩、缓存断了
//! 照事件数。

use miyu_kernel::event::Event;

use super::*;

/// 一条事件，照日志的写法。
fn event(line: &str) -> Event {
    serde_json::from_str(line).expect("合写法")
}

fn called(seq: u64, extra: &str) -> Event {
    event(&format!(
        r#"{{"seq":{seq},"at":"2026-10-08T01:00:00.000Z","kind":"model.called","turn":2,"by":{{"kind":"kernel"}},"body":{{"seen":{},"messages":2,"result":"ok"{extra}}}}}"#,
        seq - 1
    ))
}

#[test]
fn requests_usage_and_amounts_follow_the_usage_index() {
    let sent = r#","endpoint":"deepseek","model":"deepseek-v4","request":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa""#;
    let usage = r#","usage":{"uncached":100,"cache_read":40,"cache_write":10,"output":7}"#;
    let events = [
        called(
            3,
            &format!(
                r#"{sent}{usage},"cost":{{"currency":"USD","amount":0.5,"price":{{"input":0.15,"output":0.6}},"multiplier":1,"source":"catalog:deepseek/deepseek-v4"}}"#
            ),
        ),
        called(
            5,
            &format!(
                r#"{sent}{usage},"cost":{{"currency":"USD","amount":0.25,"price":{{"input":0.15,"output":0.6}},"multiplier":1,"source":"catalog:deepseek/deepseek-v4"}}"#
            ),
        ),
        called(
            7,
            &format!(
                r#"{sent}{usage},"cost":{{"currency":"CNY","amount":1.0,"price":{{"input":1.0,"output":2.0}},"multiplier":1,"source":"catalog:deepseek/deepseek-v4"}}"#
            ),
        ),
        called(9, &format!("{sent}{usage}")),
        called(11, sent),
        called(13, ""),
    ];
    let tally = Tally::of(&events);
    assert_eq!(tally.requests, 5, "没发出去的（没有供应商、模型）不算");
    assert_eq!(
        (
            tally.usage.uncached,
            tally.usage.cache_read,
            tally.usage.cache_write,
            tally.usage.output
        ),
        (400, 160, 40, 28),
        "没报用量的记 0"
    );
    assert_eq!(tally.amounts.get("USD"), Some(&0.75));
    assert_eq!(tally.amounts.get("CNY"), Some(&1.0));
    assert_eq!(tally.unpriced, 1, "有用量、没金额的一次；没用量的不算");
    let mut split = Tally::of(&events[..2]);
    split.add(&events[2..]);
    assert_eq!(split, tally, "一批批加和从头算一样");
}

#[test]
fn compactions_are_counted_and_only_unexpected_cache_breaks() {
    // 口径照终端的侧边栏（施工 9-6 再补）。
    let diff = r#","first_difference":{"part":"message","index":0,"role":"user"}"#;
    let compacted = event(
        r#"{"seq":6,"at":"2026-10-08T01:00:00.000Z","kind":"context.compacted","by":{"kind":"kernel"},"body":{"upto":5,"summary":"s"}}"#,
    );
    let reverted = event(
        r#"{"seq":16,"at":"2026-10-08T01:00:00.000Z","kind":"turn.reverted","by":{"kind":"person","account":"alice"},"body":{"turns":[12]}}"#,
    );
    let events = [
        called(3, ""),
        called(5, diff),
        compacted,
        called(8, &format!(r#"{diff},"compaction":"auto""#)),
        called(10, diff),
        called(12, diff),
        called(14, &format!(r#"{diff},"purpose":"recap""#)),
        called(9, diff),
        reverted,
        called(18, diff),
        called(20, diff),
    ];
    let tally = Tally::of(&events);
    assert_eq!(tally.compactions, 1);
    assert_eq!(
        tally.cache_breaks, 3,
        "第 5 条断了算；摘要请求不算、不用掉免数；压缩以后第一个（第 10 条）免；第 12 条算；回顾不算；看到的比之前少的\
         （老日志的摘要请求）不算；撤销以后第一个（第 18 条）免；第 20 条算"
    );
    let mut split = Tally::of(&events[..5]);
    split.add(&events[5..]);
    assert_eq!(
        split, tally,
        "一批批加和从头算一样：免数、最远看到第几条跨批记着"
    );
}

#[test]
fn main_requests_leave_out_the_helpers() {
    let sent = r#","endpoint":"deepseek","model":"deepseek-v4","usage":{"uncached":100,"cache_read":40,"cache_write":0,"output":7}"#;
    let events = [
        called(3, sent),
        called(5, &format!(r#"{sent},"purpose":"recap""#)),
        called(7, &format!(r#"{sent},"compaction":"auto""#)),
    ];
    let tally = Tally::of(&events);
    assert_eq!(tally.requests, 3);
    assert_eq!(tally.usage.uncached, 300, "累计的算全部");
    assert_eq!(
        (
            tally.main.uncached,
            tally.main.cache_read,
            tally.main.output
        ),
        (200, 80, 14),
        "主请求不算回顾这类辅助请求，摘要请求算"
    );
}
