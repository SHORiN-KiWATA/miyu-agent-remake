//! 探针和随机日志共用的（`docs/designs/08-上下文投影.md` 第七节「测试门禁」，施工 1-14、2-9 下）。
//!
//! 会话由真内核跑出来：执行器替身（`miyu_kernel::testkit`）照剧本回模型、回工具，组装用出厂的
//! 那一套。替身发过的每一次请求，连同发它时的情形（[`Sent`]），交给 [`check`] 查五条性质。
//! 1-14 那时还没有回合状态机，这里是一个照图纸推日志的假内核；2-9（下）换成了真会话。

#![allow(dead_code, unused_imports, reason = "几个测试各用其中一部分")]

mod anchor;
mod archive;
pub mod reminder;
mod sent;
pub mod sight;
mod texts;
mod titled;

pub use anchor::anchored;
pub use archive::{face_files, files, matches_the_archive, matches_the_archive_with_faces};
pub use sent::{Sent, sent};
pub use texts::{LINES, SUMMARIZE, VENUE, recap, title, vision};
use texts::{driver_texts, texts};
pub use titled::titled_stage;

use std::collections::BTreeMap;

use miyu_assemble::{DefaultAssembler, Stable};
use miyu_drivers::openai_chat::{self, Compat, Encoded};
use miyu_drivers::{Call, Inputs};
use miyu_drivers::{anthropic, openai_responses};
use miyu_kernel::block::{Block, Text};
use miyu_kernel::estimate::Flat;
use miyu_kernel::event::{Body, Event};
use miyu_kernel::facts::{Environment, FactTemplates};
use miyu_kernel::id::{CallId, ModelName, Seq};
use miyu_kernel::origin::By;
use miyu_kernel::raw::RawJson;
use miyu_kernel::request::{Message, Request, ToolSpec};
use miyu_kernel::session::{Compaction, Policy};
use miyu_kernel::testkit::{Line, Stage};
use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_kernel::tool::{Access, ToolRule, ToolTextSources, ToolTexts};

/// 会话开始的时刻：东九区 16:00。
const START: &str = "2026-09-25T07:00:00.000Z";
/// 探针里子会话的父会话。
pub const PARENT: &str = "01a0d75d-2180-7a3c-9e41-5b7d2c8f6a10";

/// 探针的人设：软件工程师的那一句。
const PERSONA: &str = "You are a helpful software engineer.";

/// 两件工具的参数格式：一个路径。
const PATH: &str =
    r#"{"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}"#;

/// 探针和随机日志的策略：出厂的组装、事实模板、写给模型的句子；读、写两件工具；一个回合最多
/// 请求三次模型。system 是人设，空一行接核心的几行（施工 2-7 补）。
pub fn policy() -> Policy {
    policy_with(format!("{PERSONA}\n\n{}", LINES.trim_end()))
}

/// 子代理这张脸的策略（施工 7-5）：system 照拼快照的规矩在人设后面空一行接上场所说明，再空一行接核心的几行，别的和
/// [`policy`] 一样。
pub fn child_policy() -> Policy {
    policy_with(format!(
        "{PERSONA}\n\n{}\n\n{}",
        VENUE.trim_end(),
        LINES.trim_end()
    ))
}

/// 同 [`policy`]，system 是 `system`。
fn policy_with(system: String) -> Policy {
    let rule = |access: Access| ToolRule {
        access,
        parameters: serde_json::from_str(PATH).expect("参数格式是 JSON"),
    };
    Policy {
        foreground: false,
        assembler: Box::new(DefaultAssembler::new(stable(system), texts())),
        facts: templates(),
        tools: BTreeMap::from([
            ("read".to_string(), rule(Access::Read)),
            ("write".to_string(), rule(Access::Write)),
        ]),
        step_limit: Some(3),
        tool_texts: tool_texts(),
        attended: true,
        resumes: 3,
        compaction: Some(Compaction {
            reserve_cap: 20_000,
            margin: 13_000,
            line_percent: 100,
            margin_percent: 100,
            tail: 16_000,
            lead: 0,
            price: Flat {
                image: 2000,
                file: 2000,
            },
            rebuild: None,
            pause: None,
            shorten: None,
            isolate: false,
        }),
        notes: None,
        reports: miyu_kernel::session::Reports {
            chars: 30_000,
            omitted: miyu_kernel::template::Template::parse("").expect("空的模板读得进来"),
        },
        titles: None,
        peers: miyu_kernel::session::Peers {
            burst: 5,
            window: 600,
            unread: 50,
            watch_hours: 12,
            status_chars: 200,
        },
    }
}

/// 替身回摘要请求的那一句：先草稿，再摘要。
pub fn summarizes(stage: &mut Stage) {
    stage.summarize_with(
        SUMMARIZE,
        Line::says(
            "<analysis>\nThe user explored the repository.\n</analysis>\n\n<summary>\n1. Primary Request and Intent: explore src and tests.\n</summary>",
        ),
    );
}

/// 一个替身：照 [`policy`] 造的会话，在 `~/src/miyu`，从东九区 16:00 开始。
pub fn stage() -> Stage {
    stage_with(policy)
}

/// 同 [`stage`]，策略照 `policy` 造（施工 2-7 补：切权限那张脸拿以前造的快照比）。
pub fn stage_with(policy: fn() -> Policy) -> Stage {
    Stage::new(policy, environment(), start())
}

/// 同 [`stage`]，造的是 [`PARENT`] 派出来的子会话，照 [`child_policy`]（施工 7-5）：人说的话都是父会话发的。
pub fn child_stage() -> Stage {
    Stage::child(child_policy, environment(), start(), PARENT)
}

/// 替身的环境：东九区，在 `~/src/miyu`。
fn environment() -> Environment {
    Environment {
        offset: UtcOffset::from_minutes(540).expect("东九区在范围里"),
        cwd: "~/src/miyu".to_string(),
        dirs: Vec::new(),
    }
}

/// 会话开始的时刻。
fn start() -> Timestamp {
    Timestamp::parse(START).expect("开始的时刻合写法")
}

/// 替身的日志，一条一行。
pub fn lines(stage: &Stage) -> Vec<String> {
    stage.log().iter().map(Event::to_line).collect()
}

/// 查每一次请求的五条性质：同样的日志出同样的字节（由调用的一方造两遍来比）之外的四条：
/// 是上一次的前缀延伸，除非中间压缩过、撤销过；工具调用和结果成对，结果按调用的先后紧跟着；
/// 没有连着的两条 user 消息；每个回合第一次请求的最后一块，是触发它的那条消息（这一轮注入了角色扮演提示的，触发后面
/// 跟着它，施工 P-1 补）。
///
/// 前缀延伸在线上这一层也查：编码成 OpenAI 兼容接口的字节（[`wire`]），也是上一次的前缀延伸
/// （施工 3-4 上）；编码成 Anthropic 消息接口的字节去掉打点（[`anthropic_wire`]、`anthropic::unmarked`）也是（施工 8-12）；编码成
/// Responses 接口的字节也是（[`responses_wire`]，施工 8-13）。
/// 缓存命中看的是真发出去的字节。
///
/// # Errors
///
/// 哪一次请求、哪一条不成立，写成一句话。
pub fn check(sent: &[Sent]) -> Result<(), String> {
    for (index, now) in sent.iter().enumerate() {
        let number = index + 1;
        let request = &now.request;
        paired(request).map_err(|why| format!("第 {number} 次请求：{why}"))?;
        no_users_in_a_row(request).map_err(|why| format!("第 {number} 次请求：{why}"))?;
        if let Some(trigger) = &now.trigger {
            ends_with(request, trigger, now.reminder.as_ref())
                .map_err(|why| format!("第 {number} 次请求：{why}"))?;
        }
        if index > 0 && !now.rewritten {
            let then = &sent[index - 1];
            // 上一次是摘要请求的：和它去掉摘要指令的那一份比，指令只在那一次请求里（施工 6-2 上）。
            let before = match then.summary {
                true => without_instruction(&then.request),
                false => then.request.clone(),
            };
            let shared = match now.summary {
                // 摘要请求截到 N：接着上一次往下长；上一次出错、没回复的，N 退到它看到的以前，那就是上一次的前缀。
                true => grown_or_shrunk(request, &before),
                false => grown(request, &before),
            };
            shared.map_err(|why| format!("第 {number} 次请求不是上一次的前缀延伸：{why}"))?;
        }
    }
    Ok(())
}

/// 这一次是上一次的前缀延伸：统一的请求和线上的字节两层。
fn grown(now: &Request, before: &Request) -> Result<(), String> {
    extends(now, before)?;
    wire_extends(&wire(now), &baseline(before)).map_err(|why| format!("编码以后：{why}"))?;
    let plain = |request: &Request| anthropic::unmarked(&anthropic_wire(request));
    wire_extends(&plain(now), &plain(before))
        .map_err(|why| format!("编码成 Anthropic 的、去掉打点以后：{why}"))?;
    wire_extends(&responses_wire(now), &responses_wire(before))
        .map_err(|why| format!("编码成 Responses 的：{why}"))
}

/// 摘要请求：接着上一次往下长；或者去掉摘要指令以后，是上一次的前缀。
fn grown_or_shrunk(summary: &Request, before: &Request) -> Result<(), String> {
    grown(summary, before).or_else(|longer| {
        grown(before, &without_instruction(summary))
            .map_err(|shorter| format!("{longer}；也不是它的前缀：{shorter}"))
    })
}

/// 摘要请求去掉摘要指令：指令是最后一条 user 的最后一块，这条只有它的，整条去掉。
fn without_instruction(request: &Request) -> Request {
    let mut request = request.clone();
    if let Some(Message::User { blocks }) = request.messages.last_mut() {
        blocks.pop();
        if blocks.is_empty() {
            request.messages.pop();
        }
    }
    request
}

/// 最后一块是摘要指令。
pub fn is_summary(request: &Request) -> bool {
    matches!(
        request.messages.last(),
        Some(Message::User { blocks })
            if matches!(blocks.last(), Some(Block::Text(Text { text })) if text == SUMMARIZE)
    )
}

/// 前缀延伸（08 第七节）：工具面、system 不变；上一次的每条消息原样都在；只有最后一条
/// 可以在后面多出几块。
fn extends(now: &Request, before: &Request) -> Result<(), String> {
    if now.tools != before.tools {
        return Err("工具面变了".to_string());
    }
    if now.system != before.system {
        return Err("system 变了".to_string());
    }
    if now.stable != before.stable {
        return Err("稳定区的条数变了".to_string());
    }
    let Some((last, earlier)) = before.messages.split_last() else {
        return Ok(());
    };
    if now.messages.len() < before.messages.len() {
        return Err("消息少了".to_string());
    }
    if let Some(index) = earlier
        .iter()
        .zip(&now.messages)
        .position(|(then, now)| then != now)
    {
        return Err(format!("第 {} 条消息变了", index + 1));
    }
    let index = earlier.len();
    match (last, &now.messages[index]) {
        (then, now) if then == now => Ok(()),
        (Message::User { blocks: then }, Message::User { blocks: now })
            if now.starts_with(then) =>
        {
            Ok(())
        }
        _ => Err(format!("第 {} 条消息变了", index + 1)),
    }
}

/// 编码成 OpenAI 兼容接口的字节，用 DeepSeek 那一套（驱动出厂的 `Compat::deepseek`）：模型
/// `deepseek-v4`，输出上限 8192，每条 assistant 都带 `reasoning_content`，会接着写。探针的线上存档
/// 也是它。
pub fn wire(request: &Request) -> Encoded {
    let call = Call {
        model: ModelName::parse("deepseek-v4").expect("模型名合写法"),
        max_output: Some(8192),
        inputs: Inputs::default(),
        effort: None,
        temperature: None,
    };
    openai_chat::encode(
        request,
        &call,
        &Compat::deepseek(),
        &driver_texts(),
        &BTreeMap::new(),
    )
    .expect("探针里没有图片、文件，不要 blob")
}

/// 编码成 Anthropic 消息接口的字节（施工 8-12）：模型 `claude-opus-5`，输出上限 8192，没有思考强度。这一家不会接着写，带着
/// 接着写记号的照原样发，所以不另找比的那一份。
pub fn anthropic_wire(request: &Request) -> Encoded {
    let call = Call {
        model: ModelName::parse("claude-opus-5").expect("模型名合写法"),
        max_output: Some(8192),
        inputs: Inputs::default(),
        effort: None,
        temperature: None,
    };
    anthropic::encode(request, &call, &driver_texts(), &BTreeMap::new())
        .expect("探针里没有图片、文件，不要 blob")
}

/// 编码成 Responses 接口的字节（施工 8-13）：模型 `gpt-5.4`，没有输出上限、没有思考强度。这一家不会接着写，带着接着写
/// 记号的照原样发。
pub fn responses_wire(request: &Request) -> Encoded {
    let call = Call {
        model: ModelName::parse("gpt-5.4").expect("模型名合写法"),
        max_output: None,
        inputs: Inputs::default(),
        effort: None,
        temperature: None,
    };
    openai_responses::encode(request, &call, &driver_texts(), &BTreeMap::new())
        .expect("探针里没有图片、文件，不要 blob")
}

/// 查线上的前缀延伸时拿来比的那一份：接着写的请求，照不接着写的编码。接着写的那一次去掉了最后那句
/// 提示、半截那条加了字段，下一次请求里又换回普通的写法：这是登记在案的改写（08 第七节）。
fn baseline(request: &Request) -> Encoded {
    wire(&Request {
        continuation: false,
        ..request.clone()
    })
}

/// 线上的前缀延伸：上一次最后一条消息之前的字节一个不差；上一次的最后一条，要么一样，要么只在
/// 后面接着长（去掉收尾的 `"}` 或 `]}` 以后，是这一次那一条的开头）；消息后面的工具面这些也一样。
fn wire_extends(now: &Encoded, before: &Encoded) -> Result<(), String> {
    let Some(last) = before.messages.last() else {
        return Ok(());
    };
    if now.body.get(..last.start) != before.body.get(..last.start) {
        return Err("上一次最后一条消息之前的字节变了".to_string());
    }
    let Some(grown) = now.messages.get(before.messages.len() - 1) else {
        return Err("消息少了".to_string());
    };
    let then = &before.body[last.clone()];
    let now_last = &now.body[grown.clone()];
    let open = &then[..then.len().saturating_sub(2)];
    if now_last != then && !now_last.starts_with(open) {
        return Err("上一次的最后一条不是在后面接着长的".to_string());
    }
    let tail = |encoded: &Encoded| {
        let end = encoded.messages.last().map_or(0, |range| range.end);
        encoded.body[end..].to_vec()
    };
    if tail(now) != tail(before) {
        return Err("消息后面的工具面、参数变了".to_string());
    }
    Ok(())
}

/// 工具调用和结果成对：每条回复里的调用，按先后各有一条结果紧跟在回复后面；没有落单的结果。
fn paired(request: &Request) -> Result<(), String> {
    let messages = &request.messages;
    let mut index = 0;
    while index < messages.len() {
        match &messages[index] {
            Message::Assistant { blocks } => {
                let calls: Vec<CallId> = blocks
                    .iter()
                    .filter_map(|block| match block {
                        Block::ToolCall(call) => Some(call.call_id),
                        _ => None,
                    })
                    .collect();
                for (k, call) in calls.iter().enumerate() {
                    match messages.get(index + 1 + k) {
                        Some(Message::Tool { call_id, .. }) if call_id == call => {}
                        _ => {
                            return Err(format!(
                                "第 {} 条消息的调用 {call} 后面没有紧跟着它的结果",
                                index + 1
                            ));
                        }
                    }
                }
                index += 1 + calls.len();
            }
            Message::Tool { call_id, .. } => {
                return Err(format!(
                    "第 {} 条消息是调用 {call_id} 的结果，前面没有这次调用",
                    index + 1
                ));
            }
            Message::User { .. } => index += 1,
        }
    }
    Ok(())
}

/// 没有连着的两条 user 消息：人这一边挨着的块都合成了一条。
fn no_users_in_a_row(request: &Request) -> Result<(), String> {
    let row = request
        .messages
        .windows(2)
        .position(|pair| matches!(pair, [Message::User { .. }, Message::User { .. }]));
    match row {
        Some(index) => Err(format!("第 {}、{} 条都是 user 消息", index + 1, index + 2)),
        None => Ok(()),
    }
}

/// 请求的最后一块是 `trigger`：当前要回应的那句话离生成位置最近（08 C2）。
fn ends_with(request: &Request, trigger: &Block, reminder: Option<&Block>) -> Result<(), String> {
    let tail: Vec<&Block> = [Some(trigger), reminder].into_iter().flatten().collect();
    match request.messages.last() {
        Some(Message::User { blocks }) if blocks.iter().rev().take(tail.len()).rev().eq(tail) => {
            Ok(())
        }
        _ => Err("最后一块不是触发这一回合的那条消息（有角色扮演提示的，后面跟着它）".to_string()),
    }
}

/// 探针的稳定区：两件假工具，system 是 `system`。
fn stable(system: String) -> Stable {
    let tool = |name: &str, description: &str| ToolSpec {
        name: name.to_string(),
        description: description.to_string(),
        parameters: serde_json::from_str::<RawJson>(
            r#"{"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}"#,
        )
        .expect("参数格式是 JSON"),
    };
    Stable {
        tools: vec![
            tool("write", "Write a text file."),
            tool("read", "Read a text file or list a directory."),
        ],
        system,
        demos: vec![],
    }
}

/// 出厂的五个事实模板。
fn templates() -> FactTemplates {
    FactTemplates::new(
        include_str!("../../../../resources/core/facts/env.txt"),
        include_str!("../../../../resources/core/facts/permission.txt"),
        include_str!("../../../../resources/core/facts/reply-cut.txt"),
        Some(include_str!("../../../../resources/core/facts/session.txt")),
        Some(include_str!(
            "../../../../resources/core/facts/permission-changed.txt"
        )),
    )
    .expect("出厂的模板用得了")
}

/// 出厂的那几句：内核替工具写给模型的。
fn tool_texts() -> ToolTexts {
    ToolTexts::new(ToolTextSources {
        unknown: include_str!("../../../../resources/core/tool-results/unknown.txt"),
        not_an_object: include_str!("../../../../resources/core/tool-results/not-an-object.txt"),
        cancelled_before: include_str!(
            "../../../../resources/core/tool-results/cancelled-before.txt"
        ),
        cancelled_running: include_str!(
            "../../../../resources/core/tool-results/cancelled-running.txt"
        ),
        skipped: include_str!("../../../../resources/core/tool-results/skipped.txt"),
        read_only: include_str!("../../../../resources/core/tool-results/read-only.txt"),
        denied: include_str!("../../../../resources/core/tool-results/denied.txt"),
        denied_with_reason: include_str!(
            "../../../../resources/core/tool-results/denied-with-reason.txt"
        ),
        unattended: include_str!("../../../../resources/core/tool-results/unattended.txt"),
        question_interrupted: include_str!(
            "../../../../resources/core/tool-results/question-interrupted.txt"
        ),
        question_voided: include_str!(
            "../../../../resources/core/tool-results/question-voided.txt"
        ),
        question_unattended: include_str!(
            "../../../../resources/core/tool-results/question-unattended.txt"
        ),
        restarted: include_str!("../../../../resources/core/tool-results/restarted.txt"),
    })
    .expect("出厂的几句用得了")
}
