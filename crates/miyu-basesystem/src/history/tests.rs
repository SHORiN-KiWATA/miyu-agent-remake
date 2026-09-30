//! `history` 的测试（施工 6-4）：哪些算一条（撤掉的、撤回的不算，以前的摘要算）；四种筛；时刻的写法和时区；参数
//! 不对；读不了日志；叫停。找在 `tests/find.rs`，读在 `tests/read.rs`，人切权限级别在 `tests/permission.rs`（施工 2-7 补）。
//!
//! 日志用样本会话（`docs/designs/samples/events/`）第 86 条以前的：第 42 轮撤掉了、第 54 条压缩、第 59 条撤回、
//! 第 76 轮撤了又恢复。还算数的是 52、54、55、63、64、67、71、72、75、77、81、82；52、63 是人切权限级别（施工 2-7 补）。

mod find;
mod harness;
mod peers;
mod permission;
mod read;

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use miyu_kernel::event::{Body, Event};
use miyu_kernel::time::UtcOffset;
use miyu_tool::{Call, Done, Log, Progress, ReadLog, Stop, Tool};

use super::History;
use crate::common::Common;

/// 源码树里的资源目录。
fn resources() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources")
}

/// 假的日志：几段事件，照先后交。`broken` 的读不了。
struct Segments {
    segments: Vec<Vec<Event>>,
    broken: Option<String>,
}

impl ReadLog for Segments {
    fn read(&self, each: &mut dyn FnMut(Vec<Event>) -> bool) -> Result<(), String> {
        if let Some(why) = &self.broken {
            return Err(why.clone());
        }
        for segment in &self.segments {
            if !each(segment.clone()) {
                break;
            }
        }
        Ok(())
    }
}

/// 样本会话第 86 条以前的事件，照序号排，切成三段。
fn sample() -> Vec<Vec<Event>> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/designs/samples/events");
    let mut events: Vec<Event> = std::fs::read_dir(dir)
        .expect("样本目录在")
        .flat_map(|entry| {
            let text = std::fs::read_to_string(entry.expect("读得了").path()).expect("读得了");
            text.lines()
                .map(|line| Event::from_line(line).expect("样本读得懂"))
                .collect::<Vec<_>>()
        })
        .filter(|event| event.seq.get() <= 86)
        // 带 `parent` 的 `session.created` 是样本会话派的子代理自己日志里的第一条，不是这个会话的（施工 7-1）。
        .filter(|event| !matches!(&event.body, Body::SessionCreated(created) if created.parent.is_some()))
        .collect();
    events.sort_by_key(|event| event.seq);
    let (first, rest) = events.split_at(20);
    let (second, third) = rest.split_at(15);
    vec![first.to_vec(), second.to_vec(), third.to_vec()]
}

/// 手写的一条事件。
fn event(seq: u64, at: &str, kind: &str, turn: Option<u64>, by: Value, body: Value) -> Event {
    let mut line = json!({"seq": seq, "at": at, "kind": kind, "by": by, "body": body});
    if let Some(turn) = turn {
        line["turn"] = json!(turn);
    }
    Event::from_line(&line.to_string()).expect("手写的事件读得懂")
}

/// alice。
fn alice() -> Value {
    json!({"kind": "person", "account": "alice"})
}

/// 她（模型）。
fn model() -> Value {
    json!({"kind": "model", "endpoint": "deepseek", "model": "deepseek-v4"})
}

/// 一次调用：参数 `args`，日志 `log`，时区比 UTC 早 `minutes` 分钟。
fn call(args: Value, log: Option<Log>, minutes: i32) -> Call {
    Call {
        args: args.to_string(),
        cwd: String::new(),
        home: None,
        data_root: None,
        seen: Default::default(),
        stop: Stop::default(),
        sandbox: None,
        log,
        offset: UtcOffset::from_minutes(minutes).expect("在范围里"),
        agents: None,
        messages: None,
        jobs: None,
    }
}

/// 在这几段日志上照 `args` 跑一次，时区 +09:00。
async fn run_on(segments: Vec<Vec<Event>>, args: Value) -> Done {
    run(call(args, Some(log(segments)), 540)).await
}

fn log(segments: Vec<Vec<Event>>) -> Log {
    Log::new(Segments {
        segments,
        broken: None,
    })
}

async fn run(call: Call) -> Done {
    let resources = resources();
    let tool = History::load(
        &resources,
        Common::load(&resources).expect("共用的字读得出来"),
    )
    .expect("history 的字读得出来");
    tool.run(call, Progress::new(|_| {})).await
}

/// 给她看的字。
fn text(done: &Done) -> String {
    done.blocks
        .iter()
        .map(|block| match block {
            miyu_kernel::block::Block::Text(text) => text.text.clone(),
            other => panic!("只有字：{other:?}"),
        })
        .collect()
}

/// 读出来的每一条的序号：「读」的头一行是 `#<序号> …`。
fn seqs(done: &Done) -> Vec<u64> {
    text(done)
        .lines()
        .filter_map(|line| line.strip_prefix('#'))
        .map(|rest| rest.split(' ').next().unwrap().parse().unwrap())
        .collect()
}

/// 给人看的那一句的编号和字段。
fn human(done: &Done) -> String {
    let said = done.human.as_ref().expect("带着说法");
    format!("{said:?}")
}

#[tokio::test]
async fn undone_and_withdrawn_entries_do_not_count_but_the_summary_does() {
    let done = run_on(sample(), json!({})).await;
    assert!(!done.error);
    assert_eq!(
        seqs(&done),
        [52, 54, 55, 63, 64, 67, 71, 72, 75, 77, 81, 82]
    );
    // 撤掉的第 42 轮、撤回的第 59 条找不到；以前的摘要找得到，算她说的。
    let done = run_on(sample(), json!({"query": "README"})).await;
    assert_eq!(text(&done), "No entries found\n");
    let done = run_on(sample(), json!({"query": "src"})).await;
    assert_eq!(seqs(&done), [54]);
    assert!(text(&done).starts_with(
        "#54 2026-09-25 16:30 assistant: The user asked to look at the src directory."
    ));
}

#[tokio::test]
async fn entries_can_be_picked_by_number_time_and_who() {
    let pick = |args: Value| async move { seqs(&run_on(sample(), args).await) };
    assert_eq!(pick(json!({"by": "user"})).await, [52, 55, 63, 64, 75]);
    assert_eq!(pick(json!({"by": "tool"})).await, [71, 81]);
    assert_eq!(pick(json!({"from": 64, "to": 72})).await, [64, 67, 71, 72]);
    assert_eq!(pick(json!({"from": "75"})).await, [75, 77, 81, 82]);
    // 16:33 那一分钟里的四条（照 +09:00）。
    let minute = json!({"since": "2026-09-25 16:33", "until": "2026-09-25 16:33"});
    assert_eq!(pick(minute).await, [64, 67, 71, 72]);
    assert_eq!(
        pick(json!({"until": "2026-09-25 16:31"})).await,
        [52, 54, 55]
    );
    // 第 55 条正好在 16:31:00：到 16:30 那一分钟完为止的不含它。
    assert_eq!(pick(json!({"until": "2026-09-25 16:30"})).await, [52, 54]);
    assert_eq!(
        pick(json!({"since": "2026-09-25T16:40:00"})).await,
        [75, 77, 81, 82]
    );
    // 只写日期：那一整天。
    assert_eq!(
        pick(json!({"since": "2026-09-25", "until": "2026-09-25"}))
            .await
            .len(),
        12
    );
    assert_eq!(
        pick(json!({"since": "2026-09-26"})).await,
        Vec::<u64>::new()
    );
    // 几种筛一起。
    assert_eq!(
        pick(json!({"by": "assistant", "since": "2026-09-25 16:33"})).await,
        [67, 72, 77, 82]
    );
}

#[tokio::test]
async fn times_are_written_in_the_session_time_zone() {
    let india = run(call(
        json!({"from": 54, "to": 54}),
        Some(log(sample())),
        330,
    ))
    .await;
    assert!(
        text(&india).starts_with("#54 2026-09-25 13:00 assistant\n"),
        "{}",
        text(&india)
    );
    let west = run(call(
        json!({"from": 54, "to": 54}),
        Some(log(sample())),
        -300,
    ))
    .await;
    assert!(
        text(&west).starts_with("#54 2026-09-25 02:30 assistant\n"),
        "{}",
        text(&west)
    );
    // 筛的时刻也照这个时区读。
    let early = run(call(
        json!({"until": "2026-09-25 02:30"}),
        Some(log(sample())),
        -300,
    ))
    .await;
    assert_eq!(seqs(&early), [52, 54]);
}

#[tokio::test]
async fn bad_arguments_say_what_is_wrong() {
    for (args, says) in [
        (
            json!({"limit": 0}),
            "limit must be a positive integer, got 0",
        ),
        (
            json!({"limit": -3}),
            "limit must be a positive integer, got -3",
        ),
        (
            json!({"by": "me"}),
            "by must be user, assistant or tool, got me",
        ),
        (json!({"from": "one"}), "invalid value"),
        (json!({"query": 7}), "invalid type"),
    ] {
        let done = run_on(sample(), args.clone()).await;
        assert!(done.error, "{args}");
        assert!(text(&done).contains(says), "{args}: {}", text(&done));
    }
    let done = run_on(sample(), json!({"since": "yesterday"})).await;
    assert!(done.error);
    assert_eq!(
        text(&done),
        "\"yesterday\" is not a time. Write it like 2026-09-29 14:00, or just 2026-09-29.\n"
    );
    for bad in [
        "2026-02-30",
        "2026-09-29 24:00",
        "2026-09-29 9:5",
        "2026/09/29",
        "26-09-29",
    ] {
        assert!(
            run_on(sample(), json!({ "until": bad })).await.error,
            "{bad}"
        );
    }
    // 别的参数不认，也不报错；当没传的几种写法。
    let done = run_on(
        sample(),
        json!({"verbose": true, "by": "", "query": "null"}),
    )
    .await;
    assert!(!done.error);
    assert_eq!(seqs(&done).len(), 12);
}

#[tokio::test]
async fn without_a_log_or_with_a_broken_one_it_says_so() {
    let done = run(call(json!({}), None, 0)).await;
    assert!(done.error);
    assert_eq!(
        text(&done),
        "Could not read the log: this call has no log\n"
    );
    let broken = Log::new(Segments {
        segments: Vec::new(),
        broken: Some("line 3 is not readable".to_string()),
    });
    let done = run(call(json!({}), Some(broken), 0)).await;
    assert!(done.error);
    assert_eq!(
        text(&done),
        "Could not read the log: line 3 is not readable\n"
    );
    assert!(human(&done).contains("history/no-log"), "{}", human(&done));
}

/// 数着交了几段的日志。
struct Counting(std::sync::Arc<std::sync::atomic::AtomicUsize>);

impl ReadLog for Counting {
    fn read(&self, each: &mut dyn FnMut(Vec<Event>) -> bool) -> Result<(), String> {
        for segment in sample() {
            self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if !each(segment) {
                break;
            }
        }
        Ok(())
    }
}

#[tokio::test]
async fn a_raised_flag_stops_before_the_next_segment() {
    let read = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let call = call(json!({}), Some(Log::new(Counting(read.clone()))), 0);
    call.stop.raise();
    let done = run(call).await;
    assert!(done.stopped);
    assert_eq!(
        read.load(std::sync::atomic::Ordering::Relaxed),
        1,
        "看到旗就不读下去"
    );
}

#[tokio::test]
async fn pictures_and_files_are_placeholders_and_thinking_is_left_out() {
    let events = vec![
        event(
            1,
            "2026-09-29T05:00:00.000Z",
            "session.created",
            None,
            json!({"kind": "kernel"}),
            json!({"owner": "alice", "venue": "local", "policy": "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855", "permission": {"level": "workspace", "read_only": false}}),
        ),
        event(
            2,
            "2026-09-29T05:01:00.000Z",
            "message.user",
            None,
            alice(),
            json!({"blocks": [
                {"type": "text", "text": "看这张图和这份表"},
                {"type": "image", "blob": "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855", "media_type": "image/png", "width": 2, "height": 2},
                {"type": "file", "blob": "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855", "name": "plan.pdf", "media_type": "application/pdf"}
            ]}),
        ),
        event(
            3,
            "2026-09-29T05:01:00.000Z",
            "turn.started",
            Some(3),
            json!({"kind": "kernel"}),
            json!({"trigger": 2}),
        ),
        // 只想了没说就被打断的：一个字都没有，不算。
        event(
            4,
            "2026-09-29T05:01:05.000Z",
            "message.assistant",
            Some(3),
            model(),
            json!({"blocks": [{"type": "reasoning", "text": "先想想"}], "seen": 3, "interrupted": true}),
        ),
        event(
            5,
            "2026-09-29T05:01:09.000Z",
            "message.assistant",
            Some(3),
            model(),
            json!({"blocks": [{"type": "reasoning", "text": "图里是表格"}, {"type": "text", "text": "看到了。"}], "seen": 3}),
        ),
    ];
    let done = run_on(vec![events], json!({})).await;
    assert_eq!(
        text(&done),
        "#2 2026-09-29 14:01 user\n看这张图和这份表\n[image]\n[file plan.pdf]\n\n#5 2026-09-29 14:01 assistant\n看到了。\n"
    );
    assert!(human(&done).contains("history/read"), "{}", human(&done));
}

#[tokio::test]
async fn her_own_lookups_are_not_entries() {
    let call = |seq: u64, text: Option<&str>| {
        let mut blocks = Vec::new();
        if let Some(text) = text {
            blocks.push(json!({"type": "text", "text": text}));
        }
        blocks.push(json!({"type": "tool_call", "call_id": format!("call_{seq}_1"), "name": "history", "args": "{\"query\":\"blue\"}"}));
        blocks.push(json!({"type": "tool_call", "call_id": format!("call_{seq}_2"), "name": "read", "args": "{\"file_path\":\"blue.txt\"}"}));
        event(
            seq,
            "2026-09-29T05:02:00.000Z",
            "message.assistant",
            Some(3),
            model(),
            json!({"blocks": blocks, "seen": seq - 1}),
        )
    };
    let result = |seq: u64, call: &str, text: &str| {
        event(
            seq,
            "2026-09-29T05:02:01.000Z",
            "tool.result",
            Some(3),
            json!({"kind": "tool", "call_id": call}),
            json!({"call_id": call, "status": "ok", "blocks": [{"type": "text", "text": text}]}),
        )
    };
    let events = vec![
        event(
            2,
            "2026-09-29T05:01:00.000Z",
            "message.user",
            None,
            alice(),
            json!({"blocks": [{"type": "text", "text": "blue whale"}]}),
        ),
        call(4, Some("我翻一下。")),
        result(5, "call_4_1", "#2 2026-09-29 14:01 user: blue whale"),
        result(6, "call_4_2", "1\tblue"),
        call(7, None),
        result(8, "call_7_1", "#2 2026-09-29 14:01 user: blue whale"),
        result(9, "call_7_2", "1\tblue"),
    ];
    // 找：自己这一次、以前几次的调用和翻出来的结果都不算，别的工具照算。
    let done = run_on(vec![events.clone()], json!({"query": "blue"})).await;
    assert_eq!(seqs(&done), [9, 7, 6, 4, 2]);
    assert!(!text(&done).contains("→ history"), "{}", text(&done));
    let done = run_on(vec![events], json!({"from": 4, "to": 5})).await;
    assert_eq!(
        text(&done),
        "#4 2026-09-29 14:02 assistant\n我翻一下。\n→ read {\"file_path\":\"blue.txt\"}\n"
    );
}
