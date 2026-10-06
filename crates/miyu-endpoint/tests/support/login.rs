//! 身份的测试共用的（施工 W-8）：照凭据握手、出示本机令牌连上、要一次性码、设好用户名和密码，读拒绝的原因码。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_session::testkit::Script;
use miyu_tool::Catalog;

use super::{Client, Home, TOKEN};

impl Home {
    /// 一份核心，一次性码 `ttl` 有效：测试里设短的，不用真等 5 分钟。
    pub fn core_code_ttl(&self, ttl: Duration) -> Arc<Core> {
        Arc::new(
            self.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
                .with_sandbox(miyu_sandbox::Availability::Usable("miyu-sandbox".into()))
                .with_code_ttl(ttl),
        )
    }
}

/// 握手：中文，凭据照 `credentials`（`token`、`code`、`login`、`user` 和 `password` 这几格）。
pub async fn hello_with(client: &mut Client, credentials: Value) -> Value {
    let mut params = json!({
        "protocol": [1, 1],
        "head": {"kind": "web", "version": "0.0.0"},
        "locale": "zh-CN",
    });
    if let (Some(params), Value::Object(more)) = (params.as_object_mut(), credentials) {
        params.extend(more);
    }
    client.call("h", "hello", params).await
}

/// 出示本机令牌连上的一个客户端。
pub async fn local(core: &Arc<Core>) -> Client {
    let mut client = Client::connect(Arc::clone(core));
    client.hello().await;
    client
}

/// 要一个一次性码，交回回应的 `result`。
pub async fn setup_code(core: &Arc<Core>) -> Value {
    let mut client = local(core).await;
    let reply = client.call("c", "account.setup_code", json!({})).await;
    reply["result"].clone()
}

/// 用一次性码进来、设好 `username`、`password`，交回登录令牌。
pub async fn set_up(core: &Arc<Core>, username: &str, password: &str) -> String {
    let code = setup_code(core).await["code"]
        .as_str()
        .expect("有码")
        .to_string();
    let mut client = Client::connect(Arc::clone(core));
    let reply = hello_with(&mut client, json!({"code": code})).await;
    assert_eq!(reply["result"]["setup"], true, "{reply}");
    let reply = client
        .call(
            "s",
            "account.setup",
            json!({"username": username, "password": password}),
        )
        .await;
    reply["result"]["login"]["token"]
        .as_str()
        .expect("{reply}")
        .to_string()
}

pub fn why(reply: &Value) -> &str {
    reply["error"]["data"]["reason"].as_str().unwrap_or("")
}

pub fn is_hex64(text: &str) -> bool {
    text.len() == 64
        && text
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

/// 这个连接断开了没有：发一条请求，读到头的是断开了，读到回应、两秒没动静的都是还连着。光等两秒读不到东西分不出
/// 「断开了」和「连着但没人说话」。
pub async fn is_closed(client: &mut Client) -> bool {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
    let request = json!({"jsonrpc": "2.0", "id": "alive", "method": "session.list", "params": {}});
    let Some(writer) = client.writer.as_mut() else {
        return true;
    };
    if writer
        .write_all(format!("{request}\n").as_bytes())
        .await
        .is_err()
    {
        return true;
    }
    let mut line = String::new();
    matches!(
        tokio::time::timeout(Duration::from_secs(2), client.reader.read_line(&mut line)).await,
        Ok(Ok(0) | Err(_))
    )
}
