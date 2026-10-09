//! `session.respond {session, to, facts}`（施工 O-14 上，`docs/blueprint/chat.md` 第七条第 3 条第 1 项）：照已经旁听记下的几条开
//! 一轮，回应 `{"events": [...]}`（同 `session.send`）。`to` 是 1 到 64 条序号，内核照序号排好、去重；`facts` 可以不写，每块
//! `{kind, text}`，`kind` 照事实类别的写法，`text` 最多 4 KiB，原样记成 `context.injected`。写错的 `bad_params`，什么都不记。
//! 记成谁同 `events.append`：核心拉起的扩展是那个包（模块），别的是管理员。内核拒的照原因码回，`not_ambient`、
//! `already_answered` 的 `data.messages` 是不合的那几条。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::event::ContextInjected;
use miyu_kernel::id::{FactKind, Seq, SessionId};
use miyu_kernel::session::Command;

use crate::Core;
use crate::appending::recorder;
use crate::hello::Caller;
use crate::methods::command_by;
use crate::refusal::Refusal;
use crate::wire::Request;

/// `to` 最多几条。
const TRIGGERS: usize = 64;
/// 一块事实的 `text` 最多几个字节。
const FACT: usize = 4 * 1024;

/// `session.respond` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RespondParams {
    session: String,
    to: Vec<u64>,
    #[serde(default)]
    facts: Vec<FactParams>,
}

/// 一块事实。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FactParams {
    kind: String,
    text: String,
}

/// `session.respond`：见模块的说明。
pub(crate) async fn respond(
    core: &Arc<Core>,
    caller: &Caller,
    request: &Request,
    params: RespondParams,
) -> Result<Value, Refusal> {
    let session = SessionId::parse(&params.session).map_err(|_| Refusal::BAD_PARAMS)?;
    if params.to.is_empty() || params.to.len() > TRIGGERS {
        return Err(Refusal::BAD_PARAMS);
    }
    let to = params
        .to
        .iter()
        .map(|&seq| Seq::new(seq).ok_or(Refusal::BAD_PARAMS))
        .collect::<Result<Vec<_>, _>>()?;
    let facts = params
        .facts
        .into_iter()
        .map(|fact| {
            let kind = FactKind::parse(&fact.kind).map_err(|_| Refusal::BAD_PARAMS)?;
            if fact.text.len() > FACT {
                return Err(Refusal::BAD_PARAMS);
            }
            Ok(ContextInjected {
                kind,
                text: fact.text,
                refs: Vec::new(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let found = core.sessions.get(core, &session).await?;
    let by = recorder(core, caller)?;
    let command = Command::Respond { to, facts };
    let events = command_by(core, request, &session, &found.handle, by, command).await?;
    Ok(json!({ "events": events }))
}
