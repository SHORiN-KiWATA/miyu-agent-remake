//! 在一条连接上收发（蓝图 `tui.md`「连核心」「后台命令、子代理和侧边栏」「切进子会话」）：命令对着主会话，或者切进了
//! 的子会话；推来的照会话分，主会话的直接交给界面，另外订阅着的包成 [`Update::Elsewhere`]。

use std::collections::{BTreeSet, HashMap};
use std::io;

use serde_json::{Value, json};
use tokio::sync::mpsc;

use super::connect::{create, cwd, subscribe};
use super::limits::Limits;
use super::request::request;
use super::rpc::{self, Rpc};
use super::{Command, JobOutput, Level, Report, Served, Update, push, upload};

/// 跨重连都记着的：主会话、会话还没开时切的权限级别、另外订阅着的会话、命令对着哪个会话。
#[derive(Debug, Default)]
pub(super) struct Link {
    /// 主会话：界面开的那个。`/new` 以后、说第一句话以前没有。
    pub main: Option<String>,
    /// 会话还没开时切的权限级别：开会话时补发（「权限级别」第 2 条）。
    pub pending: Option<Level>,
    /// 另外订阅着的：子代理的会话、`/new` 以后还有任务在跑的旧会话。
    pub watched: BTreeSet<String>,
    /// 切进了哪个子会话：命令对着它；没切是 `None`，对着主会话（「切进子会话」第 3 条）。
    pub viewing: Option<String>,
}

impl Link {
    /// 命令对着的会话。
    fn target(&self) -> Option<String> {
        self.viewing.clone().or_else(|| self.main.clone())
    }
}

/// 等着回应、回应要另外办的请求（记着请求的编号）。
#[derive(Debug, Clone, PartialEq, Eq)]
enum Awaiting {
    /// 撤销：回应里给人看的几样交给界面。
    Revert,
    /// 恢复：同上。
    Unrevert,
    /// 说一句话：成了不用管，拒了要撤掉先画上的那句。
    Send,
    /// 重做：同上。
    Redo,
    /// 订阅另一个会话：回应里的限额交给界面，说明是哪个会话。
    Watch(String),
    /// 读一条后台命令的输出：哪个会话、任务编号。
    Output(String, String),
}

/// 在一条连接上收发，直到界面关了或者连接断了。
pub(super) async fn serve(
    rpc: &mut Rpc,
    link: &mut Link,
    commands: &mut mpsc::UnboundedReceiver<Command>,
    notify: &impl Fn(Update) -> bool,
) -> Served {
    let cwd = cwd();
    let mut awaiting: HashMap<String, Awaiting> = HashMap::new();
    // 重连以后，另外订阅着的照旧订阅上（「连核心」第 7 条）。
    for session in link.watched.clone() {
        match watch(rpc, &session).await {
            Ok(id) => awaiting.insert(id, Awaiting::Watch(session)),
            Err(_) => return Served::Lost,
        };
    }
    loop {
        tokio::select! {
            command = commands.recv() => {
                let Some(command) = command else { return Served::Quit };
                match command {
                    // 懒着开（施工会话 09-30 建议）：等第一句话再开，连按几下不留空会话。旧的有任务在跑的照样订阅着。
                    Command::New { keep } => {
                        link.viewing = None;
                        if let Some(old) = link.main.take() {
                            if keep {
                                link.watched.insert(old);
                            } else if unsubscribe(rpc, &old).await.is_err() {
                                return Served::Lost;
                            }
                        }
                        continue;
                    }
                    Command::Watch(session) => {
                        let fresh = Some(&session) != link.main.as_ref() && link.watched.insert(session.clone());
                        if fresh {
                            match watch(rpc, &session).await {
                                Ok(id) => awaiting.insert(id, Awaiting::Watch(session)),
                                Err(_) => return Served::Lost,
                            };
                        }
                        continue;
                    }
                    Command::Unwatch(session) => {
                        let gone = link.watched.remove(&session) && Some(&session) != link.main.as_ref();
                        if gone && unsubscribe(rpc, &session).await.is_err() {
                            return Served::Lost;
                        }
                        continue;
                    }
                    Command::View(session) => {
                        link.viewing = session;
                        continue;
                    }
                    // 读输出对着派它的那个会话，不管现在对着哪个（`protocol.md` 的 `job.output`）。
                    Command::Output { session, job, tail } => {
                        let params = json!({"session": session, "job": job, "tail": tail});
                        match rpc.send("job.output", params).await {
                            Ok(id) => awaiting.insert(id, Awaiting::Output(session, job)),
                            Err(_) => return Served::Lost,
                        };
                        continue;
                    }
                    Command::Level(level) if link.target().is_none() => {
                        link.pending = Some(level);
                        continue;
                    }
                    Command::Send { .. } if link.target().is_none() => {
                        match fresh(rpc).await {
                            Ok((id, limits)) => {
                                if !notify(Update::Ready(id.clone())) || !notify(Update::Limits(limits)) {
                                    return Served::Quit;
                                }
                                // 会话还没开时切过权限级别：先补发，再说话。
                                if let Some(level) = link.pending.take() {
                                    let mut params = level.permission();
                                    params["session"] = json!(id);
                                    if rpc.send("session.set_permission_level", params).await.is_err() {
                                        return Served::Lost;
                                    }
                                }
                                link.main = Some(id);
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
                let Some(session) = link.target() else { continue };
                match send(rpc, command, &session, &cwd, notify).await {
                    Sent::Lost => return Served::Lost,
                    Sent::Quit => return Served::Quit,
                    Sent::Skipped => {}
                    Sent::Awaiting(id, kind) => {
                        awaiting.insert(id, kind);
                    }
                }
            }
            message = rpc.next() => {
                let Some(message) = message else { return Served::Lost };
                if !take(rpc, link, &message, &mut awaiting, notify).await {
                    return Served::Quit;
                }
            }
        }
    }
}

/// 一个命令发出去的结果。
enum Sent {
    /// 发出去了，回应要另外办。
    Awaiting(String, Awaiting),
    /// 发出去了不用管，或者没发（没有对应的方法、附件传不上已经告诉界面了）。
    Skipped,
    /// 连接断了。
    Lost,
    /// 界面关了。
    Quit,
}

/// 照命令发一条请求给 `session`：带附件的先把每个文件交给核心，传不上的整句不发（「输入框」第 12 条）；重做换了附件的
/// 也一样，换成没有附件的带一个空的（「输入框」第 13 条）。
async fn send(
    rpc: &mut Rpc,
    command: Command,
    session: &str,
    cwd: &str,
    notify: &impl Fn(Update) -> bool,
) -> Sent {
    let kind = match command {
        Command::Revert => Some(Awaiting::Revert),
        Command::Unrevert => Some(Awaiting::Unrevert),
        Command::Send { .. } => Some(Awaiting::Send),
        Command::Redo { .. } => Some(Awaiting::Redo),
        _ => None,
    };
    let files = match &command {
        Command::Send { files, .. } if !files.is_empty() => Some(files.clone()),
        Command::Redo { files, .. } => files.clone(),
        _ => None,
    };
    let mut attachments = None;
    if let Some(files) = files {
        match upload::attach(rpc, &files).await {
            Ok(got) => attachments = Some(got),
            Err(Update::Disconnected) => return Sent::Lost,
            // 传不上：这一句没发出去（「输入框」第 12 条）。
            Err(Update::Refused { reason, message }) => {
                let told = notify(Update::Unsent { reason, message });
                return if told { Sent::Skipped } else { Sent::Quit };
            }
            Err(update) => {
                return if notify(update) {
                    Sent::Skipped
                } else {
                    Sent::Quit
                };
            }
        }
    }
    let Some((method, mut params)) = request(command, session, cwd) else {
        return Sent::Skipped;
    };
    if let Some(attachments) = attachments {
        params["attachments"] = json!(attachments);
    }
    match (rpc.send(method, params).await, kind) {
        (Err(_), _) => Sent::Lost,
        (Ok(id), Some(kind)) => Sent::Awaiting(id, kind),
        (Ok(_), None) => Sent::Skipped,
    }
}

/// `/new` 以后的第一句话：开会话、订阅。
async fn fresh(rpc: &mut Rpc) -> Result<(String, Limits), Update> {
    let id = create(rpc).await?;
    let limits = subscribe(rpc, &id).await?;
    Ok((id, limits))
}

/// 订阅另一个会话，不等回应（回应里的限额由 [`take`] 交给界面）。
async fn watch(rpc: &mut Rpc, session: &str) -> io::Result<String> {
    let params = json!({"session": session, "stream": "events"});
    rpc.send("subscribe", params).await
}

/// 退订一个会话。
async fn unsubscribe(rpc: &mut Rpc, session: &str) -> io::Result<String> {
    let params = json!({"session": session, "stream": "events"});
    rpc.send("unsubscribe", params).await
}

/// 处理一条读进来的：推送、回应。交回界面还在不在。
async fn take(
    rpc: &mut Rpc,
    link: &Link,
    message: &Value,
    awaiting: &mut HashMap<String, Awaiting>,
    notify: &impl Fn(Update) -> bool,
) -> bool {
    let kind = message["id"].as_str().and_then(|id| awaiting.remove(id));
    if let Some(error) = message.get("error") {
        let reason = error["data"]["reason"].as_str().map(str::to_string);
        let message = error["message"].as_str().unwrap_or_default().to_string();
        return match kind {
            Some(Awaiting::Send | Awaiting::Redo) => notify(Update::Unsent { reason, message }),
            // 订阅不上（那个会话已经删了）：不用说。
            Some(Awaiting::Watch(_)) => true,
            // 读不了输出（任务没了、是子代理）：不用说，界面不再等它。
            Some(Awaiting::Output(session, job)) => notify(Update::Output {
                session,
                job,
                output: None,
            }),
            _ => notify(Update::Refused { reason, message }),
        };
    }
    match kind {
        Some(Awaiting::Revert | Awaiting::Unrevert) => {
            let restore = kind == Some(Awaiting::Unrevert);
            let report = Report::read(&message["result"]);
            return notify(Update::Undone { restore, report });
        }
        // 说的话、重做成了：落盘、开轮都照推送来，回应不用管。
        Some(Awaiting::Send | Awaiting::Redo) => return true,
        Some(Awaiting::Output(session, job)) => {
            let output = Some(JobOutput::read(&message["result"]));
            return notify(Update::Output {
                session,
                job,
                output,
            });
        }
        Some(Awaiting::Watch(session)) => {
            return Limits::of(message).is_none_or(|limits| {
                let update = Box::new(Update::Limits(limits));
                notify(Update::Elsewhere { session, update })
            });
        }
        None => {}
    }
    // 掉队后重新订阅主会话的回应：限额照样带着，照它更新（核心重启以后载入的也是这样）。
    if let Some(limits) = Limits::of(message) {
        return notify(Update::Limits(limits));
    }
    let Some(session) = message["params"]["session"].as_str() else {
        return true;
    };
    // 只收主会话和另外订阅着的：`/new` 以后，旧会话退订之前推来的不要（蓝图「斜杠命令」`/new`）。
    let main = link.main.as_deref() == Some(session);
    if !main && !link.watched.contains(session) {
        return true;
    }
    let wrap = |update: Update| {
        if main {
            update
        } else {
            let session = session.to_string();
            Update::Elsewhere {
                session,
                update: Box::new(update),
            }
        }
    };
    match message["method"].as_str() {
        Some("event") => push::read(&message["params"]["event"], &rpc::owns)
            .into_iter()
            .all(|p| notify(wrap(Update::Push(p)))),
        // 掉了队：重新订阅，掉了的不补（`protocol.md`「慢和掉队」）。发不出去是连接断了，下一条读不到，照断开重连。
        Some("resync") => {
            if let Ok(id) = watch(rpc, session).await
                && !main
            {
                awaiting.insert(id, Awaiting::Watch(session.to_string()));
            }
            true
        }
        _ => true,
    }
}
