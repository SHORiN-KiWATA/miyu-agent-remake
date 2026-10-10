//! 在一条连接上收发（蓝图 `tui.md`「连核心」「后台命令、子代理和侧边栏」「切进子会话」）：命令对着主会话，或者切进了
//! 的子会话；推来的照会话分，主会话的直接交给界面，另外订阅着的包成 [`Update::Elsewhere`]。

use std::collections::{BTreeSet, HashMap};
use std::io;

use serde_json::json;
use tokio::sync::mpsc;

use super::awaiting::Awaiting;
use super::connect::{create, subscribe};
use super::replay::Replay;
use super::request::request;
use super::rpc::Rpc;
use super::sessions;
use super::spawn::Served;
use super::{Command, Level, Update, page, upload};
use super::{asides, config};

/// 跨重连都记着的：主会话、会话还没开时切的权限级别、另外订阅着的会话、命令对着哪个会话。
#[derive(Debug, Default)]
pub struct Link {
    /// 主会话：界面开的那个。`/new` 以后、说第一句话以前没有。
    pub main: Option<String>,
    /// 会话还没开时切的权限级别：开会话时补发（「权限级别」第 2 条）。
    pub pending: Option<Level>,
    /// 另外订阅着的：子代理的会话、`/new` 以后还有任务在跑的旧会话。
    pub watched: BTreeSet<String>,
    /// 切进了哪个子会话：命令对着它；没切是 `None`，对着主会话（「切进子会话」第 3 条）。
    pub viewing: Option<String>,
    /// 每个会话读到的最后一个序号：同一个序号只办一次，掉了队照它补（「会话列表」第 6、7 条）。
    pub seen: HashMap<String, u64>,
    /// 正在补发的会话：补发来的照 [`Replay`] 读，回应到了算补完。
    pub replays: HashMap<String, Replay>,
    /// 按页读的会话读到哪了：再往前要哪一页（核心 9-6 下）。
    pub pages: HashMap<String, page::Marks>,
    /// 还没开会话时在 `/model` 选的：开会话时带上（「配置与模型」第 1 条）。
    pub pending_model: Option<String>,
    /// 新会话的人格框选的：开会话时带上，造成了才清（人格写错被拒的，下一次照样带，不悄悄换默认）。
    pub pending_persona: Option<String>,
    /// 新会话的预设框选的：和 `pending_persona` 一样。
    pub pending_preset: Option<String>,
    /// 还没开会话时 `/workspace` 换的、验过的目录：开会话时带上（核心 9-7 下）。
    pub pending_cwd: Option<String>,
    /// 连上以后要带 `after` 订阅主会话（启动时进最近的那个，「会话列表」第 8 条）。
    pub replay_main: bool,
}

impl Link {
    /// 命令对着的会话。
    fn target(&self) -> Option<String> {
        self.viewing.clone().or_else(|| self.main.clone())
    }

    /// 不再订阅 `session`：读到哪了都不记。
    fn forget(&mut self, session: &str) {
        self.seen.remove(session);
        self.pages.remove(session);
    }
}

/// 在一条连接上收发，直到界面关了或者连接断了。
pub(super) async fn serve(
    rpc: &mut Rpc,
    link: &mut Link,
    commands: &mut mpsc::UnboundedReceiver<Command>,
    notify: &impl Fn(Update) -> bool,
) -> Served {
    let mut awaiting: HashMap<String, Awaiting> = HashMap::new();
    // 订阅配置流、读一次界面语言（「界面语言」）。
    match config::follow(rpc).await {
        Ok(id) => awaiting.insert(id, Awaiting::UiLanguage),
        Err(_) => return Served::Lost,
    };
    // 订阅会话列表（核心 9-5）：`/sessions`、别处来的话写来处都照这一份，变了推过来（「会话列表」）。
    match sessions::follow(rpc).await {
        Ok(id) => awaiting.insert(id, Awaiting::SessionsStream),
        Err(_) => return Served::Lost,
    };
    // 启动时进最近的那个会话：连上以后先读最新一页，再带 `after` 订阅（「会话列表」第 5、8 条）。
    if std::mem::take(&mut link.replay_main)
        && let Some(main) = link.main.clone()
    {
        match page::latest(rpc, link, &main).await {
            Ok(id) => awaiting.insert(id, Awaiting::Page(main)),
            Err(_) => return Served::Lost,
        };
    }
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
                // 不对着会话的请求（列会话、置顶、删、模型资料、给人看的字、写配置、读输出）：照 `asides.rs` 的表发。
                if let Some((method, params, kind)) = asides::request(&command) {
                    match rpc.send(method, params).await {
                        Ok(id) => {
                            if let Some(kind) = kind {
                                awaiting.insert(id, kind);
                            }
                        }
                        Err(_) => return Served::Lost,
                    }
                    continue;
                }
                // 读一个 blob：一段段读，读完存成文件（`links.rs`）。
                if let Command::FetchBlob(blob) = &command {
                    if super::links::fetch(rpc, blob, &mut awaiting).await.is_err() {
                        return Served::Lost;
                    }
                    continue;
                }
                match command {
                    // 懒着开（施工会话 09-30 建议）：等第一句话再开，连按几下不留空会话。旧的有任务在跑的照样订阅着。
                    Command::New { keep } => {
                        link.viewing = None;
                        link.pending_cwd = None;
                        if let Some(old) = link.main.take() {
                            if keep {
                                link.watched.insert(old);
                            } else {
                                link.forget(&old);
                                if unsubscribe(rpc, &old).await.is_err() {
                                    return Served::Lost;
                                }
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
                        link.forget(&session);
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
                    // 更早的一页：没按页读的（核心旧）不要，告诉界面没有了。
                    Command::Older(session) => {
                        match page::older(rpc, link, &session).await {
                            Some(Ok(id)) => {
                                awaiting.insert(id, Awaiting::Older(session));
                            }
                            Some(Err(_)) => return Served::Lost,
                            None => {
                                let none = Update::Older { pushes: Vec::new(), more: false, failed: None };
                                if !notify(Update::Elsewhere { session, update: Box::new(none) }) {
                                    return Served::Quit;
                                }
                            }
                        }
                        continue;
                    }
                    // 切会话：原来的还忙着的照样订阅着，不然退订；没订阅着的先读最新一页（「会话列表」第 4、5 条）。
                    Command::Open { session, keep } => {
                        if let Some(old) = link.main.clone().filter(|old| *old != session) {
                            if keep {
                                link.watched.insert(old);
                            } else {
                                link.forget(&old);
                                if unsubscribe(rpc, &old).await.is_err() {
                                    return Served::Lost;
                                }
                            }
                        }
                        link.viewing = None;
                        link.main = Some(session.clone());
                        if !link.watched.remove(&session) {
                            match page::latest(rpc, link, &session).await {
                                Ok(id) => awaiting.insert(id, Awaiting::Page(session)),
                                Err(_) => return Served::Lost,
                            };
                        }
                        continue;
                    }
                    // 还没开会话：记着，开会话时带上，当场算换成了（「配置与模型」第 1 条）。
                    Command::Configure(reference) if link.target().is_none() => {
                        link.pending_model = Some(reference.clone());
                        if !notify(Update::Configured(reference)) {
                            return Served::Quit;
                        }
                        continue;
                    }
                    // 新会话的人格：记着，开会话时带上；会话开着时不能换，不管（「新会话：人格、工作区」第 2 条）。
                    Command::Persona(id) => {
                        if link.target().is_none() {
                            link.pending_persona = Some(id);
                        }
                        continue;
                    }
                    Command::Preset(id) => {
                        if link.target().is_none() {
                            link.pending_preset = Some(id);
                        }
                        continue;
                    }
                    // 新会话开在哪（核心 9-7 下）：记着，开会话时带上。
                    Command::NewWorkspace(dir) => {
                        link.pending_cwd = dir;
                        continue;
                    }
                    // 还没开会话时验一个目录：连问两次，交回结果（`workspace.rs`）。
                    Command::CheckDir { path, cwd } => {
                        let result = match super::workspace::check(rpc, &path, &cwd).await {
                            Ok(result) => result,
                            Err(_) => return Served::Lost,
                        };
                        if !notify(Update::DirChecked(result)) {
                            return Served::Quit;
                        }
                        continue;
                    }
                    Command::Level(level) if link.target().is_none() => {
                        link.pending = Some(level);
                        continue;
                    }
                    Command::Send { .. } if link.target().is_none() => {
                        let chosen = super::connect::Chosen {
                            model: link.pending_model.take(),
                            persona: link.pending_persona.clone(),
                            preset: link.pending_preset.clone(),
                            cwd: link.pending_cwd.clone(),
                        };
                        match fresh(rpc, &chosen).await {
                            Ok((id, joined)) => {
                                if !notify(Update::Ready(id.clone()))
                                    || !notify(Update::Limits(joined.limits))
                                    || joined.current.is_some_and(|c| !notify(Update::CurrentModel(c)))
                                    || joined.persona.is_some_and(|p| !notify(Update::SessionPersona(p)))
                                    || joined.preset.is_some_and(|p| !notify(Update::SessionPreset(p)))
                                    || joined.workspace.is_some_and(|cwd| {
                                        !notify(Update::Workspace { cwd, joined: false })
                                    })
                                {
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
                                link.pending_persona = None;
                                link.pending_preset = None;
                                link.pending_cwd = None;
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
                match send(rpc, command, &session, notify).await {
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
                if !super::take::take(rpc, link, &message, &mut awaiting, notify).await {
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
    notify: &impl Fn(Update) -> bool,
) -> Sent {
    let kind = match command {
        Command::Revert => Some(Awaiting::Revert),
        Command::Unrevert => Some(Awaiting::Unrevert),
        Command::Send { .. } => Some(Awaiting::Send),
        Command::Redo { .. } => Some(Awaiting::Redo),
        Command::Recap => Some(Awaiting::Recap),
        Command::Configure(ref reference) => Some(Awaiting::Configure(reference.clone())),
        Command::Rename(ref title) => Some(Awaiting::Rename(title.clone())),
        Command::Answer {
            session: ref asker,
            ref call,
            ..
        } => Some(Awaiting::Answer(
            asker.clone().unwrap_or_else(|| session.to_string()),
            call.clone(),
        )),
        Command::Run(_) | Command::Workspace { .. } => Some(Awaiting::CommandRun),
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
    let Some((method, mut params)) = request(command, session) else {
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
async fn fresh(
    rpc: &mut Rpc,
    chosen: &super::connect::Chosen,
) -> Result<(String, super::connect::Joined), Update> {
    let id = create(rpc, chosen).await?;
    let joined = subscribe(rpc, &id).await?;
    Ok((id, joined))
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
