use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use miyu_store::human::Human;
use miyu_store::resources::ResourceRoot;

use super::*;
use crate::ask::Target;
use crate::language::Language;

/// 根目录下面的 `parts`：Windows 上得带盘符才算绝对路径。
fn under(parts: &[&str]) -> PathBuf {
    let root = PathBuf::from(if cfg!(windows) { "C:\\" } else { "/" });
    parts.iter().fold(root, |path, part| path.join(part))
}

/// 在 `/work` 里说「hi」；给人看的字照出厂的，家目录是 `/home`。
fn plan(format: Format, language: Language) -> Plan {
    let resources = ResourceRoot::at(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"));
    Plan {
        text: "hi".to_string(),
        target: Target::New,
        format,
        cwd: under(&["work"]).to_string_lossy().into_owned(),
        language,
        human: Human::load(&resources, language.code()).expect("出厂的字读得出来"),
        home: Some(under(&["home"])),
    }
}

/// 会话 s1 里的一条推送。
fn event(kind: &str, turn: u64, cause: &str, body: Value) -> Value {
    json!({"jsonrpc": "2.0", "method": "event", "params": {"session": "s1", "event": {
        "kind": kind, "turn": turn, "cause": cause, "body": body,
    }}})
}

/// 第 3 轮的一段增量。
fn delta(body: Value) -> Value {
    event("model.delta", 3, "ask-4", body)
}

/// 自己发的 ask-4 开的第 3 轮：先想「想一想」，再答「你好。」，用量 60 + 40 + 10。增量的先后照驱动真实的：
/// 两块都等流完了才一起收（`samples/drivers/openai-chat/streams/deepseek-reasoning-tools.txt`）。
fn a_turn(ended: &str) -> Vec<Value> {
    vec![
        event("turn.started", 3, "ask-4", json!({"trigger": 2})),
        delta(json!({"seen": 5, "index": 0, "start": "reasoning"})),
        delta(json!({"seen": 5, "index": 0, "text": "想一想"})),
        delta(json!({"seen": 5, "index": 1, "start": "text"})),
        delta(json!({"seen": 5, "index": 1, "text": "你好"})),
        delta(json!({"seen": 5, "index": 1, "text": "。"})),
        delta(json!({"seen": 5, "index": 0, "end": true})),
        delta(json!({"seen": 5, "index": 1, "end": true})),
        event(
            "message.assistant",
            3,
            "ask-4",
            json!({"seen": 5, "blocks": [{"type": "reasoning", "text": "想一想"}, {"type": "text", "text": "你好。"}]}),
        ),
        event(
            "model.called",
            3,
            "ask-4",
            json!({"seen": 5, "endpoint": "deepseek", "messages": 1, "result": "ok",
                "usage": {"uncached": 60, "cache_read": 40, "cache_write": 0, "output": 10}}),
        ),
        event("turn.ended", 3, "ask-4", json!({"reason": ended})),
    ]
}

/// 一卷屏幕：标准输出、标准错误照先后记在一起，像终端里看到的那样。只看各自的缓冲区看不出两条通道交错的
/// 先后：施工 3-9 下的真机验收里，思考和回答之间的空行跑到了回答后面，分开的缓冲区里一个字节都没错。
#[derive(Clone, Default)]
struct Tape(Arc<Mutex<Vec<(bool, u8)>>>);

/// 往屏幕上写的一支笔：`err` 的写标准错误。
struct Pen {
    tape: Tape,
    err: bool,
}

impl std::io::Write for Pen {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut tape = self.tape.0.lock().expect("没 panic");
        tape.extend(buf.iter().map(|byte| (self.err, *byte)));
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Tape {
    /// 照 `pick` 挑出来的字节，写成字。
    fn text(&self, pick: impl Fn(bool) -> bool) -> String {
        let tape = self.0.lock().expect("没 panic");
        let bytes: Vec<u8> = tape
            .iter()
            .filter(|(err, _)| pick(*err))
            .map(|(_, byte)| *byte)
            .collect();
        String::from_utf8(bytes).expect("UTF-8")
    }
}

/// 喂完一轮看到的：最后一步、标准输出、标准错误、整块屏幕。
struct Fed {
    step: Step,
    out: String,
    err: String,
    screen: String,
}

/// 一条条喂给跟着第 3 轮的。
fn feed(plan: &Plan, gray: bool, messages: &[Value]) -> Fed {
    let tape = Tape::default();
    let (mut out, mut err) = (
        Pen {
            tape: tape.clone(),
            err: false,
        },
        Pen {
            tape: tape.clone(),
            err: true,
        },
    );
    let mut screen = Screen {
        out: &mut out,
        err: &mut err,
        gray,
    };
    let mut follow = Follow::new("s1", "ask-4", plan);
    let mut step = Step::Going;
    for message in messages {
        step = follow.take(message, &mut screen);
        if step != Step::Going {
            break;
        }
    }
    Fed {
        step,
        out: tape.text(|err| !err),
        err: tape.text(|err| err),
        screen: tape.text(|_| true),
    }
}

#[test]
fn thinking_goes_to_stderr_and_the_answer_to_stdout() {
    let plan = plan(Format::Text, Language::Chinese);
    let Fed {
        step,
        out,
        err,
        screen,
    } = feed(&plan, false, &a_turn("completed"));
    assert_eq!(step, Step::Done(exit::OK));
    assert_eq!(
        screen, "想一想\n\n你好。\n· 输入 100 · 命中缓存 40（40%）· 输出 10\n",
        "终端里看到的：思考，空一行，回答，用量"
    );
    assert_eq!(out, "你好。\n", "回答说完补一个换行");
    assert_eq!(
        err, "想一想\n\n· 输入 100 · 命中缓存 40（40%）· 输出 10\n",
        "思考和回答之间空一行"
    );
}

#[test]
fn in_a_terminal_thinking_and_usage_are_gray_piece_by_piece() {
    let plan = plan(Format::Text, Language::English);
    let Fed {
        out, err, screen, ..
    } = feed(&plan, true, &a_turn("completed"));
    assert_eq!(
        screen,
        "\x1b[90m想一想\x1b[0m\n\n你好。\n\x1b[90m· input 100 · cache hit 40 (40%) · output 10\x1b[0m\n"
    );
    assert_eq!(out, "你好。\n", "回答不上色");
    assert_eq!(
        err,
        "\x1b[90m想一想\x1b[0m\n\n\x1b[90m· input 100 · cache hit 40 (40%) · output 10\x1b[0m\n"
    );
}

#[test]
fn other_turns_sessions_and_commands_are_not_followed() {
    let plan = plan(Format::Text, Language::Chinese);
    let mut messages = vec![
        event("turn.started", 9, "ask-9", json!({})),
        event(
            "model.delta",
            9,
            "ask-9",
            json!({"index": 0, "start": "text"}),
        ),
        event(
            "model.delta",
            9,
            "ask-9",
            json!({"index": 0, "text": "别人的"}),
        ),
        event("turn.ended", 9, "ask-9", json!({"reason": "completed"})),
        json!({"jsonrpc": "2.0", "method": "event", "params": {"session": "s2", "event": {
            "kind": "turn.started", "turn": 3, "cause": "ask-4", "body": {}}}}),
        json!({"jsonrpc": "2.0", "id": "ask-5", "result": {}}),
    ];
    messages.extend(a_turn("completed"));
    let Fed { step, out, .. } = feed(&plan, false, &messages);
    assert_eq!(step, Step::Done(exit::OK));
    assert_eq!(out, "你好。\n");
}

#[test]
fn json_prints_one_line_at_the_end_and_nothing_else() {
    let plan = plan(Format::Json, Language::Chinese);
    let Fed { step, out, err, .. } = feed(&plan, true, &a_turn("completed"));
    assert_eq!(step, Step::Done(exit::OK));
    assert_eq!(err, "");
    let printed: Value = serde_json::from_str(out.trim_end()).expect("一行 JSON");
    assert_eq!(
        printed,
        json!({"session": "s1", "turns": [{"text": "你好。", "usage":
            {"input": 100, "cache_read": 40, "cache_write": 0, "output": 10}}]})
    );
    assert!(out.ends_with('\n') && out.matches('\n').count() == 1);
}

/// 第 3 轮出错：`endpoint` 是空的就是没发出去。
fn failing(endpoint: Value, class: &str, message: &str) -> Vec<Value> {
    vec![
        event("turn.started", 3, "ask-4", json!({})),
        event(
            "model.called",
            3,
            "ask-4",
            json!({"seen": 5, "endpoint": endpoint, "messages": 1, "result": "error",
                "error": {"class": class, "message": message}}),
        ),
        event("turn.ended", 3, "ask-4", json!({"reason": "error"})),
    ]
}

#[test]
fn no_model_is_5_and_other_errors_are_1() {
    let plan = plan(Format::Text, Language::Chinese);
    let Fed { step, out, err, .. } = feed(
        &plan,
        false,
        &failing(Value::Null, "auth", "no model: set DEEPSEEK_API_KEY"),
    );
    assert_eq!(step, Step::Done(exit::NO_MODEL));
    assert_eq!(out, "");
    assert_eq!(err, "没有可用的模型：设环境变量 DEEPSEEK_API_KEY\n");
    let Fed { step, err, .. } = feed(
        &plan,
        false,
        &failing(json!("deepseek"), "auth", "HTTP 401: bad key"),
    );
    assert_eq!(step, Step::Done(exit::ERROR));
    assert_eq!(err, "出错了：认证失败：HTTP 401: bad key\n");
    let plan = super::super::Plan {
        format: Format::Json,
        ..plan
    };
    let Fed { out, .. } = feed(
        &plan,
        false,
        &failing(json!("deepseek"), "rate_limited", "HTTP 429"),
    );
    let printed: Value = serde_json::from_str(out.trim_end()).expect("一行 JSON");
    assert_eq!(
        printed["error"],
        json!({"class": "rate_limited", "message": "HTTP 429"})
    );
}

#[test]
fn a_retry_that_worked_is_not_an_error() {
    let plan = plan(Format::Text, Language::Chinese);
    let mut messages = failing(json!("deepseek"), "retryable", "HTTP 500");
    messages.pop();
    messages.extend(a_turn("completed").into_iter().skip(1));
    let Fed { step, out, err, .. } = feed(&plan, false, &messages);
    assert_eq!(step, Step::Done(exit::OK));
    assert_eq!(out, "你好。\n");
    assert!(
        err.ends_with("· 输入 100 · 命中缓存 40（40%）· 输出 10\n"),
        "{err}"
    );
    // 给脚本的那一行也不带出错。
    let plan = super::super::Plan {
        format: Format::Json,
        ..plan
    };
    let Fed { out, .. } = feed(&plan, false, &messages);
    let printed: Value = serde_json::from_str(out.trim_end()).expect("一行 JSON");
    assert!(printed.get("error").is_none(), "{printed}");
}

#[test]
fn how_a_turn_ends_decides_the_exit_code() {
    let plan = plan(Format::Text, Language::Chinese);
    let Fed { step, err, .. } = feed(&plan, false, &a_turn("interrupted"));
    assert_eq!(step, Step::Done(exit::INTERRUPTED));
    assert!(err.ends_with("打断了\n"), "{err}");
    let Fed { step, err, .. } = feed(&plan, false, &a_turn("restarted"));
    assert_eq!(step, Step::Done(exit::ERROR));
    assert!(err.ends_with("这一轮没走完：restarted\n"), "{err}");
}

#[test]
fn thinking_cut_short_still_ends_its_line() {
    let plan = plan(Format::Text, Language::Chinese);
    let messages = [
        event("turn.started", 3, "ask-4", json!({})),
        delta(json!({"seen": 5, "index": 0, "start": "reasoning"})),
        delta(json!({"seen": 5, "index": 0, "text": "想一"})),
        event("turn.ended", 3, "ask-4", json!({"reason": "interrupted"})),
    ];
    let Fed { step, screen, .. } = feed(&plan, false, &messages);
    assert_eq!(step, Step::Done(exit::INTERRUPTED));
    assert_eq!(screen, "想一\n打断了\n", "只想了没答，思考那一行收了尾");
}

#[test]
fn a_refused_message_says_why_and_a_lagging_one_resubscribes() {
    let plan = plan(Format::Text, Language::Chinese);
    let refused = json!({"jsonrpc": "2.0", "id": "ask-4", "error": {"code": -32010, "message": "没有这个会话。"}});
    let Fed { step, err, .. } = feed(&plan, false, &[refused]);
    assert_eq!(step, Step::Done(exit::ERROR));
    assert_eq!(err, "没有这个会话。\n");
    let resync = json!({"jsonrpc": "2.0", "method": "resync", "params": {"session": "s1", "stream": "events"}});
    let Fed { step, .. } = feed(&plan, false, &[resync]);
    assert_eq!(step, Step::Resubscribe);
}

mod asides;
mod sample;
mod unattended;
