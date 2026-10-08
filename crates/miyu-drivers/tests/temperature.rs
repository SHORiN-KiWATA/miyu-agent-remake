//! 三种驱动的温度（施工 8-22，`docs/blueprint/models.md`「驱动要守的约定」第 14 条）：没有的一个字节不变；有的写顶层
//! `"temperature":<数>`，接在思考强度那几样的前面，数照最短的十进制写；Anthropic 开着思考（档位、`on`）不带。

use std::collections::BTreeMap;

use crate::support::{call, claude, deepseek, gpt, text, texts};
use miyu_drivers::openai_chat::Compat;
use miyu_drivers::{Call, EFFORT_OFF, EFFORT_ON, Inputs, anthropic, openai_chat, openai_responses};
use miyu_kernel::request::{Message, Request};

/// 一句话的请求。
fn request() -> Request {
    Request {
        tools: Vec::new(),
        system: "You are a helpful assistant.".to_string(),
        messages: vec![Message::User {
            blocks: vec![text("你好")],
        }],
        stable: 0,
        continuation: false,
        described: Default::default(),
    }
}

/// `call` 带上温度 `temperature`、思考强度 `effort`。
fn with(mut call: Call, temperature: Option<f64>, effort: Option<&str>) -> Call {
    call.temperature = temperature;
    call.effort = effort.map(str::to_string);
    call
}

fn chat(call: &Call, compat: &Compat) -> String {
    let encoded = openai_chat::encode(&request(), call, compat, &texts(), &BTreeMap::new())
        .expect("不要 blob 的请求编码得了");
    String::from_utf8(encoded.body).expect("请求是 UTF-8")
}

fn messages(call: &Call) -> String {
    let encoded =
        anthropic::encode(&request(), call, &texts(), &BTreeMap::new()).expect("编码得了");
    String::from_utf8(encoded.body).expect("请求是 UTF-8")
}

fn responses(call: &Call) -> String {
    let encoded =
        openai_responses::encode(&request(), call, &texts(), &BTreeMap::new()).expect("编码得了");
    String::from_utf8(encoded.body).expect("请求是 UTF-8")
}

/// 去掉收尾的 `}`。
fn open(body: &str) -> &str {
    body.strip_suffix('}').expect("以 } 收尾")
}

#[test]
fn without_a_temperature_nothing_is_added() {
    let chat_plain = chat(&call(Inputs::default(), Some(4096)), &deepseek());
    let messages_plain = messages(&claude(Inputs::default(), None));
    let responses_plain = responses(&gpt(Inputs::default(), None));
    for body in [&chat_plain, &messages_plain, &responses_plain] {
        assert!(!body.contains("temperature"), "{body}");
    }
}

#[test]
fn openai_chat_writes_it_after_the_output_limit_and_before_the_effort() {
    let compat = deepseek();
    let plain = chat(&call(Inputs::default(), Some(4096)), &compat);
    let warm = chat(
        &with(call(Inputs::default(), Some(4096)), Some(0.7), None),
        &compat,
    );
    assert_eq!(warm, format!(r#"{},"temperature":0.7}}"#, open(&plain)));
    let both = chat(
        &with(call(Inputs::default(), Some(4096)), Some(1.0), Some("high")),
        &compat,
    );
    assert_eq!(
        both,
        format!(
            r#"{},"temperature":1,"reasoning_effort":"high"}}"#,
            open(&plain)
        ),
        "1 写成 1，在思考强度前面"
    );
    let cold = chat(
        &with(
            call(Inputs::default(), Some(4096)),
            Some(0.0),
            Some(EFFORT_OFF),
        ),
        &compat,
    );
    assert_eq!(
        cold,
        format!(
            r#"{},"temperature":0,"thinking":{{"type":"disabled"}}}}"#,
            open(&plain)
        ),
        "0 也是写了的"
    );
}

#[test]
fn openai_responses_writes_it_before_the_reasoning() {
    let plain = responses(&gpt(Inputs::default(), None));
    let warm = responses(&with(gpt(Inputs::default(), None), Some(0.1), None));
    assert_eq!(warm, format!(r#"{},"temperature":0.1}}"#, open(&plain)));
    let both = responses(&with(gpt(Inputs::default(), None), Some(1.3), Some("low")));
    assert!(
        both.starts_with(&format!(
            r#"{},"temperature":1.3,"reasoning":"#,
            open(&plain)
        )),
        "{both}"
    );
}

#[test]
fn anthropic_takes_it_only_while_thinking_is_not_on() {
    let plain = messages(&claude(Inputs::default(), None));
    let warm = messages(&with(claude(Inputs::default(), None), Some(0.5), None));
    assert_eq!(warm, format!(r#"{},"temperature":0.5}}"#, open(&plain)));
    let off = messages(&with(
        claude(Inputs::default(), None),
        Some(0.5),
        Some(EFFORT_OFF),
    ));
    assert_eq!(
        off,
        format!(
            r#"{},"temperature":0.5,"thinking":{{"type":"disabled"}}}}"#,
            open(&plain)
        ),
        "关着思考的照带"
    );
    for effort in [EFFORT_ON, "high"] {
        let thinking = messages(&with(
            claude(Inputs::default(), None),
            Some(0.5),
            Some(effort),
        ));
        let bare = messages(&with(claude(Inputs::default(), None), None, Some(effort)));
        assert_eq!(thinking, bare, "开着思考（{effort}）不带温度");
    }
}
