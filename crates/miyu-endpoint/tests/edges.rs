//! 协议的几个边角（施工 4-9 再补三上）：握手的时限；数组的 `params` 不收；握手被拒照它报的语言说；人格的目录在、
//! 里面读不了的是内部出错。

use std::time::Duration;

use serde_json::json;

use crate::support::*;
use miyu_session::testkit::Script;

/// 连上不握手：到了时限就断开，不然一个连上不说话的本机进程能让核心一直不空闲退出。握手了的不受它管。
#[tokio::test]
async fn a_connection_that_never_says_hello_is_closed() {
    let home = Home::new();
    let core = home.core_waiting_hello(&Script::new([]), Duration::from_millis(100));
    let mut silent = Client::connect(core.clone());
    let closed = tokio::time::timeout(Duration::from_secs(60), silent.next()).await;
    assert!(matches!(closed, Ok(None)), "没握手的断开：{closed:?}");
    let mut client = Client::connect(core);
    client.hello().await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let reply = client
        .call("c1", "session.create", json!({"cwd": "~"}))
        .await;
    assert!(
        reply["result"]["session"].is_string(),
        "握手了的照常：{reply}"
    );
}

/// `params` 写成数组的：参数不对（方法都照名字取参数，照位置读的回应找不到该交给哪个订阅）。
#[tokio::test]
async fn positional_params_are_refused() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    // 照位置读也读得成的一条：空的数组，`session.list` 的几格都可以不写。
    client
        .line(r#"{"jsonrpc":"2.0","id":"c1","method":"session.list","params":[]}"#)
        .await;
    let reply = client.next().await.expect("有回应");
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
    assert_eq!(reply["id"], json!("c1"));
}

/// 第一次握手就被拒：话照这一次报的语言说，不一律英文。
#[tokio::test]
async fn a_refused_hello_speaks_the_language_it_asked_for() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    let reply = client
        .call(
            "h1",
            "hello",
            json!({"protocol": [1, 1], "head": {"kind": "test", "version": "0"}, "token": "wrong", "locale": "zh-CN"}),
        )
        .await;
    assert_eq!(reason(&reply), Some("bad_token"), "{reply}");
    assert_eq!(reply["error"]["message"], json!("本机令牌不对。"));
}

/// 人格的目录不存在：没有这个人格。目录在、里面读不了（安装坏了）：内部出错。
#[tokio::test]
async fn a_broken_persona_is_an_internal_error() {
    let home = Home::new();
    let resources = home.work.join("resources");
    std::fs::create_dir_all(resources.join("personas/broken")).expect("建得了目录");
    // 默认预设要在（施工 P-2 上）：不然先报 unknown_preset，走不到人格。
    std::fs::create_dir_all(resources.join("presets")).expect("建得了目录");
    std::fs::write(resources.join("presets/full.toml"), "").expect("写得了");
    let mut client = Client::connect(home.core_with_resources(&Script::new([]), resources));
    client.hello().await;
    let create = |persona: &str| json!({"cwd": "~", "persona": persona});
    let reply = client.call("c1", "session.create", create("ghost")).await;
    assert_eq!(reason(&reply), Some("unknown_persona"), "{reply}");
    let reply = client.call("c2", "session.create", create("broken")).await;
    assert_eq!(reason(&reply), Some("internal_error"), "{reply}");
}
