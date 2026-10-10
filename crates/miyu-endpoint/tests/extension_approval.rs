//! 扩展能力的审批（施工 9-4 下上，`docs/blueprint/extensions.md`「能力」）：管理员装的包声明了能力，开的那一下要批；
//! 批过的记在开关的文件里，能力多了只问多的；`always` 的没批不拉起、不拦空闲；出厂的包算批过的。

use std::sync::Arc;

use serde_json::{Value, json};

use crate::support::extensions::*;
use crate::support::*;

/// 开 `id`，带上 `approve`（不写的是没带）。
async fn enable(client: &mut Client, id: &str, approve: Option<&[&str]>) -> Value {
    let mut params = json!({"package": id});
    if let Some(approve) = approve {
        params["approve"] = json!(approve);
    }
    client.call("e", "extension.enable", params).await
}

/// 拒绝的原因。
fn refused(reply: &Value) -> Option<&str> {
    reply["error"]["data"]["reason"].as_str()
}

/// 开关的文件。
fn switches(home: &Home) -> String {
    read(&home.root.system().join("extensions.json"))
}

#[tokio::test]
async fn an_installed_extension_asks_for_its_capabilities_when_enabled() {
    let home = Home::new();
    let program = Program::new();
    let waits = steps(&["hello", "wait"]);
    install_asking(
        &home,
        "echo",
        &program.name(),
        "manual",
        &waits,
        &["network", "events.read"],
    );
    let core = core(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let off = status(&mut client, "echo").await;
    assert_eq!(
        off["capabilities"],
        json!([
            {"id": "events.read", "name": "读会话", "summary": "读会话里的全部记录"},
            {"id": "network", "name": "联网", "summary": "访问网络、开端口"},
        ]),
        "照能力表的先后，名字照连接的语言：{off}"
    );
    assert_eq!(off["unapproved"], json!(["events.read", "network"]));

    let asked = enable(&mut client, "echo", None).await;
    assert_eq!(refused(&asked), Some("needs_approval"), "{asked}");
    assert_eq!(
        asked["error"]["data"]["capabilities"],
        json!(["events.read", "network"])
    );
    assert_eq!(switches(&home), "", "没批：开关的文件一个字没写");
    assert_eq!(status(&mut client, "echo").await["state"], "off");

    let partly = enable(&mut client, "echo", Some(&["network"])).await;
    assert_eq!(refused(&partly), Some("needs_approval"), "{partly}");
    assert_eq!(
        partly["error"]["data"]["capabilities"],
        json!(["events.read"]),
        "只说还没盖住的"
    );
    for approve in [&["network", "telepathy"][..], &["tools"][..]] {
        let wrong = enable(&mut client, "echo", Some(approve)).await;
        assert_eq!(refused(&wrong), Some("bad_params"), "{approve:?}：{wrong}");
    }

    let approved = enable(&mut client, "echo", Some(&["events.read", "network"])).await;
    assert_eq!(approved["result"]["on"], true, "{approved}");
    assert!(approved["result"].get("unapproved").is_none(), "{approved}");
    until_state(&mut client, "echo", |one| one["state"] == "running").await;
    assert_eq!(
        switches(&home),
        "{\"version\":1,\"on\":{\"echo\":true},\"approved\":{\"echo\":[\"events.read\",\"network\"]}}\n"
    );
    let disabled = call(&mut client, "extension.disable", "echo").await;
    assert_eq!(disabled["result"]["state"], "off");
    let again = enable(&mut client, "echo", None).await;
    assert_eq!(again["result"]["on"], true, "关了不清批准：{again}");
    core.stop_extensions().await;
}

#[tokio::test]
async fn an_always_one_waits_for_approval_without_keeping_the_core_busy() {
    let home = Home::new();
    let program = Program::new();
    let waits = steps(&["hello", "wait"]);
    install_asking(
        &home,
        "auto",
        &program.name(),
        "always",
        &waits,
        &["network"],
    );
    let core = core(&home, quick());
    assert!(core.idle().await, "没批的不拉起，不拦空闲");
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let waiting = status(&mut client, "auto").await;
    assert_eq!(
        (&waiting["on"], &waiting["state"], &waiting["reason"]),
        (&json!(true), &json!("stopped"), &json!("needs_approval")),
        "{waiting}"
    );
    let restarted = call(&mut client, "extension.restart", "auto").await;
    assert_eq!(refused(&restarted), Some("needs_approval"), "{restarted}");
    assert_eq!(
        restarted["error"]["data"]["capabilities"],
        json!(["network"])
    );
    let approved = enable(&mut client, "auto", Some(&["network"])).await;
    assert_eq!(approved["result"]["on"], true, "{approved}");
    until_state(&mut client, "auto", |one| one["state"] == "running").await;
    core.stop_extensions().await;
}

#[tokio::test]
async fn more_capabilities_after_an_upgrade_ask_only_for_the_new_ones() {
    let home = Home::new();
    let program = Program::new();
    let waits = steps(&["hello", "wait"]);
    install_asking(
        &home,
        "echo",
        &program.name(),
        "manual",
        &waits,
        &["network"],
    );
    let first = core(&home, quick());
    let mut client = Client::connect(Arc::clone(&first));
    client.hello().await;
    let approved = enable(&mut client, "echo", Some(&["network"])).await;
    assert_eq!(approved["result"]["on"], true, "{approved}");
    drop(client);
    first.stop_extensions().await;

    install_asking(
        &home,
        "echo",
        &program.name(),
        "manual",
        &waits,
        &["network", "sessions.drive"],
    );
    let second = core(&home, quick());
    let mut client = Client::connect(Arc::clone(&second));
    client.hello().await;
    let upgraded = status(&mut client, "echo").await;
    assert_eq!(
        upgraded["unapproved"],
        json!(["sessions.drive"]),
        "{upgraded}"
    );
    assert_eq!(
        (&upgraded["state"], &upgraded["reason"]),
        (&json!("stopped"), &json!("needs_approval")),
        "开着、多了没批的：不拉起"
    );
    let approved = enable(&mut client, "echo", Some(&["sessions.drive"])).await;
    assert_eq!(approved["result"]["on"], true, "只批多的那个：{approved}");
    until_state(&mut client, "echo", |one| one["state"] == "running").await;
    assert!(
        switches(&home).contains("\"approved\":{\"echo\":[\"sessions.drive\",\"network\"]}"),
        "记下这时声明的全部，照能力表的先后：{}",
        switches(&home)
    );
    second.stop_extensions().await;
}

#[tokio::test]
async fn a_shipped_extension_counts_as_approved() {
    let home = Home::new();
    let core = core(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let bridge = status(&mut client, "onebot").await;
    let declared: Vec<&str> = bridge["capabilities"]
        .as_array()
        .unwrap_or_else(|| panic!("出厂的桥声明了能力：{bridge}"))
        .iter()
        .filter_map(|one| one["id"].as_str())
        .collect();
    assert_eq!(
        declared,
        [
            "tools",
            "events.read",
            "events.write",
            "sessions.drive",
            "act_for_external",
            "network"
        ]
    );
    assert!(bridge.get("unapproved").is_none(), "出厂的不用批：{bridge}");
}
