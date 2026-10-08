//! 斜杠命令由核心解析（施工 O-6，`docs/designs/04-核心协议.md` P4，`docs/blueprint/protocol.md`「`command.run`」）：头把人打的
//! 原文交过来，核心认命令、判谁能用、执行、记一条 `command.ran`，回执的那一句照连接的语言写好交回去。
//!
//! 头一批两个命令（2026-10-07 项目主人定）：`/clear`（别名 `/reset`）清空上下文；`/stop` 全停：打断这一轮（排着的话留着，
//! 不撤回、不接着开），再停掉她派出去的后台命令和子代理。终端、网页、通讯平台用同一套名字。施工 9-7 下加 `/workspace`
//! （`workspace.rs`）：换会话在哪个目录干活，只有主人本人能用。`/remember <话>` 直接记一条记忆（施工 R-3 补，`memory.md`
//! 「协议」）：不经过模型，只在本机的会话里。
//!
//! 命令本身照请求的编号交给内核，内核照编号只生效一次；`command.ran` 用派生的编号 `<编号>/ran` 记，执行了的才记。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::Words;
use miyu_kernel::id::{CommandId, SessionId};
use miyu_kernel::origin::{By, Role};
use miyu_kernel::session::{Command, Outcome, Queued, Reason};
use miyu_session::Handle;
use miyu_store::human::Human;

use crate::Core;
use crate::hello::Peer;
use crate::list::LOCAL;
use crate::memory;
use crate::refusal::Refusal;
use crate::sessions::admin;
use crate::venues::{self, AsParams};

/// `command.run` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RunParams {
    session: String,
    text: String,
    #[serde(default, rename = "as")]
    as_external: Option<AsParams>,
    /// 头所在的目录：只用来接 `/workspace` 后面相对的路径（施工 9-7 下）。
    #[serde(default)]
    cwd: Option<String>,
}

/// 认得出的命令。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Slash {
    Clear,
    Stop,
    Workspace,
    Remember,
}

impl Slash {
    /// 照名字认：别名换成正名。认不出的没有。
    fn of(name: &str) -> Option<Slash> {
        match name {
            "clear" | "reset" => Some(Slash::Clear),
            "stop" => Some(Slash::Stop),
            "workspace" => Some(Slash::Workspace),
            "remember" => Some(Slash::Remember),
            _ => None,
        }
    }

    /// 正名。
    fn name(self) -> &'static str {
        match self {
            Slash::Clear => "clear",
            Slash::Stop => "stop",
            Slash::Workspace => "workspace",
            Slash::Remember => "remember",
        }
    }
}

/// 回执那一句：`core/human/<语言>.json` 的键，和要填的字。
struct Said {
    key: &'static str,
    fields: Vec<(&'static str, String)>,
}

impl Said {
    /// 不用填字的一句。
    fn plain(key: &'static str) -> Said {
        Said {
            key,
            fields: Vec::new(),
        }
    }
}

/// `command.run`：认命令、判谁能用、执行、记下，交回 `{"command", "events", "said"}`。
pub(crate) async fn run(
    core: &Arc<Core>,
    peer: &Peer,
    id: &CommandId,
    params: RunParams,
) -> Result<Value, Refusal> {
    let (slash, rest) = parse(&params.text)?;
    if params
        .cwd
        .as_deref()
        .is_some_and(|cwd| !workspace::head_cwd_ok(cwd))
    {
        return Err(Refusal::BAD_PARAMS);
    }
    let noted = CommandId::parse(&format!("{id}/ran")).map_err(|_| Refusal::BAD_PARAMS)?;
    let session = SessionId::parse(&params.session).map_err(|_| Refusal::BAD_PARAMS)?;
    let found = core.sessions.get(core, &session).await?;
    let handle = found.handle.clone();
    let local = handle.venue().as_str() == LOCAL;
    let by = match (local, params.as_external) {
        (true, None) => admin(core),
        (true, Some(_)) => return Err(Refusal::BAD_PARAMS),
        (false, None) => return Err(Refusal::VENUE_SESSION),
        (false, Some(speaking)) => venues::speaker(core, handle.venue(), handle.owner(), speaking)?,
    };
    if !may_run(&by) {
        return Err(Refusal::COMMAND_NOT_ALLOWED);
    }
    // 换工作区动的是沙盒能写的地方：管理的人不行，只有主人本人。
    if slash == Slash::Workspace && !is_owner(&by) {
        return Err(Refusal::OWNER_ONLY);
    }
    let mut events = Vec::new();
    let said = match slash {
        Slash::Clear => {
            let outcome = command(core, &session, &handle, id, &by, Command::Clear).await?;
            events.extend(accepted(outcome)?);
            Said::plain("commands/cleared")
        }
        Slash::Workspace => {
            let head = params.cwd.as_deref();
            let (moved, said) = workspace::run(core, &session, &found, id, &by, rest, head).await?;
            events.extend(moved);
            said
        }
        Slash::Stop => {
            let interrupt = Command::Interrupt {
                queued: Queued::Keep,
            };
            match command(core, &session, &handle, id, &by, interrupt).await? {
                // 没有在进行的回合：照样往下停后台的。
                Outcome::Rejected {
                    reason: Reason::NotRunning,
                    ..
                } => {}
                outcome => events.extend(accepted(outcome)?),
            }
            if handle.stop_jobs(by.clone(), id.clone()).await.is_err() {
                core.sessions.forget(&session).await;
                return Err(Refusal::STOPPED);
            }
            Said::plain("commands/stopped")
        }
        Slash::Remember => {
            let remembered = memory::remember_in(core, &handle, id, by.clone(), rest).await?;
            Said {
                key: "commands/remembered",
                fields: vec![("id", remembered.to_string())],
            }
        }
    };
    let note = Command::Ran {
        text: params.text,
        command: slash.name().to_string(),
    };
    events.extend(accepted(
        command(core, &session, &handle, &noted, &by, note).await?,
    )?);
    let said = words(core, peer, &said).await;
    Ok(json!({"command": slash.name(), "events": events, "said": said}))
}

/// 原文的头一个词是命令名：开头的空白不算，`/` 开头，名字紧跟着 `/`、到空白为止。交回命令和后面跟的字（去掉前后空白；
/// 只有 `/workspace` 用它）。不是 `/` 开头的参数不对，认不出的 `unknown_command`。
fn parse(text: &str) -> Result<(Slash, &str), Refusal> {
    let after = text
        .trim_start()
        .strip_prefix('/')
        .ok_or(Refusal::BAD_PARAMS)?;
    let (name, rest) = after.split_once(char::is_whitespace).unwrap_or((after, ""));
    let slash = Slash::of(name).ok_or(Refusal::UNKNOWN_COMMAND)?;
    Ok((slash, rest.trim()))
}

/// 谁能用（`18-通讯平台.md` 第十二节）：本人（本机的头、私聊里对应表认出的本人），对应表里有的外部身份（群里的主人），场所里
/// 管理的人。
fn may_run(by: &By) -> bool {
    match by {
        By::Person(_) => true,
        By::External(external) => {
            external.account.is_some() || external.role == Some(Role::Manager)
        }
        _ => false,
    }
}

/// 主人本人：本机的头、私聊里对应表认出的本人、对应表里有的外部身份（群里的主人）。
fn is_owner(by: &By) -> bool {
    match by {
        By::Person(_) => true,
        By::External(external) => external.account.is_some(),
        _ => false,
    }
}

/// 交给会话 `session` 一个命令，等回应。会话停了的从表里拿掉。
async fn command(
    core: &Core,
    session: &SessionId,
    handle: &Handle,
    id: &CommandId,
    by: &By,
    command: Command,
) -> Result<Outcome, Refusal> {
    match handle.command(id.clone(), by.clone(), command).await {
        Ok(outcome) => Ok(outcome),
        Err(_) => {
            core.sessions.forget(session).await;
            Err(Refusal::STOPPED)
        }
    }
}

/// 接受了的交回序号；拒绝的照内核的原因说。
fn accepted(outcome: Outcome) -> Result<Vec<u64>, Refusal> {
    match outcome {
        Outcome::Accepted { events } => Ok(events.iter().map(|seq| seq.get()).collect()),
        Outcome::Rejected { reason, .. } => Err(Refusal::kernel(reason)),
        _ => Err(Refusal::INTERNAL),
    }
}

/// 回执那一句，照这个连接的语言；资源读不出来的是空的，记一行运行日志。
async fn words(core: &Core, peer: &Peer, said: &Said) -> String {
    let resources = core.resources.clone();
    let language = peer.language.to_string();
    let loaded = tokio::task::spawn_blocking(move || Human::load(&resources, &language)).await;
    let fields: Vec<(&str, &str)> = said
        .fields
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect();
    match loaded {
        Ok(Ok(human)) => Words::sentence(&human, said.key, &fields).unwrap_or_default(),
        _ => {
            tracing::warn!(target: "miyu::endpoint", "command receipt words not read");
            String::new()
        }
    }
}

#[cfg(test)]
mod tests;
mod workspace;
