//! 真会话来回一趟（施工 1-13 再补从 `tests.rs` 挪出来）：内核的替身跑一段真会话，照三条一批写进日志，关掉再打开，读回来
//! 交给内核载入，过得了账本。

use std::collections::BTreeMap;

use miyu_kernel::assemble::Assembler;
use miyu_kernel::facts::{Environment, FactTemplates};
use miyu_kernel::history::History;
use miyu_kernel::id::SessionId;
use miyu_kernel::request::Request;
use miyu_kernel::session::{Policy, Session};
use miyu_kernel::testkit::{Line, Play, Stage};
use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_kernel::tool::{Access, ToolRule, ToolTextSources, ToolTexts};

use super::*;

/// 替身用的组装：日志这一层不看请求，给一份空的。
struct Nothing;

impl Assembler for Nothing {
    fn assemble(&self, _history: &History) -> Request {
        Request {
            tools: Vec::new(),
            system: String::new(),
            messages: Vec::new(),
            stable: 0,
            continuation: false,
        }
    }

    fn summarize(&self, history: &History, _: Seq, _: Option<Seq>, _: Option<&str>) -> Request {
        self.assemble(history)
    }
    fn summarize_isolated(
        &self,
        history: &History,
        _upto: Seq,
        _cut: Option<Seq>,
        _instructions: Option<&str>,
    ) -> Request {
        self.assemble(history)
    }

    fn summary(&self, _reply: &[miyu_kernel::block::Block]) -> Option<String> {
        None
    }
}

/// 替身用的策略：一件读的工具，句子短，一眼认得出。
fn policy() -> Policy {
    Policy {
        assembler: Box::new(Nothing),
        facts: FactTemplates::new(
            r#"<e t="{time}" d="{cwd}"/>"#,
            r#"<p l="{level}"/>"#,
            "<reply-cut/>",
            None,
            None,
        )
        .unwrap(),
        tools: BTreeMap::from([(
            "read".to_string(),
            ToolRule {
                access: Access::Read,
                parameters: serde_json::from_str(r#"{"type":"object"}"#).unwrap(),
            },
        )]),
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
        .unwrap(),
        attended: true,
        resumes: 3,
        compaction: None,
        notes: None,
        reports: miyu_kernel::session::Reports {
            chars: 30_000,
            omitted: miyu_kernel::template::Template::parse("").expect("空的模板读得进来"),
        },
        titles: None,
    }
}

fn environment() -> Environment {
    Environment {
        offset: UtcOffset::from_minutes(540).unwrap(),
        cwd: "~/src/miyu".to_string(),
        dirs: Vec::new(),
    }
}

#[test]
fn a_real_session_goes_to_disk_and_loads_back() {
    // 替身跑一段真会话：调一次工具，说完。
    let start = Timestamp::parse("2026-09-25T07:00:00.000Z").unwrap();
    let mut stage = Stage::new(policy, environment(), start);
    stage.model([
        Line::calls("我看看。", &[("read", r#"{"path":"a"}"#)]),
        Line::says("好了。"),
    ]);
    stage.tools([Play::done("A")]);
    stage.say("看看 a");
    // 照三条一批写进日志，关掉再打开。
    let scratch = Scratch::new();
    let dir = dir(&scratch);
    let mut log = SessionLog::create(&dir, 300).unwrap();
    for batch in stage.log().chunks(3) {
        log.append(batch).unwrap();
    }
    drop(log);
    let (_, events) = SessionLog::open(&dir, 300).unwrap();
    assert_eq!(events, stage.log());
    assert!(segment_names(&dir).len() > 1, "上限调小了，要换过段");
    // 读回来的交给内核载入：过得了账本。
    let at = Timestamp::parse("2026-09-25T08:00:00.000Z").unwrap();
    let session = SessionId::parse(miyu_kernel::testkit::SESSION).unwrap();
    let (_, actions) = Session::load(session, events, at, policy(), environment()).unwrap();
    assert!(actions.is_empty(), "走完了的会话，载入时什么都不补");
}
