//! 连核心：找数据根、连上（给了 `MIYU_CORE_BIN` 的，没在跑就拉起来）、握手、开会话、订阅，然后收发。
//!
//! 走法照 `miyu ask`（`docs/blueprint/cli/ask.md`「怎么走」）。界面的主循环是同步的，这里在单独的线程里跑
//! 一个 tokio 运行时；两边只靠通道说话：界面发 [`Command`]，这里回 [`Update`]。
//!
//! 拉起核心只认 `MIYU_CORE_BIN`，不去 PATH 里找 `miyu`：装着旧版的机器上，PATH 里的 `miyu` 是旧版，
//! 给它 `core` 这个参数，它会把这个词当成一句话发给旧版的后台。

mod asides;
mod awaiting;
mod backoff;
mod config;
mod connect;
mod efforts;
mod kinds;
mod limits;
mod links;
mod mermaid;
mod models;
mod output;
mod push;
mod replay;
mod request;
mod rpc;
mod serve;
mod sessions;
mod switch;
mod take;
mod todos;
mod undo;
mod upload;

use std::thread;

use tokio::sync::mpsc;

use backoff::Backoff;
use connect::{connect, subscribe};

pub use efforts::{EffortList, Efforts};
pub use kinds::{EndReason, Level, ToolStatus};
pub use limits::Limits;
pub use links::{Card, CardKind, blob_path, cards_dir};
pub use mermaid::{Marks, Rendered};
pub use models::{Choice, ChoiceState, Current};
pub use output::JobOutput;
pub use push::{
    Asking, Block, CallError, Compaction, JobEnd, JobReason, JobStart, Push, Sender, Usage,
};
use rpc::Rpc;
pub use sessions::{Change as SessionChange, SessionInfo, apply as apply_session_change};
pub use todos::TodoItem;
pub use undo::{Report, UndoFile};

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
    /// 开新会话（`/new`）：等第一句话再开（蓝图「斜杠命令」`/new`）。`keep`：旧的还有后台任务在跑，照样订阅着，等界面
    /// 说它的任务都报完了再退订（「后台命令、子代理和侧边栏」）；不然马上退订。
    New {
        /// 旧会话照样订阅着。
        keep: bool,
    },
    /// 切到别的会话（`/sessions`，蓝图「会话列表」第 4、5 条）：它成了主会话；原来的 `keep` 为真时照样订阅着（还忙着），
    /// 不然退订。没订阅着的带 `after: 0` 订阅，以前的事件补发过来。
    Open {
        /// 切到哪个会话。
        session: String,
        /// 原来那个照样订阅着。
        keep: bool,
    },
    /// 列出会话（`session.list`），交回 [`Update::Sessions`]。
    ListSessions,
    /// 置顶、取消置顶一个会话（`session.set_meta` 的 `pinned`），不管对着哪个会话。
    Pin {
        /// 哪个会话。
        session: String,
        /// 置顶。
        pinned: bool,
    },
    /// 删掉一个会话（`session.delete`），不管对着哪个会话。
    Delete(String),
    /// 要 `/model` 框里的一行行（`model.list`），交回 [`Update::Choices`]。
    ListChoices,
    /// 要一个网址的链接卡片（`link.preview`，核心 W-7）。
    LinkPreview(String),
    /// 读回一个 blob 存成文件（卡片的封面图、图标；`blob.get`，核心 W-6）。
    FetchBlob(String),
    /// 把一张 mermaid 图的源码交给核心画成 SVG（`mermaid.render`，核心 W-4）。
    RenderMermaid(String),
    /// 手动换的模型也记成新会话的默认：写个人设置的 `models.chat`（2026-10-02 项目主人定，`models.md`「头的约定」）。
    SetChat(String),
    /// `/effort` 要的：每个模型有哪几级、配置的默认、配置键（`model.list`，核心 8-18 补）。
    ListEfforts,
    /// 写个人设置里一个模型的思考强度：配置键、哪一级（`None` 是去掉这一项，回到供应商定）。
    SetEffort {
        /// 配置键（照 `facts.effort.key` 抄）。
        key: String,
        /// 哪一级。
        level: Option<String>,
    },
    /// `@` 文件列表问核心（`fs.list`、`fs.find`，核心 W-2）：哪个词问的、方法、参数。
    Files {
        /// 哪个词问的：回应照它认。
        word: crate::mention::Word,
        /// `fs.list` 或 `fs.find`。
        method: &'static str,
        /// 参数。
        params: serde_json::Value,
    },
    /// 这个会话换模型（`session.configure`，下一个回合开始生效，核心 8-10）：引用。还没开会话的记着，开会话时带上。
    Configure(String),
    /// 要模型资料（`model.list`），交回冷却着的最早什么时候恢复（[`Update::CoolingUntil`]，「配置与模型」第 7 条）。
    ListModels,
    /// 要这种语言的给人看的字（`human.get`，核心 W-1）：语言代码。回 [`Update::Human`]。
    FetchHuman(String),
    /// 界面语言写进个人设置（`ui.language`，`auto` 或者语言代码；蓝图「界面语言」）。
    SetLanguage(String),
    /// 另外订阅一个会话：子代理的会话（「后台命令、子代理和侧边栏」、「切进子会话」）。它推来的包成 [`Update::Elsewhere`]。
    Watch(String),
    /// 退订另外订阅着的一个会话。
    Unwatch(String),
    /// 命令对着哪个会话：切进了的子会话；`None` 是主会话（「切进子会话」第 3 条）。
    View(Option<String>),
    /// 停掉一个后台任务（`job.stop`，带任务编号）。
    Stop(String),
    /// 读一条后台命令到这时为止的输出（`job.output`）：哪个会话派的、任务编号、要最后几行。
    Output {
        /// 哪个会话派的。
        session: String,
        /// 任务编号。
        job: String,
        /// 要最后几行。
        tail: usize,
    },
    /// 切权限级别（`session.set_permission_level`）：切到这一级。会话还没开的记着，开了再发。
    Level(Level),
    /// 一条斜杠命令交给核心办（`command.run`，核心 O-6）：`/stop`、`/clear`，原文照规范的名字。
    Run(String),
    /// 要一段回顾（`session.recap`，`/recap`）。
    Recap,
    /// 改名（`session.set_meta`，`/rename`）：`None` 是去掉标题。
    Rename(Option<String>),
    /// 回答一次确认或提问（`session.answer`，核心 D-1）：问的那个会话（主会话的是 `None`）、调用编号、`decision` 或
    /// `answers`（连同 `reason`）那几格。
    Answer {
        /// 问的那个会话；`None` 是主会话。
        session: Option<String>,
        /// 调用编号。
        call: String,
        /// `{"decision": …}` 或 `{"answers": […]}`。
        body: serde_json::Value,
    },
    /// 不对着会话的一条请求，回应原样交回（[`Update::Answer`]）：配置页用（蓝图「配置页」第 23 条），不为每个方法再加一对命令。
    Ask {
        /// 界面自己的编号：回应照它认。
        tag: u64,
        /// 方法。
        method: &'static str,
        /// 参数。
        params: serde_json::Value,
    },
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
    /// `/model` 框里的一行行（[`Command::ListChoices`] 的回应）；要不到的是空的。
    Choices(Vec<Choice>),
    /// 一个网址的卡片（[`Command::LinkPreview`]）；要不到的是 `None`。
    LinkCard {
        /// 网址。
        url: String,
        /// 卡片。
        card: Option<Card>,
    },
    /// 一个 blob 存成了文件（[`Command::FetchBlob`]）；读不成的是 `None`。
    BlobSaved {
        /// 哪个 blob。
        blob: String,
        /// 存在哪。
        path: Option<std::path::PathBuf>,
    },
    /// 一张 mermaid 图画好了（[`Command::RenderMermaid`]）；核心拒了、没编进 mermaid 的是 `None`。
    Mermaid {
        /// 源码。
        source: String,
        /// 画好的。
        svg: Option<Rendered>,
    },
    /// `/effort` 框里的几级（[`Command::ListEfforts`] 的回应）；要不到的是空的。
    Efforts(EffortList),
    /// `@` 文件列表的回应（[`Command::Files`]）：哪个词问的、回应的 `result`（回了错的是 `None`）。
    Files {
        /// 哪个词问的。
        word: crate::mention::Word,
        /// 回应。
        result: Option<serde_json::Value>,
    },
    /// 换模型成了（[`Command::Configure`] 的回应）：引用。
    Configured(String),
    /// 会话现在用的模型（订阅的回应里的 `model`）。
    CurrentModel(Current),
    /// 冷却着的模型里最早什么时候恢复（[`Command::ListModels`] 的回应）；没有的是 `None`。
    CoolingUntil(Option<jiff::Timestamp>),
    /// 给人看的字（[`Command::FetchHuman`] 的回应）。
    Human(crate::human::Human),
    /// 配置里界面语言的最终值（`ui.language`：`auto` 或者语言代码）：连上时读一次，别处改了再读（「界面语言」）。
    UiLanguage(String),
    /// 会话列表（[`Command::ListSessions`] 的回应）：只有主会话，照核心交回的先后。
    Sessions(Vec<SessionInfo>),
    /// 改名成了（`None` 是去掉了标题）：弹一句提示，标题照推送换（蓝图「改名」第 3 条）。
    Renamed(Option<String>),
    /// 斜杠命令办了（`command.run` 的回应）：规范的命令名、照连接语言写好的回执。
    CommandRan {
        /// `stop`、`clear`。
        command: String,
        /// 回执那一句；读不出来的是空的。
        said: String,
    },
    /// 回顾交回的是上一句（`cached`：上次回顾以后没有新内容，核心不推 `session.recapped`），照它画（蓝图「回顾」第 3 条）。
    Recap(String),
    /// 撤销（`restore` 为假）或恢复成了：核心算好的给人看的几样（`protocol/undo.md`）。
    Undone {
        /// 是恢复。
        restore: bool,
        /// 回应里给人看的几样。
        report: Report,
    },
    /// 另外订阅着的会话推来的（推送、限额），带着是哪个会话。
    Elsewhere {
        /// 哪个会话。
        session: String,
        /// 推来的。
        update: Box<Update>,
    },
    /// 说的话、重做没发出去（核心拒了、附件传不上）：和 [`Update::Refused`] 一样的两样；先画进正文的那句要撤掉，
    /// 字放回输入框（「输入框」第 12、13 条）。
    Unsent {
        /// 原因码，例如 `not_redoable`。
        reason: Option<String>,
        /// 核心的原话。
        message: String,
    },
    /// [`Command::Ask`] 的回应：成了是 `result`，拒了是原因码和原话。
    Answer {
        /// 界面发的时候给的编号。
        tag: u64,
        /// 回应。
        result: Result<serde_json::Value, Refusal>,
    },
    /// 回答确认、提问被拒（[`Command::Answer`]）：调用编号、原因码、原话。`not_asking` 是已经答过、了结了。
    AnswerRefused {
        /// 问的那个会话（整个编号）。
        session: String,
        /// 调用编号。
        call: String,
        /// 原因码。
        reason: Option<String>,
        /// 核心的原话。
        message: String,
    },
    /// 会话列表变了（`sessions.changed`，核心 9-5）：整项换、删掉。
    SessionChanged(SessionChange),
    /// 配置变了（推来的 `config.changed`，哪个头、哪一层改的都算）：开着配置页的重读（「配置页」第 25 条）。
    ConfigChanged,
    /// 一条后台命令的输出（[`Command::Output`] 的回应）；读不了的（任务没了、是子代理）是 `None`。
    Output {
        /// 哪个会话派的。
        session: String,
        /// 任务编号。
        job: String,
        /// 读到的。
        output: Option<JobOutput>,
    },
}

/// 核心拒了一条请求：原因码（`data.reason`，没有的是 `None`）、原话，和 `data` 整个（`config_conflict` 的 `current` 在里面）。
#[derive(Debug, Clone, PartialEq)]
pub struct Refusal {
    /// 原因码。
    pub reason: Option<String>,
    /// 核心照握手时的语言说的原话。
    pub message: String,
    /// 错误的 `data`。
    pub data: serde_json::Value,
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

/// 启动时进哪个会话。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Start {
    /// 照配置 `ui.startup`：`recent` 的进最近的那个，别的等第一句话再开。
    Usual,
    /// 进指定的会话（`--resume`）。
    Resume(String),
    /// 不进任何会话、不看 `ui.startup`（`--page config`，蓝图「配置页」第 1 条）。
    Bare,
}

/// 起一个线程去连核心。`reconnect` 是连不上时隔多久再试（`layout.json` 的 `reconnect_ms`）；`notify` 把消息
/// 交给界面，界面那头关了就交回 `false`，这边跟着停。
pub fn spawn(
    reconnect: [u64; 2],
    start: Start,
    notify: impl Fn(Update) -> bool + Send + 'static,
) -> Core {
    let (commands, receiver) = mpsc::unbounded_channel();
    thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build();
        match runtime {
            Ok(runtime) => runtime.block_on(run(receiver, Backoff::new(reconnect), start, &notify)),
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
    start: Start,
    notify: &impl Fn(Update) -> bool,
) {
    let recent = start == Start::Usual;
    let resume = match start {
        Start::Resume(id) => Some(id),
        Start::Usual | Start::Bare => None,
    };
    // 主会话、另外订阅着的、命令对着哪个：断了重连也记着（`serve.rs`）。
    let mut link = serve::Link::default();
    // 启动时进最近的那个会话只在头一次连上时看（「会话列表」第 8 条）；之后重连、`/new` 照旧。
    let mut first = true;
    loop {
        // 连不上一直试；这期间界面发的命令在通道里排着，连上再发（第 1、7 条）。
        let mut rpc = loop {
            match open(
                link.main
                    .as_deref()
                    .or(if first { resume.as_deref() } else { None }),
                first,
                recent,
            )
            .await
            {
                Ok((rpc, opened, limits)) => {
                    // 进了已有的会话：订阅留给收发时带 `after` 做，以前的补发过来。
                    link.replay_main = limits.is_none() && opened.is_some();
                    first = false;
                    // 新开的会话（刚启动；按过 `/new` 还没说话就断了的）告诉界面编号，订阅原来的只说又连上了。
                    let said = match (&link.main, &opened) {
                        (None, Some(id)) => notify(Update::Ready(id.clone())),
                        _ => notify(Update::Reconnected),
                    };
                    let (limits, current) = limits.map_or((None, None), |(l, c)| (Some(l), c));
                    if !said
                        || limits.is_some_and(|l| !notify(Update::Limits(l)))
                        || current.is_some_and(|c| !notify(Update::CurrentModel(c)))
                    {
                        return;
                    }
                    link.main = opened;
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
        match serve::serve(&mut rpc, &mut link, &mut commands, notify).await {
            Served::Quit => return,
            Served::Lost if !notify(Update::Disconnected) => return,
            Served::Lost => {}
        }
    }
}

/// 连上；有会话的订阅它。还没有的（刚启动、`/new` 以后）不开，和 `/new` 一样等第一句话时才开（蓝图「连核心」第 4 条：
/// 没说话就退出的不留空会话）。`first`：头一次连上，配置 `ui.startup` 是 `recent` 的进最近的那个已有会话，不在这里
/// 订阅（限额交回 `None`），收发时带 `after` 订阅；一个都没有的照样等第一句话。显式恢复的先验证 ID，
/// 成功后从头补发，失败不回退到 recent。交回连接、会话和限额。
async fn open(
    session: Option<&str>,
    first: bool,
    recent: bool,
) -> Result<(Rpc, Option<String>, Option<(Limits, Option<Current>)>), Update> {
    let mut rpc = connect().await?;
    if first && let Some(id) = session {
        // 先验证指定会话可载入，成功以后统一从头补发；失败留在原 ID，不挑 recent。
        subscribe(&mut rpc, id).await?;
        return Ok((rpc, Some(id.to_string()), None));
    }
    if session.is_none() && first && recent && switch::wants_recent(&mut rpc).await {
        let list = rpc
            .call("session.list", serde_json::json!({}))
            .await
            .map_err(connect::refused)?;
        if let Some(id) = switch::recent(&list) {
            return Ok((rpc, Some(id), None));
        }
    }
    let Some(session) = session else {
        return Ok((rpc, None, None));
    };
    let (limits, current) = subscribe(&mut rpc, session).await?;
    Ok((rpc, Some(session.to_string()), Some((limits, current))))
}

#[cfg(test)]
mod tests;
