//! 握手以后的方法（`docs/designs/04-核心协议.md` 第九节「先做的几样怎么写」）：造会话、说话、打断，
//! 列出会话（施工 3-9 下），撤销、恢复（施工 4-7 上；回应带上给人看的几样，施工 4-7 下），手动压缩（施工 6-8），切权限级别
//! （施工 3-8 再补），清空上下文（施工 6-8 补），停掉一个任务（施工 7-4）。命令交给会话，等它的回应：接受的回 `events`
//! （切权限级别、停掉任务的回 `{}`），拒绝的回原因码。造会话、说话的
//! 回应再带上会话实际在哪个目录里干活（施工 4-5 下）。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::Level;
use miyu_kernel::id::{JobId, Seq, SessionId, TurnId};
use miyu_kernel::session::{Command, Outcome, Queued};
use miyu_session::Handle;

use crate::Core;
use crate::hello::Peer;
use crate::list;
use crate::refusal::Refusal;
use crate::sessions::{Opening, admin};
use crate::undo;
use crate::wire::Request;

/// 没写人格时用的：出厂的软件工程师（施工 3-6 上）。
const PERSONA: &str = "engineer";

/// `session.create` 的参数。
#[derive(Debug, Deserialize)]
struct CreateParams {
    #[serde(default)]
    persona: Option<String>,
    cwd: String,
    /// 一次性的：`miyu ask` 开的（施工 3-9 下）。
    #[serde(default)]
    oneshot: bool,
    /// 加进来的目录（施工 5-10 上）：和工作区一样能读能写。
    #[serde(default)]
    dirs: Vec<String>,
}

/// `session.list` 的参数（施工 3-9 下）。
#[derive(Debug, Deserialize)]
struct ListParams {
    /// 只要一次性的。
    #[serde(default)]
    oneshot: bool,
    /// 最多几个。
    #[serde(default)]
    limit: Option<usize>,
}

/// `session.send` 的参数。
#[derive(Debug, Deserialize)]
struct SendParams {
    session: String,
    text: String,
    #[serde(default)]
    urgent: bool,
    #[serde(default)]
    cwd: Option<String>,
    /// 加进来的目录（施工 5-10 上）：不写的照旧。
    #[serde(default)]
    dirs: Option<Vec<String>>,
}

/// `session.interrupt` 的参数。
#[derive(Debug, Deserialize)]
struct InterruptParams {
    session: String,
    queued: QueuedParam,
}

/// `session.revert` 的参数（施工 4-7 上）：从哪一轮起撤，回合编号就是那一轮 `turn.started` 的序号；不写的撤
/// 最后一轮（施工 4-7 下）。
#[derive(Debug, Deserialize)]
struct RevertParams {
    session: String,
    #[serde(default)]
    turn: Option<u64>,
}

/// `session.unrevert` 的参数（施工 4-7 上）。
#[derive(Debug, Deserialize)]
struct UnrevertParams {
    session: String,
}

/// `session.clear` 的参数（施工 6-8 补）。
#[derive(Debug, Deserialize)]
struct ClearParams {
    session: String,
}

/// `session.compact` 的参数（施工 6-8）：人附的要求可以不写，原样交给内核（只有空白的由内核当没写）。
#[derive(Debug, Deserialize)]
struct CompactParams {
    session: String,
    #[serde(default)]
    instructions: Option<String>,
}

/// `session.set_permission_level` 的参数（施工 3-8 再补）：常用的那一级、只读开关，改哪样写哪样；两格都不写的是参数不对。
#[derive(Debug, Deserialize)]
struct PermissionParams {
    session: String,
    #[serde(default)]
    level: Option<LevelParam>,
    #[serde(default)]
    read_only: Option<bool>,
}

/// 协议上能切到的常用的那一级。只认这两种，别的是参数不对：内核的 `unknown_level` 从协议上碰不到，和 `queued`、`stream`
/// 一样，值不在表里的算参数读不成（`protocol.md` 的 `session.set_permission_level`）。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum LevelParam {
    Workspace,
    Full,
}

/// `job.stop` 的参数（施工 7-4）：哪个会话的哪个任务。
#[derive(Debug, Deserialize)]
struct JobStopParams {
    session: String,
    job: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum QueuedParam {
    Send,
    Return,
}

/// 照方法办一条请求：交回回应的 `result`，或者拒绝。
pub(crate) async fn call(
    core: &Arc<Core>,
    peer: Peer,
    request: &Request,
) -> Result<Value, Refusal> {
    match request.method.as_str() {
        "session.create" => {
            let params: CreateParams = params(request)?;
            let persona = params.persona.as_deref().unwrap_or(PERSONA);
            let who = Opening {
                attended: peer.input,
                oneshot: params.oneshot,
            };
            let created = core
                .sessions
                .create(
                    core,
                    request.id.clone(),
                    persona,
                    params.cwd,
                    params.dirs,
                    who,
                )
                .await?;
            Ok(json!({"session": created.id.as_str(), "events": [1], "cwd": created.cwd}))
        }
        "session.list" => {
            let params: ListParams = params(request)?;
            let sessions = list::list(core, params.oneshot, params.limit).await?;
            Ok(json!({"sessions": sessions}))
        }
        "session.send" => {
            let params: SendParams = params(request)?;
            let blocks = match params.text.is_empty() {
                true => Vec::new(),
                false => vec![Block::Text(Text { text: params.text })],
            };
            let command = Command::Send {
                blocks,
                urgent: params.urgent,
            };
            let session = session(&params.session)?;
            let found = core
                .sessions
                .get(
                    core,
                    &session,
                    params.cwd.as_deref(),
                    params.dirs.as_deref(),
                )
                .await?;
            let events = command_to(core, request, &session, &found.handle, command).await?;
            Ok(json!({"events": events, "cwd": found.cwd}))
        }
        "session.interrupt" => {
            let params: InterruptParams = params(request)?;
            let queued = match params.queued {
                QueuedParam::Send => Queued::Send,
                QueuedParam::Return => Queued::Return,
            };
            let session = session(&params.session)?;
            let found = core.sessions.get(core, &session, None, None).await?;
            let command = Command::Interrupt { queued };
            let events = command_to(core, request, &session, &found.handle, command).await?;
            Ok(json!({ "events": events }))
        }
        "session.revert" => {
            let params: RevertParams = params(request)?;
            let turn = params
                .turn
                .map(|turn| Seq::new(turn).map(TurnId::new).ok_or(Refusal::BAD_PARAMS))
                .transpose()?;
            let session = session(&params.session)?;
            let found = core.sessions.get(core, &session, None, None).await?;
            let command = Command::Revert { turn };
            let events = command_to(core, request, &session, &found.handle, command).await?;
            Ok(undo::reply(core, &session, &found.cwd, events).await)
        }
        "session.unrevert" => {
            let params: UnrevertParams = params(request)?;
            let session = session(&params.session)?;
            let found = core.sessions.get(core, &session, None, None).await?;
            let command = Command::Unrevert;
            let events = command_to(core, request, &session, &found.handle, command).await?;
            Ok(undo::reply(core, &session, &found.cwd, events).await)
        }
        "session.compact" => {
            let params: CompactParams = params(request)?;
            let session = session(&params.session)?;
            let found = core.sessions.get(core, &session, None, None).await?;
            let command = Command::Compact {
                instructions: params.instructions,
            };
            let events = command_to(core, request, &session, &found.handle, command).await?;
            Ok(json!({ "events": events }))
        }
        "session.set_permission_level" => {
            let params: PermissionParams = params(request)?;
            if params.level.is_none() && params.read_only.is_none() {
                return Err(Refusal::BAD_PARAMS);
            }
            let level = params.level.map(|level| match level {
                LevelParam::Workspace => Level::Workspace,
                LevelParam::Full => Level::Full,
            });
            let session = session(&params.session)?;
            let found = core.sessions.get(core, &session, None, None).await?;
            let command = Command::SetPermission {
                level,
                read_only: params.read_only,
            };
            command_to(core, request, &session, &found.handle, command).await?;
            Ok(json!({}))
        }
        "session.clear" => {
            let params: ClearParams = params(request)?;
            let session = session(&params.session)?;
            let found = core.sessions.get(core, &session, None, None).await?;
            let events = command_to(core, request, &session, &found.handle, Command::Clear).await?;
            Ok(json!({ "events": events }))
        }
        "job.stop" => {
            let params: JobStopParams = params(request)?;
            let session = session(&params.session)?;
            let job = JobId::parse(&params.job).map_err(|_| Refusal::BAD_PARAMS)?;
            let found = core.sessions.get(core, &session, None, None).await?;
            match found
                .handle
                .stop_job(job, admin(core), request.id.clone())
                .await
            {
                Ok(Ok(())) => Ok(json!({})),
                Ok(Err(_)) => Err(Refusal::UNKNOWN_JOB),
                Err(_) => {
                    core.sessions.forget(&session).await;
                    Err(Refusal::STOPPED)
                }
            }
        }
        _ => Err(Refusal::UNKNOWN_METHOD),
    }
}

/// 把命令交给会话 `session`（把手是 `handle`），等它的回应：接受的交回它产生的事件的序号。会话停了的
/// 从表里拿掉。
async fn command_to(
    core: &Core,
    request: &Request,
    session: &SessionId,
    handle: &Handle,
    command: Command,
) -> Result<Vec<u64>, Refusal> {
    match handle
        .command(request.id.clone(), admin(core), command)
        .await
    {
        Ok(Outcome::Accepted { events }) => Ok(events.iter().map(|seq| seq.get()).collect()),
        Ok(Outcome::Rejected { reason }) => Err(Refusal::kernel(reason)),
        Err(_) => {
            core.sessions.forget(session).await;
            Err(Refusal::STOPPED)
        }
    }
}

/// 读参数；读不成的是参数不对。
fn params<T: serde::de::DeserializeOwned>(request: &Request) -> Result<T, Refusal> {
    serde_json::from_value(request.params.clone()).map_err(|_| Refusal::BAD_PARAMS)
}

/// 会话编号；不合写法的是参数不对。
fn session(text: &str) -> Result<SessionId, Refusal> {
    SessionId::parse(text).map_err(|_| Refusal::BAD_PARAMS)
}
