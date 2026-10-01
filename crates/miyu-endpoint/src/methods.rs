//! 握手以后的方法（`docs/designs/04-核心协议.md` 第九节「先做的几样怎么写」）：造会话、说话、打断，
//! 列出会话（施工 3-9 下），撤销、恢复（施工 4-7 上；回应带上给人看的几样，施工 4-7 下），重做（施工 4-7 再补），手动压缩
//! （施工 6-8），切权限级别（施工 3-8 再补），清空上下文（施工 6-8 补），回顾（施工 3-8 四补，回应是那一句），停掉一个任务（施工 7-4），读后台命令的输出（施工 7-4 补），
//! 传附件（施工 3-9 三补），改标题、置顶，删除会话（施工 3-8 三补）。别的 harness 带着名字说话（`session.send` 的 `from`，
//! 施工 7-10）。命令交给会话，等它的回应：接受的回 `events`（切权限级别、停掉任务、改标题的回 `{}`），拒绝的回原因码；删除
//! 由会话表办。造会话、说话的回应再带上会话实际在哪个目录里干活（施工 4-5 下），这个目录的项目配置还没问过信不信任的，
//! 再带上它在哪（`untrusted_project`，施工 8-2）。查配置的三个方法在 `config/methods.rs`（施工 8-2），改配置的 `config.set`、
//! 信任项目配置的 `config.trust` 在 `config/set.rs`、`config/trusting.rs`（施工 8-3）。密钥的 `secret.set`、`secret.delete`、
//! `secret.list` 在 `secrets.rs`（施工 8-5）。`model.list` 在 `models.rs`（施工 8-7）。`session.create` 带 `model` 的照这时的
//! 配置解析好再造，解析不出的回 `unknown_model`、什么都不造（施工 8-8，`models::record`）。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::Level;
use miyu_kernel::id::{JobId, Seq, SessionId, TurnId};
use miyu_kernel::origin::By;
use miyu_kernel::session::{Command, Outcome, Queued};
use miyu_session::Handle;

use crate::Core;
use crate::attach::{self, Attachment};
use crate::config;
use crate::from;
use crate::hello::Peer;
use crate::job_output;
use crate::list;
use crate::meta::MetaParams;
use crate::models;
use crate::refusal::Refusal;
use crate::secrets;
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
    /// 用哪个模型（施工 8-8）：模型、`@池` 或者挡位；不写、写 `null` 的照这时的 `models.chat`。
    #[serde(default)]
    model: Option<String>,
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
    /// 附件（施工 3-9 三补）：`blob.put` 的回应，照先后接在文字后面；不写、写 `null` 的是没有。
    #[serde(default)]
    attachments: Option<Vec<Attachment>>,
    /// 别的 harness 报的名字（施工 7-10）：写了的，这一句是它说的；不写、写 `null` 的是本人。
    #[serde(default)]
    from: Option<String>,
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

/// `session.redo` 的参数（施工 4-7 再补）：开这一轮的那一句换成的话、换成的附件，写法照 `session.send`；不写、写 `null` 的
/// 照原来那一句的。
#[derive(Debug, Deserialize)]
struct RedoParams {
    session: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    attachments: Option<Vec<Attachment>>,
}

/// `session.clear` 的参数（施工 6-8 补）。
#[derive(Debug, Deserialize)]
struct ClearParams {
    session: String,
}

/// `session.recap` 的参数（施工 3-8 四补）。
#[derive(Debug, Deserialize)]
struct RecapParams {
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

/// `session.delete` 的参数（施工 3-8 三补）。
#[derive(Debug, Deserialize)]
struct DeleteParams {
    session: String,
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
            // 解析不出的什么都不造（施工 8-8）。
            let model = params
                .model
                .map(|text| models::record(core, &text))
                .transpose()?;
            let who = Opening {
                attended: peer.input,
                oneshot: params.oneshot,
                model,
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
            let mut reply =
                json!({"session": created.id.as_str(), "events": [1], "cwd": created.cwd});
            if let Some(file) = created.untrusted {
                reply["untrusted_project"] = json!(file);
            }
            Ok(reply)
        }
        "session.list" => {
            let params: ListParams = params(request)?;
            let sessions = list::list(core, params.oneshot, params.limit).await?;
            Ok(json!({"sessions": sessions}))
        }
        "session.send" => {
            let params: SendParams = params(request)?;
            // 别的 harness 报的名字先查（施工 7-10）：不对的，会话里什么都不送。
            let by = match &params.from {
                Some(name) => from::harness(name)?,
                None => admin(core),
            };
            let mut blocks = said(params.text);
            let session = session(&params.session)?;
            // 附件先查，再找会话：不对的，会话里什么都不送，`cwd`、`dirs` 也不送（施工 3-9 三补）。
            let attachments = params.attachments.unwrap_or_default();
            blocks.extend(attach::blocks(core, attachments).await?);
            let command = Command::Send {
                blocks,
                urgent: params.urgent,
            };
            let found = core
                .sessions
                .get(
                    core,
                    &session,
                    params.cwd.as_deref(),
                    params.dirs.as_deref(),
                )
                .await?;
            let events = command_by(core, request, &session, &found.handle, by, command).await?;
            let mut reply = json!({"events": events, "cwd": found.cwd});
            let untrusted = core.config().untrusted(&found.cwd);
            if let Some(file) = untrusted {
                reply["untrusted_project"] = json!(file);
            }
            Ok(reply)
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
        "session.redo" => {
            let params: RedoParams = params(request)?;
            let session = session(&params.session)?;
            // 附件照 `session.send` 先查，再找会话。
            let attachments = match params.attachments {
                Some(attachments) => Some(attach::blocks(core, attachments).await?),
                None => None,
            };
            let found = core.sessions.get(core, &session, None, None).await?;
            let command = Command::Redo {
                text: params.text.map(said),
                attachments,
            };
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
        "session.recap" => {
            let params: RecapParams = params(request)?;
            let session = session(&params.session)?;
            let found = core.sessions.get(core, &session, None, None).await?;
            let by = admin(core);
            match outcome(core, request, &session, &found.handle, by, Command::Recap).await? {
                Outcome::Recapped { text, upto, cached } => {
                    Ok(json!({"text": text, "upto": upto.get(), "cached": cached}))
                }
                _ => Err(Refusal::INTERNAL),
            }
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
        "job.output" => job_output::read(core, params(request)?).await,
        "config.schema" => config::methods::schema(core, peer, params(request)?),
        "config.get" => config::methods::get(core, peer, params(request)?),
        "config.check" => config::methods::check(core, peer, params(request)?),
        "config.set" => config::set::set(core, peer, &request.id, params(request)?),
        "config.trust" => config::trusting::trust(core, peer, &request.id, params(request)?),
        "secret.set" => secrets::set(core, peer, &request.id, params(request)?),
        "secret.delete" => secrets::delete(core, peer, &request.id, params(request)?),
        "secret.list" => Ok(secrets::list(core)),
        "model.list" => models::list(core, params(request)?).await,
        "blob.put" => attach::put(core, params(request)?).await,
        "session.set_meta" => {
            let params: MetaParams = params(request)?;
            let command = params.command()?;
            let session = session(&params.session)?;
            let found = core.sessions.get(core, &session, None, None).await?;
            command_to(core, request, &session, &found.handle, command).await?;
            Ok(json!({}))
        }
        "session.delete" => {
            let params: DeleteParams = params(request)?;
            let session = session(&params.session)?;
            core.sessions.delete(core, &session).await?;
            Ok(json!({}))
        }
        _ => Err(Refusal::UNKNOWN_METHOD),
    }
}

/// 把命令交给会话 `session`（把手是 `handle`），记成管理员发的，等它的回应：接受的交回它产生的事件的序号。会话停了的
/// 从表里拿掉。
async fn command_to(
    core: &Core,
    request: &Request,
    session: &SessionId,
    handle: &Handle,
    command: Command,
) -> Result<Vec<u64>, Refusal> {
    command_by(core, request, session, handle, admin(core), command).await
}

/// 同 [`command_to`]，记成 `by` 发的：别的 harness 发来的话（施工 7-10）。
async fn command_by(
    core: &Core,
    request: &Request,
    session: &SessionId,
    handle: &Handle,
    by: By,
    command: Command,
) -> Result<Vec<u64>, Refusal> {
    match outcome(core, request, session, handle, by, command).await? {
        Outcome::Accepted { events } => Ok(events.iter().map(|seq| seq.get()).collect()),
        // 只有回顾回它（施工 3-8 四补），回顾不走这里。
        _ => Err(Refusal::INTERNAL),
    }
}

/// 把命令交给会话，记成 `by` 发的，交回它的结局：拒绝的是内核的原因码；会话停了的从表里拿掉。
async fn outcome(
    core: &Core,
    request: &Request,
    session: &SessionId,
    handle: &Handle,
    by: By,
    command: Command,
) -> Result<Outcome, Refusal> {
    match handle.command(request.id.clone(), by, command).await {
        Ok(Outcome::Rejected { reason }) => Err(Refusal::kernel(reason)),
        Ok(outcome) => Ok(outcome),
        Err(_) => {
            core.sessions.forget(session).await;
            Err(Refusal::STOPPED)
        }
    }
}

/// 人说的一句话写成内容块：照原样成一块文字，空的一块都没有（`session.send`、`session.redo` 的 `text`）。
fn said(text: String) -> Vec<Block> {
    match text.is_empty() {
        true => Vec::new(),
        false => vec![Block::Text(Text { text })],
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
