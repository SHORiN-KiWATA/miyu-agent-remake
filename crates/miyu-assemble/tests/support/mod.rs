//! 探针和随机日志共用的（`docs/designs/08-上下文投影.md` 第七节「测试门禁」，施工 1-14、2-9 下）。
//!
//! 会话由真内核跑出来：执行器替身（`miyu_kernel::testkit`）照剧本回模型、回工具，组装用出厂的
//! 那一套。替身发过的每一次请求，连同发它时的情形（[`Sent`]），交给 [`check`] 查五条性质。
//! 1-14 那时还没有回合状态机，这里是一个照图纸推日志的假内核；2-9（下）换成了真会话。

#![allow(dead_code, reason = "两个测试各用其中一部分")]

use std::collections::BTreeMap;

use miyu_assemble::{DefaultAssembler, Stable, Texts, TurnEndedTexts};
use miyu_drivers::openai_chat::{self, Compat, Encoded, ReasoningField, ReasoningReplay};
use miyu_drivers::{Call, DriverTextSources, DriverTexts, Inputs};
use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, Event};
use miyu_kernel::facts::{Environment, FactTemplates};
use miyu_kernel::id::{CallId, ModelName, Seq};
use miyu_kernel::raw::RawJson;
use miyu_kernel::request::{Message, Request, ToolSpec};
use miyu_kernel::session::Policy;
use miyu_kernel::testkit::Stage;
use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_kernel::tool::{Access, ToolRule, ToolTextSources, ToolTexts};

/// 会话开始的时刻：东九区 16:00。
const START: &str = "2026-09-25T07:00:00.000Z";
/// 两件工具的参数格式：一个路径。
const PATH: &str =
    r#"{"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}"#;

/// 一次请求，和发它时的情形。
pub struct Sent {
    /// 组装出来的请求。
    pub request: Request,
    /// 上一次请求以后撤销、恢复或者压缩过：前缀可以改写。
    pub rewritten: bool,
    /// 这是一个回合的第一次请求，由人的一句话触发：那句话的最后一块。
    pub trigger: Option<Block>,
}

/// 探针和随机日志的策略：出厂的组装、事实模板、写给模型的句子；读、写两件工具；一个回合最多
/// 请求三次模型。
pub fn policy() -> Policy {
    let rule = |access: Access| ToolRule {
        access,
        parameters: serde_json::from_str(PATH).expect("参数格式是 JSON"),
    };
    Policy {
        assembler: Box::new(DefaultAssembler::new(stable(), texts())),
        facts: templates(),
        tools: BTreeMap::from([
            ("read".to_string(), rule(Access::Read)),
            ("write".to_string(), rule(Access::Write)),
        ]),
        step_limit: Some(3),
        tool_texts: tool_texts(),
        attended: true,
        resumes: 3,
    }
}

/// 一个替身：照 [`policy`] 造的会话，在 `~/src/miyu`，从东九区 16:00 开始。
pub fn stage() -> Stage {
    let environment = Environment {
        offset: UtcOffset::from_minutes(540).expect("东九区在范围里"),
        cwd: "~/src/miyu".to_string(),
    };
    let start = Timestamp::parse(START).expect("开始的时刻合写法");
    Stage::new(policy, environment, start)
}

/// 替身的日志，一条一行。
pub fn lines(stage: &Stage) -> Vec<String> {
    stage.log().iter().map(Event::to_line).collect()
}

/// 替身发过的每一次请求，和发它时的情形：上一次请求看到的之后、这一次看到的为止，有撤销、恢复、
/// 压缩的，算改写过；一个回合的第一次请求，触发它的是人的消息的，记下那句话的最后一块。
pub fn sent(stage: &Stage) -> Vec<Sent> {
    let log = stage.log();
    let mut before: Option<Seq> = None;
    let mut sent = Vec::new();
    for (seen, request) in stage.requests() {
        let since =
            |event: &&Event| before.is_none_or(|before| event.seq > before) && event.seq <= *seen;
        let rewritten = log.iter().filter(since).any(|event| {
            matches!(
                event.body,
                Body::TurnReverted(_) | Body::TurnUnreverted(_) | Body::ContextCompacted(_)
            )
        });
        let trigger = log
            .iter()
            .filter(|event| event.seq <= *seen)
            .rev()
            .find_map(|event| match &event.body {
                Body::TurnStarted(started) => Some((event.seq, started.trigger)),
                _ => None,
            })
            .filter(|(turn, _)| before.is_none_or(|before| before < *turn))
            .and_then(|(_, trigger)| log.iter().find(|event| event.seq == trigger))
            .and_then(|event| match &event.body {
                Body::MessageUser(message) => message.blocks.last().cloned(),
                _ => None,
            });
        sent.push(Sent {
            request: request.clone(),
            rewritten,
            trigger,
        });
        before = Some(*seen);
    }
    sent
}

/// 查每一次请求的五条性质：同样的日志出同样的字节（由调用的一方造两遍来比）之外的四条：
/// 是上一次的前缀延伸，除非中间压缩过、撤销过；工具调用和结果成对，结果按调用的先后紧跟着；
/// 没有连着的两条 user 消息；每个回合第一次请求的最后一块，是触发它的那条消息。
///
/// 前缀延伸在线上这一层也查：编码成 OpenAI 兼容接口的字节（[`wire`]），也是上一次的前缀延伸
/// （施工 3-4 上）。缓存命中看的是真发出去的字节。
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
            ends_with(request, trigger).map_err(|why| format!("第 {number} 次请求：{why}"))?;
        }
        if index > 0 && !now.rewritten {
            let before = &sent[index - 1].request;
            extends(request, before)
                .map_err(|why| format!("第 {number} 次请求不是上一次的前缀延伸：{why}"))?;
            wire_extends(&wire(request), &wire(before))
                .map_err(|why| format!("第 {number} 次请求编码以后不是上一次的前缀延伸：{why}"))?;
        }
    }
    Ok(())
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

/// 编码成 OpenAI 兼容接口的字节，用 DeepSeek 那一套：模型 `deepseek-v4`，输出上限 8192，每条
/// assistant 都带 `reasoning_content`。探针的线上存档也是它。
pub fn wire(request: &Request) -> Encoded {
    let call = Call {
        model: ModelName::parse("deepseek-v4").expect("模型名合写法"),
        max_output: Some(8192),
        inputs: Inputs::default(),
    };
    let compat = Compat {
        reasoning: ReasoningReplay::Replay {
            field: ReasoningField::ReasoningContent,
            always: true,
        },
        ..Compat::default()
    };
    openai_chat::encode(request, &call, &compat, &driver_texts(), &BTreeMap::new())
        .expect("探针里没有图片、文件，不要 blob")
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

/// 出厂的驱动占位，从资源目录读。
fn driver_texts() -> DriverTexts {
    DriverTexts::new(DriverTextSources {
        image_omitted: include_str!("../../../../resources/core/drivers/image-omitted.txt"),
        file_omitted: include_str!("../../../../resources/core/drivers/file-omitted.txt"),
        no_output: include_str!("../../../../resources/core/drivers/no-output.txt"),
        tool_attachments: include_str!("../../../../resources/core/drivers/tool-attachments.txt"),
        tool_attachments_only: include_str!(
            "../../../../resources/core/drivers/tool-attachments-only.txt"
        ),
    })
    .expect("出厂的占位用得了")
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
fn ends_with(request: &Request, trigger: &Block) -> Result<(), String> {
    match request.messages.last() {
        Some(Message::User { blocks }) if blocks.last() == Some(trigger) => Ok(()),
        _ => Err("最后一块不是触发这一回合的那条消息".to_string()),
    }
}

/// 探针的稳定区：两件假工具，一句 system。真的等施工 3-6。
fn stable() -> Stable {
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
        system: "You are a helpful software engineer.".to_string(),
        demos: vec![],
    }
}

/// 出厂的英文，资源目录里的真文件。
fn texts() -> Texts {
    Texts {
        checkpoint_open: include_str!("../../../../resources/core/checkpoint-open.txt").to_string(),
        checkpoint_close: include_str!("../../../../resources/core/checkpoint-close.txt")
            .to_string(),
        turn_ended: TurnEndedTexts {
            interrupted: include_str!("../../../../resources/core/turn-ended/interrupted.txt")
                .to_string(),
            error: include_str!("../../../../resources/core/turn-ended/error.txt").to_string(),
            step_limit: include_str!("../../../../resources/core/turn-ended/step_limit.txt")
                .to_string(),
            aborted: include_str!("../../../../resources/core/turn-ended/aborted.txt").to_string(),
            restarted: include_str!("../../../../resources/core/turn-ended/restarted.txt")
                .to_string(),
        },
    }
}

/// 出厂的两个事实模板。
fn templates() -> FactTemplates {
    FactTemplates::new(
        include_str!("../../../../resources/core/facts/env.txt"),
        include_str!("../../../../resources/core/facts/permission.txt"),
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
