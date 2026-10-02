//! 几个测试共用的：本机回环上照路径回的假服务器（也当假代理用：代理收到的请求行写的是整个地址），一个用完就删的
//! blob 目录，造 `LinkPreview` 的几种办法。

#![allow(dead_code, reason = "几个测试各用其中一部分")]

use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use miyu_net::testkit::Testing;
use miyu_net::{Card, LinkPreview, Preview, Why};
use miyu_store::blob::Blobs;

/// 一份 PNG 的开头：魔数认得出就算图。
pub const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR";

/// 一份回应。
#[derive(Debug, Clone)]
pub struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    /// 写 `Content-Length`；不写的，身子写完关连接。
    sized: bool,
    /// 身子写完不关，停住不动，直到对方断开。
    stall: bool,
}

impl Reply {
    /// 200，`text/html`。
    pub fn html(text: &str) -> Reply {
        Reply::bytes("text/html; charset=utf-8", text.as_bytes())
    }

    /// 200，照给的类型；给的是空的就不写 `Content-Type`。
    pub fn bytes(content_type: &str, body: &[u8]) -> Reply {
        let headers = match content_type {
            "" => Vec::new(),
            given => vec![("Content-Type".to_string(), given.to_string())],
        };
        Reply {
            status: 200,
            headers,
            body: body.to_vec(),
            sized: true,
            stall: false,
        }
    }

    /// 302，跳到 `location`。
    pub fn redirect(location: &str) -> Reply {
        Reply {
            status: 302,
            headers: vec![("Location".to_string(), location.to_string())],
            body: Vec::new(),
            sized: true,
            stall: false,
        }
    }

    /// 只有一个状态码。
    pub fn status(status: u16) -> Reply {
        Reply {
            status,
            headers: Vec::new(),
            body: Vec::new(),
            sized: true,
            stall: false,
        }
    }

    /// 不写 `Content-Length`：身子写完关连接。
    pub fn without_length(mut self) -> Reply {
        self.sized = false;
        self
    }

    /// 身子写完不关、不说多长，停住不动：读的一方不自己停就一直等到时限。
    pub fn stalled(mut self) -> Reply {
        self.sized = false;
        self.stall = true;
        self
    }
}

/// 收到的一个请求：请求行里的地址（直连的是路径，经代理的是整个地址）、`Host`、`User-Agent`。
#[derive(Debug, Clone)]
pub struct Seen {
    pub target: String,
    pub host: String,
    pub user_agent: String,
}

/// 跑着的假服务器：照请求行里的地址回，没有的回 404。收到的请求都记下来。
pub struct Site {
    pub port: u16,
    seen: Arc<Mutex<Vec<Seen>>>,
}

impl Site {
    /// 在本机回环上起一个，端口由系统挑。
    pub async fn start(routes: Vec<(String, Reply)>) -> Site {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("绑得上本机回环");
        let port = listener.local_addr().expect("有地址").port();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        let routes = Arc::new(routes);
        tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                let (log, routes) = (Arc::clone(&log), Arc::clone(&routes));
                tokio::spawn(async move { serve(socket, &routes, &log).await });
            }
        });
        Site { port, seen }
    }

    /// 这台服务器上的一个地址，写 IP。
    pub fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }

    /// 收到的请求，照先后。
    pub fn seen(&self) -> Vec<Seen> {
        self.seen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// 请求行里写着 `target` 的收到过几次。
    pub fn hits(&self, target: &str) -> usize {
        self.seen()
            .iter()
            .filter(|seen| seen.target == target)
            .count()
    }
}

/// 一个连接：读请求头，照地址回。
async fn serve(mut socket: TcpStream, routes: &[(String, Reply)], log: &Mutex<Vec<Seen>>) {
    let mut data = Vec::new();
    let mut buffer = [0u8; 4096];
    while !data.windows(4).any(|window| window == b"\r\n\r\n") {
        match socket.read(&mut buffer).await {
            Ok(0) | Err(_) => return,
            Ok(read) => data.extend_from_slice(&buffer[..read]),
        }
    }
    let head = String::from_utf8_lossy(&data).into_owned();
    let mut lines = head.split("\r\n");
    let target = lines
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .unwrap_or_default()
        .to_string();
    let header = |name: &str| {
        head.split("\r\n")
            .filter_map(|line| line.split_once(':'))
            .find(|(key, _)| key.trim().eq_ignore_ascii_case(name))
            .map(|(_, value)| value.trim().to_string())
            .unwrap_or_default()
    };
    log.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(Seen {
            target: target.clone(),
            host: header("host"),
            user_agent: header("user-agent"),
        });
    let reply = routes
        .iter()
        .find(|(path, _)| *path == target)
        .map_or_else(|| Reply::status(404), |(_, reply)| reply.clone());
    let mut out = format!("HTTP/1.1 {} X\r\n", reply.status);
    for (name, value) in &reply.headers {
        out.push_str(&format!("{name}: {value}\r\n"));
    }
    if reply.sized {
        out.push_str(&format!("Content-Length: {}\r\n", reply.body.len()));
    }
    out.push_str("Connection: close\r\n\r\n");
    if socket.write_all(out.as_bytes()).await.is_err()
        || socket.write_all(&reply.body).await.is_err()
        || socket.flush().await.is_err()
    {
        return;
    }
    if reply.stall {
        // 停住，直到对方断开：读到 0 个字节就是断开了。
        while let Ok(read) = socket.read(&mut buffer).await {
            if read == 0 {
                return;
            }
        }
        return;
    }
    let _closed = socket.shutdown().await;
}

/// 一个用完就删的 blob 目录。
pub struct Store {
    dir: PathBuf,
    pub blobs: Blobs,
}

impl Store {
    pub fn new() -> Store {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("miyu-net-{}-{n}", std::process::id()));
        Store {
            blobs: Blobs::new(dir.join("blobs")),
            dir,
        }
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        let _removed = std::fs::remove_dir_all(&self.dir);
    }
}

/// 源码树里的资源目录。
pub fn resources() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources")
}

/// 一个地址。
pub fn ip(text: &str) -> IpAddr {
    text.parse().expect(text)
}

/// 测试的口子：回环当公网，`site.test` 解析到回环，`inner.test` 解析到内网；不走代理。
pub fn local() -> Testing {
    Testing {
        loopback_public: true,
        hosts: vec![
            ("site.test".to_string(), ip("127.0.0.1")),
            ("inner.test".to_string(), ip("10.0.0.1")),
        ],
        proxy: None,
        no_proxy: String::new(),
    }
}

/// 照出厂的规矩、打开 `testing` 这个口子的一份。
pub fn previewer(store: &Store, testing: Testing) -> LinkPreview {
    LinkPreview::new(&resources(), store.blobs.clone()).testing(testing)
}

/// 抓一次，等最多一分钟（CI 的慢机器留够）。
pub async fn preview(links: &LinkPreview, url: &str) -> Preview {
    tokio::time::timeout(Duration::from_secs(60), links.preview(url))
        .await
        .unwrap_or_else(|_| panic!("一分钟内没抓完 {url}"))
        .expect("link_preview.json 读得懂")
}

/// 抓一次，应该做成卡片。
pub async fn card(links: &LinkPreview, url: &str) -> Card {
    match preview(links, url).await {
        Preview::Card(card) => card,
        Preview::Miss(why) => panic!("{url} 没做成卡片：{why:?}"),
    }
}

/// 抓一次，应该没做成。
pub async fn miss(links: &LinkPreview, url: &str) -> Why {
    match preview(links, url).await {
        Preview::Miss(why) => why,
        Preview::Card(card) => panic!("{url} 不该做成卡片：{card:?}"),
    }
}

/// 一页带着标题的 HTML，`head` 里另加 `extra`。
pub fn page(title: &str, extra: &str) -> String {
    format!("<html><head><title>{title}</title>{extra}</head><body>hello</body></html>")
}
