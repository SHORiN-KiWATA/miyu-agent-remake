//! 握手以后的方法（`docs/designs/04-核心协议.md` 第九节「先做的几样怎么写」）：造会话、说话、打断。
//! 命令交给会话，等它的回应：接受的回 `events`，拒绝的回原因码。

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::id::SessionId;
use miyu_kernel::session::{Command, Outcome, Queued};

use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;
use crate::sessions::admin;
use crate::wire::Request;

/// 没写人格时用的：出厂的软件工程师（施工 3-6 上）。
const PERSONA: &str = "engineer";

/// `session.create` 的参数。
#[derive(Debug, Deserialize)]
struct CreateParams {
    #[serde(default)]
    persona: Option<String>,
    cwd: String,
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
}

/// `session.interrupt` 的参数。
#[derive(Debug, Deserialize)]
struct InterruptParams {
    session: String,
    queued: QueuedParam,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum QueuedParam {
    Send,
    Return,
}

/// 照方法办一条请求：交回回应的 `result`，或者拒绝。
pub(crate) async fn call(core: &Core, peer: Peer, request: &Request) -> Result<Value, Refusal> {
    match request.method.as_str() {
        "session.create" => {
            let params: CreateParams = params(request)?;
            let persona = params.persona.as_deref().unwrap_or(PERSONA);
            let session = core
                .sessions
                .create(core, request.id.clone(), persona, params.cwd, peer.input)
                .await?;
            Ok(json!({"session": session.as_str(), "events": [1]}))
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
            command_to(core, request, &session, params.cwd.as_deref(), command).await
        }
        "session.interrupt" => {
            let params: InterruptParams = params(request)?;
            let queued = match params.queued {
                QueuedParam::Send => Queued::Send,
                QueuedParam::Return => Queued::Return,
            };
            let session = session(&params.session)?;
            command_to(core, request, &session, None, Command::Interrupt { queued }).await
        }
        _ => Err(Refusal::UNKNOWN_METHOD),
    }
}

/// 把命令交给会话 `session`，等它的回应。会话停了的从表里拿掉。
async fn command_to(
    core: &Core,
    request: &Request,
    session: &SessionId,
    cwd: Option<&str>,
    command: Command,
) -> Result<Value, Refusal> {
    let handle = core.sessions.get(core, session, cwd).await?;
    match handle
        .command(request.id.clone(), admin(core), command)
        .await
    {
        Ok(Outcome::Accepted { events }) => {
            let events: Vec<u64> = events.iter().map(|seq| seq.get()).collect();
            Ok(json!({"events": events}))
        }
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
