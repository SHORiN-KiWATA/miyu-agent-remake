//! 测试共用的零件：照先后拼一段合规的日志；一套替身的固定字；把消息写成一眼看得懂的样子。

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::Event;
use miyu_kernel::history::History;
use miyu_kernel::ledger::Ledger;
use miyu_kernel::request::Message;

use crate::texts::{Texts, TurnEndedTexts};

const KERNEL: &str = r#"{"kind":"kernel"}"#;
const ALICE: &str = r#"{"kind":"person","account":"alice"}"#;
const MODEL: &str = r#"{"kind":"model","endpoint":"deepseek","model":"deepseek-v4"}"#;
const CREATED: &str = r#"{"owner":"alice","venue":"local","policy":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","permission":{"level":"workspace","read_only":false}}"#;

/// 替身的固定字：短，一眼认得出是哪一句。测的是拼法，和出厂的措辞无关。
pub(crate) fn texts() -> Texts {
    Texts {
        checkpoint_open: "<checkpoint>\n".to_string(),
        checkpoint_close: "\n</checkpoint>\n".to_string(),
        turn_ended: TurnEndedTexts {
            interrupted: "<interrupted/>".to_string(),
            error: "<error/>".to_string(),
            step_limit: "<step-limit/>".to_string(),
            aborted: "<aborted/>".to_string(),
            restarted: "<restarted/>".to_string(),
        },
    }
}

/// 一个文本块。
pub(crate) fn text(text: &str) -> Block {
    Block::Text(Text {
        text: text.to_string(),
    })
}

/// 一段字写成 JSON 字符串。
fn quoted(text: &str) -> String {
    serde_json::to_string(text).unwrap()
}

/// 一个文本块的 JSON。
pub(crate) fn text_json(text: &str) -> String {
    format!(r#"{{"type":"text","text":{}}}"#, quoted(text))
}

/// 一段日志：照先后追加，序号自动往后编。每一条都先交给账本查过，保证测的是合规的日志；
/// 查过的同时交给有效历史。
pub(crate) struct Log {
    ledger: Ledger,
    history: History,
    /// 正在进行的回合，由 [`Log::start`] 开、[`Log::end`] 关。
    turn: Option<u64>,
}

impl Log {
    /// 一个刚创建的会话：第 1 条是 `session.created`。
    pub(crate) fn new() -> Log {
        let mut log = Log {
            ledger: Ledger::default(),
            history: History::default(),
            turn: None,
        };
        log.push(KERNEL, "session.created", CREATED);
        log
    }

    /// 到现在为止的有效历史。
    pub(crate) fn history(&self) -> &History {
        &self.history
    }

    /// 下一条会是几号。
    pub(crate) fn next(&self) -> u64 {
        self.ledger.next_seq().get()
    }

    /// 追加一条：`by` 引起的、种类是 `kind` 的事件，`body` 照原文。回合里追加的带上回合。
    /// 返回它的序号。
    pub(crate) fn push(&mut self, by: &str, kind: &str, body: &str) -> u64 {
        let seq = self.next();
        let turn = self
            .turn
            .map(|turn| format!(r#""turn":{turn},"#))
            .unwrap_or_default();
        let line = format!(
            r#"{{"seq":{seq},"at":"2026-09-25T07:00:00.000Z","kind":"{kind}",{turn}"by":{by},"body":{body}}}"#
        );
        let event = Event::from_line(&line).unwrap();
        self.ledger.append(&event).unwrap();
        self.history.append(event);
        seq
    }

    /// alice 发来一条消息，内容块照原文（JSON 数组）。返回它的序号。
    pub(crate) fn send(&mut self, blocks: &str) -> u64 {
        self.push(ALICE, "message.user", &format!(r#"{{"blocks":{blocks}}}"#))
    }

    /// alice 说一句话。返回它的序号。
    pub(crate) fn say(&mut self, words: &str) -> u64 {
        self.send(&format!("[{}]", text_json(words)))
    }

    /// 开一个回合，由第 `trigger` 条触发。
    pub(crate) fn start(&mut self, trigger: u64) {
        self.turn = Some(self.next());
        self.push(
            KERNEL,
            "turn.started",
            &format!(r#"{{"trigger":{trigger}}}"#),
        );
    }

    /// 注入一块事实。
    pub(crate) fn fact(&mut self, fact: &str) {
        let body = format!(r#"{{"kind":"env","text":{}}}"#, quoted(fact));
        self.push(KERNEL, "context.injected", &body);
    }

    /// 模型回复，内容块照原文（JSON 数组）。它的请求看到了上一条为止。返回它的序号。
    pub(crate) fn reply(&mut self, blocks: &str) -> u64 {
        let seen = self.next() - 1;
        let body = format!(r#"{{"blocks":{blocks},"seen":{seen}}}"#);
        self.push(MODEL, "message.assistant", &body)
    }

    /// 模型回一句话，再调用一次 `read`。返回这次调用的编号。
    pub(crate) fn reply_calling(&mut self, words: &str) -> String {
        let call = format!("call_{}_1", self.next());
        let tool_call =
            format!(r#"{{"type":"tool_call","call_id":"{call}","name":"read","args":"{{}}"}}"#);
        self.reply(&format!("[{},{tool_call}]", text_json(words)));
        call
    }

    /// 调用 `call` 的结果：状态，和一句给模型看的话。
    pub(crate) fn result(&mut self, call: &str, status: &str, words: &str) {
        let body = format!(
            r#"{{"call_id":"{call}","status":"{status}","blocks":[{}]}}"#,
            text_json(words)
        );
        self.push(KERNEL, "tool.result", &body);
    }

    /// 结束回合，返回那条 `turn.ended` 的序号。
    pub(crate) fn end(&mut self, reason: &str) -> u64 {
        let seq = self.push(KERNEL, "turn.ended", &format!(r#"{{"reason":"{reason}"}}"#));
        self.turn = None;
        seq
    }

    /// 压缩：摘要替代到第 `upto` 条为止。
    pub(crate) fn compact(&mut self, upto: u64, summary: &str) {
        let body = format!(r#"{{"upto":{upto},"summary":{}}}"#, quoted(summary));
        self.push(KERNEL, "context.compacted", &body);
    }
}

/// 一串消息写成一眼看得懂的样子，一条一行：`user: 块 | 块`、`assistant: 块`、
/// `tool 调用编号 ok: 块`（出错写 `error`）。文本块写它的字，别的块写 `[种类]`，
/// 工具调用写 `[call 工具名]`。
pub(crate) fn shape(messages: &[Message]) -> Vec<String> {
    messages
        .iter()
        .map(|message| match message {
            Message::User { blocks } => format!("user: {}", blocks_shape(blocks)),
            Message::Assistant { blocks } => format!("assistant: {}", blocks_shape(blocks)),
            Message::Tool {
                call_id,
                error,
                blocks,
            } => {
                let status = if *error { "error" } else { "ok" };
                format!("tool {call_id} {status}: {}", blocks_shape(blocks))
            }
        })
        .collect()
}

fn blocks_shape(blocks: &[Block]) -> String {
    let parts: Vec<String> = blocks
        .iter()
        .map(|block| match block {
            Block::Text(text) => text.text.clone(),
            Block::Reasoning(_) => "[reasoning]".to_string(),
            Block::Image(_) => "[image]".to_string(),
            Block::File(_) => "[file]".to_string(),
            Block::ToolCall(call) => format!("[call {}]", call.name),
            Block::Unknown(_) => "[unknown]".to_string(),
        })
        .collect();
    parts.join(" | ")
}
