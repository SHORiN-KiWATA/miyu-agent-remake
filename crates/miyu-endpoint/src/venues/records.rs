//! `venue.records {session, msg, count}`（施工 O-24，`docs/construction/O-24-判官看的群聊记录.md`）：判官看的群聊记录，回应
//! `{"records": "…", "current": "…"}`。照会话日志读（同 `view.page`，不载入会话），留着一切的有效历史（撤掉的、撤回的不算，压缩
//! 换出去的照样算），照会话快照里钉下的时区和字渲染（群会话）；快照里没有的（私聊）照核心这时的时区和出厂的字。
//! `count` 1 到 100；`msg` 不是带 `venue` 的 `message.user` 的 `bad_params`。

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::event::Body;
use miyu_kernel::history::History;
use miyu_kernel::id::{Seq, SessionId};
use miyu_policy::{GroupChat, Snapshot};
use miyu_store::blob::Blobs;
use miyu_store::log::{OpenError, read_events};

use crate::Core;
use crate::refusal::Refusal;

/// `count` 最多几条。
const MOST: u64 = 100;

/// `venue.records` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecordsParams {
    session: String,
    msg: u64,
    count: u64,
}

/// `venue.records`：见模块的说明。
pub(crate) async fn records(core: &Core, params: RecordsParams) -> Result<Value, Refusal> {
    let session = SessionId::parse(&params.session).map_err(|_| Refusal::BAD_PARAMS)?;
    let msg = Seq::new(params.msg).ok_or(Refusal::BAD_PARAMS)?;
    if !(1..=MOST).contains(&params.count) {
        return Err(Refusal::BAD_PARAMS);
    }
    let count = usize::try_from(params.count).map_err(|_| Refusal::BAD_PARAMS)?;
    let owner = core
        .sessions
        .owner(core, &session)
        .await
        .ok_or(Refusal::NOT_FOUND)?;
    let dir = core.root.session_dir(&owner, &session);
    let blobs = Blobs::new(core.root.blobs(&owner));
    let resources = core.resources.clone();
    let offset = crate::sessions::offset().minutes();
    let rendered = tokio::task::spawn_blocking(move || {
        let events = match read_events(&dir) {
            Ok(events) => events,
            Err(OpenError::Missing(_)) => return Err(Refusal::NOT_FOUND),
            Err(error) => {
                tracing::warn!(target: "miyu::endpoint", error = %error, "venue records not read");
                return Err(Refusal::BROKEN);
            }
        };
        // 群会话照快照里钉下的；没有的照核心这时的时区和出厂的字。
        let pinned = match events.first().map(|event| &event.body) {
            Some(Body::SessionCreated(created)) => blobs
                .get(&created.policy)
                .ok()
                .and_then(|bytes| Snapshot::from_bytes(&bytes).ok())
                .and_then(|snapshot| snapshot.group),
            _ => None,
        };
        let chat = pick(pinned, || {
            resources.group_chat(offset).map_err(|error| {
                tracing::warn!(target: "miyu::endpoint", error = %error, "group texts not read");
                Refusal::INTERNAL
            })
        })?;
        let texts = chat.texts().map_err(|_| Refusal::INTERNAL)?;
        let mut history = History::whole();
        for event in events {
            history.append(event);
        }
        miyu_assemble::group::records(&history, msg, count, &texts).ok_or(Refusal::BAD_PARAMS)
    })
    .await
    .map_err(|_| Refusal::INTERNAL)??;
    Ok(json!({"records": rendered.records, "current": rendered.current}))
}

/// 渲染照哪一份：群会话快照里钉下的（`pinned`），没有的照这时的（`now`）。
fn pick(
    pinned: Option<GroupChat>,
    now: impl FnOnce() -> Result<GroupChat, Refusal>,
) -> Result<GroupChat, Refusal> {
    match pinned {
        Some(chat) => Ok(chat),
        None => now(),
    }
}

#[cfg(test)]
mod tests;
