//! 测试里的假内核（`docs/designs/08-上下文投影.md` 第七节「测试门禁」，施工 1-14）。
//!
//! 真的回合状态机在施工 2-x 做。这里照图纸推它会怎么记日志：每一条事件先过账本；在回合开始、
//! 回合中途切了权限级别以后的下一步、回合中途压缩以后的下一步，用 1-13 的函数注入变了的事实；
//! 每发一次请求，就把那一刻的有效历史组装成请求记下来，连同上一次请求以后有没有压缩、撤销过。
//! 记下的请求交给 [`check`] 查五条性质。

#![allow(dead_code, reason = "两个测试各用其中一部分")]

use std::collections::BTreeMap;

use miyu_assemble::{DefaultAssembler, Stable, Texts, TurnEndedTexts};
use miyu_kernel::assemble::Assembler;
use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Event, Level, Permission};
use miyu_kernel::facts::{Environment, FactTemplates, changed};
use miyu_kernel::history::History;
use miyu_kernel::id::CallId;
use miyu_kernel::ledger::Ledger;
use miyu_kernel::origin::By;
use miyu_kernel::raw::RawJson;
use miyu_kernel::request::{Message, Request, ToolSpec};
use miyu_kernel::time::{Timestamp, UtcOffset};

const KERNEL: &str = r#"{"kind":"kernel"}"#;
const ALICE: &str = r#"{"kind":"person","account":"alice"}"#;
const MODEL: &str = r#"{"kind":"model","endpoint":"deepseek","model":"deepseek-v4"}"#;
const CREATED: &str = r#"{"owner":"alice","venue":"local","policy":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","permission":{"level":"workspace","read_only":false}}"#;
/// 会话开始的时刻：东九区 16:00。
const START: &str = "2026-09-25T07:00:00.000Z";

/// 一次请求，和发它时的情形。
pub struct Sent {
    /// 组装出来的请求。
    pub request: Request,
    /// 上一次请求以后压缩过或撤销过：前缀可以改写。
    pub rewritten: bool,
    /// 这是一个回合的第一次请求：触发这一回合的那条消息的最后一块。
    pub trigger: Option<Block>,
}

/// 一个终端会话，照先后追加事件。
pub struct Session {
    ledger: Ledger,
    history: History,
    /// 日志的每一行。
    lines: Vec<String>,
    assembler: DefaultAssembler,
    templates: FactTemplates,
    /// 现在，Unix 毫秒数。每追加一条事件往后走一秒。
    now: i64,
    permission: Permission,
    /// 正在进行的回合。
    turn: Option<u64>,
    /// 这一回合还没发第一次请求：触发它的那条消息的最后一块。
    trigger: Option<Block>,
    /// 下一步要把事实查一遍：回合中途切了权限级别，或者压缩了。
    refresh: bool,
    /// 上一次请求以后压缩过或撤销过。
    rewritten: bool,
    /// 每条人的消息的最后一块，按序号。
    said: BTreeMap<u64, Block>,
    /// 发过的请求。
    sent: Vec<Sent>,
}

impl Session {
    /// 一个刚创建的会话：常用的那一级是工作区，只读关着。
    pub fn new() -> Session {
        let mut session = Session {
            ledger: Ledger::default(),
            history: History::default(),
            lines: Vec::new(),
            assembler: DefaultAssembler::new(stable(), texts()),
            templates: templates(),
            now: Timestamp::parse(START)
                .expect("开始的时刻合写法")
                .unix_millis(),
            permission: Permission {
                level: Level::Workspace,
                read_only: false,
            },
            turn: None,
            trigger: None,
            refresh: false,
            rewritten: false,
            said: BTreeMap::new(),
            sent: Vec::new(),
        };
        session.push(KERNEL, "session.created", CREATED);
        session
    }

    /// 日志的每一行。
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// 发过的每一次请求。
    pub fn sent(&self) -> &[Sent] {
        &self.sent
    }

    /// 只读开着没有。
    pub fn is_read_only(&self) -> bool {
        self.permission.read_only
    }

    /// 时间往后走几分钟。
    pub fn advance(&mut self, minutes: i64) {
        self.now += minutes * 60_000;
    }

    /// 追加一条：`by` 引起的、种类是 `kind` 的事件，`body` 照原文。回合里的带上回合。
    /// 返回它的序号。
    pub fn push(&mut self, by: &str, kind: &str, body: &str) -> u64 {
        let seq = self.ledger.next_seq().get();
        let at = Timestamp::from_unix_millis(self.now).expect("会话的时间在范围里");
        let turn = self
            .turn
            .map(|turn| format!(r#""turn":{turn},"#))
            .unwrap_or_default();
        let line =
            format!(r#"{{"seq":{seq},"at":"{at}","kind":"{kind}",{turn}"by":{by},"body":{body}}}"#);
        let event = Event::from_line(&line).unwrap_or_else(|e| panic!("{line} 读不出来：{e}"));
        self.ledger
            .append(&event)
            .unwrap_or_else(|e| panic!("{line} 过不了账本：{e}"));
        self.lines.push(event.to_line());
        self.history.append(event);
        self.now += 1000;
        seq
    }

    /// alice 说一句话。返回它的序号。
    pub fn say(&mut self, words: &str) -> u64 {
        let body = format!(r#"{{"blocks":[{}]}}"#, text_json(words));
        let seq = self.push(ALICE, "message.user", &body);
        self.said.insert(seq, text(words));
        seq
    }

    /// 由第 `trigger` 条开一个回合，注入变了的事实。返回回合的编号。
    pub fn start(&mut self, trigger: u64) -> u64 {
        let turn = self.ledger.next_seq().get();
        self.turn = Some(turn);
        self.push(
            KERNEL,
            "turn.started",
            &format!(r#"{{"trigger":{trigger}}}"#),
        );
        self.trigger = self.said.get(&trigger).cloned();
        self.inject_changed();
        turn
    }

    /// 发一次请求：该查事实的先查一遍，再把这一刻的有效历史组装成请求记下来。
    /// 返回这次请求看到了第几条为止。
    pub fn request(&mut self) -> u64 {
        if std::mem::take(&mut self.refresh) {
            self.inject_changed();
        }
        self.sent.push(Sent {
            request: self.assembler.assemble(&self.history),
            rewritten: std::mem::take(&mut self.rewritten),
            trigger: self.trigger.take(),
        });
        self.ledger.next_seq().get() - 1
    }

    /// 模型回了一句话，调用了 `calls` 里的这几件工具（工具名、参数原文）。返回调用编号。
    pub fn reply(&mut self, seen: u64, words: &str, calls: &[(&str, &str)]) -> Vec<String> {
        self.respond(seen, words, calls, false)
    }

    /// 模型回到一半被打断：收到的这句话和收全了的这几次调用。返回调用编号。
    pub fn cut_off(&mut self, seen: u64, words: &str, calls: &[(&str, &str)]) -> Vec<String> {
        self.respond(seen, words, calls, true)
    }

    fn respond(
        &mut self,
        seen: u64,
        words: &str,
        calls: &[(&str, &str)],
        interrupted: bool,
    ) -> Vec<String> {
        let seq = self.ledger.next_seq().get();
        let ids: Vec<String> = (1..=calls.len())
            .map(|k| format!("call_{seq}_{k}"))
            .collect();
        let mut blocks = vec![text_json(words)];
        for (id, (name, args)) in ids.iter().zip(calls) {
            blocks.push(format!(
                r#"{{"type":"tool_call","call_id":"{id}","name":"{name}","args":{}}}"#,
                quoted(args)
            ));
        }
        let cut = if interrupted {
            r#","interrupted":true"#
        } else {
            ""
        };
        let body = format!(r#"{{"blocks":[{}],"seen":{seen}{cut}}}"#, blocks.join(","));
        self.push(MODEL, "message.assistant", &body);
        ids
    }

    /// 调用 `call` 的结果：状态，和一句给模型看的话。
    pub fn result(&mut self, call: &str, status: &str, words: &str) {
        let by = format!(r#"{{"kind":"tool","call_id":"{call}"}}"#);
        let body = format!(
            r#"{{"call_id":"{call}","status":"{status}","blocks":[{}]}}"#,
            text_json(words)
        );
        self.push(&by, "tool.result", &body);
    }

    /// 结束回合。
    pub fn end(&mut self, reason: &str) {
        self.push(KERNEL, "turn.ended", &format!(r#"{{"reason":"{reason}"}}"#));
        self.turn = None;
        self.trigger = None;
        self.refresh = false;
    }

    /// 开关只读。回合中途切的，下一步把事实查一遍。
    pub fn read_only(&mut self, on: bool) {
        self.permission.read_only = on;
        let body = format!(r#"{{"permission":{{"level":"workspace","read_only":{on}}}}}"#);
        self.push(ALICE, "session.policy_changed", &body);
        self.refresh |= self.turn.is_some();
    }

    /// 撤销一个回合。
    pub fn undo(&mut self, turn: u64) {
        self.push(ALICE, "turn.reverted", &format!(r#"{{"turns":[{turn}]}}"#));
        self.rewritten = true;
    }

    /// 压缩：摘要替代到上一条为止。回合中途压的，下一步把事实查一遍。
    pub fn compact(&mut self, summary: &str) {
        let upto = self.ledger.next_seq().get() - 1;
        let body = format!(r#"{{"upto":{upto},"summary":{}}}"#, quoted(summary));
        self.push(KERNEL, "context.compacted", &body);
        self.rewritten = true;
        self.refresh |= self.turn.is_some();
    }

    /// 环境和权限两块，变了的注入（08 C10）。
    fn inject_changed(&mut self) {
        let now = Timestamp::from_unix_millis(self.now).expect("会话的时间在范围里");
        let environment = Environment {
            offset: UtcOffset::from_minutes(540).expect("东九区在范围里"),
            cwd: "~/src/miyu".to_string(),
        };
        let facts = vec![
            self.templates.env(now, &environment),
            self.templates.permission(&self.permission),
        ];
        for fact in changed(&self.history, &By::Kernel, facts) {
            let body = format!(
                r#"{{"kind":"{}","text":{}}}"#,
                fact.kind.as_str(),
                quoted(&fact.text)
            );
            self.push(KERNEL, "context.injected", &body);
        }
    }
}

/// 查每一次请求的五条性质：同样的日志出同样的字节（由调用的一方造两遍来比）之外的四条：
/// 是上一次的前缀延伸，除非中间压缩过、撤销过；工具调用和结果成对，结果按调用的先后紧跟着；
/// 没有连着的两条 user 消息；每个回合第一次请求的最后一块，是触发它的那条消息。
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
            extends(request, &sent[index - 1].request)
                .map_err(|why| format!("第 {number} 次请求不是上一次的前缀延伸：{why}"))?;
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

/// 一个文本块。
fn text(words: &str) -> Block {
    Block::Text(Text {
        text: words.to_string(),
    })
}

/// 一段字写成 JSON 字符串。
fn quoted(words: &str) -> String {
    serde_json::to_string(words).expect("字符串写得成 JSON")
}

/// 一个文本块的 JSON。
fn text_json(words: &str) -> String {
    format!(r#"{{"type":"text","text":{}}}"#, quoted(words))
}
