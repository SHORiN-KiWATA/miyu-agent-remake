//! 量尺（施工 6-4）：`history` 每次从头读整份日志，一个五万条事件的日志要多久。只量不断言，跑法：
//! `cargo test --release -p miyu-session --test history_speed -- --ignored --nocapture`。

mod support;

use std::path::Path;
use std::time::Instant;

use miyu_kernel::event::Event;
use miyu_kernel::time::UtcOffset;
use miyu_store::log::{SEGMENT_LIMIT, SessionLog, read_segments};
use miyu_tool::{Call, Log, Progress, ReadLog};

use support::Scratch;

/// 照会话的目录一段一段读，和执行器交给调用的是同一个读法。
struct Dir(std::path::PathBuf);

impl ReadLog for Dir {
    fn read(&self, each: &mut dyn FnMut(Vec<Event>) -> bool) -> Result<(), String> {
        read_segments(&self.0, each).map_err(|error| error.to_string())
    }
}

/// 第 `seq` 条：人说的一句，两百来个字。
fn said(seq: u64) -> Event {
    let text = format!(
        "第 {seq} 句：{}",
        "今天把数据库那张表按会话分区，顺便整理迁移脚本。".repeat(8)
    );
    Event::from_line(
        &serde_json::json!({
            "seq": seq, "at": "2026-09-29T05:00:00.000Z", "kind": "message.user",
            "by": {"kind": "person", "account": "alice"},
            "body": {"blocks": [{"type": "text", "text": text}]},
        })
        .to_string(),
    )
    .expect("读得懂")
}

#[tokio::test]
#[ignore = "量尺，只量不断言"]
async fn how_long_fifty_thousand_events_take() {
    let scratch = Scratch::new();
    let dir = scratch.0.join("s");
    let mut log = SessionLog::create(&dir, SEGMENT_LIMIT).expect("建得了");
    let created = Event::from_line(r#"{"seq":1,"at":"2026-09-29T05:00:00.000Z","kind":"session.created","by":{"kind":"kernel"},"body":{"owner":"alice","venue":"local","policy":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","permission":{"level":"workspace","read_only":false}}}"#)
        .expect("读得懂");
    log.append(&[created]).expect("写得进");
    for batch in 0..50 {
        let events: Vec<Event> = (0..1000).map(|k| said(2 + batch * 1000 + k)).collect();
        log.append(&events).expect("写得进");
    }
    let bytes: u64 = std::fs::read_dir(&dir)
        .expect("读得了")
        .map(|entry| entry.expect("读得了").metadata().expect("读得了").len())
        .sum();
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let tools = miyu_basesystem::tools(&resources).expect("出厂的资源读得出来");
    let history = tools
        .into_iter()
        .find(|tool| tool.spec().name == "history")
        .expect("有 history");
    for (what, args) in [
        ("找，命中一条", serde_json::json!({"query": "第 12345 句"})),
        ("找，全都命中", serde_json::json!({"query": "分区"})),
        ("读，从第 40000 条起", serde_json::json!({"from": 40000})),
    ] {
        let call = Call {
            args: args.to_string(),
            cwd: String::new(),
            home: None,
            data_root: None,
            seen: Default::default(),
            stop: Default::default(),
            sandbox: None,
            log: Some(Log::new(Dir(dir.clone()))),
            offset: UtcOffset::UTC,
            agents: None,
            messages: None,
            jobs: None,
            sessions: None,
            usage: None,
        };
        let started = Instant::now();
        let done = history.run(call, Progress::new(|_| {})).await;
        let took = started.elapsed();
        assert!(!done.error);
        println!(
            "{what}：{} 毫秒（日志 {} 字节，50001 条）",
            took.as_millis(),
            bytes
        );
    }
}
