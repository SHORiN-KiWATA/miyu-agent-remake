//! 身份的运行日志（施工 W-8，`docs/blueprint/web-module.md`「出错」的运行日志表）：设密码、造登录令牌、作废、握手被拒各记一行；
//! 把能走的路都走一遍，整份运行日志里搜不到一次性码、密码、登录令牌，`TRACE` 也搜不到。
//!
//! 全局装一个写进内存的订阅者，一个进程只能装一次，所以这个文件单独一个测试程序，只有一个测试。

mod support;

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_log::{LevelFilter, Memory};
use miyu_session::testkit::Script;

use support::*;

async fn hello_with(client: &mut Client, credentials: Value) -> Value {
    let mut params = json!({"protocol": [1, 1], "head": {"kind": "web", "version": "0.0.0"}});
    if let (Some(params), Value::Object(more)) = (params.as_object_mut(), credentials) {
        params.extend(more);
    }
    client.call("h", "hello", params).await
}

#[tokio::test]
async fn the_log_says_what_happened_and_never_a_secret() {
    let memory = Memory::new();
    tracing::subscriber::set_global_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::TRACE,
        None,
    ))
    .expect("这个测试程序只装这一次");
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut terminal = Client::connect(Arc::clone(&core));
    terminal.hello().await;
    let code = terminal.call("c", "account.setup_code", json!({})).await["result"]["code"]
        .as_str()
        .expect("有码")
        .to_string();
    let password = "correct horse battery";
    let mut browser = Client::connect(Arc::clone(&core));
    hello_with(&mut browser, json!({"code": code})).await;
    let first = browser
        .call(
            "s",
            "account.setup",
            json!({"username": "shorin", "password": password}),
        )
        .await["result"]["login"]["token"]
        .as_str()
        .expect("有令牌")
        .to_string();
    let mut second = Client::connect(Arc::clone(&core));
    let token = hello_with(&mut second, json!({"user": "shorin", "password": password})).await
        ["result"]["login"]["token"]
        .as_str()
        .expect("有令牌")
        .to_string();
    let mut third = Client::connect(Arc::clone(&core));
    hello_with(&mut third, json!({"login": first})).await;
    for credentials in [
        json!({"code": code}),
        json!({"login": "0".repeat(64)}),
        json!({"user": "shorin", "password": "wrong horse"}),
    ] {
        let mut stranger = Client::connect(Arc::clone(&core));
        hello_with(&mut stranger, credentials).await;
    }
    for _ in 0..5 {
        let mut stranger = Client::connect(Arc::clone(&core));
        hello_with(
            &mut stranger,
            json!({"user": "shorin", "password": "wrong horse"}),
        )
        .await;
    }
    terminal
        .call("o", "account.logout", json!({"all": true}))
        .await;
    let lines = memory.lines();
    let has = |what: &str| lines.iter().any(|line| line.contains(what));
    for wanted in [
        "password set first=true",
        "login issued via=setup",
        "login issued via=password",
        "via=code",
        "via=password",
        "via=login",
        "bad code",
        "bad login",
        "bad password",
        "login throttled",
        "logins revoked count=2",
    ] {
        assert!(has(wanted), "{wanted}: {lines:#?}");
    }
    for secret in [
        code.as_str(),
        first.as_str(),
        token.as_str(),
        password,
        "wrong horse",
    ] {
        assert!(!has(secret), "运行日志里有 {secret}");
    }
}
