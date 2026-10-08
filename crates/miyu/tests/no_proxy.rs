//! 本机的服务不走代理（施工 8-11 补，`docs/blueprint/http.md`「客户端」第 5 条）：机器上设了代理，供应商的地址又是回环
//! 的，照样连得上——环境变量里的代理不会自动放行回环地址，`NO_PROXY` 没写回环地址的机器以前连不上。非回环的地址照旧
//! 走代理：指到一台记请求的假代理，看它收到了。

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader as AsyncReader};

use crate::support::{Home, within};
use miyu_http::testkit::{Piece, Received, Reply, Server};

/// 一段事件流：说 `text`，然后 `[DONE]`。
fn said(text: &str) -> Reply {
    let chunk = json!({"choices": [{"index": 0,
        "delta": {"role": "assistant", "content": text}, "finish_reason": "stop"}]});
    let body = format!("data: {chunk}\n\ndata: [DONE]\n\n");
    Reply::stream(vec![Piece::Bytes(body.into_bytes())])
}

/// 列一个模型。
fn listing(models: &[&str]) -> Reply {
    let data: Vec<serde_json::Value> = models.iter().map(|id| json!({"id": id})).collect();
    let body = json!({"object": "list", "data": data}).to_string();
    Reply::stream(vec![Piece::Bytes(body.into_bytes())])
}

/// 在连接上说 JSON-RPC：发一条，读到它的回应为止（推送跳过）。
struct Rpc {
    read: AsyncReader<tokio::io::ReadHalf<miyu_ipc::Connection>>,
    write: tokio::io::WriteHalf<miyu_ipc::Connection>,
}

impl Rpc {
    async fn call(
        &mut self,
        id: &str,
        method: &str,
        params: serde_json::Value,
    ) -> serde_json::Value {
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.write
            .write_all(format!("{request}\n").as_bytes())
            .await
            .expect("写得进");
        loop {
            let mut line = String::new();
            within(&format!("{method} 的回应"), self.read.read_line(&mut line))
                .await
                .expect("读得到");
            let reply: serde_json::Value = serde_json::from_str(&line).expect("是 JSON");
            if reply["id"] == json!(id) {
                assert!(reply.get("error").is_none(), "{method}：{reply}");
                return reply["result"].clone();
            }
        }
    }
}

/// 拉起核心：`providers_toml` 是系统配置里 `[providers.dev]` 那一段，另加 `env` 这几条核心的环境变量。等它写来 `ready`。
fn start(home: &Home, providers_toml: &str, env: &[(&str, &str)]) -> std::process::Child {
    let config = format!("{providers_toml}\n[models]\nchat = \"dev/deepseek-chat\"\n");
    home.system_config(&config);
    let mut command = home.core();
    command.env("MIYU_TEST_KEY", "test").env_remove("NO_PROXY");
    for (name, value) in env {
        command.env(name, value);
    }
    let mut core = command.stdout(Stdio::piped()).spawn().expect("起得来");
    let mut line = String::new();
    BufReader::new(core.stdout.take().expect("接了管道"))
        .read_line(&mut line)
        .expect("读得到那一行");
    assert_eq!(line, "ready\n", "核心起来了：{}", home.core_log());
    core
}

/// 停掉 [`start`] 起的核心：平台不一样，杀法不一样。收拾不掉、等不到退出也不要紧：测试该看的都看过了，临时数据根用完就删。
#[expect(
    clippy::let_underscore_must_use,
    reason = "收拾核心是测试收尾，杀不掉也不影响这一条测试的结论"
)]
fn stop(core: &mut std::process::Child) {
    #[cfg(unix)]
    let killed = Command::new("kill").arg(core.id().to_string()).status();
    #[cfg(windows)]
    let killed = Command::new("taskkill")
        .args(["/PID", &core.id().to_string(), "/F"])
        .status();
    let _ = killed;
    let _ = core.wait();
}

/// 连上核心、握手。
async fn connected(home: &Home) -> Rpc {
    let (connection, token) = within("连上", miyu_ipc::connect(&home.root))
        .await
        .expect("连得上");
    let (read, write) = tokio::io::split(connection);
    let mut rpc = Rpc {
        read: AsyncReader::new(read),
        write,
    };
    rpc.call(
        "h1",
        "hello",
        json!({"protocol": [1, 1], "head": {"kind": "test", "version": "0.0.0"},
            "caps": {"input": false}, "token": token}),
    )
    .await;
    rpc
}

/// 列模型（`refresh: true`，拉完才答）、造会话、说一句「在吗」：和会话真发时一样的两种请求——拉列表、发请求。
async fn list_and_ask(rpc: &mut Rpc, work: &Path) {
    rpc.call(
        "l1",
        "model.list",
        json!({"provider": "dev", "refresh": true}),
    )
    .await;
    let created = rpc
        .call(
            "c1",
            "session.create",
            json!({"cwd": work.to_string_lossy()}),
        )
        .await;
    let session = created["session"].as_str().expect("有编号").to_string();
    rpc.call(
        "s1",
        "session.send",
        json!({"session": session, "text": "在吗"}),
    )
    .await;
}

/// 等服务器收到 `count` 条请求，最多十秒。
async fn wait_received(server: &Server, count: usize, why: &str) -> Vec<Received> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while server.received().len() < count && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let received = server.received();
    assert_eq!(received.len(), count, "{why}");
    received
}

#[tokio::test]
async fn a_loopback_provider_is_reached_even_with_the_proxy_pointed_at_a_dead_port() {
    let server = Server::start(vec![listing(&["deepseek-chat"]), said("你好！")]).await;
    let home = Home::new();
    let work: PathBuf = home.dir.join("work");
    std::fs::create_dir_all(&work).expect("建得了");
    let providers = format!(
        "[providers.dev]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\nkeys = [{{ env = \"MIYU_TEST_KEY\" }}]\n",
        server.base_url
    );
    // 代理的环境变量指到一台没人听的本机端口：直连才连得上，不写代码会卡在连不上代理（连接被拒绝，不是超时）上。
    let mut core = start(
        &home,
        &providers,
        &[
            ("HTTP_PROXY", "http://127.0.0.1:1"),
            ("HTTPS_PROXY", "http://127.0.0.1:1"),
            ("ALL_PROXY", "http://127.0.0.1:1"),
        ],
    );

    let mut rpc = connected(&home).await;
    list_and_ask(&mut rpc, &work).await;

    let received = wait_received(&server, 2, "死代理没挡住本机的服务").await;
    assert_eq!(received[0].path, "/v1/models", "列模型");
    assert_eq!(received[1].path, "/v1/chat/completions", "发一次请求");

    stop(&mut core);
    home.until_stopped().await;
}

#[tokio::test]
async fn a_non_loopback_provider_still_goes_through_the_proxy() {
    // 假代理：记请求，不是真的转发，但照样回剧本——从核心这一头看和转发到了目标没有区别。
    let proxy = Server::start(vec![listing(&["deepseek-chat"]), said("你好！")]).await;
    let proxy_url = proxy.base_url.trim_end_matches("/v1").to_string();
    let home = Home::new();
    let work: PathBuf = home.dir.join("work");
    std::fs::create_dir_all(&work).expect("建得了");
    // 地址不是回环：`example.invalid` 保留给测试用，解不出来也不要紧——代理替它连。
    let providers =
        "[providers.dev]\ndriver = \"openai-chat\"\nbase_url = \"http://example.invalid/v1\"\nkeys = [{ env = \"MIYU_TEST_KEY\" }]\n"
            .to_string();
    let mut core = start(
        &home,
        &providers,
        &[("HTTP_PROXY", &proxy_url), ("ALL_PROXY", &proxy_url)],
    );

    let mut rpc = connected(&home).await;
    list_and_ask(&mut rpc, &work).await;

    let received = wait_received(&proxy, 2, "非回环地址该走代理，假代理没收到").await;
    assert!(
        received[0].path.contains("example.invalid"),
        "列模型经代理转发，地址带着目标：{:?}",
        received[0].path
    );
    assert!(
        received[1].path.contains("example.invalid"),
        "发请求经代理转发，地址带着目标：{:?}",
        received[1].path
    );

    stop(&mut core);
    home.until_stopped().await;
}

/// `provider.test`（施工单目的里另列的一种）也照地址挑客户端：和列模型、发请求用的是同一个 `ModelData::fetcher_for`
/// （`route/probe.rs`），死代理照样挡不住它。
#[tokio::test]
async fn provider_test_reaches_a_loopback_provider_even_with_a_dead_proxy() {
    let server = Server::start(vec![listing(&["deepseek-chat"]), said("OK")]).await;
    let home = Home::new();
    let providers = format!(
        "[providers.dev]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\nkeys = [{{ env = \"MIYU_TEST_KEY\" }}]\n",
        server.base_url
    );
    let mut core = start(
        &home,
        &providers,
        &[
            ("HTTP_PROXY", "http://127.0.0.1:1"),
            ("HTTPS_PROXY", "http://127.0.0.1:1"),
            ("ALL_PROXY", "http://127.0.0.1:1"),
        ],
    );

    let mut rpc = connected(&home).await;
    let result = rpc
        .call("t1", "provider.test", json!({"provider": "dev"}))
        .await;
    assert_eq!(result["ok"], json!(true), "{result}");

    stop(&mut core);
    home.until_stopped().await;
}
