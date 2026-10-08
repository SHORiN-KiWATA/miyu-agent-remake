//! `miyu-onebot serve`（`onebot.md` 第一条「怎么走」第 1、2、11 条）：连核心、握手，再开监听，把几样接起来，跑到停的信号
//! 或者核心断开。
//!
//! 1. 用本机套接字连核心，没在跑就拉起（`core`）。连不上：[`Failure::Core`]。
//! 2. 只听 `127.0.0.1` 的 `onebot.listen`。被占了：[`Failure::PortInUse`]。再听 `127.0.0.1` 的 `onebot.web`（WebUI，施工
//!    O-16，第二条「怎么走」第 1 条）。被占了：[`Failure::WebPortInUse`]。听上了各说一行（[`Notice::Listening`] 带握手回的
//!    语言，[`Notice::Web`]）。令牌没设的两个也照开，NapCat 连进来一律 401，人在 WebUI 里生成令牌，NapCat 下一次连就通
//!    （O-16 补、补二，18 第三节「还没配好就 `start`」）：在两句中间说 [`Notice::NoToken`]，记一行运行日志。
//! 3. 每个 TCP 连接一个任务（NapCat 的交给 `listen`，WebUI 的交给 `web`），跟核心的那一头一个任务（`core/route.rs`），都在
//!    一组里：停下时一起停。WebUI 的 `/apply` 开好的新监听经 `Swap` 送过来，换掉旧的，旧的随之关掉；已经接进来的连接不断
//!    （O-16 补二，第二条「施工时定的」第 19 条）。
//! 4. 核心断了：[`Failure::CoreGone`]，桥退出（「施工时定的」第 5 条：9-4 以后核心拉起它，桥里不另写一套重连）。跟核心的
//!    那一头、发回话的任务崩了：[`Failure::Crashed`]，也退（第 14 条）。NapCat 断了不退。

use std::sync::Arc;
use std::sync::atomic::{AtomicU16, AtomicU64};

use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::task::JoinSet;

use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

use crate::TARGET;
use crate::core::Core;
use crate::core::route::Route;
use crate::current::Current;
use crate::listen::bots::Bots;
use crate::listen::{self, Gate};
use crate::settings::{Reload, Settings};
use crate::tuning::Tuning;
use crate::web::{self, Web};

pub use miyu_webserve::CoreCommand;

/// 起一个桥要的。
pub struct Serve {
    /// 数据根。
    pub root: DataRoot,
    /// 起来时读到的两个端口、令牌（`settings`）。
    pub settings: Settings,
    /// 核心没在跑时怎么拉起来。
    pub core: CoreCommand,
    /// 系统的语言：握手时报给核心，`ui.language` 是 `auto` 的照它定说话的语言。
    pub locale: Option<String>,
    /// 桥自己的数（`bridge.json`）。
    pub tuning: Tuning,
    /// 资源目录：WebUI 的页面（[`web::PAGES`]）、页面的字在这里。
    pub resources: ResourceRoot,
    /// 重读配置：NapCat 的令牌对不上时、WebUI 的 `/status`、`/token`、`/apply` 照它读（`crate::current`）。
    pub reload: Reload,
}

/// 说给人听的（「样子」）：怎么说照 `texts`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// 听上了：实际的端口（设的是 0 的，系统挑的那一个），握手回的语言。
    Listening {
        /// 实际听的端口。
        port: u16,
        /// 握手回的语言：`zh`、`en`、`ja` 之一。
        language: String,
    },
    /// NapCat 连上了：机器人的号，连进来时没报的是空的。
    Connected {
        /// 机器人的号。
        bot: Option<i64>,
    },
    /// NapCat 断开了。
    Disconnected {
        /// 机器人的号：一直没认出来的是空的。
        bot: Option<i64>,
    },
    /// WebUI 听上了：实际的端口。
    Web {
        /// 实际听的端口。
        port: u16,
    },
    /// 令牌没设：NapCat 连进来会被拒，说怎么设。跟在 [`Notice::Listening`] 后面说，语言照它带的。
    NoToken,
}

/// `/apply` 开好的新监听（施工 O-16 补二）：`run` 收到就换掉旧的那一个。
pub(crate) enum Swap {
    /// NapCat 的。
    Napcat(TcpListener),
    /// WebUI 的。
    Web(TcpListener),
}

/// 起不来、跑着跑着停了的（「出错」，退出码都是 1）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// 连不上核心、拉不起来、握手被拒：原因。
    Core(String),
    /// 核心断了。
    CoreGone,
    /// NapCat 的端口被占了。
    PortInUse(u16),
    /// WebUI 的端口被占了。
    WebPortInUse(u16),
    /// 别的原因起不来（听不了、起不了运行时、读不了数据根或 `bridge.json`）：原话。
    Start(String),
    /// 跟核心的那一头崩了、发回话的任务崩了（是 bug）：原话。
    Crashed(String),
}

/// 跑起来，直到 `stop` 到了（停的信号）。说给人听的交给 `tell`。
///
/// # Errors
///
/// 连不上核心、两个端口被占、听不了；跑着跑着核心断了。
pub async fn run(
    serve: Serve,
    tell: impl Fn(Notice) + Send + Sync + 'static,
    stop: impl Future<Output = ()>,
) -> Result<(), Failure> {
    let core = Core::connect(
        &serve.root,
        Arc::clone(&serve.core),
        serve.locale.as_deref(),
    )
    .await?;
    let (mut napcat, listen) = bind(serve.settings.port, Failure::PortInUse).await?;
    let (mut pages, web_port) = bind(serve.settings.web, Failure::WebPortInUse).await?;
    let tell: Arc<dyn Fn(Notice) + Send + Sync> = Arc::new(tell);
    tracing::info!(target: TARGET, port = listen, "listening");
    tell(Notice::Listening {
        port: listen,
        language: core.language.clone(),
    });
    if serve.settings.token.secret().is_none() {
        tracing::warn!(target: TARGET, "no token yet");
        tell(Notice::NoToken);
    }
    tracing::info!(target: TARGET, port = web_port, "web listening");
    tell(Notice::Web { port: web_port });
    let bots = Arc::new(Bots::default());
    let (inbound, received) = mpsc::channel(serve.tuning.inbound_queue);
    let retry = serve.tuning.accept_retry();
    let current = Arc::new(Current::new(
        serve.reload,
        serve.settings.token.clone(),
        serve.tuning.reload(),
    ));
    let (swap, mut swaps) = mpsc::unbounded_channel();
    let web = Arc::new(Web {
        root: serve.root.clone(),
        port: AtomicU16::new(web_port),
        listen: AtomicU16::new(listen),
        core: serve.core,
        resources: serve.resources,
        language: core.language.clone(),
        tuning: serve.tuning.clone(),
        bots: Arc::clone(&bots),
        current: Arc::clone(&current),
        checked: web::Checked::new(serve.tuning.web.status_cache()),
        applied: tokio::sync::Mutex::new((serve.settings.port, serve.settings.web)),
        swap,
    });
    let gate = Arc::new(Gate {
        current,
        tuning: serve.tuning,
        bots: Arc::clone(&bots),
        inbound,
        tell,
        serial: AtomicU64::new(0),
    });
    // 跟核心的那一头停了，交回为什么（核心断了、发回话的任务崩了）；接连接的停了不要紧。
    let mut tasks = JoinSet::new();
    let route = Route::new(core, bots);
    let route = tasks
        .spawn(async move { Some(route.run(received).await) })
        .id();
    tokio::pin!(stop);
    loop {
        tokio::select! {
            accepted = napcat.accept() => match accepted {
                Ok((stream, _)) => {
                    let gate = Arc::clone(&gate);
                    tasks.spawn(async move {
                        listen::accept(stream, gate).await;
                        None
                    });
                }
                Err(error) => {
                    // 接不了（打开的文件太多这类）：歇一下再接，不空转（照核心的规矩）。
                    tracing::warn!(target: TARGET, error = %error, "accept failed");
                    tokio::time::sleep(retry).await;
                }
            },
            accepted = pages.accept() => match accepted {
                Ok((stream, _)) => {
                    let web = Arc::clone(&web);
                    tasks.spawn(async move {
                        web::accept(stream, web).await;
                        None
                    });
                }
                Err(error) => {
                    tracing::warn!(target: TARGET, error = %error, "web accept failed");
                    tokio::time::sleep(retry).await;
                }
            },
            // 发的一头在 `web` 里，跑着时一直在：收不到空的。
            Some(swapped) = swaps.recv() => match swapped {
                Swap::Napcat(listener) => napcat = listener,
                Swap::Web(listener) => pages = listener,
            },
            Some(ended) = tasks.join_next_with_id() => match ended {
                Ok((_, Some(failure))) => {
                    tracing::warn!(target: TARGET, failure = ?failure, "route ended");
                    return Err(failure);
                }
                Ok((_, None)) => {}
                Err(error) if error.id() == route => {
                    tracing::error!(target: TARGET, error = %error, "route crashed");
                    return Err(Failure::Crashed(error.to_string()));
                }
                Err(error) => {
                    tracing::error!(target: TARGET, error = %error, "connection crashed");
                }
            },
            () = &mut stop => {
                tracing::info!(target: TARGET, "stopped");
                return Ok(());
            }
        }
    }
}

/// 只听 `127.0.0.1` 的 `wanted`：交回监听和实际的端口（`0` 是系统挑的那一个）。被占了的照 `in_use` 说是哪个端口。起来时、
/// `/apply` 换端口时用。
pub(crate) async fn bind(
    wanted: u16,
    in_use: fn(u16) -> Failure,
) -> Result<(TcpListener, u16), Failure> {
    let listener = match TcpListener::bind(("127.0.0.1", wanted)).await {
        Ok(listener) => listener,
        Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => return Err(in_use(wanted)),
        Err(error) => return Err(Failure::Start(error.to_string())),
    };
    let port = listener
        .local_addr()
        .map_err(|error| Failure::Start(error.to_string()))?
        .port();
    Ok((listener, port))
}
