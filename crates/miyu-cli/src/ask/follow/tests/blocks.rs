//! 执行命令、编辑标题下面有东西的是一块，前后各空一行（施工 4-11）：前面已经是空行、什么都还没印的不再空；后面欠的
//! 空行等下一样东西来了再写，两块挨着只空一行，一块后面接着回答的也只空一行。

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

/// 一次调用：编号 `call`，工具 `name`，参数 `args`。
fn call(call: &str, name: &str, args: Value) -> Value {
    json!({"type": "tool_call", "call_id": call, "name": name, "args": args.to_string()})
}

/// 调用 `call` 的结果，工具自己写的：状态、这几行字、说法。
fn result(call: &str, status: &str, text: &str, human: Value) -> Value {
    let mut result = event(
        "tool.result",
        3,
        "ask-4",
        json!({"call_id": call, "status": status,
            "blocks": [{"type": "text", "text": text}], "human": human}),
    );
    result["params"]["event"]["by"] = json!({"kind": "tool", "call_id": call});
    result
}

/// 执行 `command`，输出 `text`。
fn shell(id: &str, command: &str) -> Value {
    call(id, "shell", json!({ "command": command }))
}

/// 编辑 `notes.md`：`x` 改成 `y`。
fn edit(id: &str) -> Value {
    call(
        id,
        "edit",
        json!({"file_path": "notes.md", "edits": [{"old_string": "x", "new_string": "y"}]}),
    )
}

/// 改了 1 处。
fn edited() -> Value {
    json!({"key": "software/basesystem/edit/edited", "fields": {"count": "1"}})
}

/// 读了 3 行。
fn read_3() -> Value {
    json!({"key": "software/basesystem/read/lines", "fields": {"count": "3"}})
}

/// 一轮：`before` 里的推送，之后答 `answer`（空的不答），照常结束。
fn turn(before: Vec<Value>, answer: &str) -> Vec<Value> {
    let mut messages = vec![event("turn.started", 3, "ask-4", json!({"trigger": 2}))];
    messages.extend(before);
    if !answer.is_empty() {
        messages.push(delta(json!({"index": 0, "start": "text"})));
        messages.push(delta(json!({"index": 0, "text": answer})));
        messages.push(replied(json!([{"type": "text", "text": answer}])));
    }
    messages.push(event(
        "turn.ended",
        3,
        "ask-4",
        json!({"reason": "completed"}),
    ));
    messages
}

#[test]
fn a_block_has_a_blank_line_before_and_after() {
    let plan = plan(Format::Text, Language::Chinese);
    let messages = turn(
        vec![
            delta(json!({"index": 0, "start": "reasoning"})),
            delta(json!({"index": 0, "text": "先看看"})),
            replied(json!([
                shell("c1", "ls"),
                call("c2", "read", json!({"file_path": "notes.md"})),
                edit("c3"),
            ])),
            result("c1", "ok", "a\nb\n", Value::Null),
            result("c2", "ok", "…", read_3()),
            result("c3", "ok", "", edited()),
        ],
        "好了。",
    );
    let Fed {
        step, screen, out, ..
    } = feed(&plan, false, &messages);
    assert_eq!(step, Step::Done(exit::OK));
    assert_eq!(
        screen,
        "先看看\n\n$ ls\na\nb\n\n→ 读取 notes.md · 3 行\n\n← 编辑 notes.md · 改了 1 处\n-x\n+y\n\n好了。\n",
        "一块前后各空一行；一块后面接着回答的，也只空一行"
    );
    assert_eq!(out, "好了。\n");
}

#[test]
fn two_blocks_in_a_row_have_one_blank_line_and_the_first_has_none_before() {
    let plan = plan(Format::Text, Language::Chinese);
    let messages = turn(
        vec![
            replied(json!([shell("c1", "ls"), shell("c2", "pwd")])),
            result("c1", "ok", "a\n", Value::Null),
            result("c2", "ok", "/work\n", Value::Null),
            delta(json!({"index": 0, "start": "reasoning"})),
            delta(json!({"index": 0, "text": "看完了"})),
        ],
        "",
    );
    let Fed { screen, .. } = feed(&plan, false, &messages);
    assert_eq!(
        screen, "$ ls\na\n\n$ pwd\n/work\n\n看完了\n",
        "什么都还没印的，前面不空；两块挨着只空一行；后面的思考前面空一行"
    );
}

#[test]
fn a_block_after_the_answer_and_before_the_usage_line() {
    // 她先说了半句再执行：回答那一行先收尾，再空一行；最后一块后面还有用量那一行的，中间空一行。
    let plan = plan(Format::Text, Language::Chinese);
    let messages = vec![
        event("turn.started", 3, "ask-4", json!({"trigger": 2})),
        delta(json!({"index": 0, "start": "text"})),
        delta(json!({"index": 0, "text": "我先跑一下"})),
        replied(json!([{"type": "text", "text": "我先跑一下"}, shell("c1", "ls")])),
        result("c1", "ok", "a\n", Value::Null),
        event(
            "model.called",
            3,
            "ask-4",
            json!({"seen": 5, "endpoint": "deepseek", "messages": 1, "result": "ok",
                "usage": {"uncached": 60, "cache_read": 40, "cache_write": 0, "output": 10}}),
        ),
        event("turn.ended", 3, "ask-4", json!({"reason": "completed"})),
    ];
    let Fed { screen, out, .. } = feed(&plan, false, &messages);
    assert_eq!(
        screen,
        "我先跑一下\n\n$ ls\na\n\n· 输入 100 · 命中缓存 40（40%）· 输出 10\n"
    );
    assert_eq!(out, "我先跑一下\n");
}

#[test]
fn a_step_with_nothing_under_it_is_just_a_line() {
    // 编辑没改成：下面不印，照一行印；前面的空行是思考那一段的。
    let plan = plan(Format::Text, Language::Chinese);
    let missing = json!({"key": "software/basesystem/edit/not-found", "fields": {"index": "1"}});
    let messages = turn(
        vec![
            delta(json!({"index": 0, "start": "reasoning"})),
            delta(json!({"index": 0, "text": "改一下"})),
            replied(json!([edit("c1")])),
            result("c1", "error", "Edit 1 was not found.", missing),
        ],
        "没找到。",
    );
    let Fed { screen, .. } = feed(&plan, false, &messages);
    assert_eq!(
        screen,
        "改一下\n\n← 编辑 notes.md · 出错：第 1 处没找到\n\n没找到。\n"
    );
}
