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
fn compactions_and_cache_breaks_are_counted_from_their_events() {
    let compacted = event(
        r#"{"seq":20,"at":"2026-10-08T01:00:00.000Z","kind":"context.compacted","by":{"kind":"kernel"},"body":{"upto":19,"summary":"s"}}"#,
    );
    let broke = called(
        22,
        r#","endpoint":"deepseek","model":"deepseek-v4","first_difference":{"part":"message","index":0,"role":"user"}"#,
    );
    let unsent_broke = called(24, r#","first_difference":{"part":"tools"}"#);
    let tally = Tally::of(&[compacted.clone(), broke, unsent_broke, compacted]);
    assert_eq!(tally.compactions, 2);
    assert_eq!(tally.cache_breaks, 2, "带 first_difference 的都算");
    assert_eq!(tally.requests, 1);
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
