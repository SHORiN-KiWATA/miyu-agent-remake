//! Anthropic 消息接口的解码（`docs/blueprint/drivers/anthropic.md`「怎么走：解码」，施工 8-12）：流的样本在
//! `docs/designs/samples/drivers/anthropic/streams/`，`.sse` 是进去的字节（照官方文档手写），`.txt` 是解出来的，一行一条，
//! 逐字节比对。还查：从哪里切开喂都一样；内核的累积器一条都不拒；解出来的回复编码回去，编号、签名原样；驱动的接口走一遍；
//! `finished()` 在 `stop_reason` 到了以后才说是。

use std::collections::BTreeMap;
use std::fs;

use crate::support::{anthropic_file, claude, family_dir, text, texts};
use miyu_drivers::anthropic::{Decoder, encode};
use miyu_drivers::classify::Failure;
use miyu_drivers::{Anthropic, Driver, Ending, Inputs};
use miyu_kernel::accumulate::{Accumulator, Delta, Kind};
use miyu_kernel::event::ErrorClass;
use miyu_kernel::id::Seq;
use miyu_kernel::request::{Message, Request};
use serde_json::{Value, json};

/// 流的样本，一份一种情形。
const STREAMS: [&str; 15] = [
    "text",
    "thinking-signature",
    "thinking-omitted",
    "redacted-thinking",
    "tool-use-fragments",
    "usage-in-delta",
    "refusal",
    "max-tokens",
    "stream-error",
    "ping-and-unknown",
    "server-tool-ignored",
    "cut-off",
    "cut-in-thinking",
    "bad-json",
    "stop-without-message-stop",
];

fn stream(name: &str) -> Vec<u8> {
    let path = family_dir("anthropic")
        .join("streams")
        .join(format!("{name}.sse"));
    fs::read(&path).unwrap_or_else(|e| panic!("读不了 {}：{e}", path.display()))
}

/// 一片一片喂进去，再收尾。
fn decode(chunks: &[&[u8]]) -> (Vec<Delta>, Ending) {
    let mut decoder = Decoder::new();
    let mut deltas = Vec::new();
    for chunk in chunks {
        deltas.extend(decoder.feed(chunk));
    }
    (deltas, decoder.finish())
}

/// 解出来的，一行一条：增量、收块、用量、说完了还是出错。
fn render((deltas, ending): &(Vec<Delta>, Ending)) -> String {
    let json = |text: &str| serde_json::to_string(text).expect("字符串写得成 JSON");
    let mut lines: Vec<String> = deltas
        .iter()
        .chain(&ending.deltas)
        .map(|delta| match delta {
            Delta::Start {
                index,
                kind: Kind::Text,
            } => format!("start {index} text"),
            Delta::Start {
                index,
                kind: Kind::Reasoning,
            } => format!("start {index} reasoning"),
            Delta::Start {
                index,
                kind: Kind::ToolCall { name },
            } => format!("start {index} tool_call {name}"),
            Delta::Text { index, text } => format!("text {index} {}", json(text)),
            Delta::Private { index, private } => {
                format!("private {index} {} {}", private.driver, private.data.get())
            }
            Delta::End { index } => format!("end {index}"),
        })
        .collect();
    lines.push(match &ending.usage {
        Some(usage) => format!(
            "usage uncached={} cache_read={} cache_write={} output={}{}",
            usage.uncached,
            usage.cache_read,
            usage.cache_write,
            usage.output,
            usage
                .reasoning
                .map_or_else(String::new, |reasoning| format!(" reasoning={reasoning}"))
        ),
        None => "usage none".to_string(),
    });
    lines.push(match &ending.error {
        Some(error) => format!("error {} {}", error.class.as_str(), json(&error.message)),
        None => "ok".to_string(),
    });
    lines.join("\n") + "\n"
}

#[test]
fn every_stream_matches_its_sample() {
    for name in STREAMS {
        let got = render(&decode(&[&stream(name)]));
        anthropic_file(&format!("streams/{name}.txt"), got.as_bytes());
    }
}

#[test]
fn wherever_the_stream_is_cut_it_decodes_the_same() {
    for name in STREAMS {
        let bytes = stream(name);
        let whole = render(&decode(&[&bytes]));
        for cut in 0..=bytes.len() {
            let halves = render(&decode(&[&bytes[..cut], &bytes[cut..]]));
            assert_eq!(halves, whole, "{name} 切在第 {cut} 个字节");
        }
        let one_by_one: Vec<&[u8]> = bytes.chunks(1).collect();
        assert_eq!(
            render(&decode(&one_by_one)),
            whole,
            "{name} 一个字节一个字节地喂"
        );
    }
}

#[test]
fn the_accumulator_takes_every_delta() {
    for name in STREAMS {
        let (deltas, ending) = decode(&[&stream(name)]);
        let mut accumulator = Accumulator::default();
        for delta in deltas.into_iter().chain(ending.deltas) {
            if let Err(error) = accumulator.apply(delta) {
                panic!("{name}：累积器不收：{error}");
            }
        }
    }
}

/// 照流 `name` 解出来、拼好的回复。
fn reply(name: &str) -> Vec<miyu_kernel::block::Block> {
    let (deltas, ending) = decode(&[&stream(name)]);
    let mut accumulator = Accumulator::default();
    for delta in deltas.into_iter().chain(ending.deltas) {
        accumulator.apply(delta).expect("累积器收得下");
    }
    match ending.error {
        None => accumulator.finish(Seq::FIRST),
        Some(_) => accumulator.cut_off(Seq::FIRST),
    }
}

/// 回复编码回去，线上的第二条消息。
fn encoded_back(reply: Vec<miyu_kernel::block::Block>) -> Value {
    let request = Request {
        tools: vec![],
        system: String::new(),
        messages: vec![
            Message::User {
                blocks: vec![text("看看 src 目录")],
            },
            Message::Assistant { blocks: reply },
            Message::User {
                blocks: vec![text("接着")],
            },
        ],
        stable: 0,
        continuation: false,
        described: Default::default(),
        output_cap: None,
    };
    let encoded = encode(
        &request,
        &claude(Inputs::default(), None),
        &texts(),
        &BTreeMap::new(),
    )
    .expect("不要 blob");
    let text = String::from_utf8(encoded.body).expect("UTF-8");
    let body: Value =
        serde_json::from_str(&text.replace(r#","cache_control":{"type":"ephemeral"}"#, ""))
            .expect("是 JSON");
    body["messages"][1]["content"].clone()
}

#[test]
fn a_decoded_reply_encodes_back_with_its_ids_and_signatures() {
    assert_eq!(
        encoded_back(reply("thinking-signature")),
        json!([
            {"type": "thinking", "thinking": "用户想看目录。", "signature": "EqQBCgIYAhIM1gbcDa9GJwZA2b3hGgxBdjrkzLoky3dl1pkiMOYds"},
            {"type": "text", "text": "我先看一下目录。"},
            {"type": "tool_use", "id": "toolu_01ABC", "name": "read", "input": {"path": "src"}}
        ])
    );
    assert_eq!(
        encoded_back(reply("thinking-omitted"))[0],
        json!({"type": "thinking", "thinking": "", "signature": "EqQBCgIYAhIMomitted"})
    );
    assert_eq!(
        encoded_back(reply("redacted-thinking"))[0],
        json!({"type": "redacted_thinking", "data": "EmwKAhgBEgy3va3pzix/LafPsn4aDFIT2Xlxh0L5L8rLVyIwxtE3rAFBa8cr3qpP"})
    );
    // 断在思考里的：没有签名，编码时丢掉。
    let cut = reply("cut-in-thinking");
    assert_eq!(cut.len(), 1);
    assert_eq!(encoded_back(cut), Value::Null);
}

#[test]
fn the_driver_interface_goes_through_all_of_it() {
    let driver: Box<dyn Driver> = Box::new(Anthropic::new(texts()));
    let mut decoder = driver.decoder();
    let deltas = decoder.feed(&stream("text"));
    assert!(decoder.done(), "见到 message_stop 就不用再读了");
    let ending = decoder.finish();
    assert_eq!(
        render(&(deltas, ending)),
        render(&decode(&[&stream("text")]))
    );
    let failure = Failure {
        status: Some(429),
        headers: &[("retry-after", "3")],
        body: br#"{"type":"error","error":{"type":"rate_limit_error","message":"Number of request tokens has exceeded your per-minute rate limit"}}"#,
    };
    let classified = driver.classify(&failure);
    assert_eq!(classified.error.class, ErrorClass::RateLimited);
    assert_eq!(classified.retry_after_ms, Some(3000));
    let listed = driver
        .parse_models(
            br#"{"data":[{"id":"claude-opus-5","max_input_tokens":1000000}],"has_more":false}"#,
        )
        .expect("读得了");
    assert_eq!(listed[0].id, "claude-opus-5");
    assert_eq!(listed[0].window, Some(1_000_000));
}

/// `needle` 头一次出现在哪。
fn position(bytes: &[u8], needle: &[u8]) -> usize {
    bytes
        .windows(needle.len())
        .position(|window| window == needle)
        .expect("找得到")
}

#[test]
fn finished_says_whether_the_stop_reason_came() {
    let bytes = stream("text");
    let reason = position(&bytes, b"event: message_delta");
    let stop = position(&bytes, b"event: message_stop");
    let mut decoder = Decoder::new();
    let _unused = decoder.feed(&bytes[..reason]);
    assert!(!decoder.finished());
    let _unused = decoder.feed(&bytes[reason..stop]);
    assert!(decoder.finished());
    assert!(!decoder.done());
    let _unused = decoder.feed(&bytes[stop..]);
    assert!(decoder.done());
}

#[test]
fn an_error_event_in_the_stream_is_classified_without_a_status() {
    let (_, ending) = decode(&[&stream("stream-error")]);
    let error = ending.error.expect("出错了");
    assert_eq!(error.class, ErrorClass::Retryable);
    assert_eq!(error.message, "Overloaded");
    assert_eq!(error.status, None);
}
