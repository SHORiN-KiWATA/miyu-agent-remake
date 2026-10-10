//! `view.page`：历史按页读（施工 9-6 下，`docs/blueprint/protocol.md`「`view.page`」）；`view.detail`：一次调用的完整差异
//! （施工 9-6 三补，`view/detail.rs`）。头接一个老会话时先拿最新一页画出来，
//! 再 `subscribe {"after": last}` 只接新的；往上翻到顶拿这一页的 `first` 当 `before` 要更早的一页。页里是原始事件，写法同补发；
//! 写了 `view: true` 的换成视图投影算好的条目（施工 9-8 中，`view/project.rs`，`docs/blueprint/view.md`）；视图流订阅时
//! 照同一个切法拿最新一页（施工 9-8 下，[`newest`]）。怎么切在 `view/page.rs`。
//!
//! 只读地读会话目录里的日志（`read_events`，和列会话、找回工作目录一样），不为翻历史载入会话：读的时候会话照常跑，正在写的
//! 那半行不算。

mod detail;
mod page;
mod project;

pub(crate) use detail::detail;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::event::Event;
use miyu_kernel::id::{AccountId, SessionId};
use miyu_store::blob::Blobs;
use miyu_store::log::{OpenError, read_events};
use miyu_store::resources::ResourceRoot;
use miyu_view::{Projector, Texts};

use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;

/// 不写 `turns` 的一页几轮。
const TURNS: usize = 20;

/// 一页最多几轮。
const MOST_TURNS: usize = 50;

/// 一页到了这么多字节就在回合之间提前切：1 MiB。至少一整轮。
const CAP: usize = 1 << 20;

/// `view.page` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PageParams {
    /// 会话编号。
    session: String,
    /// 只要序号小于它的；不写的是最新一页。
    #[serde(default)]
    before: Option<u64>,
    /// 最多几轮：1 到 50，不写是 20。
    #[serde(default)]
    turns: Option<usize>,
    /// 写 `true` 的交条目、不交事件（施工 9-8 中）。
    #[serde(default)]
    view: bool,
}

/// `view.page`：回应 `{"events", "more"}`，有事件的再带 `first`、`last`，因为字节少给了轮数的带 `capped: true`，这一页里报完了、
/// 在切点前派出去的任务带 `jobs`。写了 `view: true` 的 `events` 换成 `entries`，照这个连接的语言算（施工 9-8 中）。
pub(crate) async fn page(core: &Core, peer: Peer, params: PageParams) -> Result<Value, Refusal> {
    let session = SessionId::parse(&params.session).map_err(|_| Refusal::BAD_PARAMS)?;
    let turns = params.turns.unwrap_or(TURNS);
    if !(1..=MOST_TURNS).contains(&turns) || params.before == Some(0) {
        return Err(Refusal::BAD_PARAMS);
    }
    let (owner, events) = events_of(core, &session).await?;
    // 要条目的：投影要的字照这个连接的语言读，改了多少行照属主的 blob 算。
    let view = match params.view {
        true => Some((
            core.resources.clone(),
            peer.language,
            Blobs::new(core.root.blobs(&owner)),
        )),
        false => None,
    };
    let before = params.before;
    // 切页、算条目都在阻塞线程里：一页最多 1 MiB，投影是一条条喂的纯计算。
    tokio::task::spawn_blocking(move || {
        let view = match view {
            Some((resources, language, blobs)) => Some((texts(&resources, language)?, blobs)),
            None => None,
        };
        Ok(paged(&events, before, turns, view).0)
    })
    .await
    .map_err(|_| Refusal::INTERNAL)?
}

/// 最新一页写成视图流订阅的回应的那几格（施工 9-8 下）：交回回应和喂过这一页的投影，之后的推送接着喂它。
pub(crate) fn newest(events: &[Event], texts: Texts, blobs: Blobs) -> (Value, Projector) {
    let (reply, projector) = paged(events, None, TURNS, Some((texts, blobs)));
    (
        reply,
        projector.unwrap_or_else(|| unreachable!("交了字就有投影")),
    )
}

/// 投影要的字：照资源目录读 `language` 一份、英文一份。读不成的记一行日志、回内部出错。
pub(crate) fn texts(resources: &ResourceRoot, language: &str) -> Result<Texts, Refusal> {
    project::texts(resources, language).map_err(|project::Unreadable(why)| {
        tracing::warn!(target: "miyu::endpoint", why = %why, "view words not read");
        Refusal::INTERNAL
    })
}

/// 从 `events`（整份日志）切 `before` 之前的一页写成回应：`view` 有的交条目，连同喂过这一页的投影；没有的交事件。
fn paged(
    events: &[Event],
    before: Option<u64>,
    turns: usize,
    view: Option<(Texts, Blobs)>,
) -> (Value, Option<Projector>) {
    let cut = page::page(events, before, turns, CAP, |event| {
        serde_json::to_string(event).map_or(0, |line| line.len())
    });
    let (mut reply, projector) = match view {
        Some((texts, blobs)) => {
            // 切点前的日志只学派出去的任务。
            let earlier = cut.first.map_or(events.len(), |first| {
                events.partition_point(|event| event.seq.get() < first)
            });
            let projector = project::fed(texts, blobs, &events[..earlier], &cut.events);
            (
                json!({"entries": projector.entries(), "more": cut.more}),
                Some(projector),
            )
        }
        None => (json!({"events": cut.events, "more": cut.more}), None),
    };
    if let (Some(first), Some(last)) = (cut.first, cut.last) {
        reply["first"] = json!(first);
        reply["last"] = json!(last);
    }
    if cut.capped {
        reply["capped"] = json!(true);
    }
    // 这一页里报完了、在切点前派出去的任务（施工 9-6 再补）：没有的不写。
    if !cut.jobs.is_empty() {
        reply["jobs"] = json!(cut.jobs);
    }
    (reply, projector)
}

/// 只读地读会话 `session` 的日志：照属主的家目录读（施工 O-4 上），交回属主和事件。没有这个会话的 `session_not_found`，
/// 读不下去的 `broken`。
async fn events_of(core: &Core, session: &SessionId) -> Result<(AccountId, Vec<Event>), Refusal> {
    let owner = core
        .sessions
        .owner(core, session)
        .await
        .ok_or(Refusal::NOT_FOUND)?;
    let dir = core.root.session_dir(&owner, session);
    let read = tokio::task::spawn_blocking(move || read_events(&dir))
        .await
        .map_err(|_| Refusal::INTERNAL)?;
    match read {
        Ok(events) => Ok((owner, events)),
        Err(OpenError::Missing(_)) => Err(Refusal::NOT_FOUND),
        Err(error) => {
            tracing::warn!(target: "miyu::endpoint", session = session.as_str(), error = %error, "log not read");
            Err(Refusal::BROKEN)
        }
    }
}
