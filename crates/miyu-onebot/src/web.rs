//! 桥的 WebUI（`onebot.md` 第二条「怎么走」第 1 条，施工 O-16）：只听本机的 `onebot.web`，每个请求先核对 Host，再分给谁：
//!
//! - `/ws`：原样转给核心，和网页软件的一样（`miyu_webserve::ws`）。页面自己说核心协议，登录由核心验。
//! - `/status`：只读，要登录令牌（`status`）。
//! - `/token`：令牌的值，要登录令牌（`token`，施工 O-16 补二）。
//! - `/apply`：照桥手里最新的配置换端口，要登录令牌（`apply`，补二；O-20 起推送来的端口变化也照它换）。
//! - `/human`：页面登录以前要的字（`human`）。登录以后页面照 `human.get` 取。
//!
//! 要登录令牌的三个照同一套核对（`login`）。
//! - 别的当页面文件，在资源目录的 [`PAGES`] 里（`miyu_webserve::pages`），类型表、内容安全策略照 `bridge.json` 的 `web`。
//!
//! 给页面、核对 Host 和 Origin、`/ws` 照转都是和网页软件共用的 `miyu-webserve`（`webserve.md`）。WebUI 不数忙：桥不空闲退出。

mod apply;
mod checked;
mod human;
mod login;
mod status;
#[cfg(test)]
mod tests;
mod token;

use std::convert::Infallible;
use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};

use hyper::body::Incoming;
use hyper::header::{self, HeaderValue};
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde_json::Value;
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;
use miyu_webserve::respond::{Body, empty, full, secure};
use miyu_webserve::{CoreCommand, Site};

use crate::TARGET;
use crate::current::Current;
use crate::listen::bots::Bots;
use crate::serve::Swap;
use crate::tuning::Tuning;
pub(crate) use apply::latest;
pub(crate) use checked::Checked;

/// 页面在资源目录里的位置（`onebot.md` 第二条「对外的样子」）。
pub const PAGES: &str = "software/onebot/web";

/// 跑着的 WebUI，各个连接共用。
pub(crate) struct Web {
    /// 数据根：照它找核心的套接字。
    pub(crate) root: DataRoot,
    /// WebUI 实际听的端口：Host、Origin 照它核对。`/apply` 换了的照换了的。
    pub(crate) port: AtomicU16,
    /// NapCat 连进来的端口，实际听的那一个（`/status` 的 `listen`）。`/apply` 换了的照换了的。
    pub(crate) listen: AtomicU16,
    /// 核心没在跑时怎么拉起来（`/ws`、`/status` 连核心）。
    pub(crate) core: CoreCommand,
    /// 资源目录：页面、页面的字。
    pub(crate) resources: ResourceRoot,
    /// 桥说话的语言（握手回的）：`/human` 照它给字。
    pub(crate) language: String,
    /// 桥自己的数：WebUI 的类型表、内容安全策略、记多久；问核心等多久。
    pub(crate) tuning: Tuning,
    /// 连着的机器人号：`/status` 照它说 NapCat 连没连上。
    pub(crate) bots: Arc<Bots>,
    /// 桥手里最新的配置（和 NapCat 的监听共用，施工 O-20）。
    pub(crate) current: Arc<Current>,
    /// 验过的登录令牌（只记哈希）。
    pub(crate) checked: Checked,
    /// 上一次照的两个端口（NapCat 的、WebUI 的），照配置里写的（写 0 的就是 0）：换端口（`/apply`、推送来的）照它看变了
    /// 没有。锁着它办：同时来的几个一个一个办。
    pub(crate) applied: tokio::sync::Mutex<(u16, u16)>,
    /// `/apply` 开好的新监听交给 `serve` 换上。
    pub(crate) swap: mpsc::UnboundedSender<Swap>,
}

/// `/ws` 照它连核心；Host、Origin 照它的端口核对。不数忙。
impl Site for Web {
    type Busy = ();

    fn root(&self) -> &DataRoot {
        &self.root
    }

    fn port(&self) -> u16 {
        self.port.load(Ordering::Relaxed)
    }

    fn core(&self) -> &CoreCommand {
        &self.core
    }

    fn busy(self: &Arc<Self>) {}
}

/// 接一个 TCP 连接，当 HTTP 读，直到断开（升级成 WebSocket 的，交给 `miyu_webserve::ws` 在别的任务里转）。
pub(crate) async fn accept(stream: TcpStream, web: Arc<Web>) {
    let service = hyper::service::service_fn(move |request| handle(request, Arc::clone(&web)));
    let served = hyper::server::conn::http1::Builder::new()
        .serve_connection(TokioIo::new(stream), service)
        .with_upgrades()
        .await;
    if let Err(error) = served {
        tracing::debug!(target: TARGET, error = %error, "web connection ended");
    }
}

/// 一个请求：先核对 Host（不对的 403，记一行）；再照路径分。
async fn handle(request: Request<Incoming>, web: Arc<Web>) -> Result<Response<Body>, Infallible> {
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok());
    if !web.host_allowed(host) {
        let origin = request
            .headers()
            .get(header::ORIGIN)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        tracing::warn!(target: TARGET, host = host.unwrap_or_default(), origin, "web rejected");
        return Ok(empty(StatusCode::FORBIDDEN));
    }
    let response = match request.uri().path() {
        "/ws" => miyu_webserve::ws::accept(request, web),
        "/status" => status::get(&request, &web).await,
        "/token" => token::get(&request, &web).await,
        "/apply" => apply::post(&request, &web).await,
        "/human" => human::get(&request, &web).await,
        _ => {
            let rules = &web.tuning.web;
            let pages = web.resources.path().join(PAGES);
            miyu_webserve::pages::serve(&request, &pages, &rules.csp, &rules.types).await
        }
    };
    Ok(response)
}

/// 给页面的 JSON（`/status`、`/token`、`/apply`）：状态码 `status`，不让缓存（里面有令牌、会变），带安全响应头。
fn reply(status: StatusCode, value: &Value) -> Response<Body> {
    let mut response = Response::new(full(value.to_string()));
    *response.status_mut() = status;
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    secure(headers);
    response
}
