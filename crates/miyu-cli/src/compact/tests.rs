//! `miyu compact` 印什么（施工 6-8，`docs/blueprint/cli/compact.md`）：几个词连成一句要求、不写就没有；跟着单开的那一轮
//! 照 `miyu ask` 的样子印：压好了、用量，和样本逐字节一样；改走隔离式的那一句；失败、被拒绝、被打断的几行和退出码；
//! 英文。

use std::path::Path;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use super::*;
use crate::ask::follow::Step;

/// 几个词、会话。
fn args(words: &[&str], session: Option<&str>) -> Compact {
    Compact {
        words: words.iter().map(ToString::to_string).collect(),
        session: session.map(str::to_string),
    }
}

#[test]
fn words_become_one_instruction_and_none_is_none() {
    assert_eq!(
        args(&["重点保留", "数据库设计"], None)
            .instructions()
            .as_deref(),
        Some("重点保留 数据库设计")
    );
    assert_eq!(args(&[], None).instructions(), None);
    let plan = |instructions: Option<&str>| CompactPlan {
        session: None,
        instructions: instructions.map(str::to_string),
        language: Language::Chinese,
    };
    assert_eq!(
        request("s1", &plan(Some("keep it"))),
        json!({"session": "s1", "instructions": "keep it"})
    );
    assert_eq!(
        request("s1", &plan(None)),
        json!({"session": "s1"}),
        "没附要求的不写这一格"
    );
}

/// 会话 s1 里第 9 轮的一条推送，命令是 compact-1。
fn event(kind: &str, body: Value) -> Value {
    json!({"jsonrpc": "2.0", "method": "event", "params": {"session": "s1", "event": {
        "kind": kind, "turn": 9, "cause": "compact-1", "body": body,
    }}})
}

/// 单开的那一轮：开头、两段进度，然后照 `middle`，最后照 `reason` 结束。
fn a_compaction(middle: Vec<Value>, reason: &str) -> Vec<Value> {
    let mut turn = vec![
        event("turn.started", json!({"cwd": "~"})),
        event(
            "compaction.progress",
            json!({"seen": 8, "written": 0, "expected": 20000}),
        ),
        event(
            "compaction.progress",
            json!({"seen": 8, "written": 3120, "expected": 20000}),
        ),
    ];
    turn.extend(middle);
    turn.push(event("turn.ended", json!({"reason": reason})));
    turn
}

/// 压好了：推送的先后照核心的，瞬时的压好了先到，摘要请求的记录、压缩落了盘才推。
fn compacted() -> Vec<Value> {
    vec![
        event(
            "compaction.done",
            json!({"seen": 8, "trigger": "manual", "before": 812_345, "after": 31_020}),
        ),
        event(
            "model.called",
            json!({"seen": 8, "endpoint": "deepseek", "messages": 3, "result": "ok", "compaction": "manual",
                "usage": {"uncached": 2345, "cache_read": 810_112, "cache_write": 0, "output": 2412}}),
        ),
        event(
            "context.compacted",
            json!({"upto": 8, "summary": "S", "trigger": "manual"}),
        ),
    ]
}

/// 喂完一轮看到的：最后一步、标准输出、标准错误。
struct Fed {
    step: Step,
    out: String,
    err: String,
}

/// 往一块缓冲区里写的笔。
#[derive(Clone, Default)]
struct Pen(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Pen {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("没 panic").extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Pen {
    fn text(&self) -> String {
        String::from_utf8(self.0.lock().expect("没 panic").clone()).expect("UTF-8")
    }
}

/// 一条条喂给跟着第 9 轮的；`live` 是标准错误是不是终端（是的也上色）。
fn feed(language: Language, live: bool, messages: &[Value]) -> Fed {
    let (out, err) = (Pen::default(), Pen::default());
    let (mut to_out, mut to_err) = (out.clone(), err.clone());
    let mut screen = Screen {
        out: &mut to_out,
        err: &mut to_err,
        gray: live,
        live,
    };
    let printing = printing(language);
    let mut follow = Follow::new("s1", "compact-1", &printing);
    let mut step = Step::Going;
    for message in messages {
        step = follow.take(message, &mut screen);
        if step != Step::Going {
            break;
        }
    }
    Fed {
        step,
        out: out.text(),
        err: err.text(),
    }
}

/// 蓝图 `cli/compact.md` 的例子：`docs/designs/samples/cli/compact-text.txt`。
fn sample() -> String {
    let sample = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/designs/samples/cli/compact-text.txt");
    std::fs::read_to_string(&sample).expect("有样本")
}

#[test]
fn a_compaction_prints_the_line_and_the_usage_as_the_sample() {
    let Fed { step, out, err } = feed(
        Language::Chinese,
        false,
        &a_compaction(compacted(), "completed"),
    );
    assert_eq!(step, Step::Done(exit::OK));
    assert_eq!(out, "", "标准输出上什么都不印");
    assert_eq!(err, sample(), "和样本逐字节一样");
}

/// 摘要请求里调了工具、改走隔离式（施工 6-6 下）：照 `miyu ask` 灰色说一句，接着压，压好了退出码 0。
#[test]
fn going_isolated_is_said_and_the_compaction_goes_on() {
    let isolating = event(
        "model.called",
        json!({"seen": 8, "endpoint": "deepseek", "messages": 3, "result": "error", "compaction": "manual",
            "error": {"class": "bad_summary",
                "message": "the summary reply called a tool; trying again without tools"}}),
    );
    let mut middle = vec![isolating];
    middle.extend(compacted());
    let Fed { step, err, .. } = feed(Language::Chinese, false, &a_compaction(middle, "completed"));
    assert_eq!(step, Step::Done(exit::OK));
    assert_eq!(
        err,
        format!("· 摘要请求里调了工具，改用不带工具的再压\n{}", sample())
    );
}

#[test]
fn in_a_terminal_the_progress_is_redrawn_then_replaced() {
    let Fed { err, .. } = feed(
        Language::Chinese,
        true,
        &a_compaction(compacted(), "completed"),
    );
    let gray = |text: &str| format!("\x1b[90m{text}\x1b[0m");
    assert!(
        err.starts_with(&format!(
            "\r\x1b[2K{}\r\x1b[2K{}\r\x1b[2K{}\n",
            gray("· 正在压缩上下文… 已写 0 字"),
            gray("· 正在压缩上下文… 已写 3,120 字"),
            gray("· 上下文压缩好了：812.3k → 31k token"),
        )),
        "{err:?}"
    );
}

#[test]
fn a_failed_compaction_says_why_and_exits_one() {
    let failed = event(
        "model.called",
        json!({"seen": 8, "endpoint": "deepseek", "messages": 3, "result": "error", "compaction": "manual",
            "error": {"class": "bad_summary", "message": "no summary in the reply"}}),
    );
    let Fed { step, err, .. } = feed(
        Language::Chinese,
        false,
        &a_compaction(vec![failed], "error"),
    );
    assert_eq!(step, Step::Done(exit::ERROR));
    assert_eq!(
        err,
        "· 压缩失败：取不出摘要\n出错了：取不出摘要：no summary in the reply\n"
    );
}

#[test]
fn without_a_model_it_exits_five() {
    // 核心没有 key：摘要请求没发出去就认证失败。
    let failed = event(
        "model.called",
        json!({"seen": 8, "messages": 3, "result": "error", "compaction": "manual",
            "error": {"class": "auth", "message": "no key"}}),
    );
    let Fed { step, err, .. } = feed(
        Language::Chinese,
        false,
        &a_compaction(vec![failed], "error"),
    );
    assert_eq!(step, Step::Done(exit::NO_MODEL));
    assert!(
        err.ends_with("没有可用的模型：设环境变量 DEEPSEEK_API_KEY\n"),
        "{err}"
    );
}

#[test]
fn an_interrupted_compaction_exits_three() {
    let cut = event(
        "model.called",
        json!({"seen": 8, "endpoint": "deepseek", "messages": 3, "result": "interrupted", "compaction": "manual"}),
    );
    let Fed { step, err, .. } = feed(
        Language::Chinese,
        true,
        &a_compaction(vec![cut], "interrupted"),
    );
    assert_eq!(step, Step::Done(exit::INTERRUPTED));
    assert!(err.ends_with("\n打断了\n"), "{err:?}");
}

#[test]
fn a_refusal_is_said_as_the_core_says_it() {
    let refused = json!({"jsonrpc": "2.0", "id": "compact-1", "error": {"code": -32010,
        "data": {"reason": "nothing_to_compact"},
        "message": "没有能压的：还没压过的内容都在原样留着的最近一段里。"}});
    let Fed { step, err, .. } = feed(Language::Chinese, false, &[refused]);
    assert_eq!(step, Step::Done(exit::ERROR));
    assert_eq!(
        err,
        "没有能压的：还没压过的内容都在原样留着的最近一段里。\n"
    );
}

#[test]
fn in_english_the_lines_are_english() {
    let Fed { err, .. } = feed(
        Language::English,
        false,
        &a_compaction(compacted(), "completed"),
    );
    assert_eq!(
        err,
        "· Context compacted: 812.3k → 31k tokens\n· input 812,457 · cache hit 810,112 (100%) · output 2,412\n"
    );
}
