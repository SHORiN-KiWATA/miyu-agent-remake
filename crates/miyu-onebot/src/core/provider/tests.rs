//! 桥当提供者（施工 O-26，`onebot.md` 第一条「提供者和不说话」第 1、2 条）：`provide` 的参数照出厂的说明；核心说的一行分成
//! 哪一种、请求怎么答。

use std::path::Path;
use std::sync::Arc;

use serde_json::{Value, json};

use miyu_store::resources::ResourceRoot;

use super::{Heard, heard};
use crate::core::methods::Methods;
use crate::rules::{Factory, Tools};

/// 源码树里的资源目录。
fn resources() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources")
}

/// 出厂的工具。
fn tools() -> Arc<Tools> {
    Factory::load(&ResourceRoot::at(resources()))
        .expect("出厂的读得出")
        .tools()
}

/// 出厂的 `software/onebot/<name>` 原样。
fn shipped(name: &str) -> String {
    std::fs::read_to_string(resources().join("software/onebot").join(name)).expect("读得出")
}

/// 核心反向调 `tool`，编号 `id`。
fn tool_call(id: &str, tool: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": "tool.call", "params": {
        "session": "s1", "call_id": "c1", "tool": tool, "args": {"reason": "x"}, "cwd": "/",
    }})
}

/// 是一条要答的请求：交回回应。
fn asked(heard: Heard) -> Value {
    match heard {
        Heard::Asked(reply) => reply,
        other => panic!("该答：{other:?}"),
    }
}

#[test]
fn the_provide_params_follow_the_shipped_spec() {
    let spec: Value = serde_json::from_str(&shipped("tools/skip_reply.json")).expect("是 JSON");
    assert_eq!(
        tools().provided(),
        json!({"tools": [{
            "name": "skip_reply",
            "description": spec["description"],
            "input_schema": spec["parameters"],
            "access": "read",
            "venues": ["private", "group"],
        }]})
    );
}

#[test]
fn a_tool_call_is_answered_under_its_own_id() {
    assert_eq!(
        asked(heard(
            &tools(),
            &Methods::new(),
            tool_call("core-7", "skip_reply")
        )),
        json!({"jsonrpc": "2.0", "id": "core-7", "result": {
            "blocks": [{"type": "text", "text": shipped("tool-results/skipped.txt")}],
            "error": false,
        }})
    );
    let unknown = shipped("tool-results/unknown.txt").replace("{name}", "recall");
    assert_eq!(
        asked(heard(
            &tools(),
            &Methods::new(),
            tool_call("core-8", "recall")
        )),
        json!({"jsonrpc": "2.0", "id": "core-8", "result": {
            "blocks": [{"type": "text", "text": unknown}],
            "error": true,
        }})
    );
}

#[test]
fn other_methods_are_unknown() {
    let reply = asked(heard(
        &tools(),
        &Methods::new(),
        json!({"jsonrpc": "2.0", "id": 9, "method": "tool.peek", "params": {}}),
    ));
    assert_eq!(
        reply,
        json!({"jsonrpc": "2.0", "id": 9, "error": {
            "code": -32601, "message": "unknown_method", "data": {"reason": "unknown_method"},
        }})
    );
}

#[test]
fn pushes_and_answers_pass_on_and_cancels_stop_here() {
    for message in [
        json!({"jsonrpc": "2.0", "method": "event", "params": {"session": "s1", "event": {}}}),
        json!({"jsonrpc": "2.0", "method": "extension.config", "params": {"keys": {}}}),
        json!({"jsonrpc": "2.0", "id": "onebot-1", "result": {}}),
        json!({"jsonrpc": "2.0", "id": "onebot-2", "error": {"code": -32010}}),
    ] {
        match heard(&tools(), &Methods::new(), message.clone()) {
            Heard::Other(passed) => assert_eq!(passed, message),
            other => panic!("原样交回：{other:?}"),
        }
    }
    let cancel = json!({"jsonrpc": "2.0", "method": "tool.cancel", "params": {"session": "s1", "call_id": "c1"}});
    assert!(matches!(
        heard(&tools(), &Methods::new(), cancel),
        Heard::Cancelled
    ));
}

#[test]
fn a_method_call_is_answered_by_the_methods() {
    let reply = asked(heard(
        &tools(),
        &Methods::new(),
        json!({"jsonrpc": "2.0", "id": "core-9", "method": "method.call", "params": {"method": "status", "params": {}}}),
    ));
    // 状态还没交进来：同不认识的方法（`core/methods.rs`），编号原样。
    assert_eq!(reply["id"], "core-9", "{reply}");
    assert_eq!(reply["error"]["code"], -32601, "{reply}");
}
