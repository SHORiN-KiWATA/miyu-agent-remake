//! `view.page`：历史按页读（施工 9-6 下，`docs/blueprint/protocol.md`「`view.page`」）。头接一个老会话时先拿最新一页画出来，
//! 再 `subscribe {"after": last}` 只接新的；往上翻到顶拿这一页的 `first` 当 `before` 要更早的一页。页里是原始事件，写法同补发；
//! 核心不做视图投影。怎么切在 `view/page.rs`。
//!
//! 只读地读会话目录里的日志（`read_events`，和列会话、找回工作目录一样），不为翻历史载入会话：读的时候会话照常跑，正在写的
//! 那半行不算。

mod page;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::id::SessionId;
use miyu_store::log::{OpenError, read_events};

use crate::Core;
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
}

/// `view.page`：回应 `{"events", "more"}`，有事件的再带 `first`、`last`，因为字节少给了轮数的带 `capped: true`，这一页里报完了、
/// 在切点前派出去的任务带 `jobs`。
pub(crate) async fn page(core: &Core, params: PageParams) -> Result<Value, Refusal> {
    let session = SessionId::parse(&params.session).map_err(|_| Refusal::BAD_PARAMS)?;
    let turns = params.turns.unwrap_or(TURNS);
    if !(1..=MOST_TURNS).contains(&turns) || params.before == Some(0) {
        return Err(Refusal::BAD_PARAMS);
    }
    let dir = core.root.session_dir(&core.admin, &session);
    let read = tokio::task::spawn_blocking(move || read_events(&dir))
        .await
        .map_err(|_| Refusal::INTERNAL)?;
    let events = match read {
        Ok(events) => events,
        Err(OpenError::Missing(_)) => return Err(Refusal::NOT_FOUND),
        Err(error) => {
            tracing::warn!(target: "miyu::endpoint", session = session.as_str(), error = %error, "page not read");
            return Err(Refusal::BROKEN);
        }
    };
    let cut = page::page(&events, params.before, turns, CAP, |event| {
        serde_json::to_string(event).map_or(0, |line| line.len())
    });
    let mut reply = json!({"events": cut.events, "more": cut.more});
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
    Ok(reply)
}
