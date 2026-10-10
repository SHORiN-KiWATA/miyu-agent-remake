//! `miyu-onebot serve`（`onebot.md` 第一条「怎么走」第 1、2、11 条）：在核心亲手给的管道上握手，再开监听，把几样接起来，跑到
//! 停的信号或者核心关了管道。
//!
//! 1. 在 [`Pipe`] 上握手（`core`）：程序里是标准输入输出，核心拉起桥时接好的（施工 O-18，`extensions.md`），不带凭据。等不到
//!    回应：[`Failure::NotSpawned`]；被拒、管道关了：[`Failure::Core`]。握手回了语言先告诉调的一方（之后说的都照它，端口被占
//!    那一句也是，施工 O-20，「施工时定的」第 42 条）；回应里的 `config` 读成两个端口、令牌（[`Settings::handed`]，没交的端口
//!    照清单的默认值）。握手以后读一次系统的场所规则，问题记运行日志（施工 O-21，[`Venues`]），交给跟核心的那一头，每一条
//!    消息照它套场所（施工 O-22）。限流满了发进群里的提示照握手回的语言说（施工 O-23，「群里怎么叫她」第 7 条）；那种语言的
//!    字读不懂的照系统的语言（`main.rs` 那一头也是照原来的说，「施工时定的」第 80 条）。握手以后、开监听以前登记桥的工具
//!    （施工 O-26，`core/provider.rs`）：写出去就接着起来，等回应、记运行日志的那一段放进下面第 3 条的任务组。开好两个监听、
//!    WebUI 造好以后，把它交给答后台页方法的那一头、登记方法（施工 O-28 上，`core/methods.rs`）：同样写出去就接着起来。
//! 2. 只听 `127.0.0.1` 的 `onebot.listen`。被占了：[`Failure::PortInUse`]。再听 `127.0.0.1` 的 `onebot.web`（WebUI，施工
//!    O-16，第二条「怎么走」第 1 条）。被占了：[`Failure::WebPortInUse`]。听上了各说一行（[`Notice::Listening`]、
//!    [`Notice::Web`]）。令牌没设的两个也照开，NapCat 连进来一律 401，人在 WebUI 里生成令牌，NapCat 下一次连就通
//!    （O-16 补、补二，18 第三节「还没配好就 `start`」）：在两句中间说 [`Notice::NoToken`]，记一行运行日志。
//! 3. 每个 TCP 连接一个任务（NapCat 的交给 `listen`，WebUI 的交给 `web`），跟核心的那一头一个任务（`core/route.rs`），写状态
//!    文件的一个任务（`crate::status_file`，施工 O-18），都在一组里：停下时一起停。WebUI 的 `/apply` 开好的新监听经 `Swap`
//!    送过来，换掉旧的，旧的随之关掉，状态文件跟着写；已经接进来的连接不断（O-16 补二，第二条「施工时定的」第 19 条）。
//!    核心推来的配置（`extension.config`，施工 O-20）由跟核心的那一头交过来：换上桥手里的那一份（`crate::current`），两个
//!    端口变了的另起一个任务照 `/apply` 的办法换（`web::latest`，「施工时定的」第 45 条）。
//! 4. 核心关了管道（标准输入读到头：核心请它退出，或者核心不在了）：好好停下，交回 `Ok`，退出码 0（第 11 条，「施工时定的」
//!    第 5、21 条：崩了由核心退避重启，桥里不另写一套重连）。跟核心的那一头、发回话的任务崩了：[`Failure::Crashed`]，退出
//!    （第 14 条）。NapCat 断了不退。

use std::sync::Arc;
use std::sync::atomic::{AtomicU16, AtomicU64};
use std::time::Instant;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;
use tokio::sync::{Notify, mpsc};
use tokio::task::JoinSet;

use miyu_chat::Problem;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

use crate::TARGET;
use crate::core::methods::{Methods, register};
use crate::core::route::{Personas, Reactions, Route, Slots};
use crate::core::{Core, provide};
use crate::current::Current;
use crate::listen::bots::Bots;
use crate::listen::{self, Gate};
use crate::onebot::Members;
use crate::rules::{Factory, Venues};
use crate::settings::{Defaults, Settings};
use crate::status_file;
use crate::texts::{Texts, system_language};
use crate::tuning::Tuning;
use crate::web::{self, Web};

pub use miyu_webserve::CoreCommand;

/// 跟核心说协议的管道：读的一头、写的一头（施工 O-18）。程序里是核心拉起桥时接好的标准输入输出；测试里是内存里的管道。
pub struct Pipe {
    /// 核心说的：一行一条。读到头是核心请它退出，或者核心不在了。
    pub read: Box<dyn AsyncRead + Send + Unpin>,
    /// 说给核心的：一行一条，只有协议。
    pub write: Box<dyn AsyncWrite + Send + Unpin>,
}

impl Pipe {
    /// 把读的一头 `read`、写的一头 `write` 接成一条管道。
    pub fn new(
        read: impl AsyncRead + Send + Unpin + 'static,
        write: impl AsyncWrite + Send + Unpin + 'static,
    ) -> Pipe {
        Pipe {
            read: Box::new(read),
            write: Box::new(write),
        }
    }
}

/// 起一个桥要的。
pub struct Serve {
    /// 数据根。
    pub root: DataRoot,
    /// 跟核心说协议的管道（施工 O-18）。握手的回应交来两个端口、令牌（施工 O-20）。
    pub pipe: Pipe,
    /// WebUI 连核心（`/ws`、验登录令牌，经本机套接字）时，核心没在跑怎么拉起来。
    pub core: CoreCommand,
    /// 系统的语言：握手时报给核心，`ui.language` 是 `auto` 的照它定说话的语言。
    pub locale: Option<String>,
    /// 桥自己的数（`bridge.json`）。
    pub tuning: Tuning,
    /// 资源目录：WebUI 的页面（[`web::PAGES`]）、页面的字在这里。
    pub resources: ResourceRoot,
    /// 清单 `[settings]` 里两个端口的默认值：握手没交、推来 `null` 的照它（施工 O-20）。
    pub defaults: Defaults,
    /// 出厂的场所规则、出厂参数、违规词表：握手以前读好、查过（施工 O-21，[`Factory::load`]）。
    pub factory: Factory,
}

/// 说给人听的（「样子」）：怎么说照 `texts`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// 听上了：实际的端口（设的是 0 的，系统挑的那一个）。
    Listening {
        /// 实际听的端口。
        port: u16,
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
    /// 令牌没设：NapCat 连进来会被拒，说怎么设。跟在 [`Notice::Listening`] 后面说。
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
    /// 连不上核心、握手被拒、握手时管道关了：原因。
    Core(String),
    /// 握手等不到回应（`bridge.json` 的 `hello_seconds`）：不是核心拉起的，例如人在终端里跑的（施工 O-18）。
    NotSpawned,
    /// NapCat 的端口被占了。
    PortInUse(u16),
    /// WebUI 的端口被占了。
    WebPortInUse(u16),
    /// 别的原因起不来（听不了、起不了运行时、读不了数据根或 `bridge.json`）：原话。
    Start(String),
    /// 跟核心的那一头崩了、发回话的任务崩了（是 bug）：原话。
    Crashed(String),
    /// 出厂的场所规则、出厂参数、违规词表有问题（是打包的错，施工 O-21）：每一条问题。
    Factory(Vec<Problem>),
}

/// 跑起来，直到 `stop` 到了（停的信号）或者核心关了管道。握手回了语言交给 `shaken`（之后说的话、起不来的原因都照它说），
/// 说给人听的交给 `tell`。
///
/// # Errors
///
/// 握手不成（等不到、被拒、管道关了）、两个端口被占、听不了；跑着跑着跟核心的那一头、发回话的任务崩了。
pub async fn run(
    serve: Serve,
    shaken: impl FnOnce(&str) + Send,
    tell: impl Fn(Notice) + Send + Sync + 'static,
    stop: impl Future<Output = ()>,
) -> Result<(), Failure> {
    let tools = serve.factory.tools();
    let methods = Arc::new(Methods::new());
    let core = Core::connect(
        serve.pipe,
        serve.locale.as_deref(),
        serve.tuning.hello(),
        Arc::clone(&tools),
        Arc::clone(&methods),
    )
    .await?;
    shaken(&core.language);
    // 登记桥的工具（施工 O-26）：写出去就接着起来，之后的 `venue.session` 造的会话就有它们。
    let provided = provide(&core.caller(), &tools).await;
    // 场所规则和出厂数据（施工 O-21）：读一次系统的，问题记进运行日志。跟核心的那一头每一条消息照它套场所（施工 O-22）。
    let venues = Venues::new(
        serve.factory,
        &serve.root,
        serve.tuning.rules_check(),
        Instant::now(),
    );
    let settings = Settings::handed(&core.config, &serve.defaults);
    let texts = Texts::load(serve.resources.clone(), &core.language)
        .or_else(|_| {
            Texts::load(
                serve.resources.clone(),
                system_language(serve.locale.as_deref()),
            )
        })
        .map_err(|error| Failure::Start(error.to_string()))?;
    let (mut napcat, listen) = bind(settings.port, Failure::PortInUse).await?;
    let (mut pages, web_port) = bind(settings.web, Failure::WebPortInUse).await?;
    let tell: Arc<dyn Fn(Notice) + Send + Sync> = Arc::new(tell);
    tracing::info!(target: TARGET, port = listen, "listening");
    tell(Notice::Listening { port: listen });
    if settings.token.is_none() {
        tracing::warn!(target: TARGET, "no token yet");
        tell(Notice::NoToken);
    }
    tracing::info!(target: TARGET, port = web_port, "web listening");
    tell(Notice::Web { port: web_port });
    let bots = Arc::new(Bots::default());
    let (inbound, received) = mpsc::channel(serve.tuning.inbound_queue);
    let retry = serve.tuning.accept_retry();
    let applied = (settings.port, settings.web);
    let current = Arc::new(Current::new(settings, serve.defaults));
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
        applied: tokio::sync::Mutex::new(applied),
        swap,
    });
    // 后台页调的方法（施工 O-28 上）：桥手里的状态齐了，先交给答的那一头，再登记。
    methods.ready(Arc::clone(&web));
    let registered = register(&core.caller()).await;
    let changed = Arc::new(Notify::new());
    let gate = Arc::new(Gate {
        current: Arc::clone(&current),
        tuning: serve.tuning,
        bots: Arc::clone(&bots),
        inbound,
        tell,
        serial: AtomicU64::new(0),
        changed: Arc::clone(&changed),
    });
    // 跟核心的那一头停了，交回为什么（空的是核心关了管道、发回话的任务崩了是那个原因）；接连接的、写状态文件的停了不要紧。
    let mut tasks = JoinSet::new();
    if let Some(provided) = provided {
        tasks.spawn(async move {
            provided.await;
            None
        });
    }
    if let Some(registered) = registered {
        tasks.spawn(async move {
            registered.await;
            None
        });
    }
    let (configured, mut configs) = mpsc::unbounded_channel();
    let members = Members::new(gate.tuning.member_names());
    let slots = Slots::new(gate.tuning.judge_concurrency, gate.tuning.judge_queue());
    let personas = Personas::new(gate.tuning.judge_persona());
    let recall = gate.tuning.receipt_recall();
    let reactions = Reactions::new(gate.tuning.reaction_emoji.clone(), gate.tuning.reaction());
    let parts = (
        texts,
        slots,
        personas,
        recall,
        gate.tuning.queue_expire(),
        reactions,
    );
    let route = Route::new(core, bots, venues, members, parts, configured);
    let route = tasks.spawn(async move { route.run(received).await }).id();
    tasks.spawn(status_file::keep(
        status_file::path(&serve.root),
        Arc::clone(&web),
        Arc::clone(&changed),
    ));
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
            // 核心推来的配置（施工 O-20）：令牌当场换上；端口变了另起一个任务换，换的时候不耽误接连接。发的一头在跟核心的
            // 那一头里，它停了收到空的，这一支不再看。
            Some(keys) = configs.recv() => {
                if current.change(&keys) {
                    let web = Arc::clone(&web);
                    tasks.spawn(async move {
                        if web::latest(&web).await.is_err() {
                            // 换不成的（被占）已经记了运行日志，旧的照旧：页面接着调的 `/apply` 再试一次。
                        }
                        None
                    });
                }
            },
            // 发的一头在 `web` 里，跑着时一直在：收不到空的。
            Some(swapped) = swaps.recv() => {
                match swapped {
                    Swap::Napcat(listener) => napcat = listener,
                    Swap::Web(listener) => pages = listener,
                }
                changed.notify_one();
            },
            Some(ended) = tasks.join_next_with_id() => match ended {
                Ok((_, Some(failure))) => {
                    tracing::warn!(target: TARGET, failure = ?failure, "route ended");
                    return Err(failure);
                }
                Ok((id, None)) if id == route => {
                    tracing::info!(target: TARGET, "core closed, stopping");
                    return Ok(());
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
