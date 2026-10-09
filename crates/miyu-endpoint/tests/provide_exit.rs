//! 登记过工具的扩展退出了（施工 O-2 再补，通讯平台的会话 O-26 查出来的）：核心照样察觉，照退避重新拉起，连续失败照样停下；
//! 端口被占这类配置出错照样记成 `config_error`。原来核心记着往这个连接写的那一头，连接断了也不放，看管扩展的任务一直等。

use std::sync::Arc;

use serde_json::json;

use crate::support::extensions::*;
use crate::support::*;

/// 一件给本机的工具。
fn tools() -> String {
    json!({"tools": [{"name": "ping", "description": "The ping tool.", "input_schema": {"type": "object"},
                      "access": "read", "venues": ["local"]}]})
    .to_string()
}

#[tokio::test]
async fn a_provider_that_keeps_exiting_backs_off_and_stops() {
    let home = Home::new();
    let program = Program::new();
    let provide = format!("ask:provide:{}", tools());
    install(
        &home,
        "bridge",
        &program.name(),
        "manual",
        &steps(&["err:up", "hello", &provide, "exit:3"]),
    );
    let core = core(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let enabled = call(&mut client, "extension.enable", "bridge").await;
    assert!(enabled.get("error").is_none(), "{enabled}");
    let stopped = until_state(&mut client, "bridge", |one| one["state"] == "stopped").await;
    assert_eq!(stopped["reason"], "failed_repeatedly", "{stopped}");
    assert_eq!(stopped["failures"], 5);
    assert_eq!(
        stderr(&home, "bridge"),
        "up\n".repeat(5),
        "每次退出都察觉、重新拉起"
    );
}

#[tokio::test]
async fn a_provider_whose_port_is_taken_is_a_config_error() {
    let home = Home::new();
    let program = Program::new();
    let provide = format!("ask:provide:{}", tools());
    install(
        &home,
        "bridge",
        &program.name(),
        "manual",
        &steps(&["hello", &provide, "err:port 6700 is taken", "exit:1"]),
    );
    let core = core(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let enabled = call(&mut client, "extension.enable", "bridge").await;
    assert!(enabled.get("error").is_none(), "{enabled}");
    let stopped = until_state(&mut client, "bridge", |one| one["state"] == "stopped").await;
    assert_eq!(stopped["reason"], "config_error", "{stopped}");
}
