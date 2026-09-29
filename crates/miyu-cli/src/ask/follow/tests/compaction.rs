//! 压缩那几行（施工 6-3 下）：终端里进度原地刷新、压好了换成结果；不是终端的只印结果；失败的红；英文；`--format json`
//! 不印。

use serde_json::{Value, json};

use super::*;

/// 第 3 轮一开头压：进度两段（发出去时 0 字，再 3120 字），摘要请求说完了（用量 5 + 0 + 3），再照 `after` 收尾。
fn compacted(after: Vec<Value>) -> Vec<Value> {
    let mut turn = a_turn("completed");
    let rest = turn.split_off(1);
    turn.extend([
        event(
            "compaction.progress",
            3,
            "ask-4",
            json!({"seen": 2, "written": 0, "expected": 20000}),
        ),
        event(
            "compaction.progress",
            3,
            "ask-4",
            json!({"seen": 2, "written": 3120, "expected": 20000}),
        ),
    ]);
    turn.extend(after);
    turn.extend(rest);
    turn
}

/// 摘要请求的记录：说完了的，或者出错的。
fn summary_called(result: &str, error: Option<(&str, &str)>) -> Value {
    let mut body = json!({"seen": 2, "endpoint": "deepseek", "messages": 3, "result": result,
        "usage": {"uncached": 5, "cache_read": 0, "cache_write": 0, "output": 3}});
    if let Some((class, message)) = error {
        body["error"] = json!({"class": class, "message": message});
        body["usage"] = Value::Null;
    }
    event("model.called", 3, "ask-4", body)
}

/// 压好了。
fn done() -> Value {
    event(
        "compaction.done",
        3,
        "ask-4",
        json!({"seen": 2, "before": 812_345, "after": 31_020}),
    )
}

#[test]
fn in_a_terminal_the_progress_is_redrawn_in_place_then_replaced() {
    let plan = plan(Format::Text, Language::Chinese);
    let turn = compacted(vec![summary_called("ok", None), done()]);
    let Fed { err, .. } = feed(&plan, true, &turn);
    let gray = |text: &str| format!("\x1b[90m{text}\x1b[0m");
    let expected_start = format!(
        "\r\x1b[2K{}\r\x1b[2K{}\r\x1b[2K{}\n",
        gray("· 正在压缩上下文… 已写 0 字"),
        gray("· 正在压缩上下文… 已写 3,120 字"),
        gray("· 上下文压缩好了：812.3k → 31k token"),
    );
    assert!(err.starts_with(&expected_start), "{err:?}");
}

#[test]
fn in_a_pipe_only_the_result_is_printed() {
    let plan = plan(Format::Text, Language::Chinese);
    let turn = compacted(vec![summary_called("ok", None), done()]);
    let Fed { screen, step, .. } = feed(&plan, false, &turn);
    assert_eq!(step, Step::Done(exit::OK));
    assert_eq!(
        screen,
        "· 上下文压缩好了：812.3k → 31k token\n\n想一想\n\n你好。\n· 输入 105 · 命中缓存 40（38%）· 输出 13\n",
    );
}

#[test]
fn a_failed_summary_is_red_and_says_why() {
    let plan = plan(Format::Text, Language::Chinese);
    for (message, why) in [
        ("no summary in the reply", "取不出摘要"),
        ("the summary reply called a tool", "摘要请求里调了工具"),
    ] {
        let turn = compacted(vec![summary_called(
            "error",
            Some(("bad_summary", message)),
        )]);
        let Fed { err, .. } = feed(&plan, true, &turn);
        let red = format!("\r\x1b[2K\x1b[31m· 压缩失败：{why}\x1b[0m\n");
        assert!(err.contains(&red), "{err:?}");
    }
}

#[test]
fn in_english_too() {
    let plan = plan(Format::Text, Language::English);
    let turn = compacted(vec![summary_called("ok", None), done()]);
    let Fed { screen, .. } = feed(&plan, false, &turn);
    assert!(
        screen.starts_with("· Context compacted: 812.3k → 31k tokens\n"),
        "{screen:?}"
    );
    let turn = compacted(vec![summary_called(
        "error",
        Some(("bad_summary", "no summary in the reply")),
    )]);
    let Fed { screen, .. } = feed(&plan, false, &turn);
    assert!(
        screen.starts_with("· Compaction failed: no summary in the reply\n"),
        "{screen:?}"
    );
}

#[test]
fn json_prints_none_of_it() {
    let plan = plan(Format::Json, Language::Chinese);
    let turn = compacted(vec![summary_called("ok", None), done()]);
    let Fed { err, .. } = feed(&plan, true, &turn);
    assert!(!err.contains("压缩"), "{err:?}");
}

#[test]
fn a_progress_after_a_half_said_answer_starts_on_a_new_line() {
    // 回合中途压：她先说了半句，还没换行就来了进度。照一步的规矩先换行：进度那一行擦的是自己那一行，不能把她说的擦掉。
    let plan = plan(Format::Text, Language::Chinese);
    let turn = [
        event("turn.started", 3, "ask-4", json!({"trigger": 2})),
        delta(json!({"seen": 5, "index": 0, "start": "text"})),
        delta(json!({"seen": 5, "index": 0, "text": "我先读一下。"})),
        event(
            "compaction.progress",
            3,
            "ask-4",
            json!({"seen": 7, "written": 0, "expected": 20000}),
        ),
    ];
    let Fed { screen, .. } = feed(&plan, true, &turn);
    assert!(
        screen.starts_with("我先读一下。\n\r\x1b[2K\x1b[90m· 正在压缩上下文… 已写 0 字"),
        "{screen:?}"
    );
}

/// 暂停了自动压缩（施工 6-6 上）。
fn paused(body: Value) -> Value {
    event("context.compaction_paused", 3, "ask-4", body)
}

#[test]
fn a_pause_after_the_third_failure_is_red_and_says_what_to_do() {
    let plan = plan(Format::Text, Language::Chinese);
    let turn = compacted(vec![
        summary_called("error", Some(("auth", "denied"))),
        paused(json!({"reason": "failures", "failures": 3})),
    ]);
    let Fed { err, .. } = feed(&plan, true, &turn);
    let red = |text: &str| format!("\x1b[31m{text}\x1b[0m\n");
    let failed = format!("\r\x1b[2K{}", red("· 压缩失败：认证失败"));
    let pause = red("· 自动压缩连续失败 3 次，已暂停：可以手动压缩、换一个模型，或者开新会话");
    assert!(err.contains(&format!("{failed}{pause}")), "{err:?}");
}

#[test]
fn each_pause_reason_has_its_line() {
    for (language, body, line) in [
        (
            Language::Chinese,
            json!({"reason": "too_large", "entry": 57}),
            "· 第 57 条内容太大，压完很快又满了，自动压缩已暂停",
        ),
        (
            Language::English,
            json!({"reason": "failures", "failures": 3}),
            "· Automatic compaction failed 3 times and is paused: compact manually, switch models, or start a new session",
        ),
        (
            Language::English,
            json!({"reason": "too_large", "entry": 57}),
            "· Entry 57 is too large and keeps filling the context; automatic compaction is paused",
        ),
        // 不认识的原因、缺了数的：只说暂停了、可以怎么办。
        (
            Language::Chinese,
            json!({"reason": "budget"}),
            "· 自动压缩已暂停：可以手动压缩、换一个模型，或者开新会话",
        ),
        (
            Language::Chinese,
            json!({"reason": "too_large"}),
            "· 自动压缩已暂停：可以手动压缩、换一个模型，或者开新会话",
        ),
        (
            Language::English,
            json!({"reason": "failures"}),
            "· Automatic compaction is paused: compact manually, switch models, or start a new session",
        ),
    ] {
        let plan = plan(Format::Text, language);
        let mut turn = a_turn("completed");
        turn.insert(1, paused(body.clone()));
        let Fed { screen, .. } = feed(&plan, false, &turn);
        assert!(
            screen.starts_with(&format!("{line}\n")),
            "{body} {screen:?}"
        );
    }
}

#[test]
fn a_request_that_would_not_fit_while_paused_ends_the_turn_with_the_class() {
    let plan = plan(Format::Text, Language::Chinese);
    let message =
        "the request would not fit: 126400 tokens used + 20000 reserved for output > window 128000";
    let turn = [
        event("turn.started", 3, "ask-4", json!({"trigger": 2})),
        event(
            "model.called",
            3,
            "ask-4",
            json!({"seen": 3, "messages": 12, "result": "error",
                "error": {"class": "compaction_paused", "message": message}}),
        ),
        event("turn.ended", 3, "ask-4", json!({"reason": "error"})),
    ];
    let Fed { screen, step, .. } = feed(&plan, false, &turn);
    assert_eq!(step, Step::Done(exit::ERROR));
    assert!(
        screen.ends_with(&format!("出错了：自动压缩暂停着：{message}\n")),
        "{screen:?}"
    );
}
