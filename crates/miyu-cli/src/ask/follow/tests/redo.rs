//! 跟着重做开的那一轮（施工 4-7 再补，`docs/blueprint/cli/redo.md`）：回应到了先照 `miyu undo` 印撤掉了哪一轮，第一行接
//! 「，重新做」，不说怎么恢复；会话在哪个目录里干活照回应的换上，不说目录太宽；被拒绝的照核心的话说。照样本的场景喂一轮，
//! 屏幕和 `docs/designs/samples/cli/redo-text.txt` 逐字节一样；蓝图里的那一块，门禁和同一份文件比。

use std::path::{MAIN_SEPARATOR, Path};

use serde_json::{Value, json};

use super::*;

/// 一条条喂给跟着第 3 轮的，跟的是重做开的那一轮。
fn feed_redo(plan: &Plan, gray: bool, messages: &[Value]) -> Fed {
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
        live: gray,
    };
    let mut follow = Follow::new("s1", "ask-4", plan);
    follow.redoing();
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

/// 重做的回应：`result` 照这些格。
fn replied(result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": "ask-4", "result": result})
}

/// 重做开的第 3 轮的开头：由重发的第 10 条开。
fn opened() -> Value {
    event("turn.started", 3, "ask-4", json!({"trigger": 10}))
}

/// 印重做那一轮时照的：会话在哪干活先空着，等回应（`redo.rs` 的 `printing`）。
fn printing(language: Language) -> Plan {
    Plan {
        cwd: String::new(),
        ..plan(Format::Text, language)
    }
}

#[test]
fn the_screen_is_the_sample_of_the_drawing() {
    let work = under(&["work"]);
    let messages = vec![
        opened(),
        replied(json!({
            "events": [9, 10], "cwd": work.to_string_lossy(), "turns": 1,
            "said": "把 README 的标题改成中文", "commands": 1,
            "files": [{"path": work.join("README.md").to_string_lossy(), "action": "write", "outcome": "restored"}],
        })),
        event(
            "message.assistant",
            3,
            "ask-4",
            json!({"seen": 12, "blocks": [{"type": "tool_call", "call_id": "c1", "name": "read",
                "args": json!({"file_path": work.join("README.md").to_string_lossy()}).to_string()}]}),
        ),
        event(
            "tool.result",
            3,
            "ask-4",
            json!({"call_id": "c1", "status": "ok", "blocks": [],
                "human": {"key": "software/basesystem/read/lines", "fields": {"count": "3"}}}),
        ),
        delta(json!({"index": 0, "start": "text"})),
        delta(json!({"index": 0, "text": "改好了。"})),
        event(
            "message.assistant",
            3,
            "ask-4",
            json!({"seen": 15, "blocks": [{"type": "text", "text": "改好了。"}]}),
        ),
        event(
            "model.called",
            3,
            "ask-4",
            json!({"seen": 15, "endpoint": "deepseek", "messages": 3, "result": "ok",
                "usage": {"uncached": 240, "cache_read": 160, "cache_write": 0, "output": 40}}),
        ),
        event("turn.ended", 3, "ask-4", json!({"reason": "completed"})),
    ];
    let Fed { step, screen, .. } = feed_redo(&printing(Language::Chinese), false, &messages);
    assert_eq!(step, Step::Done(exit::OK));
    let sample =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/designs/samples/cli/redo-text.txt");
    let drawn = std::fs::read_to_string(&sample).expect("有样本");
    // 样本照 Unix 的路径写。
    assert_eq!(screen.replace(MAIN_SEPARATOR, "/"), drawn);
}

#[test]
fn the_undo_lines_are_asides_without_the_restore_hint() {
    // 撤掉了压缩、没有人说的话（那一句只有附件），差异照 `miyu undo`；不说怎么恢复；标准输出上什么都没有。
    let work = under(&["work"]);
    let messages = vec![
        replied(json!({
            "events": [9, 10, 11], "cwd": work.to_string_lossy(), "turns": 1, "compactions": 1,
            "files": [{"path": work.join("a.rs").to_string_lossy(), "action": "write", "outcome": "changed",
                "diff": ["@@ -1 +1 @@", "-a", "+b"]}],
        })),
        opened(),
    ];
    let Fed { step, out, err, .. } = feed_redo(&printing(Language::Chinese), false, &messages);
    assert_eq!(step, Step::Going);
    assert_eq!(out, "");
    assert_eq!(
        err,
        "· 撤销最后一轮，重新做\n· 撤掉了压缩，上下文回到了压缩前\n· 改回 a.rs → 没动：之后又被改过\n    --- 她改完的\n    +++ 现在\n    @@ -1 +1 @@\n    -a\n    +b\n"
    );
}

#[test]
fn in_english_and_in_color() {
    let messages = vec![replied(
        json!({"events": [9, 10], "cwd": "", "turns": 1, "said": "hi"}),
    )];
    let Fed { err, .. } = feed_redo(&printing(Language::English), false, &messages);
    assert_eq!(err, "· Undid the turn \u{201c}hi\u{201d}, redoing it\n");
    let Fed { err, .. } = feed_redo(&printing(Language::English), true, &messages);
    assert_eq!(
        err,
        "\u{1b}[90m· Undid the turn \u{201c}hi\u{201d}, redoing it\u{1b}[0m\n"
    );
}

#[test]
fn the_reply_cwd_is_taken_without_saying_it_is_too_wide() {
    // 回应的目录和头这边的（空的）不一样：不说目录太宽，之后每一步照它写短。
    let work = under(&["elsewhere"]);
    let messages = vec![
        opened(),
        replied(
            json!({"events": [9, 10], "cwd": work.to_string_lossy(), "turns": 1, "said": "hi"}),
        ),
        event(
            "message.assistant",
            3,
            "ask-4",
            json!({"seen": 12, "blocks": [{"type": "tool_call", "call_id": "c1", "name": "read",
                "args": json!({"file_path": work.join("a.md").to_string_lossy()}).to_string()}]}),
        ),
        event(
            "tool.result",
            3,
            "ask-4",
            json!({"call_id": "c1", "status": "ok", "blocks": [],
                "human": {"key": "software/basesystem/read/lines", "fields": {"count": "1"}}}),
        ),
    ];
    let Fed { err, .. } = feed_redo(&printing(Language::Chinese), false, &messages);
    assert_eq!(err, "· 撤销「hi」这一轮，重新做\n→ 读取 a.md · 1 行\n");
}

#[test]
fn a_refused_redo_says_what_the_core_said() {
    let refused = json!({"jsonrpc": "2.0", "id": "ask-4",
        "error": {"code": -32010, "message": "无法重做。", "data": {"reason": "not_redoable"}}});
    let Fed { step, err, .. } = feed_redo(&printing(Language::Chinese), false, &[refused]);
    assert_eq!(step, Step::Done(exit::ERROR));
    assert!(err.contains("无法重做。"), "{err}");
}

#[test]
fn what_came_before_the_reply_is_printed_after_the_undo_lines() {
    // 核心写回应要读日志：回应到之前新的一轮已经说完了。先印撤掉了哪一轮，再照先后印攒着的，收尾照常。
    let mut messages = a_turn("completed");
    messages.push(replied(
        json!({"events": [9, 10], "cwd": "", "turns": 1, "said": "hi"}),
    ));
    let Fed { step, screen, .. } = feed_redo(&printing(Language::Chinese), false, &messages);
    assert_eq!(step, Step::Done(exit::OK));
    assert_eq!(
        screen,
        "· 撤销「hi」这一轮，重新做\n\n想一想\n\n你好。\n· 输入 100 · 命中缓存 40（40%）· 输出 10\n"
    );
}
