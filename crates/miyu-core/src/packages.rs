//! 可选软件包：核心起来时照编进来的包（cargo 开关）往查询表里登记（`web-module.md`「在哪」「起草时定的」
//! 第 19、20 条，`mermaid.md`「怎么走」，施工 W-4）。加一个包只在这里多登记一行，不改端点的中心逻辑；没编
//! 进来的包，它的方法压根不在这张表里，端点照 `unknown_method` 处理（`queries.rs`）。
//!
//! 现在有 `mermaid`（施工 W-4）、`net`（施工 W-7，`net.md`，`packages/net.rs`），发行版都默认打开。
//!
//! [`clear_uploads`]：核心起来时清掉管理员分块上传留下的暂存（施工 W-5，`web-module.md`「怎么走」第六条第 6
//! 款）。和可选软件包无关，放在这里是因为两件事都是「核心起来时做一次的登记、收拾」。

use miyu_endpoint::queries::Queries;
use miyu_kernel::id::AccountId;
use miyu_store::blob::Blobs;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

/// 照编进来的包往一张新的查询表里登记，交给 [`miyu_endpoint::Core::with_queries`]。`root`、`admin`：`net` 抓到的
/// 卡片的图存进这个账号的 blob（现在连上来的都是管理员，`net.md`「起草时定的」第 9 条）。
pub fn register(resources: &ResourceRoot, root: &DataRoot, admin: &AccountId) -> Queries {
    let queries = Queries::new();
    #[cfg(feature = "mermaid")]
    let queries = mermaid::register(resources, queries);
    #[cfg(feature = "net")]
    let queries = net::register(resources, Blobs::new(root.blobs(admin)), queries);
    #[cfg(not(feature = "net"))]
    let _ = (root, admin);
    #[cfg(not(any(feature = "mermaid", feature = "net")))]
    let _ = resources;
    queries
}

/// 核心起来时清掉账号 `admin` 的 `blobs/tmp/` 里分块上传留下的暂存：`upload-*`，崩了、被杀留下的（施工
/// W-5）。在能接连接之前做：新开的上传不会被这一步误删（崩溃留下的名字和现造的编号撞不上，但晚了做就是
/// 在和真实流量抢时间，没必要）。
pub fn clear_uploads(root: &DataRoot, admin: &AccountId) {
    let blobs = Blobs::new(root.blobs(admin));
    match blobs.clear_uploads() {
        Ok(0) => {}
        Ok(removed) => {
            tracing::info!(target: crate::TARGET, removed, "upload tmp cleared");
        }
        Err(error) => {
            tracing::warn!(target: crate::TARGET, error = %error, "upload tmp not cleared");
        }
    }
}

#[cfg(feature = "net")]
mod net;

#[cfg(feature = "mermaid")]
mod mermaid {
    use std::sync::Arc;

    use serde::Deserialize;
    use serde_json::{Value, json};

    use miyu_endpoint::Core;
    use miyu_endpoint::queries::QueryError;
    use miyu_mermaid::{Mermaid, RenderError};

    use super::{Queries, ResourceRoot};

    /// `mermaid.render` 的参数：`source` 必写（`web-module.md`「每个方法的参数和回应」）。
    #[derive(Deserialize)]
    struct Params {
        source: String,
    }

    /// 登记 `mermaid.render`：一个核心一份 [`Mermaid`]，交给闭包捕获，和核心的生命周期一样长
    /// （`mermaid.md`「怎么走」第 1 条：读字体、`style.json` 都等第一次调）。
    pub(super) fn register(resources: &ResourceRoot, queries: Queries) -> Queries {
        let mermaid = Arc::new(Mermaid::new(resources.path()));
        queries.register("mermaid.render", move |_core: Arc<Core>, params: Value| {
            let mermaid = Arc::clone(&mermaid);
            async move { render(&mermaid, params).await }
        })
    }

    /// 真正办事：参数读不成是 `bad_params`；画图在阻塞线程里（`mermaid.md`「怎么走」第 5 条）。
    async fn render(mermaid: &Arc<Mermaid>, params: Value) -> Result<Value, QueryError> {
        let params: Params = serde_json::from_value(params).map_err(|_| QueryError::BadParams)?;
        let mermaid = Arc::clone(mermaid);
        let outcome = tokio::task::spawn_blocking(move || mermaid.render(&params.source)).await;
        match outcome {
            Ok(Ok(rendered)) => Ok(json!({
                "marks": {
                    "label": rendered.marks.label,
                    "line": rendered.marks.line,
                    "text": rendered.marks.text,
                },
                "svg": rendered.svg,
            })),
            Ok(Err(RenderError::Empty)) => Err(QueryError::BadParams),
            Ok(Err(RenderError::TooLong { .. })) => Err(QueryError::Reason("mermaid_too_long")),
            Ok(Err(RenderError::Failed(detail))) => Err(QueryError::ReasonWithDetail(
                "mermaid_failed",
                "detail",
                json!(detail),
            )),
            // `style.json` 读不懂、这台机器上一种字体都读不到：`Mermaid` 自己已经记了 `WARN mermaid not ready`。
            Ok(Err(RenderError::NotReady)) => Err(QueryError::Internal),
            // 阻塞线程本身崩了：`Mermaid::render` 已经把画图的库的 panic 接住了，这里不该走到，照「不该走到
            // 的状态」当内部出错，不往上冒。
            Err(_) => Err(QueryError::Internal),
        }
    }
}
