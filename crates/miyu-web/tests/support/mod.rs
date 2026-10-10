//! 网页软件的测试共用的（施工 W-9）：临时的数据根、起一个 `serve`、手写的 HTTP 请求、WebSocket 客户端、核心的替身（`miyu-ipc`
//! 的监听：网页软件只认本机传输，不认是不是真核心）。

#![allow(dead_code, reason = "几个测试各用其中一部分")]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use miyu_ipc::{Dirs, Listener, Ready};
use miyu_store::env::{Env, Platform};
use miyu_store::root::DataRoot;
use miyu_web::serve::{CoreCommand, Serve, run};
use miyu_web::settings::Settings;

/// 一个用完就删的临时目录：数据根在 `home/`，页面在 `pages/`。
pub struct Home {
    pub dir: PathBuf,
    pub root: DataRoot,
    pub pages: PathBuf,
}

impl Home {
    #[expect(clippy::let_underscore_must_use, reason = "上一次留下的，没有就没有")]
    pub fn new() -> Home {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("miyu-web-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let env = Env {
            platform: Platform::current(),
            miyu_home: Some(dir.join("home").into_os_string()),
            home: None,
            xdg_cache_home: None,
            local_app_data: None,
            miyu_resources: None,
            exe: None,
        };
        let root = DataRoot::locate(&env).expect("绝对路径");
        root.prepare().expect("建得了骨架");
        let pages = dir.join("pages");
        std::fs::create_dir_all(&pages).expect("建得了");
        Home { dir, root, pages }
    }

    /// 写一份页面文件。
    pub fn page(&self, relative: &str, text: &str) {
        let path = self.pages.join(relative);
        std::fs::create_dir_all(path.parent().expect("有上级")).expect("建得了");
        std::fs::write(path, text).expect("写得进");
    }
}

impl Drop for Home {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// 测试用的设置：端口 0，空闲 `idle_seconds` 秒。
pub fn settings(idle_seconds: u64) -> Settings {
    let mut types = BTreeMap::new();
    types.insert("html".to_string(), "text/html; charset=utf-8".to_string());
    types.insert(
        "js".to_string(),
        "text/javascript; charset=utf-8".to_string(),
    );
    types.insert("css".to_string(), "text/css; charset=utf-8".to_string());
    types.insert("png".to_string(), "image/png".to_string());
    types.insert("mp4".to_string(), "video/mp4".to_string());
    Settings {
        port: 0,
        idle_seconds,
        csp: "default-src 'self'; connect-src 'self'; frame-ancestors 'none'".to_string(),
        backstage_csp:
            "sandbox allow-scripts allow-forms; connect-src 'none'; frame-ancestors 'self'"
                .to_string(),
        types,
        ticket_idle_seconds: 43_200,
        most_tickets: 4096,
    }
}

/// 拉不起来的核心：命令不存在。
pub fn no_core() -> CoreCommand {
    Arc::new(|| std::process::Command::new("/nonexistent/miyu-core-for-tests"))
}

/// 起一个 `serve`：交回那一行、端口（起来了的）和跑着的任务。
pub async fn start(
    home: &Home,
    port: u16,
    idle_seconds: u64,
) -> (Ready, u16, JoinHandle<Result<(), String>>) {
    start_with(home, port, settings(idle_seconds)).await
}

/// 同 [`start`]，设置照给的。
pub async fn start_with(
    home: &Home,
    port: u16,
    settings: Settings,
) -> (Ready, u16, JoinHandle<Result<(), String>>) {
    let (said, heard) = oneshot::channel();
    let serve = Serve {
        root: home.root.clone(),
        port,
        pages: home.pages.clone(),
        settings,
        core: no_core(),
    };
    let running = tokio::spawn(run(serve, move |ready| {
        if said.send(ready).is_err() {
            // 等的那头已经不在了。
        }
    }));
    let ready = tokio::time::timeout(Duration::from_secs(10), heard)
        .await
        .expect("十秒内说了")
        .expect("说了");
    let port = miyu_web::serve::address(&home.root)
        .and_then(|url| url.rsplit(':').next().and_then(|port| port.parse().ok()))
        .unwrap_or(0);
    (ready, port, running)
}

/// 一个 HTTP 回应：状态码、头（小写的名字）、正文。
pub struct Answer {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Answer {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

/// 手写一个请求：`method` `path`，Host 是 `host`，另带 `extra` 几行头。
pub async fn request(
    port: u16,
    method: &str,
    path: &str,
    host: &str,
    extra: &[(&str, &str)],
) -> Answer {
    send(port, method, path, host, extra, b"").await
}

/// 同 [`request`]，带正文 `body`（有正文的写 `Content-Length`）。
pub async fn send(
    port: u16,
    method: &str,
    path: &str,
    host: &str,
    extra: &[(&str, &str)],
    body: &[u8],
) -> Answer {
    let mut stream = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("连得上");
    let mut text = format!("{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n");
    for (name, value) in extra {
        text.push_str(&format!("{name}: {value}\r\n"));
    }
    if !body.is_empty() {
        text.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    text.push_str("\r\n");
    let mut bytes = text.into_bytes();
    bytes.extend_from_slice(body);
    stream.write_all(&bytes).await.expect("写得进");
    let mut bytes = Vec::new();
    tokio::time::timeout(Duration::from_secs(10), stream.read_to_end(&mut bytes))
        .await
        .expect("十秒内回了")
        .expect("读得到");
    let split = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("有头");
    let head = String::from_utf8_lossy(&bytes[..split]).to_string();
    let mut lines = head.lines();
    let status = lines
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .and_then(|code| code.parse().ok())
        .expect("有状态码");
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_string()))
        .collect();
    Answer {
        status,
        headers,
        body: bytes[split + 4..].to_vec(),
    }
}

/// 起一个核心的替身：交回它的监听（照真核心的位置写 `run/socket`、`run/token`）。
pub fn fake_core(home: &Home) -> Listener {
    miyu_ipc::open(&home.root, &Dirs::current())
        .expect("起得来")
        .listener
}

/// 等一件事最多十秒。
pub async fn within<T>(what: &str, future: impl std::future::Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(10), future)
        .await
        .unwrap_or_else(|_| panic!("十秒内没等到{what}"))
}

/// 这个路径在不在。
pub fn exists(path: &Path) -> bool {
    path.exists()
}
