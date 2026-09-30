//! 连核心：找数据根、连上（给了 `MIYU_CORE_BIN` 的，没在跑就拉起来）、握手、开会话、订阅，然后收发。
//!
//! 走法照 `miyu ask`（`docs/blueprint/cli/ask.md`「怎么走」）。界面的主循环是同步的，这里在单独的线程里跑
//! 一个 tokio 运行时；两边只靠通道说话：界面发 [`Command`]，这里回 [`Update`]。
//!
//! 拉起核心只认 `MIYU_CORE_BIN`，不去 PATH 里找 `miyu`：装着旧版的机器上，PATH 里的 `miyu` 是旧版，
//! 给它 `core` 这个参数，它会把这个词当成一句话发给旧版的后台。

mod backoff;
mod connect;
mod kinds;
mod limits;
mod push;
mod request;
mod rpc;
mod undo;
mod upload;

use std::collections::HashMap;
use std::thread;

use serde_json::json;
use tokio::sync::mpsc;

use backoff::Backoff;
use connect::{connect, create, cwd, subscribe};
use request::request;

pub use kinds::{EndReason, Level, ToolStatus};
pub use limits::Limits;
pub use push::{Block, CallError, Compaction, Push, Usage};
use rpc::Rpc;
pub use undo::Report;

/// 界面要核心做的事。
#[derive(Debug)]
pub enum Command {
    /// 说一句话，带着附件（本机的文件，发之前先 `blob.put`；蓝图「输入框」第 12 条）。
    Send {
        /// 说的字。
        text: String,
        /// 附件：照先后。
        files: Vec<std::path::PathBuf>,
    },
    /// 打断在进行的这一轮。`send` 为真时排着队的消息接着发（两下 Esc），为假时退回来（Ctrl+C）。
    Interrupt {
        /// 排着队的消息接着发。
        send: bool,
    },
    /// 撤销最后一轮（`session.revert`，不写 `turn`）。
    Revert,
    /// 恢复最近一次撤销（`session.unrevert`）。
    Unrevert,
    /// 现在就压缩上下文（`session.compact`，施工 6-8），带着给摘要的要求。
    Compact(Option<String>),
    /// 开新会话（`/new`）：退订现在这个，等第一句话再开（蓝图「斜杠命令」`/new`）。
    New,
    /// 切权限级别（`session.set_permission_level`）：切到这一级。会话还没开的记着，开了再发。
    Level(Level),
    /// 清空上下文（`session.clear`，`/clear`）。
    Clear,
    /// 重做最后一轮（`session.redo`，`/redo`、`/edit`）：`text` 换开这一轮的那句字，`files` 换附件（空的是不要附件），
    /// 都是 `None` 的原样重来。
    Redo {
        /// 改过的字。
        text: Option<String>,
        /// 换了的附件：本机的文件，发之前先 `blob.put`。
        files: Option<Vec<std::path::PathBuf>>,
    },
}

/// 核心那边的消息，交给界面。
#[derive(Debug)]
pub enum Update {
    /// 连上了，会话开好了，带着会话编号。
    Ready(String),
    /// 核心没在跑，也没给 `MIYU_CORE_BIN`，拉不起来。
    NoCoreBin,
    /// `MIYU_CORE_BIN` 指的程序不存在：路径（蓝图「连核心」第 8 条）。
    Missing(String),
    /// 断开以后又连上了，接着订阅着原来那个会话（第 7 条）。
    Reconnected,
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
    /// 说的话、重做没发出去（核心拒了、附件传不上）：和 [`Update::Refused`] 一样的两样；先画进正文的那句要撤掉，
    /// 字放回输入框（「输入框」第 12、13 条）。
    Unsent {
        /// 原因码，例如 `not_redoable`。
        reason: Option<String>,
        /// 核心的原话。
        message: String,
    },
}

/// 等着回应、回应要另外办的请求（`serve` 记着请求的编号）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Awaiting {
    /// 撤销：回应里给人看的几样交给界面。
    Revert,
    /// 恢复：同上。
    Unrevert,
    /// 说一句话：成了不用管，拒了要撤掉先画上的那句。
    Send,
    /// 重做：同上。
    Redo,
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

/// 起一个线程去连核心。`reconnect` 是连不上时隔多久再试（`layout.json` 的 `reconnect_ms`）；`notify` 把消息
/// 交给界面，界面那头关了就交回 `false`，这边跟着停。
pub fn spawn(reconnect: [u64; 2], notify: impl Fn(Update) -> bool + Send + 'static) -> Core {
    let (commands, receiver) = mpsc::unbounded_channel();
    thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build();
        match runtime {
            Ok(runtime) => runtime.block_on(run(receiver, Backoff::new(reconnect), &notify)),
            Err(e) => {
                notify(Update::Failed(e.to_string()));
            }
        }
    });
    Core { commands }
}

/// 一条连接用到头了：界面关了，或者连接断了。
enum Served {
    Quit,
    Lost,
}

/// 连上、开会话、订阅，然后收发；断了就重连，订阅原来那个会话（蓝图「连核心」第 7 条）。
async fn run(
    mut commands: mpsc::UnboundedReceiver<Command>,
    mut wait: Backoff,
    notify: &impl Fn(Update) -> bool,
) {
    let mut session = None;
    // 会话还没开时切的权限级别：开会话时补发（「权限级别」第 2 条）。
    let mut pending = None;
    loop {
        // 连不上一直试；这期间界面发的命令在通道里排着，连上再发（第 1、7 条）。
        let mut rpc = loop {
            match open(session.as_deref()).await {
                Ok((rpc, opened, limits)) => {
                    // 新开的会话（刚启动；按过 `/new` 还没说话就断了的）告诉界面编号，订阅原来的只说又连上了。
                    let said = match (&session, &opened) {
                        (None, Some(id)) => notify(Update::Ready(id.clone())),
                        _ => notify(Update::Reconnected),
                    };
                    if !said || limits.is_some_and(|l| !notify(Update::Limits(l))) {
                        return;
                    }
                    session = opened;
                    break rpc;
                }
                Err(update) => {
                    if !notify(update) {
                        return;
                    }
                    tokio::time::sleep(wait.next()).await;
                }
            }
        };
        wait.reset();
        match serve(&mut rpc, &mut session, &mut pending, &mut commands, notify).await {
            Served::Quit => return,
            Served::Lost if !notify(Update::Disconnected) => return,
            Served::Lost => {}
        }
    }
}

/// 连上；有会话的订阅它，还没有的（刚启动）开一个再订阅。交回连接、会话和限额。
async fn open(session: Option<&str>) -> Result<(Rpc, Option<String>, Option<Limits>), Update> {
    let mut rpc = connect().await?;
    let session = match session {
        Some(id) => id.to_string(),
        None => create(&mut rpc).await?,
    };
    let limits = subscribe(&mut rpc, &session).await?;
    Ok((rpc, Some(session), Some(limits)))
}

/// 在一条连接上收发，直到界面关了或者连接断了。
async fn serve(
    rpc: &mut Rpc,
    session: &mut Option<String>,
    pending: &mut Option<Level>,
    commands: &mut mpsc::UnboundedReceiver<Command>,
    notify: &impl Fn(Update) -> bool,
) -> Served {
    let cwd = cwd();
    // 等着回应、回应要另外办的请求：编号到是哪一种。
    let mut awaiting: HashMap<String, Awaiting> = HashMap::new();
    loop {
        tokio::select! {
            command = commands.recv() => {
                let Some(command) = command else { return Served::Quit };
                match command {
                    // 懒着开（施工会话 09-30 建议）：只退订旧的，等第一句话再开，连按几下不留空会话。
                    Command::New => {
                        if let Some(old) = session.take() {
                            let params = json!({"session": old, "stream": "events"});
                            if rpc.send("unsubscribe", params).await.is_err() {
                                return Served::Lost;
                            }
                        }
                        continue;
                    }
                    Command::Level(level) if session.is_none() => {
                        *pending = Some(level);
                        continue;
                    }
                    Command::Send { .. } if session.is_none() => {
                        match fresh(rpc).await {
                            Ok((id, limits)) => {
                                if !notify(Update::Ready(id.clone())) || !notify(Update::Limits(limits)) {
                                    return Served::Quit;
                                }
                                // 会话还没开时切过权限级别：先补发，再说话。
                                if let Some(level) = pending.take() {
                                    let mut params = level.permission();
                                    params["session"] = json!(id);
                                    if rpc.send("session.set_permission_level", params).await.is_err() {
                                        return Served::Lost;
                                    }
                                }
                                *session = Some(id);
                            }
                            Err(Update::Disconnected) => return Served::Lost,
                            Err(update) => {
                                if !notify(update) {
                                    return Served::Quit;
                                }
                                continue;
                            }
                        }
                    }
                    _ => {}
                }
                // 还没开会话时别的命令没有对象：界面那头当场说了（`app/keys.rs`）。
                let Some(session) = session.as_deref() else { continue };
                let kind = match command {
                    Command::Revert => Some(Awaiting::Revert),
                    Command::Unrevert => Some(Awaiting::Unrevert),
                    Command::Send { .. } => Some(Awaiting::Send),
                    Command::Redo { .. } => Some(Awaiting::Redo),
                    _ => None,
                };
                // 带附件的：先把每个文件交给核心，传不上的整句不发（「输入框」第 12 条）。重做换了附件的也一样，
                // 换成没有附件的带一个空的（「输入框」第 13 条）。
                let files = match &command {
                    Command::Send { files, .. } if !files.is_empty() => Some(files.clone()),
                    Command::Redo { files, .. } => files.clone(),
                    _ => None,
                };
                let mut attachments = None;
                if let Some(files) = files {
                    match upload::attach(rpc, &files).await {
                        Ok(got) => attachments = Some(got),
                        Err(Update::Disconnected) => return Served::Lost,
                        // 传不上：这一句没发出去（「输入框」第 12 条）。
                        Err(Update::Refused { reason, message }) => {
                            if !notify(Update::Unsent { reason, message }) {
                                return Served::Quit;
                            }
                            continue;
                        }
                        Err(update) => {
                            if !notify(update) {
                                return Served::Quit;
                            }
                            continue;
                        }
                    }
                }
                let Some((method, mut params)) = request(command, session, &cwd) else { continue };
                if let Some(attachments) = attachments {
                    params["attachments"] = json!(attachments);
                }
                match rpc.send(method, params).await {
                    Ok(id) => {
                        if let Some(kind) = kind {
                            awaiting.insert(id, kind);
                        }
                    }
                    Err(_) => return Served::Lost,
                }
            }
            message = rpc.next() => {
                let Some(message) = message else { return Served::Lost };
                let session = session.as_deref().unwrap_or_default();
                if !take(rpc, session, &message, &mut awaiting, notify).await {
                    return Served::Quit;
                }
            }
        }
    }
}

/// `/new` 以后的第一句话：开会话、订阅。
async fn fresh(rpc: &mut Rpc) -> Result<(String, Limits), Update> {
    let id = create(rpc).await?;
    let limits = subscribe(rpc, &id).await?;
    Ok((id, limits))
}

/// 处理一条读进来的：推送、回应。交回界面还在不在。
async fn take(
    rpc: &mut Rpc,
    session: &str,
    message: &serde_json::Value,
    awaiting: &mut HashMap<String, Awaiting>,
    notify: &impl Fn(Update) -> bool,
) -> bool {
    let kind = message["id"].as_str().and_then(|id| awaiting.remove(id));
    if let Some(error) = message.get("error") {
        let reason = error["data"]["reason"].as_str().map(str::to_string);
        let message = error["message"].as_str().unwrap_or_default().to_string();
        return notify(match kind {
            Some(Awaiting::Send | Awaiting::Redo) => Update::Unsent { reason, message },
            _ => Update::Refused { reason, message },
        });
    }
    match kind {
        Some(Awaiting::Revert | Awaiting::Unrevert) => {
            let restore = kind == Some(Awaiting::Unrevert);
            let report = Report::read(&message["result"]);
            return notify(Update::Undone { restore, report });
        }
        // 说的话、重做成了：落盘、开轮都照推送来，回应不用管。
        Some(Awaiting::Send | Awaiting::Redo) => return true,
        None => {}
    }
    // 掉队后重新订阅的回应：限额照样带着，照它更新（核心重启以后载入的也是这样）。
    if let Some(limits) = Limits::of(message) {
        return notify(Update::Limits(limits));
    }
    // 只收现在这个会话的：`/new` 以后，旧会话退订之前推来的不要（蓝图「斜杠命令」`/new`）。
    if message["params"]["session"].as_str() != Some(session) {
        return true;
    }
    match message["method"].as_str() {
        Some("event") => push::read(&message["params"]["event"])
            .into_iter()
            .all(|p| notify(Update::Push(p))),
        // 掉了队：重新订阅，掉了的不补（`protocol.md`「慢和掉队」）。发不出去是连接断了，下一条读不到，照断开重连。
        Some("resync") => {
            let params = json!({"session": session, "stream": "events"});
            let _resubscribed = rpc.send("subscribe", params).await;
            true
        }
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Command, Level, request};

    #[test]
    fn switching_the_level_writes_only_what_changes() {
        // 「权限级别」第 2 条：工作区、开放权限写常用的那一级、关掉只读；只读只开只读。
        let (method, params) = request(Command::Level(Level::Full), "s1", ".").unwrap();
        assert_eq!(method, "session.set_permission_level");
        assert_eq!(
            params,
            json!({"session": "s1", "level": "full", "read_only": false})
        );
        let (_, params) = request(Command::Level(Level::ReadOnly), "s1", ".").unwrap();
        assert_eq!(params, json!({"session": "s1", "read_only": true}));
        let (_, params) = request(Command::Level(Level::Workspace), "s1", ".").unwrap();
        assert_eq!(
            params,
            json!({"session": "s1", "level": "workspace", "read_only": false})
        );
    }

    #[test]
    fn clear_asks_for_session_clear_and_new_asks_for_nothing() {
        let (method, params) = request(Command::Clear, "s1", ".").unwrap();
        assert_eq!(
            (method, params),
            ("session.clear", json!({"session": "s1"}))
        );
        assert!(request(Command::New, "s1", ".").is_none());
    }
}
