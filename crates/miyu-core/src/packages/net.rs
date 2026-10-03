//! 可选软件包 `net` 的登记（施工 W-7，`net.md`）：`link.preview` 在后台答（「怎么走」第 11 条）；参数怎么读、
//! 卡片怎么写成回应（`kind` 一定写，`duration`、`author` 没有的不写，W-7 再补）。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_endpoint::queries::QueryError;
use miyu_net::{LinkPreview, NotReady, Picture, Preview};
use miyu_store::blob::Blobs;

use super::{Queries, ResourceRoot};

/// `link.preview` 的参数：`url` 必写（`net.md`「对外的样子」）。
#[derive(Deserialize)]
struct Params {
    url: String,
}

/// 登记 `link.preview`：一个核心一份 [`LinkPreview`]，交给闭包捕获，和核心的生命周期一样长；图存进 `blobs`。
pub(super) fn register(resources: &ResourceRoot, blobs: Blobs, queries: Queries) -> Queries {
    let links = Arc::new(LinkPreview::new(resources.path(), blobs));
    queries.register_background("link.preview", move |_core: Arc<Core>, params: Value| {
        let links = Arc::clone(&links);
        async move { preview(&links, params).await }
    })
}

/// 真正办事：参数读不成是 `bad_params`；做不出卡片不是拒绝，回 `{"card": null, "why": …}`。
async fn preview(links: &LinkPreview, params: Value) -> Result<Value, QueryError> {
    let params: Params = serde_json::from_value(params).map_err(|_| QueryError::BadParams)?;
    match links.preview(&params.url).await {
        Ok(Preview::Card(card)) => {
            let mut written = json!({
                "description": card.description,
                "icon": picture(card.icon.as_ref()),
                "image": picture(card.image.as_ref()),
                "kind": card.kind.as_str(),
                "site": card.site,
                "title": card.title,
                "url": card.url,
            });
            // 没有的不写（W-7 再补，net.md「对外的样子」）
            if let Some(duration) = card.duration {
                written["duration"] = json!(duration);
            }
            if let Some(author) = card.author {
                written["author"] = json!(author);
            }
            Ok(json!({"card": written}))
        }
        Ok(Preview::Miss(why)) => Ok(json!({"card": null, "why": why.as_str()})),
        // `link_preview.json` 读不懂：`LinkPreview` 自己已经记了 `WARN not ready`。
        Err(NotReady) => Err(QueryError::Internal),
    }
}

/// 卡片上的一张图：`{"blob", "media_type"}`，没有的是 `null`。
fn picture(picture: Option<&Picture>) -> Value {
    match picture {
        Some(picture) => json!({"blob": picture.blob.as_str(), "media_type": picture.media_type}),
        None => Value::Null,
    }
}
