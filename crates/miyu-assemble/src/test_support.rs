//! 测试共用的零件：照先后拼一段合规的日志；一套替身的固定字；把消息写成一眼看得懂的样子。

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::Event;
use miyu_kernel::history::History;
use miyu_kernel::id::ContentHash;
use miyu_kernel::ledger::Ledger;
use miyu_kernel::request::Message;

use miyu_kernel::template::Template;

use crate::texts::{
    GroupChat, HarnessTexts, IdleTexts, JobTexts, PeerTexts, Recap, RestoredWrap, Texts, Title,
    TurnEndedTexts, Vision,
};

pub(crate) const KERNEL: &str = r#"{"kind":"kernel"}"#;
const ALICE: &str = r#"{"kind":"person","account":"alice"}"#;
pub(crate) const MODEL: &str = r#"{"kind":"model","endpoint":"deepseek","model":"deepseek-v4"}"#;
const CREATED: &str = r#"{"owner":"alice","venue":"local","policy":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","permission":{"level":"workspace","read_only":false}}"#;

/// 替身的固定字：短，一眼认得出是哪一句。测的是拼法，和出厂的措辞无关。
pub(crate) fn texts() -> Texts {
    Texts {
        checkpoint_open: "<checkpoint>\n".to_string(),
        checkpoint_close: "\n</checkpoint>\n".to_string(),
        checkpoint_end: String::new(),
        restored: Some(RestoredWrap {
            open: Template::parse("<file {path}>\n").expect("模板合写法"),
            close: "\n</file>\n".to_string(),
        }),
        turn_ended: TurnEndedTexts {
            interrupted: "<interrupted/>".to_string(),
            error: "<error/>".to_string(),
            step_limit: "<step-limit/>".to_string(),
            aborted: "<aborted/>".to_string(),
            restarted: "<restarted/>".to_string(),
        },
        summarize_task: "<summarize/>".to_string(),
        truncated: "<truncated/>".to_string(),
        summarize_system: "<isolated/>".to_string(),
        summarize_instructions: "<instructions>".to_string(),
        summarize_end: "<end/>".to_string(),
        jobs: Some(job_texts()),
        harness: Some(HarnessTexts {
            open: Template::parse("<agent {name}>\n").expect("模板合写法"),
            close: "</agent>\n".to_string(),
        }),
        peers: Some(PeerTexts {
            open: Template::parse("<peer {id}>\n").expect("模板合写法"),
            close: "</peer>\n".to_string(),
            idle: Some(IdleTexts {
                open: Template::parse("<idle {id} {reason}>\n").expect("模板合写法"),
                silent: "<silent/>\n".to_string(),
                expired: "<expired/>\n".to_string(),
                gone: "<gone/>\n".to_string(),
                close: "</idle>\n".to_string(),
            }),
        }),
        recap: Some(recap_texts()),
        title: Some(title_texts()),
        vision: Some(vision_texts()),
        group: None,
    }
}

/// 群会话的替身字（施工 O-13 中）：时区是 `minutes` 分钟，空的那一条写 `<no-text>`。
pub(crate) fn group_texts(minutes: i32) -> Texts {
    Texts {
        group: Some(GroupChat {
            offset: miyu_kernel::time::UtcOffset::from_minutes(minutes).expect("在范围里"),
            no_text: "<no-text>".to_string(),
        }),
        ..texts()
    }
}

/// 替身的转述一张图的字（施工 8-17）：短，一眼认得出。
pub(crate) fn vision_texts() -> Vision {
    Vision {
        instruction: "<describe>\n".to_string(),
        question: "\n<asked>\n".to_string(),
    }
}

/// 替身的起标题的字（施工 3-8 五补）：短，一眼认得出；数照出厂的。
pub(crate) fn title_texts() -> Title {
    Title {
        instruction: "<title>\n".to_string(),
        tokens: 1_024,
    }
}

/// 替身的回顾的字（施工 3-8 四补）：短，一眼认得出；数照出厂的。
pub(crate) fn recap_texts() -> Recap {
    Recap {
        instruction: "<recap>\n".to_string(),
        user: "U: ".to_string(),
        assistant: "A: ".to_string(),
        omitted: "[omitted]\n\n".to_string(),
        excerpted: "\n[...]\n".to_string(),
        turns: 8,
        tokens: 8_192,
    }
}

/// 替身的回报写法（施工 7-2）：一行一样，一眼认得出是哪一句。
fn job_texts() -> JobTexts {
    let template = |text: &str| Template::parse(text).expect("模板合写法");
    JobTexts {
        command_open: template("<command {job} {title} {reason}>\n"),
        command_exit: template("exit {code}\n"),
        command_signal: template("signal {signal}\n"),
        command_duration: template("ms {ms}\n"),
        command_output: template("chars {chars}\n"),
        command_close: "</command>\n".to_string(),
        subagent_open: template("<subagent {job} {title} {reason}>\n"),
        subagent_person: "person\n".to_string(),
        subagent_truncated: "truncated\n".to_string(),
        subagent_silent: "silent\n".to_string(),
        subagent_close: "</subagent>\n".to_string(),
        stopped_by_user: "by user\n".to_string(),
        subagent_message_open: template("<message {job} {title}>\n"),
        subagent_message_close: "</message>\n".to_string(),
    }
}

/// 一个文本块。
pub(crate) fn text(text: &str) -> Block {
    Block::Text(Text {
        text: text.to_string(),
    })
}

/// 一段字写成 JSON 字符串。
pub(crate) fn quoted(text: &str) -> String {
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

    /// 会话 `parent` 派出来的一个子会话，刚创建（施工 C-2：父会话的话不是别的会话的）。
    pub(crate) fn child(parent: &str) -> Log {
        let mut log = Log {
            ledger: Ledger::default(),
            history: History::default(),
            turn: None,
        };
        let open = CREATED.strip_suffix('}').unwrap_or(CREATED);
        let created = format!(r#"{open},"parent":"{parent}","depth":1}}"#);
        log.push(KERNEL, "session.created", &created);
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

    /// 追加一条不带回合编号的：任务的回报（施工 7-2），回合进行中到的也不带。返回它的序号。
    pub(crate) fn detached(&mut self, by: &str, kind: &str, body: &str) -> u64 {
        let turn = self.turn.take();
        let seq = self.push(by, kind, body);
        self.turn = turn;
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

    /// 开一个没有触发的回合：手动压缩单开的那一轮（施工 6-8）。
    pub(crate) fn start_untriggered(&mut self) {
        self.turn = Some(self.next());
        self.push(KERNEL, "turn.started", "{}");
    }

    /// 注入一块事实。
    pub(crate) fn fact(&mut self, fact: &str) {
        let body = format!(r#"{{"kind":"env","text":{}}}"#, quoted(fact));
        self.push(KERNEL, "context.injected", &body);
    }

    /// `by` 注入一块 `kind` 类的事实（施工 P-1 补：角色扮演提示排在哪，看来源和类别）。
    pub(crate) fn fact_by(&mut self, by: &str, kind: &str, fact: &str) {
        let body = format!(r#"{{"kind":"{kind}","text":{}}}"#, quoted(fact));
        self.push(by, "context.injected", &body);
    }

    /// 记一次没等到回复就出了可以重试的错的请求：它看到了上一条为止。
    pub(crate) fn failed(&mut self) {
        let seen = self.next() - 1;
        let body = format!(
            r#"{{"seen":{seen},"messages":1,"result":"error","error":{{"class":"retryable","message":"503"}}}}"#
        );
        self.push(KERNEL, "model.called", &body);
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

    /// 压缩：摘要替代到第 `upto` 条为止。压缩带着它所在的回合（施工 6-9）：在 [`Log::start`] 开的回合里压。
    pub(crate) fn compact(&mut self, upto: u64, summary: &str) {
        let body = format!(r#"{{"upto":{upto},"summary":{}}}"#, quoted(summary));
        self.push(KERNEL, "context.compacted", &body);
    }

    /// 同 [`Log::compact`]，带着代码写的几段 `notes`、重读的文件 `restored`（路径和原文，施工 6-5）。`recalled` 的原文交给
    /// 有效历史，没交的那一份当 blob 读不出来。
    pub(crate) fn compact_rebuilt(
        &mut self,
        upto: u64,
        summary: &str,
        notes: &str,
        restored: &[(&str, &str)],
        recalled: bool,
    ) {
        let files: Vec<String> = restored
            .iter()
            .map(|(path, text)| {
                format!(
                    r#"{{"path":{},"blob":"{}","tokens":1}}"#,
                    quoted(path),
                    ContentHash::of(text.as_bytes())
                )
            })
            .collect();
        let body = format!(
            r#"{{"upto":{upto},"summary":{},"notes":{},"restored":[{}]}}"#,
            quoted(summary),
            quoted(notes),
            files.join(",")
        );
        self.push(KERNEL, "context.compacted", &body);
        if recalled {
            self.history.recall(
                restored
                    .iter()
                    .map(|(_, text)| (ContentHash::of(text.as_bytes()), text.to_string()))
                    .collect(),
            );
        }
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
