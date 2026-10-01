//! OpenAI 兼容对话接口的思考强度（施工 8-18，`docs/blueprint/models.md`「驱动要守的约定」第 13 条）：没写的一个字节不变；
//! 档位发 `reasoning_effort`；`off`、`on` 照档案的开关（DeepSeek 的 `thinking`），没写开关的 `off` 发
//! `reasoning_effort: "none"`、`on` 什么都不加；都接在最后，前面的字节一个不动。

mod support;

use std::collections::BTreeMap;

use miyu_drivers::openai_chat::{Compat, encode};
use miyu_drivers::{EFFORT_OFF, EFFORT_ON, Inputs};
use miyu_kernel::request::{Message, Request};
use support::{call, deepseek, text, texts};

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
    }
}

/// 带思考强度 `effort` 照 `compat` 编出来的字节，写成字。
fn body(effort: Option<&str>, compat: &Compat) -> String {
    let mut call = call(Inputs::default(), Some(4096));
    call.effort = effort.map(str::to_string);
    let encoded = encode(&request(), &call, compat, &texts(), &BTreeMap::new())
        .expect("不要 blob 的请求编码得了");
    String::from_utf8(encoded.body).expect("请求是 UTF-8")
}

/// 没写思考强度的那一份，去掉收尾的 `}`：写了的都是它接上一截。
fn head(compat: &Compat) -> String {
    let plain = body(None, compat);
    plain.strip_suffix('}').expect("以 } 收尾").to_string()
}

#[test]
fn without_an_effort_nothing_is_added() {
    for compat in [Compat::default(), deepseek()] {
        let plain = body(None, &compat);
        assert!(
            !plain.contains("reasoning_effort") && !plain.contains("thinking"),
            "{plain}"
        );
    }
}

#[test]
fn a_level_goes_out_as_reasoning_effort_at_the_end() {
    for compat in [Compat::default(), deepseek()] {
        assert_eq!(
            body(Some("high"), &compat),
            format!(r#"{},"reasoning_effort":"high"}}"#, head(&compat)),
            "有没有开关，档位都发 reasoning_effort"
        );
    }
    assert_eq!(
        body(Some("xhigh"), &Compat::default()),
        format!(
            r#"{},"reasoning_effort":"xhigh"}}"#,
            head(&Compat::default())
        ),
        "目录里的名字原样发"
    );
}

#[test]
fn off_and_on_use_the_toggle_of_the_profile() {
    let compat = deepseek();
    assert_eq!(
        body(Some(EFFORT_OFF), &compat),
        format!(r#"{},"thinking":{{"type":"disabled"}}}}"#, head(&compat))
    );
    assert_eq!(
        body(Some(EFFORT_ON), &compat),
        format!(r#"{},"thinking":{{"type":"enabled"}}}}"#, head(&compat))
    );
}

#[test]
fn without_a_toggle_off_is_none_and_on_adds_nothing() {
    let compat = Compat::default();
    assert_eq!(
        body(Some(EFFORT_OFF), &compat),
        format!(r#"{},"reasoning_effort":"none"}}"#, head(&compat))
    );
    assert_eq!(body(Some(EFFORT_ON), &compat), body(None, &compat));
}
