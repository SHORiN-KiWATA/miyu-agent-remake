//! 蓝图 `cli/ask.md` 的样本（施工 4-9 三补）：照样本的场景喂一轮，标准输出、标准错误照先后交错，和
//! `docs/designs/samples/cli/ask-text.txt` 逐字节一样；蓝图里的那一块，门禁和同一份文件比。

use std::path::{MAIN_SEPARATOR, Path};

use serde_json::{Value, json};

use super::*;

/// 第 3 轮里的一次回复。
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

/// 读 `path` 的一次调用。
fn read(call: &str, path: &Path) -> Value {
    json!({"type": "tool_call", "call_id": call, "name": "read",
        "args": json!({"file_path": path.to_string_lossy()}).to_string()})
}

#[test]
fn the_screen_is_the_sample_of_the_drawing() {
    // 在家目录里敲的命令：太宽，核心退回了账号的工作区。
    let used = under(&["home", ".miyu", "home", "admin", "workspace"]);
    let plan = Plan {
        cwd: under(&["home"]).to_string_lossy().into_owned(),
        ..plan(Format::Text, Language::Chinese)
    };
    let accepted = json!({"jsonrpc": "2.0", "id": "ask-4", "result": {"events": [5], "cwd": used.to_string_lossy()}});
    let messages = vec![
        accepted,
        event("turn.started", 3, "ask-4", json!({"trigger": 2})),
        delta(json!({"index": 0, "start": "reasoning"})),
        delta(json!({"index": 0, "text": "先读一下笔记。"})),
        replied(json!([
            {"type": "reasoning", "text": "先读一下笔记。"},
            read("c1", &used.join("notes.md")),
            read("c2", &used.join("missing.md")),
            read("c3", &under(&["home", ".gitconfig"])),
        ])),
        result(
            "c1",
            "ok",
            json!({"key": "software/basesystem/read/lines", "fields": {"count": "3"}}),
        ),
        result(
            "c2",
            "error",
            json!({"key": "software/basesystem/common/missing"}),
        ),
        result(
            "c3",
            "denied",
            json!({"key": "core/tool-results/unattended"}),
        ),
        delta(json!({"index": 0, "start": "text"})),
        delta(json!({"index": 0, "text": "读完了。"})),
        replied(json!([{"type": "text", "text": "读完了。"}])),
        event(
            "model.called",
            3,
            "ask-4",
            json!({"seen": 9, "endpoint": "deepseek", "messages": 3, "result": "ok",
                "usage": {"uncached": 240, "cache_read": 160, "cache_write": 0, "output": 40}}),
        ),
        event("turn.ended", 3, "ask-4", json!({"reason": "completed"})),
    ];
    let Fed { step, screen, .. } = feed(&plan, false, &messages);
    assert_eq!(step, Step::Done(exit::UNATTENDED));
    let sample =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/designs/samples/cli/ask-text.txt");
    let drawn = std::fs::read_to_string(&sample).expect("有样本");
    // 样本照 Unix 的路径写。
    assert_eq!(screen.replace(MAIN_SEPARATOR, "/"), drawn);
}
