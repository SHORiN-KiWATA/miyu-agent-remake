//! 替身：照剧本跑一段真会话（同 `miyu-store` 的 `log/tests/real.rs`），组装给一份空的，投影只看日志和推送。

use std::collections::BTreeMap;

use miyu_kernel::assemble::Assembler;
use miyu_kernel::block::{Block, Text};
use miyu_kernel::facts::{Environment, FactTemplates};
use miyu_kernel::history::History;
use miyu_kernel::id::Seq;
use miyu_kernel::request::{Message, Request};
use miyu_kernel::session::Policy;
use miyu_kernel::testkit::{Line, Stage};
use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_kernel::tool::{Access, ToolRule, ToolTextSources, ToolTexts};

/// 摘要请求的最后一块：替身照它认出摘要请求（[`summarizes`]）。
const SUMMARIZE: &str = "SUMMARIZE";

/// 从现在起，摘要请求照 `summary` 回。
pub fn summarizes(stage: &mut Stage, summary: &str) {
    stage.summarize_with(SUMMARIZE, Line::says(summary));
}

/// 组装给一份空的：投影不看请求；摘要请求最后接一块 [`SUMMARIZE`]。
struct Nothing;

impl Assembler for Nothing {
    fn assemble(&self, _history: &History) -> Request {
        Request {
            tools: Vec::new(),
            system: String::new(),
            messages: Vec::new(),
            stable: 0,
            continuation: false,
            described: Default::default(),
        }
    }

    fn summarize(&self, history: &History, _: Seq, _: Option<Seq>, _: Option<&str>) -> Request {
        let mut request = self.assemble(history);
        request.messages.push(Message::User {
            blocks: vec![Block::Text(Text {
                text: SUMMARIZE.to_string(),
            })],
        });
        request
    }

    fn summarize_isolated(
        &self,
        history: &History,
        _: Seq,
        _: Option<Seq>,
        _: Option<&str>,
    ) -> Request {
        self.assemble(history)
    }

    fn summary(&self, reply: &[Block]) -> Option<String> {
        reply.iter().find_map(|block| match block {
            Block::Text(text) => Some(text.text.clone()),
            _ => None,
        })
    }
}

/// 几件工具：读、写、编辑、命令、留言、问人。
pub fn policy() -> Policy {
    let rule = |access: Access| ToolRule {
        access,
        parameters: serde_json::from_str(r#"{"type":"object"}"#).expect("参数格式是 JSON"),
    };
    Policy {
        foreground: false,
        assembler: Box::new(Nothing),
        facts: FactTemplates::new(
            r#"<e t="{time}"/>"#,
            r#"<p l="{level}"/>"#,
            "<reply-cut/>",
            None,
            None,
        )
        .expect("事实模板写得对"),
        tools: BTreeMap::from([
            ("read".to_string(), rule(Access::Read)),
            ("write".to_string(), rule(Access::Write)),
            ("edit".to_string(), rule(Access::Write)),
            ("shell".to_string(), rule(Access::Execute)),
            ("send_message".to_string(), rule(Access::Read)),
            ("ask_user".to_string(), rule(Access::Read)),
        ]),
        step_limit: None,
        tool_texts: ToolTexts::new(ToolTextSources {
            unknown: "no tool {name}",
            not_an_object: "bad args {name}",
            cancelled_before: "cancelled before",
            cancelled_running: "cancelled running",
            skipped: "skipped",
            read_only: "read only",
            denied: "denied",
            denied_with_reason: "denied: {reason}",
            unattended: "unattended",
            question_interrupted: "question interrupted",
            question_voided: "question voided",
            question_unattended: "question unattended",
            restarted: "restarted",
        })
        .expect("工具的字写得对"),
        attended: true,
        resumes: 3,
        compaction: Some(miyu_kernel::session::Compaction {
            reserve_cap: 20_000,
            margin: 13_000,
            line_percent: 100,
            margin_percent: 100,
            tail: 0,
            lead: 0,
            price: miyu_kernel::estimate::Flat {
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

fn environment() -> Environment {
    Environment {
        offset: UtcOffset::from_minutes(540).expect("东九区在范围里"),
        cwd: "~/src/miyu".to_string(),
        dirs: Vec::new(),
    }
}

/// 一个替身：照 [`policy`] 造的会话，从 07:00 开始。
pub fn stage() -> Stage {
    policy_stage(policy)
}

/// 同 [`stage`]，策略照 `make` 造。
pub fn policy_stage(make: fn() -> Policy) -> Stage {
    let start = Timestamp::parse("2026-09-25T07:00:00.000Z").expect("时刻合写法");
    Stage::new(make, environment(), start)
}
