//! 她做的每一步、工作目录太宽那一句、沙盒用不了那一句（施工 5-4 下），和思考一样是旁白：一行一行的旁白之间不空行，和回答之间空一行；回答那一行
//! 没完就来了步骤，先换行（施工 4-5 下）。一块前后的空行见 `blocks.rs`（施工 4-11）。

use std::path::{MAIN_SEPARATOR, Path};

use serde_json::{Value, json};

use super::*;

/// 第 3 轮里的一次回复：`blocks` 是它的块。
fn replied(blocks: Value) -> Value {
    event(
        "message.assistant",
        3,
        "ask-4",
        json!({"seen": 5, "blocks": blocks}),
    )
}

/// 调用 `call` 的结果：状态、说法。
fn result(call: &str, status: &str, human: Value) -> Value {
    event(
        "tool.result",
        3,
        "ask-4",
        json!({"call_id": call, "status": status, "blocks": [], "human": human}),
    )
}

/// 读 `src/lib.rs`：写成工作目录里的绝对路径。
fn read_lib(call: &str) -> Value {
    let path = under(&["work", "src", "lib.rs"]);
    json!({"type": "tool_call", "call_id": call, "name": "read",
        "args": json!({"file_path": path.to_string_lossy()}).to_string()})
}

/// 读了 37 行。
fn lines_37() -> Value {
    json!({"key": "software/basesystem/read/lines", "fields": {"count": "37"}})
}

/// 一轮：先想「先读一下」，调 `read`，读了 37 行；再想「好」，答「在这里。」。
fn a_turn_with_a_step() -> Vec<Value> {
    vec![
        event("turn.started", 3, "ask-4", json!({"trigger": 2})),
        delta(json!({"index": 0, "start": "reasoning"})),
        delta(json!({"index": 0, "text": "先读一下"})),
        delta(json!({"index": 1, "start": "tool_call"})),
        replied(json!([{"type": "reasoning", "text": "先读一下"}, read_lib("c1")])),
        result("c1", "ok", lines_37()),
        delta(json!({"index": 0, "start": "reasoning"})),
        delta(json!({"index": 0, "text": "好"})),
        delta(json!({"index": 1, "start": "text"})),
        delta(json!({"index": 1, "text": "在这里。"})),
        replied(json!([{"type": "reasoning", "text": "好"}, {"type": "text", "text": "在这里。"}])),
        event("turn.ended", 3, "ask-4", json!({"reason": "completed"})),
    ]
}

#[test]
fn a_step_is_a_line_among_the_thinking_and_apart_from_the_answer() {
    let plan = plan(Format::Text, Language::Chinese);
    let sep = MAIN_SEPARATOR;
    let Fed {
        step, out, screen, ..
    } = feed(&plan, false, &a_turn_with_a_step());
    assert_eq!(step, Step::Done(exit::OK));
    assert_eq!(
        screen,
        format!("先读一下\n\n→ 读取 src{sep}lib.rs · 37 行\n\n好\n\n在这里。\n"),
        "一段思考前后空一行，和回答之间空一行（施工 4-11 验收时项目主人定，照 opencode）"
    );
    assert_eq!(out, "在这里。\n");
    // 终端里：思考灰，标题原色，结果灰（施工 4-11）。
    let Fed { err, .. } = feed(&plan, true, &a_turn_with_a_step());
    assert_eq!(
        err,
        format!(
            "\x1b[90m先读一下\x1b[0m\n\n→ 读取 src{sep}lib.rs\x1b[90m · 37 行\x1b[0m\n\n\x1b[90m好\x1b[0m\n\n"
        )
    );
}

/// 一轮：先说「我先看看。」，调 `read`；`status` 的结果；再答「看完了。」。
fn a_turn_cut_by_a_step(status: &str, human: Value) -> Vec<Value> {
    vec![
        event("turn.started", 3, "ask-4", json!({"trigger": 2})),
        delta(json!({"index": 0, "start": "text"})),
        delta(json!({"index": 0, "text": "我先看看。"})),
        replied(json!([{"type": "text", "text": "我先看看。"}, read_lib("c1")])),
        result("c1", status, human),
        delta(json!({"index": 0, "start": "text"})),
        delta(json!({"index": 0, "text": "看完了。"})),
        replied(json!([{"type": "text", "text": "看完了。"}])),
        event("turn.ended", 3, "ask-4", json!({"reason": "completed"})),
    ]
}

#[test]
fn an_answer_cut_by_a_step_ends_its_line_first() {
    let plan = plan(Format::Text, Language::Chinese);
    let sep = MAIN_SEPARATOR;
    let missing = json!({"key": "software/basesystem/common/missing"});
    let Fed { out, screen, .. } = feed(
        &plan,
        false,
        &a_turn_cut_by_a_step("error", missing.clone()),
    );
    assert_eq!(
        screen,
        format!("我先看看。\n→ 读取 src{sep}lib.rs · 出错：没有这个文件\n\n看完了。\n"),
        "这一步不接在回答后面"
    );
    assert_eq!(out, "我先看看。\n看完了。\n", "文件里两段回答也隔开了");
    // 给脚本的：不印步骤，两段回复的正文照样隔开。
    let plan = super::super::Plan {
        format: Format::Json,
        ..plan
    };
    let Fed { out, err, .. } = feed(&plan, false, &a_turn_cut_by_a_step("error", missing));
    assert_eq!(err, "");
    let printed: Value = serde_json::from_str(out.trim_end()).expect("一行 JSON");
    assert_eq!(printed["turns"][0]["text"], "我先看看。\n看完了。");
}

#[test]
fn replies_without_a_step_between_are_joined_as_they_are() {
    // 出错重试、带着半截接着说的：两段回复之间没有步骤，正文照原样接上。
    let plan = plan(Format::Json, Language::Chinese);
    let messages = [
        event("turn.started", 3, "ask-4", json!({})),
        replied(json!([{"type": "text", "text": "说到一"}])),
        replied(json!([{"type": "text", "text": "半接着说。"}])),
        event("turn.ended", 3, "ask-4", json!({"reason": "completed"})),
    ];
    let Fed { out, .. } = feed(&plan, false, &messages);
    let printed: Value = serde_json::from_str(out.trim_end()).expect("一行 JSON");
    assert_eq!(printed["turns"][0]["text"], "说到一半接着说。");
}

/// 自己发的 ask-4 的回应：会话实际在 `cwd` 里干活。
fn accepted(cwd: &Path) -> Value {
    json!({"jsonrpc": "2.0", "id": "ask-4", "result": {"events": [5], "cwd": cwd.to_string_lossy()}})
}

#[test]
fn a_directory_too_wide_is_said_once() {
    let sep = MAIN_SEPARATOR;
    let used = under(&["home", ".miyu", "home", "admin", "workspace"]);
    let plan = Plan {
        cwd: under(&["home"]).to_string_lossy().into_owned(),
        ..plan(Format::Text, Language::Chinese)
    };
    // 新会话：造会话的回应说了一次，说话的回应一样的不再说。
    let mut messages = vec![accepted(&used), accepted(&used)];
    messages.extend(a_turn("completed"));
    let Fed { step, screen, .. } = feed_after(&plan, false, None, Some(&used), &messages);
    assert_eq!(step, Step::Done(exit::OK));
    assert_eq!(
        screen,
        format!(
            "· 目录太宽（~），这次在 ~{sep}.miyu{sep}home{sep}admin{sep}workspace 里干活\n\n想一想\n\n你好。\n· 输入 100 · 命中缓存 40（40%）· 输出 10\n"
        ),
        "说一次，在最前面；后面的思考是一段，前后空一行"
    );
    // 实际的就是报的那个：不说。
    let same = under(&["home"]);
    let mut messages = vec![accepted(&same)];
    messages.extend(a_turn("completed"));
    let Fed { screen, .. } = feed(&plan, false, &messages);
    assert!(!screen.contains("目录太宽"), "{screen}");
    // 给脚本的：不说。
    let plan = super::super::Plan {
        format: Format::Json,
        ..plan
    };
    let mut messages = vec![accepted(&used)];
    messages.extend(a_turn("completed"));
    let Fed { err, .. } = feed(&plan, false, &messages);
    assert_eq!(err, "");
}

#[test]
fn paths_are_written_short_against_where_she_really_works() {
    // 退回了账号的工作区：她读那里面的文件，路径照实际干活的目录写相对的，不照敲命令时的目录。
    let used = under(&["home", ".miyu", "home", "admin", "workspace"]);
    let plan = Plan {
        cwd: under(&["home"]).to_string_lossy().into_owned(),
        ..plan(Format::Text, Language::Chinese)
    };
    let todo = used.join("todo.md");
    let messages = [
        accepted(&used),
        event("turn.started", 3, "ask-4", json!({})),
        replied(
            json!([{"type": "tool_call", "call_id": "c1", "name": "read",
            "args": json!({"file_path": todo.to_string_lossy()}).to_string()}]),
        ),
        result("c1", "ok", lines_37()),
        event("turn.ended", 3, "ask-4", json!({"reason": "completed"})),
    ];
    let Fed { err, .. } = feed(&plan, false, &messages);
    assert!(err.ends_with("\n→ 读取 todo.md · 37 行\n"), "{err}");
}

#[test]
fn an_unusable_sandbox_is_said_first_and_right_before_the_directory() {
    let sep = MAIN_SEPARATOR;
    let used = under(&["home", ".miyu", "home", "admin", "workspace"]);
    let plan = Plan {
        cwd: under(&["home"]).to_string_lossy().into_owned(),
        ..plan(Format::Text, Language::Chinese)
    };
    let mut messages = vec![accepted(&used)];
    messages.extend(a_turn("completed"));
    let Fed { step, screen, .. } =
        feed_after(&plan, false, Some("helper_missing"), Some(&used), &messages);
    assert_eq!(step, Step::Done(exit::OK));
    assert_eq!(
        screen,
        format!(
            "· 沙盒用不了（主程序旁边没有 miyu-sandbox：重装一次 Miyu）：执行命令要你确认，miyu ask 里确认不了\n· 目录太宽（~），这次在 ~{sep}.miyu{sep}home{sep}admin{sep}workspace 里干活\n\n想一想\n\n你好。\n· 输入 100 · 命中缓存 40（40%）· 输出 10\n"
        ),
        "最先说，和目录太宽那一句连着、不空行"
    );
    // 给脚本的：不说。
    let plan = super::super::Plan {
        format: Format::Json,
        ..plan
    };
    let Fed { err, .. } = feed_after(&plan, false, Some("helper_missing"), None, &messages);
    assert_eq!(err, "");
}

#[test]
fn a_session_working_elsewhere_is_said_once() {
    // 接着说的会话（施工 9-7 上）：工作区是它自己的，说话的回应里和敲命令时的目录不一样，说一句它在哪，只说一次。
    let sep = MAIN_SEPARATOR;
    let used = under(&["home", "src", "proj"]);
    let plan = Plan {
        cwd: under(&["home"]).to_string_lossy().into_owned(),
        ..plan(Format::Text, Language::Chinese)
    };
    let mut messages = vec![accepted(&used), accepted(&used)];
    messages.extend(a_turn("completed"));
    let Fed { screen, .. } = feed(&plan, false, &messages);
    assert!(
        screen.starts_with(&format!("· 这个会话在 ~{sep}src{sep}proj 里干活\n\n想一想")),
        "{screen}"
    );
    assert_eq!(screen.matches("这个会话在").count(), 1, "{screen}");
}
