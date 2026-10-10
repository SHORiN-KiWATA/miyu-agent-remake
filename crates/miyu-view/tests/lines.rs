//! 手写的几段日志（施工 9-8 上）：替身跑不出来、内核也很少走到的几种，照日志的写法直接喂。

use std::sync::Arc;

use miyu_kernel::accumulate::Kind;
use miyu_kernel::event::{Event, ModelDelta, Piece, Transient, TransientBody};
use miyu_kernel::id::Seq;
use miyu_kernel::origin::By;
use miyu_kernel::time::Timestamp;
use miyu_view::{Body, Entry, Projector};

use crate::support::*;

/// 一行一条事件，照先后喂：翻页的样子。
fn feed(lines: &[&str]) -> Vec<Entry> {
    let mut projector = Projector::new(Arc::new(texts("en")), None);
    for line in lines {
        let event = Event::from_line(line).unwrap_or_else(|e| panic!("{line}: {e}"));
        projector.event(&event);
    }
    projector.entries().to_vec()
}

const CREATED: &str = r#"{"seq":1,"at":"2026-09-25T07:00:00.000Z","kind":"session.created","by":{"kind":"person","account":"alice"},"cause":"cmd-0","body":{"owner":"alice","venue":"local","policy":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","permission":{"level":"workspace","read_only":false}}}"#;
const ASKED: &str = r#"{"seq":2,"at":"2026-09-25T07:00:01.000Z","kind":"message.user","by":{"kind":"person","account":"alice"},"cause":"cmd-1","body":{"blocks":[{"type":"text","text":"go"}]}}"#;
const STARTED: &str = r#"{"seq":3,"at":"2026-09-25T07:00:01.000Z","kind":"turn.started","turn":3,"by":{"kind":"kernel"},"cause":"cmd-1","body":{"trigger":2}}"#;

#[test]
fn a_message_is_heard_by_the_request_that_saw_up_to_it() {
    let entries = feed(&[
        CREATED,
        ASKED,
        STARTED,
        r#"{"seq":4,"at":"2026-09-25T07:00:02.000Z","kind":"message.user","by":{"kind":"person","account":"alice"},"cause":"cmd-2","body":{"blocks":[{"type":"text","text":"also"}]}}"#,
        r#"{"seq":5,"at":"2026-09-25T07:00:03.000Z","kind":"message.assistant","turn":3,"by":{"kind":"model","endpoint":"e","model":"m"},"cause":"cmd-1","body":{"blocks":[{"type":"text","text":"ok"}],"seen":4}}"#,
    ]);
    let also = json(&entries[1]);
    assert_eq!(also["text"], "also");
    assert!(
        also.get("queued").is_none(),
        "看到了第 4 条为止的请求听到了第 4 条：{also}"
    );
}

#[test]
fn a_step_without_a_result_at_the_end_of_the_turn_is_cancelled() {
    let entries = feed(&[
        CREATED,
        ASKED,
        STARTED,
        r#"{"seq":4,"at":"2026-09-25T07:00:03.000Z","kind":"message.assistant","turn":3,"by":{"kind":"model","endpoint":"e","model":"m"},"cause":"cmd-1","body":{"blocks":[{"type":"tool_call","call_id":"call_4_1","name":"read","args":"{}"}],"seen":3}}"#,
        r#"{"seq":5,"at":"2026-09-25T07:00:05.000Z","kind":"turn.ended","turn":3,"by":{"kind":"kernel"},"body":{"reason":"aborted"}}"#,
    ]);
    let tool = json(of_kind(&entries, "tool")[0]);
    assert_eq!(tool["state"], "cancelled", "{tool}");
}

#[test]
fn a_delivered_message_does_not_repeat_its_recipient_but_a_failed_one_says_why() {
    let message = |status: &str| {
        let result = format!(
            r#"{{"seq":5,"at":"2026-09-25T07:00:04.000Z","kind":"tool.result","turn":3,"by":{{"kind":"tool","call_id":"call_4_1"}},"cause":"cmd-1","body":{{"call_id":"call_4_1","status":"{status}","blocks":[{{"type":"text","text":"x"}}],"human":{{"key":"core/tool-results/denied"}}}}}}"#
        );
        let entries = feed(&[
            CREATED,
            ASKED,
            STARTED,
            r#"{"seq":4,"at":"2026-09-25T07:00:03.000Z","kind":"message.assistant","turn":3,"by":{"kind":"model","endpoint":"e","model":"m"},"cause":"cmd-1","body":{"blocks":[{"type":"tool_call","call_id":"call_4_1","name":"send_message","args":"{\"to\":\"parent\",\"text\":\"hi\"}"}],"seen":3}}"#,
            &result,
        ]);
        json(of_kind(&entries, "tool")[0])["title"].clone()
    };
    let sent = message("ok");
    assert_eq!(sent["object"], "parent session");
    assert!(sent.get("said").is_none(), "送到了不写：{sent}");
    let failed = message("error");
    assert!(failed["said"].is_string(), "没送到的写为什么：{failed}");
}

/// 请求 3 的第 `index` 块的一段增量。
fn delta(index: usize, piece: Piece) -> Transient {
    Transient {
        at: Timestamp::parse("2026-09-25T07:00:02.000Z").expect("时刻合写法"),
        turn: None,
        by: By::Kernel,
        cause: None,
        body: TransientBody::ModelDelta(ModelDelta {
            seen: Seq::new(3).expect("序号从 1 数起"),
            index,
            piece,
        }),
    }
}

#[test]
fn the_next_block_starting_finishes_the_ones_before() {
    // OpenAI 兼容接口的驱动要等整个回复收完才一起报每一块的 `end`：思考不能一直「思考中」。
    let mut projector = Projector::new(Arc::new(texts("en")), None);
    for line in [CREATED, ASKED, STARTED] {
        projector.event(&Event::from_line(line).expect("合写法"));
    }
    projector.transient(&delta(0, Piece::Start(Kind::Reasoning)));
    projector.transient(&delta(0, Piece::Text("hmm".to_string())));
    let open = |p: &Projector| {
        p.entries().iter().find_map(|e| match &e.body {
            Body::Thought(t) => Some(t.open),
            _ => None,
        })
    };
    assert_eq!(open(&projector), Some(true));
    projector.transient(&delta(
        1,
        Piece::Start(Kind::ToolCall {
            name: "read".to_string(),
        }),
    ));
    assert_eq!(
        open(&projector),
        Some(false),
        "调工具那一块开始，思考就收全了"
    );
}

/// 改了文件的一步带改了哪些（施工 9-8 三补，网页的预览工作区照它认产物）：真实位置、新建（改之前没有）、改了还是移进
/// 回收站，照效果的先后。
#[test]
fn a_step_that_changed_files_lists_them() {
    const EMPTY: &str = "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    let result = format!(
        r#"{{"seq":5,"at":"2026-09-25T07:00:04.000Z","kind":"tool.result","turn":3,"by":{{"kind":"tool","call_id":"call_4_1"}},"cause":"cmd-1","body":{{"call_id":"call_4_1","status":"ok","blocks":[{{"type":"text","text":"done"}}],"effects":[{{"kind":"file.read","path":"/w/read.md","hash":"{EMPTY}"}},{{"kind":"file.changed","path":"/w/out/a.md","after":"{EMPTY}"}},{{"kind":"file.changed","path":"/w/b.md","before":"{EMPTY}","after":"{EMPTY}"}},{{"kind":"file.trashed","path":"/w/old.md","trash":"/t/1"}}]}}}}"#
    );
    let entries = feed(&[
        CREATED,
        ASKED,
        STARTED,
        r#"{"seq":4,"at":"2026-09-25T07:00:03.000Z","kind":"message.assistant","turn":3,"by":{"kind":"model","endpoint":"e","model":"m"},"cause":"cmd-1","body":{"blocks":[{"type":"tool_call","call_id":"call_4_1","name":"write","args":"{\"file_path\":\"out/a.md\",\"content\":\"\"}"}],"seen":3}}"#,
        &result,
    ]);
    let tool = json(of_kind(&entries, "tool")[0]);
    assert_eq!(
        tool["files"],
        serde_json::json!([
            {"path": "/w/out/a.md", "action": "created"},
            {"path": "/w/b.md", "action": "changed"},
            {"path": "/w/old.md", "action": "trashed"}
        ]),
        "读的不算：{tool}"
    );
}
