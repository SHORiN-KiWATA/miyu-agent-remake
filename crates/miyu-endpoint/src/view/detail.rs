//! `view.detail`：一次调用的完整差异（施工 9-6 三补，`docs/blueprint/protocol.md`「`view.detail`」，`04-核心协议.md` 第九节
//! 「条目只带摘要，完整的 diff 头展开时按需取」）。照调用编号找那次调用的工具结果，照它的效果一个文件一项：改了的从属主的
//! blob 拿改前改后算差异（[`crate::diffs`]，不读磁盘：之后又被改过的照样是那一次的），移进回收站的只写一项。以后做视图投影，
//! 还会交长输出这些，字段只加不改。

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::event::{Body, Effect};
use miyu_kernel::id::{CallId, SessionId};
use miyu_store::blob::Blobs;

use crate::Core;
use crate::diffs;
use crate::refusal::Refusal;

/// `view.detail` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DetailParams {
    /// 会话编号。
    session: String,
    /// 那一次工具调用的编号。
    call: String,
}

/// `view.detail`：回应 `{"files": [...]}`，照那次调用的效果的先后。
pub(crate) async fn detail(core: &Core, params: DetailParams) -> Result<Value, Refusal> {
    let session = SessionId::parse(&params.session).map_err(|_| Refusal::BAD_PARAMS)?;
    let call = CallId::parse(&params.call).map_err(|_| Refusal::BAD_PARAMS)?;
    let (owner, events) = super::events_of(core, &session).await?;
    let effects = events
        .into_iter()
        .rev()
        .find_map(|event| match event.body {
            Body::ToolResult(result) if result.call_id == call => Some(result.effects),
            _ => None,
        })
        .ok_or(Refusal::UNKNOWN_CALL)?;
    let blobs = Blobs::new(core.root.blobs(&owner));
    let files = tokio::task::spawn_blocking(move || {
        effects
            .iter()
            .filter_map(|effect| file(&blobs, effect))
            .collect::<Vec<Value>>()
    })
    .await
    .map_err(|_| Refusal::INTERNAL)?;
    Ok(json!({ "files": files }))
}

/// 一个效果写成的一项：改了的带差异（算不出的说原因），移进回收站的只写一项，别的（读文件、派任务）不算。
fn file(blobs: &Blobs, effect: &Effect) -> Option<Value> {
    match effect {
        Effect::FileChanged(changed) => {
            let mut item = json!({"path": changed.path, "action": "write"});
            let whole = diffs::side(blobs, changed.before.as_ref()).and_then(|then| {
                let now = diffs::side(blobs, Some(&changed.after))?;
                diffs::unified(&then, &now)
            });
            match whole {
                Ok(whole) => {
                    item["diff"] = json!(whole.lines);
                    item["added"] = json!(whole.added);
                    item["removed"] = json!(whole.removed);
                }
                Err(skipped) => item["skipped"] = json!(skipped.as_str()),
            }
            Some(item)
        }
        Effect::FileTrashed(trashed) => Some(json!({"path": trashed.path, "action": "trash"})),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
