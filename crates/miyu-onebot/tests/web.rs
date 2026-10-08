//! 桥的 WebUI（施工 O-16，`onebot.md` 第二条「怎么走」第 1 条）：照网页软件的测试搬一份，对着 `miyu-onebot` 跑。Host 只认三种
//! 写法；页面文件不出页面目录、响应头一个不少、内容安全策略照 `bridge.json`；`/ws` 的 Origin 要对，两头一帧一行照转、一个
//! 字节不改，核心断了 1012，连不上核心发 `web.error` 再关；端口被占了说是哪个端口；`/human` 交出页面的字。核心是替身。

use std::path::PathBuf;

use futures_util::SinkExt;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;

use miyu_onebot::serve::{Failure, Serve, run};
use miyu_onebot::settings::Settings;
use miyu_onebot::web::PAGES;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

use crate::support::fake_core::{FakeCore, fake_core};
use crate::support::http::*;
use crate::support::*;

/// 一个临时的数据根，里面跑着核心的替身；另有一个临时的资源目录，页面目录里放了几份页面。
struct Site {
    dir: PathBuf,
    root: DataRoot,
    core: FakeCore,
    resources: ResourceRoot,
}

impl Site {
    fn new() -> Site {
        let (dir, root) = temp_root();
        let core = fake_core(&root);
        let resources = dir.join("resources");
        let pages = resources.join(PAGES);
        std::fs::create_dir_all(pages.join("sub")).expect("建得了");
        std::fs::write(pages.join("index.html"), "<h1>onebot</h1>").expect("写得进");
        std::fs::write(pages.join("app.js"), "let x = 1;").expect("写得进");
        std::fs::write(pages.join("sub/style.css"), "p{}").expect("写得进");
        std::fs::write(pages.join("data.unknown"), "?").expect("写得进");
        std::fs::write(resources.join("secret.txt"), "secret").expect("写得进");
        Site {
            dir,
            root,
            core,
            resources: ResourceRoot::at(resources),
        }
    }

    /// 页面目录。只有符号链接那一段用它，那一段只在 Unix 上编译；别处不标会是死代码，Windows 上 clippy 拦。
    #[cfg(unix)]
    fn pages(&self) -> PathBuf {
        self.resources.path().join(PAGES)
    }

    /// 在它上面起一个桥：资源目录照这里的。
    async fn bridge(&self) -> Bridge {
        start(Serve {
            resources: self.resources.clone(),
            ..serve(self.root.clone(), settings())
        })
        .await
    }
}

impl Drop for Site {
    fn drop(&mut self) {
        if std::fs::remove_dir_all(&self.dir).is_err() {
            // 删不掉就留在临时目录里，不影响测试。
        }
    }
}

#[tokio::test]
async fn pages_stay_inside_and_carry_the_headers() {
    let site = Site::new();
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        site.resources.path().join("secret.txt"),
        site.pages().join("link.txt"),
    )
    .expect("建得了");
    let bridge = site.bridge().await;
    let port = bridge.web;
    let index = get(port, "/", &[]).await;
    assert_eq!(index.status, 200);
    assert_eq!(index.body, b"<h1>onebot</h1>");
    assert_eq!(
        index.header("content-type"),
        Some("text/html; charset=utf-8")
    );
    assert_eq!(index.header("x-content-type-options"), Some("nosniff"));
    assert_eq!(index.header("referrer-policy"), Some("no-referrer"));
    assert_eq!(index.header("cache-control"), Some("no-cache"));
    let csp = tuning().web.csp;
    assert_eq!(index.header("content-security-policy"), Some(csp.as_str()));
    assert!(csp.contains("connect-src 'self'") && csp.contains("frame-ancestors 'none'"));
    assert_eq!(index.header("set-cookie"), None, "从不设 cookie");
    assert_eq!(
        get(port, "/app.js", &[]).await.header("content-type"),
        Some("text/javascript; charset=utf-8")
    );
    assert_eq!(
        get(port, "/sub/style.css", &[])
            .await
            .header("content-type"),
        Some("text/css; charset=utf-8")
    );
    assert_eq!(
        get(port, "/data.unknown", &[]).await.header("content-type"),
        Some("application/octet-stream"),
        "表里没有的不猜"
    );
    let head = request(port, "HEAD", "/app.js", &format!("127.0.0.1:{port}"), &[]).await;
    assert_eq!((head.status, head.body.len()), (200, 0));
    for outside in [
        "/../secret.txt",
        "/%2e%2e/secret.txt",
        "/sub/../../secret.txt",
        "/sub/../app.js",
        "/sub",
        "/nope.js",
        "/link.txt",
        "/%zz",
    ] {
        let answer = get(port, outside, &[]).await;
        assert_eq!(answer.status, 404, "{outside}");
        assert_eq!(answer.header("x-content-type-options"), Some("nosniff"));
    }
    let host = format!("127.0.0.1:{port}");
    assert_eq!(request(port, "POST", "/", &host, &[]).await.status, 405);
    bridge.stop().await.expect("停得下");
}

#[tokio::test]
async fn only_the_three_loopback_hosts_get_in() {
    let site = Site::new();
    let bridge = site.bridge().await;
    let port = bridge.web;
    for good in [
        format!("127.0.0.1:{port}"),
        format!("localhost:{port}"),
        format!("LOCALHOST:{port}"),
        format!("[::1]:{port}"),
    ] {
        assert_eq!(
            request(port, "GET", "/", &good, &[]).await.status,
            200,
            "{good}"
        );
    }
    for bad in [
        format!("evil.example:{port}"),
        "127.0.0.1".to_string(),
        format!("127.0.0.1:{}", bridge.port),
        format!("127.0.0.1:{}", port + 1),
    ] {
        for path in ["/", "/status", "/human", "/ws"] {
            let answer = request(port, "GET", path, &bad, &[]).await;
            assert_eq!(answer.status, 403, "{bad} {path}");
        }
    }
    bridge.stop().await.expect("停得下");
}

#[tokio::test]
async fn the_origin_must_be_this_page() {
    let site = Site::new();
    let bridge = site.bridge().await;
    let port = bridge.web;
    for origin in [
        None,
        Some("http://evil.example".to_string()),
        Some("https://127.0.0.1".to_string()),
        Some("null".to_string()),
        Some(format!("http://127.0.0.1:{}", bridge.port)),
        Some(format!("https://127.0.0.1:{port}")),
    ] {
        let refused = browser(port, origin.as_deref()).await.expect_err("不接");
        assert!(refused.contains("403"), "{origin:?}: {refused}");
    }
    for origin in [
        format!("http://localhost:{port}"),
        format!("http://[::1]:{port}"),
    ] {
        assert!(browser(port, Some(&origin)).await.is_ok(), "{origin}");
    }
    bridge.stop().await.expect("停得下");
}

#[tokio::test]
async fn frames_and_lines_pass_through_untouched() {
    let mut site = Site::new();
    let bridge = site.bridge().await;
    let port = bridge.web;
    let mut ws = browser(port, Some(&format!("http://127.0.0.1:{port}")))
        .await
        .expect("接了");
    let hello = r#"{"id":"h","jsonrpc":"2.0","method":"hello","params":{"user":"admin","password":"密码 \t ","protocol":[1,1]}}"#;
    ws.send(Message::text(hello)).await.expect("发得出");
    let mut forwarded = within("核心接到", site.core.forwarded.recv())
        .await
        .expect("转过来了");
    assert_eq!(forwarded.first, format!("{hello}\n"), "凭据原样到核心");
    let chinese = r#"{"text":"你好，Miyu  \t空白照留"}"#;
    ws.send(Message::text(chinese)).await.expect("发得出");
    let mut line = String::new();
    within("核心读到", forwarded.lines.read_line(&mut line))
        .await
        .expect("读得到");
    assert_eq!(line, format!("{chinese}\n"));
    let long = format!(r#"{{"x":"{}"}}"#, "字".repeat(200_000));
    forwarded
        .write
        .write_all(format!("{long}\n").as_bytes())
        .await
        .expect("写得进");
    match next(&mut ws).await {
        Some(Message::Text(text)) => assert_eq!(text.as_str(), long, "一行去掉换行是一帧"),
        other => panic!("{other:?}"),
    }
    // 核心那头断了：关 1012。
    drop(forwarded);
    assert_eq!(close_code(next(&mut ws).await), Some(CloseCode::Restart));
    bridge.stop().await.expect("停得下");
}

#[tokio::test]
async fn an_unreachable_core_is_reported_then_closed() {
    let site = Site::new();
    let bridge = site.bridge().await;
    site.core.close();
    let port = bridge.web;
    let mut ws = browser(port, Some(&format!("http://127.0.0.1:{port}")))
        .await
        .expect("接了");
    match next(&mut ws).await {
        Some(Message::Text(text)) => {
            let notice: serde_json::Value = serde_json::from_str(text.as_str()).expect("是 JSON");
            assert_eq!(notice["method"], "web.error", "{notice}");
            assert!(notice["params"]["message"].is_string());
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(close_code(next(&mut ws).await), Some(CloseCode::Normal));
}

#[tokio::test]
async fn a_taken_web_port_is_named() {
    let site = Site::new();
    let taken = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("挑得到");
    let port = taken.local_addr().expect("有地址").port();
    let serve = serve(
        site.root.clone(),
        Settings {
            web: port,
            ..settings()
        },
    );
    let ran = within("起不来", run(serve, |_| {}, std::future::pending())).await;
    assert_eq!(ran, Err(Failure::WebPortInUse(port)));
}

#[tokio::test]
async fn the_page_words_come_in_the_bridges_language() {
    let site = Site::new();
    let bridge = start(serve(site.root.clone(), settings())).await;
    let words = get(bridge.web, "/human", &[]).await;
    assert_eq!(words.status, 200);
    assert_eq!(words.header("content-type"), Some("application/json"));
    assert_eq!(words.header("x-content-type-options"), Some("nosniff"));
    let words = words.json();
    assert_eq!(words["language"], "zh", "照握手回的语言");
    let said = words["said"].as_object().expect("是表");
    assert!(!said.is_empty());
    assert!(
        said.keys()
            .all(|key| key.starts_with("software/onebot/web/")),
        "只给页面的字：{said:?}"
    );
    let host = format!("127.0.0.1:{}", bridge.web);
    assert_eq!(
        request(bridge.web, "POST", "/human", &host, &[])
            .await
            .status,
        405
    );
    bridge.stop().await.expect("停得下");
}
