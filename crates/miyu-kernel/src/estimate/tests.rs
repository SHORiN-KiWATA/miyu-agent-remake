//! 用量和压缩线的测试：几个字节算几个 token、一条消息怎么数、锚怎么挑、锚加增量、压缩线。
//! 锚盖住的正好是前 `messages + 1` 条这个前提，在 `miyu-assemble` 的探针和随机日志里用真的组装器查。

use super::*;
use crate::block::{File, Image, Private, Reasoning, Text, ToolCall};
use crate::event::Event;
use crate::id::{CallId, ContentHash, DriverFamily, FileName, MediaType, ModelName, ProviderId};
use crate::raw::RawJson;

const KERNEL: &str = r#"{"kind":"kernel"}"#;
const ALICE: &str = r#"{"kind":"person","account":"alice"}"#;

/// 拼一条事件：序号、所属回合、由谁引起、种类、`body`。
fn event(seq: u64, turn: Option<u64>, by: &str, kind: &str, body: &str) -> Event {
    let turn = turn.map(|t| format!(r#""turn":{t},"#)).unwrap_or_default();
    Event::from_line(&format!(
        r#"{{"seq":{seq},"at":"2026-09-29T07:00:00.000Z","kind":"{kind}",{turn}"by":{by},"body":{body}}}"#
    ))
    .unwrap()
}

/// 一条 `model.called`：看到第 `seen` 条、`messages` 条消息；`usage` 是四格加起来的 JSON，没有就不写；
/// `result` 是结果。发给 `deepseek/deepseek-flash`。
fn called(seq: u64, turn: u64, messages: u64, usage: Option<u64>, result: &str) -> Event {
    let usage = usage
        .map(|n| {
            format!(r#""usage":{{"uncached":{n},"cache_read":0,"cache_write":0,"output":0}},"#)
        })
        .unwrap_or_default();
    let body = format!(
        r#"{{"seen":{},"endpoint":"deepseek","model":"deepseek-flash","messages":{messages},{usage}"result":"{result}"}}"#,
        seq - 1
    );
    event(seq, Some(turn), KERNEL, "model.called", &body)
}

/// 原样的 JSON。
fn raw(text: &str) -> RawJson {
    serde_json::from_str(text).unwrap()
}

/// 一段有效历史：照先后收下这些事件。
fn history(events: Vec<Event>) -> History {
    let mut history = History::default();
    for event in events {
        history.append(event);
    }
    history
}

fn flash() -> Model {
    Model {
        endpoint: ProviderId::parse("deepseek").unwrap(),
        model: ModelName::parse("deepseek-flash").unwrap(),
    }
}

fn text(text: &str) -> Block {
    Block::Text(Text {
        text: text.to_string(),
    })
}

fn user(text_: &str) -> Message {
    Message::User {
        blocks: vec![text(text_)],
    }
}

fn assistant(text_: &str) -> Message {
    Message::Assistant {
        blocks: vec![text(text_)],
    }
}

/// 一份请求：没有工具，system 是 `system`，消息照给的。
fn request(system: &str, messages: Vec<Message>) -> Request {
    Request {
        tools: Vec::new(),
        system: system.to_string(),
        messages,
        stable: 0,
        continuation: false,
    }
}

/// 出厂的那一种：一张图、一个文件都算 2000。
const FLAT: Flat = Flat {
    image: 2000,
    file: 2000,
};

/// 图照像素算：一千像素一个 token；文件 7 个。
struct Pixels;

impl Price for Pixels {
    fn image(&self, width: u32, height: u32) -> u64 {
        u64::from(width) * u64::from(height) / 1000
    }

    fn file(&self) -> u64 {
        7
    }
}

#[test]
fn text_is_utf8_bytes_over_four_rounded_up() {
    assert_eq!(message(&user(""), &FLAT), 0);
    assert_eq!(message(&user("abcd"), &FLAT), 1);
    assert_eq!(message(&user("abcde"), &FLAT), 2);
    // 中文一个字三个字节：两个字六个字节，算两个。
    assert_eq!(message(&user("你好"), &FLAT), 2);
    assert_eq!(message(&user("你好世界"), &FLAT), 3);
}

#[test]
fn bytes_of_a_message_are_added_up_before_rounding() {
    // 三块各一个字节：加起来三个字节，算一个，不是每块向上取整算三个。
    let three = Message::User {
        blocks: vec![text("a"), text("b"), text("c")],
    };
    assert_eq!(message(&three, &FLAT), 1);
}

#[test]
fn every_kind_of_block_is_counted() {
    let private = || Private {
        driver: DriverFamily::parse("openai-chat").unwrap(),
        data: raw("\"abcdefgh\""),
    };
    let reply = Message::Assistant {
        blocks: vec![
            // 思考 4 字节，私有数据 10 字节。
            Block::Reasoning(Reasoning {
                text: "hmmm".to_string(),
                private: Some(private()),
            }),
            // 名字 4 字节，参数 2 字节，私有数据 10 字节。
            Block::ToolCall(ToolCall {
                call_id: CallId::parse("call_3_1").unwrap(),
                name: "read".to_string(),
                args: "{}".to_string(),
                private: Some(private()),
            }),
            // 认不出的块照原样的 JSON：12 字节。
            Block::Unknown(raw(r#"{"type":"x"}"#)),
        ],
    };
    assert_eq!(
        message(&reply, &FLAT),
        (4 + 10 + 4 + 2 + 10 + 12u64).div_ceil(4)
    );

    let media = Message::Tool {
        call_id: CallId::parse("call_3_1").unwrap(),
        error: false,
        blocks: vec![
            text("abcd"),
            Block::Image(Image {
                blob: ContentHash::of(b"png"),
                media_type: MediaType::parse("image/png").unwrap(),
                width: 1000,
                height: 500,
            }),
            Block::File(File {
                blob: ContentHash::of(b"pdf"),
                name: FileName::parse("a.pdf").unwrap(),
                media_type: MediaType::parse("application/pdf").unwrap(),
            }),
        ],
    };
    assert_eq!(message(&media, &FLAT), 1 + 2000 + 2000);
    assert_eq!(message(&media, &Pixels), 1 + 500 + 7);
}

#[test]
fn the_line_is_the_window_less_the_reserve_and_the_margin() {
    // deepseek-flash：窗口一百万，最大输出 393216，预留封顶 20000。
    assert_eq!(
        line(Some(1_000_000), Some(393_216), 20_000, 13_000),
        Some(967_000)
    );
    // 最大输出比封顶小，照最大输出留。
    assert_eq!(
        line(Some(128_000), Some(8_000), 20_000, 13_000),
        Some(107_000)
    );
    // 没报最大输出，按封顶留。
    assert_eq!(line(Some(128_000), None, 20_000, 13_000), Some(95_000));
}

#[test]
fn there_is_no_line_without_a_window_or_when_it_is_too_small() {
    assert_eq!(line(None, Some(8_000), 20_000, 13_000), None);
    // 正好减到 0，也算没有线：不然一请求就压。
    assert_eq!(line(Some(33_000), None, 20_000, 13_000), None);
    assert_eq!(line(Some(32_000), None, 20_000, 13_000), None);
    assert_eq!(line(Some(33_001), None, 20_000, 13_000), Some(1));
}

#[test]
fn the_anchor_is_the_latest_finished_call_with_usage() {
    let history = history(vec![
        event(1, None, ALICE, "message.user", r#"{"blocks":[]}"#),
        event(2, Some(2), KERNEL, "turn.started", r#"{"trigger":1}"#),
        called(3, 2, 1, Some(100), "ok"),
        called(4, 2, 3, Some(200), "ok"),
        // 出错的、打断的、没报用量的都不算。
        called(5, 2, 5, Some(300), "error"),
        called(6, 2, 5, None, "interrupted"),
        called(7, 2, 5, None, "ok"),
    ]);
    assert_eq!(
        anchor(&history),
        Some(Anchor {
            seq: Seq::new(4).unwrap(),
            model: flash(),
            messages: 3,
            reported: 200,
        })
    );
}

#[test]
fn the_four_parts_of_the_usage_are_added_up() {
    let body = r#"{"seen":2,"endpoint":"deepseek","model":"deepseek-flash","messages":1,"usage":{"uncached":1,"cache_read":20,"cache_write":300,"output":4000},"result":"ok"}"#;
    let history = history(vec![event(3, Some(2), KERNEL, "model.called", body)]);
    assert_eq!(anchor(&history).map(|a| a.reported), Some(4321));
}

#[test]
fn nothing_before_the_latest_compaction_is_an_anchor() {
    // 12 替代到 5：6 到 11 是留下的尾巴，里面的 7 是压缩以前的请求；11 是摘要请求自己。
    let history = history(vec![
        called(7, 6, 5, Some(90_000), "ok"),
        called(11, 6, 9, Some(95_000), "ok"),
        event(
            12,
            Some(6),
            KERNEL,
            "context.compacted",
            r#"{"upto":5,"summary":"s"}"#,
        ),
    ]);
    assert_eq!(anchor(&history), None);

    let mut history = history;
    history.append(called(13, 6, 4, Some(3_000), "ok"));
    assert_eq!(anchor(&history).map(|a| a.reported), Some(3_000));
}

#[test]
fn calls_of_undone_turns_are_not_anchors() {
    let history = history(vec![
        event(1, None, ALICE, "message.user", r#"{"blocks":[]}"#),
        event(2, Some(2), KERNEL, "turn.started", r#"{"trigger":1}"#),
        called(3, 2, 1, Some(100), "ok"),
        event(
            4,
            Some(2),
            KERNEL,
            "turn.ended",
            r#"{"reason":"completed"}"#,
        ),
        event(5, None, ALICE, "message.user", r#"{"blocks":[]}"#),
        event(6, Some(6), KERNEL, "turn.started", r#"{"trigger":5}"#),
        called(7, 6, 3, Some(500), "ok"),
        event(
            8,
            Some(6),
            KERNEL,
            "turn.ended",
            r#"{"reason":"completed"}"#,
        ),
        event(9, None, ALICE, "turn.reverted", r#"{"turns":[6]}"#),
    ]);
    assert_eq!(anchor(&history).map(|a| a.reported), Some(100));
}

#[test]
fn there_is_no_anchor_without_a_call() {
    assert_eq!(anchor(&History::default()), None);
}

/// 锚：发给 deepseek-flash、一条消息、报了 `reported`。
fn anchored(messages: u64, reported: u64) -> Anchor {
    Anchor {
        seq: Seq::new(3).unwrap(),
        model: flash(),
        messages,
        reported,
    }
}

#[test]
fn the_usage_is_the_anchor_plus_what_came_after_its_reply() {
    // 锚那次请求一条消息，回复是第 1 条；之后多了一条 4 字节、一条 8 字节的。
    let now = request(
        "sys",
        vec![
            user("hello"),
            assistant("hi"),
            user("abcd"),
            user("abcdefgh"),
        ],
    );
    assert_eq!(
        usage(&now, Some(&anchored(1, 1000)), &flash(), &FLAT),
        1000 + 1 + 2
    );
    // 锚之后什么都没有。
    let now = request("sys", vec![user("hello"), assistant("hi")]);
    assert_eq!(usage(&now, Some(&anchored(1, 1000)), &flash(), &FLAT), 1000);
}

#[test]
fn an_estimate_above_what_was_reported_wins() {
    // 锚盖住的：system 400 字节算 100，两条各 1，一共 102，比报的 50 大。
    let system = "x".repeat(400);
    let now = request(&system, vec![user("a"), assistant("b"), user("abcd")]);
    assert_eq!(
        usage(&now, Some(&anchored(1, 50)), &flash(), &FLAT),
        102 + 1
    );
}

#[test]
fn the_whole_request_is_estimated_without_a_usable_anchor() {
    let now = request("sys", vec![user("hello"), assistant("hi"), user("abcd")]);
    // 3 字节的 system 算 1，5 字节算 2，2 字节算 1，4 字节算 1。
    let whole = 1 + 2 + 1 + 1;
    assert_eq!(usage(&now, None, &flash(), &FLAT), whole);

    // 换过模型：锚是别的模型报的。
    let other = Model {
        endpoint: ProviderId::parse("deepseek").unwrap(),
        model: ModelName::parse("deepseek-v4-pro").unwrap(),
    };
    assert_eq!(usage(&now, Some(&anchored(1, 1000)), &other, &FLAT), whole);

    // 这一次的消息没比锚多：锚说有三条，回复该是第 3 条，可一共只有三条。
    assert_eq!(
        usage(&now, Some(&anchored(3, 1000)), &flash(), &FLAT),
        whole
    );
}

#[test]
fn the_tool_face_counts_as_its_json() {
    let mut now = request("", vec![user("abcd")]);
    now.tools.push(crate::request::ToolSpec {
        name: "read".to_string(),
        description: "Read a file.".to_string(),
        parameters: raw(r#"{"type":"object"}"#),
    });
    let face = json(&now.tools).len() as u64;
    assert_eq!(usage(&now, None, &flash(), &FLAT), face.div_ceil(4) + 1);
}
