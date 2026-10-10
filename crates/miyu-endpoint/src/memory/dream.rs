//! 现在就整理记忆（施工 R-7 补，`docs/blueprint/memory.md` 第七条第 9 款，`protocol.md` 的 `memory.dream`、`/dream`）：带
//! `session` 的叫那个会话先把还没抽的抽了再合（[`Handle::dream`]），带 `persona`、都不写的照那一间直接合
//! （[`miyu_session::Keeper::dream`]）。交回几样数；这一间正在合的 `memory_busy`，没合成的 `dream_failed`。几秒到十几秒，
//! 在后台答（`methods::answered_later`）。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::id::SessionId;
use miyu_session::{Dreamed, Handle, NotDreamed};

use super::params::Where;
use super::{find, installed, read, remembers, session_id, using};
use crate::Core;
use crate::refusal::Refusal;
use crate::sessions::offset;

/// `memory.dream`：照参数找哪一间（同 `memory.*`），现在就整理，交回 `{"given", "revised", "retired", "summary"}`。
pub(crate) async fn call(core: &Arc<Core>, params: &Value) -> Result<Value, Refusal> {
    if !installed(core) {
        return Err(Refusal::MEMORY_NOT_INSTALLED);
    }
    let at: Where = read(params)?;
    let dreamed = match (&at.session, &at.persona, &at.as_external) {
        (Some(session), None, None) => {
            let session = session_id(session)?;
            let found = core.sessions.get(core, &session).await?;
            in_session(core, &session, &found.handle).await?
        }
        // 别的组合（写错的也在里面）照 `find` 判。
        _ => {
            let keeper = find(core, at).await?;
            // 合并的请求照这时的配置发，记在管理员名下（M8 只有他，和 `model.call` 一样）。
            let using = using(core);
            answered(keeper.dream(using.config, using.owner, offset()).await)?
        }
    };
    Ok(json!({
        "given": dreamed.given,
        "revised": dreamed.revised,
        "retired": dreamed.retired,
        "summary": dreamed.summary,
    }))
}

/// 会话 `session`（句柄 `handle`）现在就整理（带 `session` 的 `memory.dream`、`/dream`）：那个会话先抽、再合。
///
/// # Errors
///
/// 这个会话没有记忆：`memory_unavailable`；会话停了：`stopped`（从会话表里拿掉）；别的照 [`answered`]。
pub(crate) async fn in_session(
    core: &Core,
    session: &SessionId,
    handle: &Handle,
) -> Result<Dreamed, Refusal> {
    if !remembers(core, handle) {
        return Err(Refusal::MEMORY_UNAVAILABLE);
    }
    match handle.dream().await {
        Ok(dreamed) => answered(dreamed),
        Err(_) => {
            core.sessions.forget(session).await;
            Err(Refusal::STOPPED)
        }
    }
}

/// 会话那一层交回的写成拒绝：正在合的 `memory_busy`，没有记忆可整理的 `memory_unavailable`，没合成的 `dream_failed`。
fn answered(dreamed: Result<Dreamed, NotDreamed>) -> Result<Dreamed, Refusal> {
    dreamed.map_err(|not| match not {
        NotDreamed::Busy => Refusal::MEMORY_BUSY,
        NotDreamed::Off => Refusal::MEMORY_UNAVAILABLE,
        NotDreamed::Failed(why) => Refusal::dream_failed(why),
    })
}
