//! `miyu-web serve`（`web-ui.md`「怎么走」第一条）：单实例、只听 `127.0.0.1`、写 `run/web` 和那一行、空闲退出；每个
//! 请求先核对 Host，`/ws` 交给 `ws`，`/media` 交给 `media`（施工 W-10），别的当页面文件（`pages`）。
//!
//! 1. 先拿 `run/web.lock`，拿不到写 `running` 走。
//! 2. 听端口：被占了写 `error port <端口> in use`（`open` 认这个前缀，照人的语言说），别的起不来写 `error <原因>`。
//! 3. 地址写进 `run/web`（先写临时文件再改名），写一行 `ready`。
//! 4. 没有 WebSocket 连着、没有 `/media` 在给，连续空闲到点就退出；收到停的信号也退出。退出时先删 `run/web`、再放锁。

use std::convert::Infallible;
use std::fs::{File, OpenOptions, TryLockError};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::header::{self, HeaderValue};
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

use miyu_ipc::Ready;
use miyu_store::root::DataRoot;

use crate::media::Media;
use crate::settings::Settings;
use crate::{TARGET, media, pages, ws};

/// 单实例的锁，在 `run/` 里。
pub const LOCK: &str = "web.lock";

/// 网页的地址，在 `run/` 里：一行，`http://127.0.0.1:<端口>`。
pub const ADDRESS: &str = "web";

/// 回应的正文：整段的，或者 `/media` 一块块给的。
pub(crate) type Body = http_body_util::combinators::BoxBody<Bytes, std::io::Error>;

/// 拉起核心的命令：主程序 `miyu` 加 `core`。
pub type CoreCommand = Arc<dyn Fn() -> Command + Send + Sync>;

/// 起一个网页软件要的。
pub struct Serve {
    /// 数据根。
    pub root: DataRoot,
    /// 听哪个端口：`0` 是系统挑一个空的。
    pub port: u16,
    /// 页面目录。
    pub pages: PathBuf,
    /// `web.json`。
    pub settings: Settings,
    /// 核心没在跑时怎么拉起来。
    pub core: CoreCommand,
}

/// 跑着的这一个：各个连接共用。
pub(crate) struct Site {
    pub(crate) root: DataRoot,
    pub(crate) port: u16,
    pub(crate) pages: PathBuf,
    pub(crate) settings: Settings,
    pub(crate) core: CoreCommand,
    /// `/media` 的票据、连着的核心。
    pub(crate) media: Media,
    /// 连着几个 WebSocket、几个 `/media` 在给。
    active: AtomicUsize,
    /// 最后一个走的时候。
    quiet_since: Mutex<Instant>,
}

/// 连着的一个：数着，走的时候减掉、记下时刻。
pub(crate) struct Busy(Arc<Site>);

impl Drop for Busy {
    fn drop(&mut self) {
        *self
            .0
            .quiet_since
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Instant::now();
        self.0.active.fetch_sub(1, Ordering::AcqRel);
    }
}

impl Site {
    /// 数上一个。
    pub(crate) fn busy(self: &Arc<Site>) -> Busy {
        self.active.fetch_add(1, Ordering::AcqRel);
        Busy(Arc::clone(self))
    }

    /// 空闲了多久；有连着的是空的。
    fn idle_for(&self) -> Option<Duration> {
        if self.active.load(Ordering::Acquire) > 0 {
            return None;
        }
        Some(
            self.quiet_since
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .elapsed(),
        )
    }

    /// Host、Origin 认的三种写法（不带协议）。
    pub(crate) fn hosts(&self) -> [String; 3] {
        let port = self.port;
        [
            format!("127.0.0.1:{port}"),
            format!("localhost:{port}"),
            format!("[::1]:{port}"),
        ]
    }

    /// Host 对不对：只认三种写法，`localhost` 不分大小写。
    fn host_allowed(&self, host: Option<&str>) -> bool {
        host.is_some_and(|host| {
            self.hosts()
                .iter()
                .any(|allowed| allowed.eq_ignore_ascii_case(host))
        })
    }
}

/// 跑起来，直到空闲到点、收到停的信号。`said` 交那一行：起来了、已经有一个在跑、起不来。
///
/// # Errors
///
/// 起不来（锁、端口、写 `run/web`）：原话已经交给了 `said`。
pub async fn run(serve: Serve, said: impl FnOnce(Ready)) -> Result<(), String> {
    let run = serve.root.run();
    let _lock = match lock(&run.join(LOCK)) {
        Ok(Some(lock)) => lock,
        Ok(None) => {
            said(Ready::Running);
            return Ok(());
        }
        Err(error) => {
            said(Ready::Failed(error.clone()));
            return Err(error);
        }
    };
    let listener = match TcpListener::bind(("127.0.0.1", serve.port)).await {
        Ok(listener) => listener,
        Err(error) => {
            let reason = match error.kind() {
                std::io::ErrorKind::AddrInUse => format!("port {} in use", serve.port),
                _ => error.to_string(),
            };
            said(Ready::Failed(reason.clone()));
            return Err(reason);
        }
    };
    let port = listener
        .local_addr()
        .map_err(|error| error.to_string())?
        .port();
    let url = format!("http://127.0.0.1:{port}");
    if let Err(error) = write_address(&run, &url) {
        said(Ready::Failed(error.clone()));
        return Err(error);
    }
    said(Ready::Ready);
    tracing::info!(target: TARGET, url = %url, "listening");
    let media = Media::new(&serve.settings);
    let site = Arc::new(Site {
        root: serve.root,
        port,
        pages: serve.pages,
        settings: serve.settings,
        core: serve.core,
        media,
        active: AtomicUsize::new(0),
        quiet_since: Mutex::new(Instant::now()),
    });
    let reason = accept(&listener, &site).await;
    remove_address(&run);
    tracing::info!(target: TARGET, reason, "stopped");
    Ok(())
}

/// 接连接，直到空闲到点、收到停的信号：交回为什么停。
async fn accept(listener: &TcpListener, site: &Arc<Site>) -> &'static str {
    let idle = site.settings.idle();
    let mut tick =
        tokio::time::interval((idle / 4).clamp(Duration::from_millis(10), Duration::from_secs(1)));
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                if let Ok((stream, _)) = accepted {
                    let site = Arc::clone(site);
                    tokio::spawn(async move {
                        let service = hyper::service::service_fn(move |request| {
                            handle(request, Arc::clone(&site))
                        });
                        let served = hyper::server::conn::http1::Builder::new()
                            .serve_connection(TokioIo::new(stream), service)
                            .with_upgrades()
                            .await;
                        if let Err(error) = served {
                            tracing::debug!(target: TARGET, error = %error, "connection ended");
                        }
                    });
                }
            }
            _ = tick.tick() => {
                site.media.sweep();
                if site.idle_for().is_some_and(|quiet| quiet >= idle) {
                    return "idle";
                }
            }
            () = stopped() => return "signal",
        }
    }
}

/// 一个请求：先核对 Host；`/ws` 交给 WebSocket；`/media` 换票据、照票据给；`GET`、`HEAD` 当页面文件；别的方法 405。
async fn handle(request: Request<Incoming>, site: Arc<Site>) -> Result<Response<Body>, Infallible> {
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok());
    if !site.host_allowed(host) {
        let origin = request
            .headers()
            .get(header::ORIGIN)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        tracing::warn!(target: TARGET, host = host.unwrap_or_default(), origin, "rejected");
        return Ok(empty(StatusCode::FORBIDDEN));
    }
    let path = request.uri().path();
    if path == "/ws" {
        return Ok(ws::accept(request, site));
    }
    if path == "/media" {
        return Ok(media::post(request, site).await);
    }
    if path.starts_with("/media/") {
        return Ok(media::get(request, site).await);
    }
    if request.method() != Method::GET && request.method() != Method::HEAD {
        return Ok(empty(StatusCode::METHOD_NOT_ALLOWED));
    }
    let Some(file) = pages::find(&site.pages, request.uri().path()) else {
        return Ok(empty(StatusCode::NOT_FOUND));
    };
    let Ok(bytes) = tokio::fs::read(&file).await else {
        return Ok(empty(StatusCode::NOT_FOUND));
    };
    let body = match request.method() == Method::HEAD {
        true => Bytes::new(),
        false => Bytes::from(bytes),
    };
    let mut response = Response::new(full(body));
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, value(site.settings.type_of(&file)));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    headers.insert(header::CONTENT_SECURITY_POLICY, value(&site.settings.csp));
    secure(headers);
    Ok(response)
}

/// 整段的正文。
pub(crate) fn full(bytes: impl Into<Bytes>) -> Body {
    Full::new(bytes.into())
        .map_err(|never| match never {})
        .boxed()
}

/// 没有内容的回应，带两个一律有的头。
pub(crate) fn empty(status: StatusCode) -> Response<Body> {
    let mut response = Response::new(full(Bytes::new()));
    *response.status_mut() = status;
    secure(response.headers_mut());
    response
}

/// 一律带的：不猜类型、不带 Referer。从来不设 cookie（「起草时定的」第 13 条）。
pub(crate) fn secure(headers: &mut hyper::HeaderMap) {
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
}

/// 字写成头的值；写不成的（有控制字符）是空的。
fn value(text: &str) -> HeaderValue {
    HeaderValue::from_str(text).unwrap_or_else(|_| HeaderValue::from_static(""))
}

/// 拿单实例的锁：拿不到（有一个在跑）是空的。
fn lock(path: &std::path::Path) -> Result<Option<File>, String> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|error| format!("{} not opened: {error}", path.display()))?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(TryLockError::WouldBlock) => Ok(None),
        Err(TryLockError::Error(error)) => Err(format!("{} not locked: {error}", path.display())),
    }
}

/// 有没有一个网页软件在跑：锁有人拿着。
pub fn running(root: &DataRoot) -> bool {
    matches!(lock(&root.run().join(LOCK)), Ok(None))
}

/// 地址写进 `run/web`：先写临时文件再改名。
fn write_address(run: &std::path::Path, url: &str) -> Result<(), String> {
    let temp = run.join(format!("{ADDRESS}.{}.tmp", std::process::id()));
    std::fs::write(&temp, format!("{url}\n"))
        .and_then(|()| std::fs::rename(&temp, run.join(ADDRESS)))
        .map_err(|error| format!("run/{ADDRESS} not written: {error}"))
}

/// 删 `run/web`：没有的不要紧。
fn remove_address(run: &std::path::Path) {
    if let Err(error) = std::fs::remove_file(run.join(ADDRESS))
        && error.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(target: TARGET, error = %error, "address not removed");
    }
}

/// 读 `run/web` 里的地址：没有的、读不了的是空的。
pub fn address(root: &DataRoot) -> Option<String> {
    let text = std::fs::read_to_string(root.run().join(ADDRESS)).ok()?;
    let url = text.trim();
    (!url.is_empty()).then(|| url.to_string())
}

/// 收到停的信号：Ctrl+C，Unix 上另有 SIGTERM。
async fn stopped() {
    #[cfg(unix)]
    {
        let Ok(mut term) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        else {
            return std::future::pending().await;
        };
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        if tokio::signal::ctrl_c().await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}
