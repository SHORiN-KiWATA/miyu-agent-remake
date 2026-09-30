//! 量尺（施工 3-8 七补）：250 个会话、日志共十几兆，列一次要多久。整份读（没有索引，就是改之前的读法）、照日志填索引的那一
//! 次、之后照索引读、有几个会话多写了几条以后。只量不断言，跑法：
//! `cargo test --release -p miyu-endpoint --lib speed -- --ignored --nocapture`。

use std::fs;
use std::time::Instant;

use serde_json::json;

use super::*;

/// 多少个会话。
const SESSIONS: u64 = 250;
/// 一个会话多少条事件：一条两百来字节，一个会话五十来 KB。
const EVENTS: u64 = 130;

fn line(seq: u64) -> String {
    let body = json!({"blocks": [{"type": "text", "text": format!("第 {seq} 句：{}", "今天把会话列表换成读索引。".repeat(6))}]});
    json!({"seq": seq, "at": "2026-09-25T07:00:00.000Z", "kind": "message.user", "by": {"kind": "person", "account": "alice"}, "body": body}).to_string()
}

fn id(n: u64) -> SessionId {
    SessionId::parse(&format!("0192f3a0-{n:04x}-7abc-8def-001122334455")).expect("合写法")
}

#[test]
#[ignore = "量尺，只量不断言"]
fn how_long_listing_two_hundred_fifty_sessions_takes() {
    let site = Site::new(&[]);
    let mut bytes = 0;
    for n in 0..SESSIONS {
        let dir = site.1.session_dir(&alice(), &id(n));
        fs::create_dir_all(&dir).expect("建得了");
        let mut text = format!("{CREATED}\n");
        for seq in 2..=EVENTS {
            text.push_str(&line(seq));
            text.push('\n');
        }
        bytes += text.len();
        fs::write(dir.join("000000000001.jsonl"), text).expect("写得进");
    }
    let (index, _) = SessionIndex::open(&site.1.index(&alice()).join(FILE));
    let once = |index: Option<&SessionIndex>| {
        let started = Instant::now();
        let listed = scan(
            &site.1,
            &alice(),
            index,
            &BTreeSet::new(),
            |_| true,
            None,
            &Stop::default(),
        )
        .expect("读得了");
        assert_eq!(listed.len() as u64, SESSIONS);
        started.elapsed().as_secs_f64() * 1000.0
    };
    println!("{SESSIONS} 个会话，日志共 {bytes} 字节");
    for k in 1..=3 {
        println!("整份读（改之前）第 {k} 次：{:.1} 毫秒", once(None));
    }
    println!("照日志填索引：{:.1} 毫秒", once(Some(&index)));
    for k in 1..=3 {
        println!("照索引读第 {k} 次：{:.1} 毫秒", once(Some(&index)));
    }
    for n in 0..10 {
        let path = site
            .1
            .session_dir(&alice(), &id(n))
            .join("000000000001.jsonl");
        let mut text = fs::read_to_string(&path).expect("读得了");
        for seq in EVENTS + 1..=EVENTS + 5 {
            text.push_str(&line(seq));
            text.push('\n');
        }
        fs::write(path, text).expect("写得进");
    }
    println!("十个会话各多了五条：{:.1} 毫秒", once(Some(&index)));
    println!("之后：{:.1} 毫秒", once(Some(&index)));
}
