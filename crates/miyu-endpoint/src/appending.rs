//! `events.append {session, kind, body}`（施工 O-13 上，`docs/blueprint/chat.md` 第七条第 3 条第 2 项）：往会话里记一条不带回合
//! 编号的事件，回应 `{"seq": n}`。任何时候都收（回合中途也行），不开回合、不打断。只收两类：
//!
//! - `ext.<包>.<名字>`：扩展自己命名空间的，`<名字>` 一段或几段、点隔开，每段照短名字的写法（小写字母开头，小写字母、数字、
//!   `_`、`-`），整个种类最多 128 个字符；`body` 是 JSON 对象，序列化以后最多 16 KiB。核心拉起的扩展的连接，`<包>` 必须是
//!   它自己的包编号；本机的头的连接都收。不渲染，撤销、压缩都不动它。
//! - 核心认得的两种场所的事件 `venue.recalled`、`venue.delivered`：照格查，只收场所会话的。
//!
//! 写错的 `bad_params`，什么都不记。记成谁：核心拉起的扩展是那个包（模块），别的是管理员。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::event::{VenueDelivered, VenueRecalled};
use miyu_kernel::id::{ContentHash, ExternalId, ModuleId, Seq, SessionId, TurnId};
use miyu_kernel::origin::{By, Module};
use miyu_kernel::session::{Appended, Command, ExtEvent};

use crate::Core;
use crate::hello::Caller;
use crate::list;
use crate::methods::command_by;
use crate::refusal::Refusal;
use crate::sessions::admin;
use crate::venues::platform_id;
use crate::wire::Request;

/// `ext.*` 的 `body` 序列化以后最多几个字节。
const BODY: usize = 16 * 1024;

/// `events.append` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AppendParams {
    session: String,
    kind: String,
    body: Value,
}

/// `venue.recalled` 的 `body`。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecalledParams {
    msg: String,
    by: String,
}

/// `venue.delivered` 的 `body`。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeliveredParams {
    line: String,
    turn: u64,
    to: Vec<String>,
    msg: String,
    text: String,
    #[serde(default)]
    images: Vec<String>,
}

/// `events.append`：见模块的说明。
pub(crate) async fn append(
    core: &Arc<Core>,
    caller: &Caller,
    request: &Request,
    params: AppendParams,
) -> Result<Value, Refusal> {
    let session = SessionId::parse(&params.session).map_err(|_| Refusal::BAD_PARAMS)?;
    let (event, venue_only) = appended(caller, &params.kind, params.body)?;
    let found = core.sessions.get(core, &session).await?;
    if venue_only && found.handle.venue().as_str() == list::LOCAL {
        return Err(Refusal::BAD_PARAMS);
    }
    let by = match &caller.package {
        Some(package) => By::Module(Module {
            id: ModuleId::parse(package).map_err(|_| Refusal::INTERNAL)?,
        }),
        None => admin(core),
    };
    let command = Command::Append { event };
    let events = command_by(core, request, &session, &found.handle, by, command).await?;
    let seq = events.first().copied().ok_or(Refusal::INTERNAL)?;
    Ok(json!({ "seq": seq }))
}

/// 照种类查 `body`，交回要记的那一条和它是不是只收场所会话的。
fn appended(caller: &Caller, kind: &str, body: Value) -> Result<(Appended, bool), Refusal> {
    match kind {
        "venue.recalled" => {
            let body: RecalledParams = serde_json::from_value(body).map_err(bad)?;
            let recalled = VenueRecalled {
                msg: platform_id(body.msg)?,
                by: ExternalId::parse(&body.by).map_err(bad)?,
            };
            Ok((Appended::Recalled(recalled), true))
        }
        "venue.delivered" => {
            let body: DeliveredParams = serde_json::from_value(body).map_err(bad)?;
            let delivered = VenueDelivered {
                line: SessionId::parse(&body.line).map_err(bad)?,
                turn: Seq::new(body.turn)
                    .map(TurnId::new)
                    .ok_or(Refusal::BAD_PARAMS)?,
                to: body
                    .to
                    .iter()
                    .map(|who| ExternalId::parse(who).map_err(bad))
                    .collect::<Result<_, _>>()?,
                msg: platform_id(body.msg)?,
                text: body.text,
                images: body
                    .images
                    .iter()
                    .map(|hash| ContentHash::parse(hash).map_err(bad))
                    .collect::<Result<_, _>>()?,
            };
            Ok((Appended::Delivered(delivered), true))
        }
        _ => Ok((Appended::Ext(ext(caller, kind, body)?), false)),
    }
}

/// `ext.<包>.<名字>` 的一条：每段的写法、整个最多 128 个字节照内核的种类查（[`ExtEvent::new`]）；这里多查至少有包、名字两段，
/// 包是这个连接自己的，`body` 是对象、不超过上限。
fn ext(caller: &Caller, kind: &str, body: Value) -> Result<ExtEvent, Refusal> {
    let mut segments = kind.split('.');
    let (Some("ext"), Some(package), Some(_)) = (segments.next(), segments.next(), segments.next())
    else {
        return Err(Refusal::BAD_PARAMS);
    };
    if caller.package.as_deref().is_some_and(|own| own != package) {
        return Err(Refusal::BAD_PARAMS);
    }
    if !body.is_object() || body.to_string().len() > BODY {
        return Err(Refusal::BAD_PARAMS);
    }
    ExtEvent::new(kind, body).ok_or(Refusal::BAD_PARAMS)
}

/// 读不成、写法不对的一律 `bad_params`。
fn bad<E>(_: E) -> Refusal {
    Refusal::BAD_PARAMS
}
