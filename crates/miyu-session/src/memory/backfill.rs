//! 补齐旧会话（施工 R-2 下，`docs/blueprint/memory.md`「怎么走」第一条第 9 款）：人格那一间的回合库是新建的、重建过的，在
//! 后台照这个账号的会话一个个补。哪些会话照载入时的判法（最近一次换上的快照：主会话、范围是 `persona`、人格是这一间的），
//! 怎么补照载入时那一套（[`Turns::connect`]：照到的位置以后的补，一条都没照过的整份补）。在跑的会话照样补：补的一批落后的
//! 检索库不写（`recall.md`），不会把会话自己撤销了的又放回去。
//!
//! 只看这一间的账号名下的会话：系统账号的场所会话、记忆归这个账号的，随 O-4。

use std::sync::Arc;

use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::{AccountId, SessionId};
use miyu_policy::Snapshot;
use miyu_policy::memory::MemoryScope;
use miyu_store::blob::Blobs;
use miyu_store::log::read_events;
use miyu_store::recall::{RecallIndexes, Room};
use miyu_store::root::DataRoot;

use crate::TARGET;
use crate::agents::LOCAL;
use crate::open::current_policy;

use super::{Turns, scope};

/// 补房间 `room`（人格那一间；别的不补）：一个个会话读日志、照判法补，读不了的记一行跳过，补完记一行补了几个。
pub(crate) fn backfill(recall: &Arc<RecallIndexes>, root: &DataRoot, room: &Room) {
    let Room::Persona { account, persona } = room else {
        return;
    };
    let room_name = room.to_string();
    let sessions = match root.sessions(account) {
        Ok(sessions) => sessions,
        Err(error) => {
            tracing::warn!(target: TARGET, room = room_name.as_str(), error = %error, "memory index not backfilled");
            return;
        }
    };
    let blobs = Blobs::new(root.blobs(account));
    let mut filled = 0;
    for session in sessions {
        match one(recall, root, &blobs, room, account, persona, &session) {
            Ok(true) => filled += 1,
            Ok(false) => {}
            Err(why) => {
                tracing::warn!(target: TARGET, session = %session, error = why.as_str(), "memory index not backfilled");
            }
        }
    }
    tracing::info!(target: TARGET, room = room_name.as_str(), sessions = filled, "memory index backfilled");
}

/// 补一个会话：照载入时的判法该进这一间的，照载入时那一套补，交回 `true`；不该进的 `false`。
fn one(
    recall: &Arc<RecallIndexes>,
    root: &DataRoot,
    blobs: &Blobs,
    room: &Room,
    account: &AccountId,
    persona: &str,
    session: &SessionId,
) -> Result<bool, String> {
    let events =
        read_events(&root.session_dir(account, session)).map_err(|error| error.to_string())?;
    let Some(Event {
        body: Body::SessionCreated(created),
        ..
    }) = events.first()
    else {
        return Err("the first event is not session.created".to_string());
    };
    let hash = current_policy(&events).unwrap_or(&created.policy);
    let bytes = blobs.get(hash).map_err(|error| error.to_string())?;
    let snapshot = Snapshot::from_bytes(&bytes).map_err(|error| error.to_string())?;
    let scope = scope(created.parent.is_some(), true, snapshot.memory_scope());
    // 场所会话的回合先不进回合库（施工 R-2 再补，和载入时一样判，`memory.rs` 的 `connect`）。
    if scope != MemoryScope::Persona
        || snapshot.persona.as_deref() != Some(persona)
        || created.venue.as_str() != LOCAL
    {
        return Ok(false);
    }
    Turns::connect(recall, room, session, &events);
    Ok(true)
}
