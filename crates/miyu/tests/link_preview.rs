//! 链接卡片走环境变量里的代理（施工 W-7，`net.md`「怎么走」第 5、11 条）：真的核心，代理的环境变量指到本机一台记请求的
//! 假代理。目标写测性能的段 `198.18.0.1`（当公网，不用解析；从本机连不上，只有经代理才到得了）：卡片做得出、图用
//! `blob.get` 读得回来，假代理收到的请求行写着整个地址。慢的 `link.preview` 不挡同一个连接上后面的请求，回应照 `id`
//! 对上。环境变量只设在拉起的核心上，不碰这个测试进程的。

mod support;

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader as AsyncReader};

use miyu_http::testkit::{Piece, Reply, Server};
use support::{Home, within};

/// 一份 PNG 的开头。
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR";

/// 代理的几个环境变量，大写小写都算：开发机上设了的不能漏进来。
const PROXY_VARIABLES: [&str; 9] = [
    "HTTP_PROXY",
    "http_proxy",
    "HTTPS_PROXY",
    "https_proxy",
    "ALL_PROXY",
    "all_proxy",
    "NO_PROXY",
    "no_proxy",
    "REQUEST_METHOD",
];

/// 200，`content_type`，一片一片地写。
fn reply(content_type: &str, body: Vec<Piece>) -> Reply {
    Reply {
        status: 200,
        headers: vec![("Content-Type".to_string(), content_type.to_string())],
        body,
    }
}

/// 拉起核心，`HTTP_PROXY` 指到 `proxy`。等它写来 `ready`。
fn start(home: &Home, proxy: &Server) -> Child {
    let mut command = home.core();
    for name in PROXY_VARIABLES {
        command.env_remove(name);
    }
    let proxy_url = proxy.base_url.trim_end_matches("/v1").to_string();
    command.env("HTTP_PROXY", proxy_url);
    let mut core = command.stdout(Stdio::piped()).spawn().expect("起得来");
    let mut line = String::new();
    BufReader::new(core.stdout.take().expect("接了管道"))
        .read_line(&mut line)
        .expect("读得到那一行");
    assert_eq!(line, "ready\n", "核心起来了：{}", home.core_log());
    core
}

/// 停掉 [`start`] 起的核心：照它的进程号。
#[expect(
    clippy::let_underscore_must_use,
    reason = "收拾核心是测试收尾，杀不掉也不影响这一条测试的结论"
)]
fn stop(core: &mut Child) {
    #[cfg(unix)]
    let killed = Command::new("kill").arg(core.id().to_string()).status();
    #[cfg(windows)]
    let killed = Command::new("taskkill")
        .args(["/PID", &core.id().to_string(), "/F"])
        .status();
    let _ = killed;
    let _ = core.wait();
}

/// 在连接上说 JSON-RPC：握过手了。
struct Head {
    read: AsyncReader<tokio::io::ReadHalf<miyu_ipc::Connection>>,
    write: tokio::io::WriteHalf<miyu_ipc::Connection>,
}

impl Head {
    async fn connect(home: &Home) -> Head {
        let (connection, token) = within("连上", miyu_ipc::connect(&home.root))
            .await
            .expect("连得上");
        let (read, write) = tokio::io::split(connection);
        let mut head = Head {
            read: AsyncReader::new(read),
            write,
        };
        head.send(
            "h1",
            "hello",
            json!({"protocol": [1, 1], "head": {"kind": "test", "version": "0.0.0"},
                "caps": {"input": false}, "token": token}),
        )
        .await;
        let hello = head.next().await;
        assert!(hello.get("error").is_none(), "{hello}");
        head
    }

    /// 发一条，不等回应。
    async fn send(&mut self, id: &str, method: &str, params: Value) {
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.write
            .write_all(format!("{request}\n").as_bytes())
            .await
            .expect("写得进");
    }

    /// 下一条回应（推送跳过），最多等一分钟：抓页面、存图在 CI 的慢机器上留够。
    async fn next(&mut self) -> Value {
        tokio::time::timeout(Duration::from_secs(60), async {
            loop {
                let mut line = String::new();
                let read = self.read.read_line(&mut line).await.expect("读得了");
                assert!(read > 0, "核心断开了");
                let reply: Value = serde_json::from_str(&line).expect("是 JSON");
                if reply.get("id").is_some() {
                    return reply;
                }
            }
        })
        .await
        .unwrap_or_else(|_| panic!("一分钟内没等到回应"))
    }
}

#[tokio::test]
async fn a_card_comes_through_the_proxy_in_the_environment() {
    let html = r#"<html><head><title>Through the proxy</title>
        <meta property="og:image" content="/card.png"></head><body>"#;
    // 先是页面；图和图标一起抓，谁先连上不一定，两个都回同一张图
    let proxy = Server::start(vec![
        reply("text/html", vec![Piece::Bytes(html.as_bytes().to_vec())]),
        reply("image/png", vec![Piece::Bytes(PNG.to_vec())]),
        reply("image/png", vec![Piece::Bytes(PNG.to_vec())]),
    ])
    .await;
    let home = Home::new();
    let mut core = start(&home, &proxy);
    let mut head = Head::connect(&home).await;

    head.send(
        "p1",
        "link.preview",
        json!({"url": "http://198.18.0.1/page"}),
    )
    .await;
    let reply = head.next().await;
    let card = &reply["result"]["card"];
    assert_eq!(card["title"], "Through the proxy", "{reply}");
    assert_eq!(card["url"], "http://198.18.0.1/page");
    assert_eq!(card["site"], "198.18.0.1");
    // W-7 再补：`kind` 一定写；`duration`、`author` 没有的不写
    assert_eq!(card["kind"], "page");
    assert!(
        card.get("duration").is_none() && card.get("author").is_none(),
        "{card}"
    );
    assert_eq!(card["image"]["media_type"], "image/png");
    let blob = card["image"]["blob"]
        .as_str()
        .expect("图是 blob")
        .to_string();
    head.send("b1", "blob.get", json!({"blob": blob})).await;
    let got = head.next().await;
    assert_eq!(got["result"]["size"], PNG.len(), "{got}");

    let received = proxy.received();
    assert_eq!(
        received.first().map(|request| request.path.as_str()),
        Some("http://198.18.0.1/page"),
        "代理收到的请求行写着整个地址"
    );
    assert_eq!(received.len(), 3, "页面、图、图标都经代理");

    stop(&mut core);
    home.until_stopped().await;
}

#[tokio::test]
async fn a_slow_link_preview_does_not_hold_up_the_next_request() {
    let gate = Arc::new(tokio::sync::Barrier::new(2));
    // 页面写了一半停在闸上：测试放行以前，这一张卡片做不完
    let proxy = Server::start(vec![
        reply(
            "text/html",
            vec![
                Piece::Bytes(b"<html><head><title>Slow</title>".to_vec()),
                Piece::Gate(Arc::clone(&gate)),
                Piece::Bytes(b"</head><body>".to_vec()),
            ],
        ),
        Reply::error(404, &[], ""),
    ])
    .await;
    let home = Home::new();
    let mut core = start(&home, &proxy);
    let mut head = Head::connect(&home).await;

    head.send(
        "p1",
        "link.preview",
        json!({"url": "http://198.18.0.1/slow"}),
    )
    .await;
    head.send("q1", "human.get", json!({})).await;
    let first = head.next().await;
    assert_eq!(first["id"], "q1", "后面的请求先回来：{first}");
    gate.wait().await;
    let second = head.next().await;
    assert_eq!(second["id"], "p1", "回应照 id 对上");
    assert_eq!(second["result"]["card"]["title"], "Slow", "{second}");

    stop(&mut core);
    home.until_stopped().await;
}
