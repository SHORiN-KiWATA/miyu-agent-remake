//! 握手以后的方法（`docs/designs/04-核心协议.md` 第九节「先做的几样怎么写」）：造会话、说话、打断，
//! 列出会话（施工 3-9 下），撤销、恢复（施工 4-7 上；回应带上给人看的几样，施工 4-7 下），重做（施工 4-7 再补），手动压缩
//! （施工 6-8），切权限级别（施工 3-8 再补），清空上下文（施工 6-8 补），回顾（施工 3-8 四补，回应是那一句），停掉一个任务（施工 7-4），读后台命令的输出（施工 7-4 补），
//! 传附件（施工 3-9 三补），改标题、置顶，删除会话（施工 3-8 三补）。别的 harness 带着名字说话（`session.send` 的 `from`，
//! 施工 7-10）。命令交给会话，等它的回应：接受的回 `events`（切权限级别、停掉任务、改标题的回 `{}`），拒绝的回原因码；删除
//! 由会话表办。造会话、说话的回应再带上会话实际在哪个目录里干活（施工 4-5 下），这个目录的项目配置还没问过信不信任的，
//! 再带上它在哪（`untrusted_project`，施工 8-2）。查配置的三个方法在 `config/methods.rs`（施工 8-2），改配置的 `config.set`、
//! 信任项目配置的 `config.trust` 在 `config/set.rs`、`config/trusting.rs`（施工 8-3）。密钥的 `secret.set`、`secret.delete`、
//! `secret.list` 在 `secrets.rs`（施工 8-5）。`model.list` 在 `models.rs`（施工 8-7）。`session.create` 带 `model` 的照这时的
//! 配置解析好再造，解析不出的回 `unknown_model`、什么都不造（施工 8-8，`models::record`）。换模型 `session.configure`：先查
//! 参数、再找会话、再照这时的配置解析，交给内核，回 `{}`（施工 8-10）。给人看的字 `human.get` 在 `human.rs`（施工 W-1）。
//! 列文件、找文件、换真实位置、分块读 `fs.list`、`fs.find`、`fs.realpath`、`fs.read` 在 `files.rs`
//! （施工 W-2、W-3、W-6）。第一次接入的
//! `provider.detect`、`provider.catalog`、`provider.test` 在 `providers.rs`（施工 8-11）。分块上传
//! `blob.open`、`blob.write`、`blob.close` 在 `uploads.rs`（施工 W-5），要这个连接的上传表 `uploads`。分块读一个 blob
//! `blob.get` 在 `attach.rs`（施工 W-6）。用量汇总的 `usage.query` 在 `usage.rs`（施工 8-15）。
//! 各方法的参数在 `methods/params.rs`（W-5 合并时这一份过了 500 行，挪出去的）。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::Level;
use miyu_kernel::id::{CallId, JobId, Seq, SessionId, TurnId};
use miyu_kernel::origin::By;
use miyu_kernel::session::{Command, Outcome, Queued};
use miyu_policy::memory::MemoryScope;
use miyu_session::Handle;

use crate::Core;
use crate::attach;
use crate::commands;
use crate::config;
use crate::files;
use crate::from;
use crate::hello::Peer;
use crate::human;
use crate::job_output;
use crate::list;
use crate::meta::MetaParams;
use crate::models;
use crate::personas;
use crate::providers;
use crate::refusal::Refusal;
use crate::secrets;
use crate::sessions::{Opening, admin};
use crate::undo;
use crate::uploads::{self, Uploads};
use crate::venues;
use crate::wire::Request;

mod params;

use params::*;

/// 照方法办一条请求：交回回应的 `result`，或者拒绝。
pub(crate) async fn call(
    core: &Arc<Core>,
    peer: Peer,
    request: &Request,
    uploads: &mut Uploads,
) -> Result<Value, Refusal> {
    match request.method.as_str() {
        "session.create" => {
            let params: CreateParams = params(request)?;
            // 解析不出的什么都不造（施工 8-8）。
            let model = params
                .model
                .map(|text| models::record(core, &text))
                .transpose()?;
            let memory = params
                .memory
                .map(|text| MemoryScope::parse(&text).ok_or(Refusal::BAD_PARAMS))
                .transpose()?;
            let who = Opening {
                attended: peer.input,
                oneshot: params.oneshot,
                model,
                venue: None,
                memory,
                preset: params.preset,
            };
            let created = core
                .sessions
                .create(
                    core,
                    request.id.clone(),
                    params.persona.as_deref(),
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
        "check" => crate::check::check(core, peer, params(request)?).await,
        "persona.list" => personas::list(core, peer).await,
        "package.list" => crate::packages::list(core, peer),
        "persona.get" => personas::get(core, params(request)?).await,
        "preset.list" => crate::presets::list(core, peer).await,
        "preset.get" => crate::presets::get(core, params(request)?).await,
        "session.list" => {
            let params: ListParams = params(request)?;
            let sessions = list::list(core, params.oneshot, params.limit).await?;
            Ok(json!({"sessions": sessions}))
        }
        "venue.session" => venues::session(core, request.id.clone(), params(request)?).await,
        "session.send" => {
            let params: SendParams = params(request)?;
            // 别的 harness 报的名字先查（施工 7-10）：不对的，会话里什么都不送。代表外部的人（施工 O-3）和它不能同时写。
            if params.from.is_some() && params.as_external.is_some() {
                return Err(Refusal::BAD_PARAMS);
            }
            let mut by = match &params.from {
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
            // 场所会话只收代表外部的人说的话，本机的会话不收（施工 O-3，`venues.md`）。
            let local = found.handle.venue().as_str() == list::LOCAL;
            match (local, params.as_external) {
                (true, None) => {}
                (true, Some(_)) => return Err(Refusal::BAD_PARAMS),
                (false, None) => return Err(Refusal::VENUE_SESSION),
                (false, Some(speaking)) => {
                    by = venues::speaker(core, found.handle.venue(), &core.admin, speaking)?;
                }
            }
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
                QueuedParam::Keep => Queued::Keep,
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
        "command.run" => commands::run(core, &peer, &request.id, params(request)?).await,
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
        "session.answer" => {
            // 先查参数，再找会话（施工 D-1）。
            let params: AnswerParams = params(request)?;
            let call_id = CallId::parse(&params.call).map_err(|_| Refusal::BAD_PARAMS)?;
            let session = session(&params.session)?;
            let answer = params.answer()?;
            let found = core.sessions.get(core, &session, None, None).await?;
            let command = Command::Answer { call_id, answer };
            let events = command_to(core, request, &session, &found.handle, command).await?;
            Ok(json!({ "events": events }))
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
        "human.get" => human::get(core, peer, params(request)?).await,
        "config.schema" => config::methods::schema(core, peer, params(request)?),
        "config.get" => config::methods::get(core, peer, params(request)?),
        "config.check" => config::methods::check(core, peer, params(request)?),
        "config.set" => config::set::set(core, peer, &request.id, params(request)?),
        "config.trust" => config::trusting::trust(core, peer, &request.id, params(request)?),
        "secret.set" => secrets::set(core, peer, &request.id, params(request)?),
        "secret.delete" => secrets::delete(core, peer, &request.id, params(request)?),
        "secret.list" => Ok(secrets::list(core)),
        "model.list" => models::list(core, params(request)?).await,
        "provider.detect" => providers::detect(core).await,
        "provider.catalog" => providers::catalog(core, params(request)?).await,
        "provider.test" => providers::test(core, params(request)?).await,
        "model.call" => models::call(core, params(request)?).await,
        "usage.query" => crate::usage::query(core, params(request)?).await,
        "blob.put" => attach::put(core, params(request)?).await,
        "blob.open" => uploads::open(core, uploads, params(request)?).await,
        "blob.write" => uploads::write(core, uploads, params(request)?).await,
        "blob.close" => uploads::close(core, uploads, params(request)?).await,
        "blob.get" => attach::get(core, params(request)?).await,
        "session.configure" => {
            let params: models::ConfigureParams = params(request)?;
            let text = params.model()?;
            let session = session(&params.session)?;
            let found = core.sessions.get(core, &session, None, None).await?;
            let model = models::record(core, text)?;
            let command = Command::Configure { model };
            command_to(core, request, &session, &found.handle, command).await?;
            Ok(json!({}))
        }
        "fs.list" => files::list(core, params(request)?).await,
        "fs.find" => files::find(core, params(request)?).await,
        "fs.realpath" => files::realpath(core, params(request)?).await,
        "fs.read" => files::read(core, params(request)?).await,
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
        other => match core.queries.get(other) {
            // 可选软件包登记的查询（施工 W-4，`queries.rs`）：没登记的方法，这张表之外当没有这个方法。
            Some(handler) => handler(Arc::clone(core), request.params.clone())
                .await
                .map_err(Refusal::from),
            None => Err(Refusal::UNKNOWN_METHOD),
        },
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
