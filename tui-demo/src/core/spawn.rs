//! 起一条和核心说话的线程：连上、断了照退避重连，界面的命令照顺序发（从 `mod.rs` 分出来）。

use super::*;

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
pub(super) enum Served {
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
                    let (limits, current, snapshot, workspace) = limits
                        .map_or((None, None, None, None), |j| {
                            (Some(j.limits), j.current, j.snapshot, j.workspace)
                        });
                    let workspace = workspace.map(|cwd| Update::Workspace { cwd, joined: false });
                    if !said
                        || limits.is_some_and(|l| !notify(Update::Limits(l)))
                        || current.is_some_and(|c| !notify(Update::CurrentModel(c)))
                        || snapshot.is_some_and(|s| !notify(Update::Snapshot(s)))
                        || workspace.is_some_and(|w| !notify(w))
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
) -> Result<(Rpc, Option<String>, Option<connect::Joined>), Update> {
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
    // 重连订阅原来那个会话：人格、预设正文里记着（头一次订阅时读的），这里不用再读。
    let joined = subscribe(&mut rpc, session).await?;
    Ok((rpc, Some(session.to_string()), Some(joined)))
}
