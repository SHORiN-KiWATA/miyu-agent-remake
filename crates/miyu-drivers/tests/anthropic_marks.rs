//! Anthropic 的缓存打点（`docs/blueprint/drivers/anthropic.md`「缓存打点」，施工 8-12）：四处各在哪，第一次请求、system 空、
//! `stable` 是 0 的，思考块不打、往前找，同一块只打一次、最多 4 处；`unmarked` 去掉以后，一段随机的会话每次请求都是上一次的
//! 前缀延伸。

use std::collections::BTreeMap;

use crate::support::{
    anthropic_sample, claude, family_call, id, private_thought, read_tool, text, texts,
};
use miyu_drivers::anthropic::{encode, unmarked};
use miyu_drivers::{Encoded, Inputs};
use miyu_kernel::block::Block;
use miyu_kernel::request::{Message, Request};
use serde_json::Value;

const MARK: &str = r#","cache_control":{"type":"ephemeral"}"#;

fn user(blocks: Vec<Block>) -> Message {
    Message::User { blocks }
}

fn assistant(blocks: Vec<Block>) -> Message {
    Message::Assistant { blocks }
}

fn tool(call_id: &str, blocks: Vec<Block>) -> Message {
    Message::Tool {
        call_id: id(call_id),
        error: false,
        blocks,
    }
}

fn call(call_id: &str, provider: &str) -> Block {
    family_call(
        "anthropic",
        call_id,
        "read",
        r#"{"path":"a.txt"}"#,
        Some(provider),
    )
}

fn signed(text: &str, signature: &str) -> Block {
    private_thought(
        text,
        "anthropic",
        &format!(r#"{{"signature":"{signature}"}}"#),
    )
}

fn encoded(request: &Request) -> Encoded {
    encode(
        request,
        &claude(Inputs::default(), None),
        &texts(),
        &BTreeMap::new(),
    )
    .expect("不要 blob 的请求编码得了")
}

/// 打了点的地方：`system.<第几块>`、`tools.<第几件>`、`messages.<第几条>.<第几块>`。
fn marks(request: &Request) -> Vec<String> {
    let value: Value = serde_json::from_slice(&encoded(request).body).expect("请求字节是 JSON");
    let mut found = Vec::new();
    let marked = |block: &Value| block.get("cache_control").is_some();
    for (field, list) in [("system", &value["system"]), ("tools", &value["tools"])] {
        for (index, block) in list.as_array().into_iter().flatten().enumerate() {
            if marked(block) {
                found.push(format!("{field}.{index}"));
            }
        }
    }
    for (m, message) in value["messages"]
        .as_array()
        .expect("有消息")
        .iter()
        .enumerate()
    {
        for (b, block) in message["content"]
            .as_array()
            .expect("是数组")
            .iter()
            .enumerate()
        {
            if marked(block) {
                found.push(format!("messages.{m}.{b}"));
            }
            for inner in block["content"].as_array().into_iter().flatten() {
                assert!(!marked(inner), "工具结果里面的块不打点");
            }
        }
    }
    found
}

/// 两条示范对话，接着一轮工具循环。
fn demos() -> Vec<Message> {
    vec![
        user(vec![text("示范：读文件")]),
        assistant(vec![text("示范：好的")]),
    ]
}

fn loop_request() -> Request {
    let mut messages = demos();
    messages.extend([
        user(vec![text("<fact>now</fact>\n"), text("读 a.txt")]),
        assistant(vec![signed("先读", "sig-1"), call("call_1_1", "toolu_1")]),
        tool("call_1_1", vec![text("A")]),
    ]);
    Request {
        tools: vec![read_tool()],
        system: "You are Miyu.".to_string(),
        messages,
        stable: 2,
        continuation: false,
        described: Default::default(),
        output_cap: None,
    }
}

#[test]
fn four_marks_at_the_ends_of_tools_and_system_the_demos_the_last_request_and_this_one() {
    let request = loop_request();
    anthropic_sample("cache-marks", &encoded(&request).body);
    assert_eq!(
        marks(&request),
        ["system.0", "messages.1.0", "messages.2.1", "messages.4.0"]
    );
    // 一份请求里打点的写法只有一种。
    let body = String::from_utf8(encoded(&request).body).expect("UTF-8");
    assert_eq!(body.matches("cache_control").count(), 4);
    assert_eq!(body.matches(MARK).count(), 4);
    assert!(!body.contains("\"ttl\""));
}

#[test]
fn the_first_request_of_a_session_has_no_mark_for_the_last_request() {
    let mut request = loop_request();
    request.messages.truncate(3);
    anthropic_sample("cache-marks-first", &encoded(&request).body);
    assert_eq!(
        marks(&request),
        ["system.0", "messages.1.0", "messages.2.1"]
    );
}

#[test]
fn without_a_system_prompt_the_first_mark_goes_on_the_last_tool() {
    let mut request = loop_request();
    request.system.clear();
    request.stable = 0;
    anthropic_sample("cache-marks-no-system", &encoded(&request).body);
    assert_eq!(marks(&request), ["tools.0", "messages.2.1", "messages.4.0"]);
    request.tools.clear();
    assert_eq!(marks(&request), ["messages.2.1", "messages.4.0"]);
}

#[test]
fn a_thinking_block_is_never_marked_the_mark_walks_back() {
    let mut request = loop_request();
    request.messages[1] = assistant(vec![text("示范：好的"), signed("想", "sig-0")]);
    assert_eq!(
        marks(&request),
        ["system.0", "messages.1.0", "messages.2.1", "messages.4.0"]
    );
    // 整条都没有能打的：这一处不打。
    request.messages[1] = assistant(vec![signed("", "sig-0")]);
    assert_eq!(
        marks(&request),
        ["system.0", "messages.2.1", "messages.4.0"]
    );
}

#[test]
fn the_stable_mark_counts_the_blocks_the_demos_wrote() {
    // 示范对话的最后一条是 user、跟着的也是 user：合成一条，打在示范写出的那一块上，不打在整条的最后一块上。
    let request = Request {
        tools: vec![],
        system: "You are Miyu.".to_string(),
        messages: vec![
            user(vec![text("示范")]),
            user(vec![text("真的"), text("问题")]),
        ],
        stable: 1,
        continuation: false,
        described: Default::default(),
        output_cap: None,
    };
    assert_eq!(
        marks(&request),
        ["system.0", "messages.0.0", "messages.0.2"]
    );
}

#[test]
fn a_block_is_marked_once_and_never_more_than_four() {
    // 示范只有一条 user：稳定区的末尾就是上一次请求的末尾那一块。
    let request = Request {
        tools: vec![read_tool()],
        system: "You are Miyu.".to_string(),
        messages: vec![
            user(vec![text("示范")]),
            assistant(vec![text("好")]),
            user(vec![text("再来")]),
        ],
        stable: 1,
        continuation: false,
        described: Default::default(),
        output_cap: None,
    };
    assert_eq!(
        marks(&request),
        ["system.0", "messages.0.0", "messages.2.0"]
    );
    for request in session(7) {
        assert!(marks(&request).len() <= 4);
    }
}

#[test]
fn unmarked_takes_out_every_mark_and_fixes_the_ranges() {
    let marked = encoded(&loop_request());
    let plain = unmarked(&marked);
    let text = String::from_utf8(plain.body.clone()).expect("UTF-8");
    assert!(!text.contains("cache_control"));
    assert_eq!(plain.path, marked.path);
    assert_eq!(plain.messages.len(), marked.messages.len());
    for range in &plain.messages {
        let message: Value =
            serde_json::from_slice(&plain.body[range.clone()]).expect("每条是一个 JSON");
        assert!(message["role"].is_string());
    }
    let marked_text = String::from_utf8(marked.body).expect("UTF-8");
    assert_eq!(marked_text.replace(MARK, ""), text);
}

#[test]
fn every_request_of_a_random_session_extends_the_last_one_once_the_marks_are_out() {
    for seed in 1..=60 {
        let requests = session(seed);
        for pair in requests.windows(2) {
            let (before, now) = (unmarked(&encoded(&pair[0])), unmarked(&encoded(&pair[1])));
            if let Err(why) = extends(&now, &before) {
                panic!("种子 {seed}：{why}");
            }
        }
    }
}

/// 线上的前缀延伸：上一次最后一条消息之前的字节一个不差；上一次的最后一条要么一样，要么只在后面接着长（去掉收尾的
/// `]}` 以后是这一次那一条的开头）；消息后面的也一样。
fn extends(now: &Encoded, before: &Encoded) -> Result<(), String> {
    let last = before.messages.last().ok_or("上一次没有消息")?;
    if now.body.get(..last.start) != before.body.get(..last.start) {
        return Err("上一次最后一条消息之前的字节变了".to_string());
    }
    let grown = now
        .messages
        .get(before.messages.len() - 1)
        .ok_or("消息少了")?;
    let then = &before.body[last.clone()];
    let now_last = &now.body[grown.clone()];
    if now_last != then && !now_last.starts_with(&then[..then.len() - 2]) {
        return Err("上一次的最后一条不是在后面接着长的".to_string());
    }
    let tail =
        |encoded: &Encoded| encoded.body[encoded.messages.last().map_or(0, |r| r.end)..].to_vec();
    if tail(now) != tail(before) {
        return Err("消息后面的变了".to_string());
    }
    Ok(())
}

/// 一段随机的会话：每一次请求是上一次的统一的请求接着长（`seed` 定怎么长）。人说话、事实接在人说的话里、回复带思考和
/// 几次工具调用、结果、没等到回复又说了一句。
fn session(seed: u64) -> Vec<Request> {
    let mut random = Lcg(seed);
    let mut request = Request {
        tools: vec![read_tool()],
        system: "You are Miyu.".to_string(),
        messages: demos(),
        stable: 2,
        continuation: false,
        described: Default::default(),
        output_cap: None,
    };
    request.messages.push(user(vec![text("开始")]));
    let mut requests = vec![request.clone()];
    let mut reply = 0;
    for _ in 0..12 {
        match random.next() % 3 {
            // 回复，可能带思考、几次调用；调用了的，结果跟着。
            0 | 1 => {
                reply += 1;
                let mut blocks = Vec::new();
                if random.next().is_multiple_of(2) {
                    blocks.push(signed("想", &format!("sig-{reply}")));
                }
                blocks.push(text("好"));
                let calls = random.next() % 4;
                for n in 1..=calls {
                    blocks.push(call(
                        &format!("call_{reply}_{n}"),
                        &format!("toolu_{reply}_{n}"),
                    ));
                }
                request.messages.push(assistant(blocks));
                if calls == 0 {
                    request
                        .messages
                        .push(user(vec![text("<fact>t</fact>\n"), text("接着")]));
                }
                for n in 1..=calls {
                    let blocks = (0..random.next() % 30)
                        .map(|k| text(&format!("行 {k}")))
                        .collect();
                    request
                        .messages
                        .push(tool(&format!("call_{reply}_{n}"), blocks));
                }
            }
            // 没等到回复，人又说了一句：接在最后那条 user 后面。
            _ => request.messages.push(user(vec![text("还有")])),
        }
        requests.push(request.clone());
    }
    requests
}

/// 线性同余：测试里够用的随机数，种子一样出的一样。
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
}
