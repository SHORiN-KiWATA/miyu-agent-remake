//! 连核心：找数据根、连上（给了 `MIYU_CORE_BIN` 的，没在跑就拉起来）、握手、开会话、订阅，然后收发。
//!
//! 走法照 `miyu ask`（`docs/blueprint/cli/ask.md`「怎么走」）。界面的主循环是同步的，这里在单独的线程里跑
//! 一个 tokio 运行时；两边只靠通道说话：界面发 [`Command`]，这里回 [`Update`]。
//!
//! 拉起核心只认 `MIYU_CORE_BIN`，不去 PATH 里找 `miyu`：装着旧版的机器上，PATH 里的 `miyu` 是旧版，
//! 给它 `core` 这个参数，它会把这个词当成一句话发给旧版的后台。

mod kinds;
mod limits;
mod push;
mod rpc;
mod undo;

use std::collections::HashMap;
use std::process::Command as Process;
use std::thread;

use serde_json::json;
use tokio::sync::mpsc;

use miyu_store::env::Env;
use miyu_store::root::DataRoot;

pub use kinds::{EndReason, Level, ToolStatus};
pub use limits::Limits;
pub use push::{Block, Compaction, Push, Usage};
use rpc::{Failure, Rpc};
pub use undo::Report;

/// 界面要核心做的事。
#[derive(Debug)]
pub enum Command {
    /// 说一句话。
    Send(String),
    /// 打断在进行的这一轮。`send` 为真时排着队的消息接着发（两下 Esc），为假时退回来（Ctrl+C）。
    Interrupt {
        /// 排着队的消息接着发。
        send: bool,
    },
    /// 撤销最后一轮（`session.revert`，不写 `turn`）。
    Revert,
    /// 恢复最近一次撤销（`session.unrevert`）。
    Unrevert,
}

/// 核心那边的消息，交给界面。
#[derive(Debug)]
pub enum Update {
    /// 连上了，会话开好了，带着会话编号。
    Ready(String),
    /// 核心没在跑，也没给 `MIYU_CORE_BIN`，拉不起来。
    NoCoreBin,
    /// 连不上、握手或开会话被拒：原因。
    Failed(String),
    /// 一条请求被拒绝：原因码（`data.reason`，没有的是 `None`），和核心照握手时的语言说的原话。
    Refused {
        /// 原因码，例如 `nothing_to_unrevert`（`protocol/undo.md`）。
        reason: Option<String>,
        /// 核心的原话。
        message: String,
    },
    /// 核心断开了。
    Disconnected,
    /// 会话的限额（订阅的回应里的 `limits`）：窗口、压缩线，侧边栏和框下面那一行照它写上下文。
    Limits(Limits),
    /// 会话里的事。
    Push(Push),
    /// 撤销（`restore` 为假）或恢复成了：核心算好的给人看的几样（`protocol/undo.md`）。
    Undone {
        /// 是恢复。
        restore: bool,
        /// 回应里给人看的几样。
        report: Report,
    },
}

/// 连着核心的这一头，界面拿着它发命令。
pub struct Core {
    commands: mpsc::UnboundedSender<Command>,
}

impl Core {
    /// 发一个命令。连上之前发的排着，连上再发（`13-终端界面.md` 第九节「先画后连」）。
    pub fn send(&self, command: Command) {
        // 收的一头只在连核心的线程退出时关掉，那时已经报过「断开了」，丢了也无妨。
        if self.commands.send(command).is_err() {}
    }
}

/// 起一个线程去连核心。`notify` 把消息交给界面，界面那头关了就交回 `false`，这边跟着停。
pub fn spawn(notify: impl Fn(Update) -> bool + Send + 'static) -> Core {
    let (commands, receiver) = mpsc::unbounded_channel();
    thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build();
        match runtime {
            Ok(runtime) => runtime.block_on(run(receiver, &notify)),
            Err(e) => {
                notify(Update::Failed(e.to_string()));
            }
        }
    });
    Core { commands }
}

async fn run(mut commands: mpsc::UnboundedReceiver<Command>, notify: &impl Fn(Update) -> bool) {
    let (mut rpc, session, limits) = match open().await {
        Ok(opened) => opened,
        Err(update) => {
            notify(update);
            return;
        }
    };
    if !notify(Update::Ready(session.clone())) || !notify(Update::Limits(limits)) {
        return;
    }
    let cwd = cwd();
    // 等着回应、回应要交给界面的请求：编号到「是不是恢复」。
    let mut undos: HashMap<String, bool> = HashMap::new();
    loop {
        tokio::select! {
            command = commands.recv() => {
                let Some(command) = command else { return };
                let restore = match command {
                    Command::Revert => Some(false),
                    Command::Unrevert => Some(true),
                    _ => None,
                };
                let (method, params) = match command {
                    Command::Send(text) => ("session.send", json!({"session": session, "text": text, "cwd": cwd})),
                    Command::Interrupt { send } => {
                        let queued = if send { "send" } else { "return" };
                        ("session.interrupt", json!({"session": session, "queued": queued}))
                    }
                    Command::Revert => ("session.revert", json!({"session": session})),
                    Command::Unrevert => ("session.unrevert", json!({"session": session})),
                };
                match rpc.send(method, params).await {
                    Ok(id) => {
                        if let Some(restore) = restore {
                            undos.insert(id, restore);
                        }
                    }
                    Err(_) => {
                        notify(Update::Disconnected);
                        return;
                    }
                }
            }
            message = rpc.next() => {
                let Some(message) = message else {
                    notify(Update::Disconnected);
                    return;
                };
                if !take(&mut rpc, &session, &message, &mut undos, notify).await {
                    return;
                }
            }
        }
    }
}

/// 处理一条读进来的：推送、回应。交回界面还在不在。
async fn take(
    rpc: &mut Rpc,
    session: &str,
    message: &serde_json::Value,
    undos: &mut HashMap<String, bool>,
    notify: &impl Fn(Update) -> bool,
) -> bool {
    let restore = message["id"].as_str().and_then(|id| undos.remove(id));
    if let Some(error) = message.get("error") {
        return notify(Update::Refused {
            reason: error["data"]["reason"].as_str().map(str::to_string),
            message: error["message"].as_str().unwrap_or_default().to_string(),
        });
    }
    if let Some(restore) = restore {
        let report = Report::read(&message["result"]);
        return notify(Update::Undone { restore, report });
    }
    // 掉队后重新订阅的回应：限额照样带着，照它更新（核心重启以后载入的也是这样）。
    if let Some(limits) = Limits::of(message) {
        return notify(Update::Limits(limits));
    }
    match message["method"].as_str() {
        Some("event") => push::read(&message["params"]["event"])
            .into_iter()
            .all(|p| notify(Update::Push(p))),
        // 掉了队：重新订阅，掉了的不补（`protocol.md`「慢和掉队」）。
        Some("resync") => {
            let params = json!({"session": session, "stream": "events"});
            rpc.send("subscribe", params).await.is_ok() || notify(Update::Disconnected)
        }
        _ => true,
    }
}

/// 连上、握手、开会话、订阅。交回连接、会话编号和订阅的回应里的限额。
async fn open() -> Result<(Rpc, String, Limits), Update> {
    let env = Env::current();
    let root = DataRoot::locate(&env).map_err(|e| Update::Failed(e.to_string()))?;
    root.prepare().map_err(|e| Update::Failed(e.to_string()))?;
    let connected = match std::env::var_os("MIYU_CORE_BIN") {
        Some(bin) => miyu_ipc::connect_or_start(&root, move || {
            let mut core = Process::new(bin);
            core.arg("core");
            core
        })
        .await
        .map_err(|e| Update::Failed(e.to_string())),
        None => miyu_ipc::connect(&root).await.map_err(|e| match e {
            miyu_ipc::ConnectError::NotRunning => Update::NoCoreBin,
            e => Update::Failed(e.to_string()),
        }),
    };
    let (connection, token) = connected?;
    let mut rpc = Rpc::new(connection);
    // 还没有确认的抽屉，先说没人能当场回答：要确认的那一步，核心当场拒绝，不会一直等着。
    let hello = json!({
        "protocol": [1, 1],
        "head": {"kind": "tui", "version": env!("CARGO_PKG_VERSION")},
        "locale": "zh-CN",
        "caps": {"input": false},
        "token": token,
    });
    rpc.call("hello", hello).await.map_err(refused)?;
    let created = rpc
        .call("session.create", json!({"cwd": cwd()}))
        .await
        .map_err(refused)?;
    let session = created["session"].as_str().unwrap_or_default().to_string();
    let subscribed = rpc
        .call("subscribe", json!({"session": session, "stream": "events"}))
        .await
        .map_err(refused)?;
    let limits = Limits::of(&json!({ "result": subscribed })).unwrap_or_default();
    Ok((rpc, session, limits))
}

fn refused(failure: Failure) -> Update {
    match failure {
        Failure::Io(e) => Update::Failed(e.to_string()),
        Failure::Disconnected => Update::Disconnected,
        Failure::Refused(reason) => Update::Failed(reason),
    }
}

/// 启动时的目录，读不出来的写 `.`（照 `miyu ask`）。
fn cwd() -> String {
    std::env::current_dir().map_or_else(|_| ".".to_string(), |d| d.display().to_string())
}
