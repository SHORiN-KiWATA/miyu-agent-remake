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
    // 施工 O-31 起四件：`recall`、`poke` 给私聊和群，`mute` 只给群；三件平台工具是 `venue`（在场所里做的事），`skip_reply` 是
    // `read`（`onebot.md` 施工时定的第 177 条）。施工 O-33 加 `fetch_media`：`venue`，只给群。
    let spec = |name: &str| -> Value {
        serde_json::from_str(&shipped(&format!("tools/{name}.json"))).expect("是 JSON")
    };
    let entry = |name: &str, access: &str, venues: Value| {
        json!({
            "name": name,
            "description": spec(name)["description"],
            "input_schema": spec(name)["parameters"],
            "access": access,
            "venues": venues,
        })
    };
    let both = json!(["private", "group"]);
    assert_eq!(
        tools().provided(),
        json!({"tools": [
            entry("skip_reply", "read", both.clone()),
            entry("recall", "venue", both.clone()),
            entry("mute", "venue", json!(["group"])),
            entry("poke", "venue", both),
            entry("fetch_media", "venue", json!(["group"])),
        ]})
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
    let unknown = shipped("tool-results/unknown.txt").replace("{name}", "kick");
    assert_eq!(
        asked(heard(
            &tools(),
            &Methods::new(),
            tool_call("core-8", "kick")
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

#[test]
fn platform_tool_calls_pass_on_to_the_route() {
    // 施工 O-31：撤回、禁言、戳一戳要投影和 NapCat 的连接，读的一头不当场答，原样交给跟核心的那一头（「平台工具（一）」第 2 条）。
    // 施工 O-33：`fetch_media` 要会话发到哪、这一轮取了几次，也交过去。
    for tool in ["recall", "mute", "poke", "fetch_media"] {
        let message = tool_call("core-10", tool);
        match heard(&tools(), &Methods::new(), message.clone()) {
            Heard::Other(passed) => assert_eq!(passed, message),
            other => panic!("{tool} 交给那一头：{other:?}"),
        }
    }
}
