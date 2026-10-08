//! OpenAI 兼容对话接口的解码（`docs/designs/05-内核接口.md` 第七节「怎么解码」）：流的样本在
//! `docs/designs/samples/drivers/openai-chat/streams/`，`.sse` 是进去的字节（照各家文档和调研手写），
//! `.txt` 是解出来的，一行一条，逐字节比对。还查：从哪里切开喂都一样；内核的累积器一条都不拒；
//! 解出来的回复编码回去，用的是供应商的编号；驱动的接口走一遍。

use std::collections::BTreeMap;
use std::fs;

use crate::support::{call, deepseek, dir, sample_file, text, texts};
use miyu_drivers::classify::Failure;
use miyu_drivers::openai_chat::Decoder;
use miyu_drivers::{Driver, Ending, Inputs, OpenAiChat};
use miyu_kernel::accumulate::{Accumulator, Delta, Kind};
use miyu_kernel::event::ErrorClass;
use miyu_kernel::id::Seq;
use miyu_kernel::request::{Message, Request};

/// 流的样本，一份一种情形。
const STREAMS: [&str; 13] = [
    "deepseek-reasoning-tools",
    "openai-text",
    "tool-call-fragments",
    "late-name",
    "moonshot-usage-in-choice",
    "gateway-noise",
    "stream-error",
    "cut-off",
    "content-filter",
    "length",
    "bad-json",
    "done-without-finish",
    "ended-early",
];

fn stream(name: &str) -> Vec<u8> {
    let path = dir().join("streams").join(format!("{name}.sse"));
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
            "usage uncached={} cache_read={} cache_write={} output={}",
            usage.uncached, usage.cache_read, usage.cache_write, usage.output
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
        sample_file(&format!("streams/{name}.txt"), got.as_bytes());
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

#[test]
fn a_decoded_reply_encodes_back_with_the_providers_ids() {
    let (deltas, ending) = decode(&[&stream("deepseek-reasoning-tools")]);
    assert!(ending.error.is_none());
    let mut accumulator = Accumulator::default();
    for delta in deltas.into_iter().chain(ending.deltas) {
        accumulator.apply(delta).expect("累积器收得下");
    }
    let reply = accumulator.finish(Seq::FIRST);
    let request = Request {
        tools: vec![],
        system: String::new(),
        messages: vec![
            Message::User {
                blocks: vec![text("看看 src 目录")],
            },
            Message::Assistant { blocks: reply },
        ],
        stable: 0,
        continuation: false,
        described: Default::default(),
    };
    let encoded = miyu_drivers::openai_chat::encode(
        &request,
        &call(Inputs::default(), None),
        &deepseek(),
        &texts(),
        &BTreeMap::new(),
    )
    .expect("不要 blob");
    let body: serde_json::Value = serde_json::from_slice(&encoded.body).expect("是 JSON");
    let assistant = &body["messages"][1];
    // 正文、思考照原样拼回去；工具调用用的是供应商的编号，参数一字不差。
    assert_eq!(assistant["content"], "我先看一下目录。");
    assert_eq!(assistant["reasoning_content"], "用户想看目录。");
    assert_eq!(assistant["tool_calls"][0]["id"], "call_00_abc");
    assert_eq!(
        assistant["tool_calls"][0]["function"]["arguments"],
        r#"{"path":"src"}"#
    );
}

#[test]
fn the_driver_interface_goes_through_all_three() {
    let driver: Box<dyn Driver> = Box::new(OpenAiChat::new(deepseek(), texts()));
    assert_eq!(driver.family(), "openai-chat");
    let request = Request {
        tools: vec![],
        system: "You are a helpful assistant.".to_string(),
        messages: vec![Message::User {
            blocks: vec![text("你好")],
        }],
        stable: 0,
        continuation: false,
        described: Default::default(),
    };
    let call = call(Inputs::default(), None);
    assert!(driver.blobs_needed(&request, &call).is_empty());
    let through = driver
        .encode(&request, &call, &BTreeMap::new())
        .expect("编码得了");
    assert_eq!(through.path, "/chat/completions");
    let direct =
        miyu_drivers::openai_chat::encode(&request, &call, &deepseek(), &texts(), &BTreeMap::new())
            .expect("编码得了");
    assert_eq!(through, direct);
    let mut decoder = driver.decoder();
    let deltas = decoder.feed(&stream("openai-text"));
    assert!(decoder.done(), "见到 [DONE] 就不用再读了");
    let ending = decoder.finish();
    assert_eq!(
        render(&(deltas, ending)),
        render(&decode(&[&stream("openai-text")]))
    );
    let failure = Failure {
        status: Some(429),
        headers: &[("retry-after", "3")],
        body: b"{}",
    };
    let classified = driver.classify(&failure);
    assert_eq!(classified.error.class, ErrorClass::RateLimited);
    assert_eq!(classified.retry_after_ms, Some(3000));
}

/// 一段有正文、说完了的流，`error` 那一格照给的写。
fn with_error(error: &str) -> Vec<u8> {
    format!(
        "data: {{\"choices\":[{{\"index\":0,\"delta\":{{\"content\":\"好\"}},\
         \"finish_reason\":\"stop\"}}],\"error\":{error}}}\n\ndata: [DONE]\n\n"
    )
    .into_bytes()
}

/// `needle` 头一次出现在哪。
fn position(bytes: &[u8], needle: &[u8]) -> usize {
    bytes
        .windows(needle.len())
        .position(|window| window == needle)
        .expect("找得到")
}

#[test]
fn an_error_that_says_nothing_is_noise() {
    // 网关的噪声：照常解，不算出错（施工 4-9 再补三下：原来 `false`、`0`、`[]` 也算出错）。
    for noise in ["null", "\"\"", "{}", "false", "0", "0.0", "[]"] {
        let (deltas, ending) = decode(&[&with_error(noise)]);
        assert!(ending.error.is_none(), "{noise}：{:?}", ending.error);
        assert!(
            deltas
                .iter()
                .any(|delta| matches!(delta, Delta::Text { text, .. } if text == "好")),
            "{noise}"
        );
    }
    // 真有内容的照旧算出错。
    for said in [
        "true",
        "1",
        "-2.5",
        "[1]",
        "\"boom\"",
        r#"{"message":"boom"}"#,
    ] {
        let (_, ending) = decode(&[&with_error(said)]);
        assert!(ending.error.is_some(), "{said}");
    }
}

#[test]
fn a_rate_limit_in_the_stream_keeps_how_long_to_wait() {
    // 流里报的限速说了要等多久：连同出错一起交回，数据行、`event: error` 都是（施工 4-9 再补三下：
    // 原来丢掉）。
    let said = r#"{"error":{"message":"Rate limit reached. Please try again in 1.5s","type":"rate_limit_error","code":429}}"#;
    for bytes in [
        format!("data: {said}\n\n"),
        format!("event: error\ndata: {said}\n\n"),
    ] {
        let (_, ending) = decode(&[bytes.as_bytes()]);
        let class = ending.error.map(|error| error.class);
        assert_eq!(class, Some(ErrorClass::RateLimited), "{bytes}");
        assert_eq!(ending.retry_after_ms, Some(1500), "{bytes}");
    }
    // 没说要等多久的，没有；正常说完的也没有。
    let (_, ending) = decode(&[&stream("stream-error")]);
    assert!(ending.error.is_some());
    assert_eq!(ending.retry_after_ms, None);
    let (_, ending) = decode(&[&stream("openai-text")]);
    assert_eq!(ending.retry_after_ms, None);
}

#[test]
fn finished_says_whether_the_finish_reason_came() {
    let bytes = stream("openai-text");
    let reason = position(&bytes, br#""finish_reason":"stop""#);
    let done = position(&bytes, b"data: [DONE]");
    let driver = OpenAiChat::new(deepseek(), texts());
    let mut decoder = driver.decoder();
    let _unused = decoder.feed(&bytes[..reason]);
    assert!(!decoder.finished());
    let _unused = decoder.feed(&bytes[reason..done]);
    // `finish_reason` 到了，还没见到 `[DONE]`（施工 4-9 再补三下）。
    assert!(decoder.finished());
    assert!(!decoder.done());
}
