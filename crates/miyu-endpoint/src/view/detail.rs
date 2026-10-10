//! `view.detail`：一次调用的完整差异（施工 9-6 三补，`docs/blueprint/protocol.md`「`view.detail`」，`04-核心协议.md` 第九节
//! 「条目只带摘要，完整的 diff 头展开时按需取」）。照调用编号找那次调用的工具结果，照它的效果一个文件一项：改了的从属主的
//! blob 拿改前改后算差异（[`crate::diffs`]，不读磁盘：之后又被改过的照样是那一次的），移进回收站的只写一项。施工 9-8 中
//! 多交结果原文 `output`，另认 `compaction` 交那一次压缩的摘要；字段只加不改。

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::block::Block as Content;
use miyu_kernel::event::{Body, Effect};
use miyu_kernel::id::{CallId, SessionId};
use miyu_store::blob::Blobs;

use crate::Core;
use crate::diffs;
use crate::refusal::Refusal;

/// `view.detail` 的参数：`call`、`compaction` 正好写一个。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DetailParams {
    /// 会话编号。
    session: String,
    /// 那一次工具调用的编号。
    #[serde(default)]
    call: Option<String>,
    /// 那一次压缩替代到的序号：压缩那一条的编号 `c<序号>` 里的数（施工 9-8 中）。
    #[serde(default)]
    compaction: Option<u64>,
}

/// `view.detail`：调用的回应 `{"files": [...], "output"}`，`files` 照那次调用的效果的先后，`output` 是结果原文（施工 9-8 中）；
/// 压缩的回应 `{"summary"}`，检查点的摘要原文。
pub(crate) async fn detail(core: &Core, params: DetailParams) -> Result<Value, Refusal> {
    let session = SessionId::parse(&params.session).map_err(|_| Refusal::BAD_PARAMS)?;
    match (params.call, params.compaction) {
        (Some(call), None) => {
            let call = CallId::parse(&call).map_err(|_| Refusal::BAD_PARAMS)?;
            call_detail(core, &session, call).await
        }
        (None, Some(upto)) => compaction(core, &session, upto).await,
        _ => Err(Refusal::BAD_PARAMS),
    }
}

/// 一次调用：改了的文件、结果原文。
async fn call_detail(core: &Core, session: &SessionId, call: CallId) -> Result<Value, Refusal> {
    let (owner, events) = super::events_of(core, session).await?;
    let result = events
        .into_iter()
        .rev()
        .find_map(|event| match event.body {
            Body::ToolResult(result) if result.call_id == call => Some(result),
            _ => None,
        })
        .ok_or(Refusal::UNKNOWN_CALL)?;
    let output = output(&result.blocks);
    let blobs = Blobs::new(core.root.blobs(&owner));
    let files = tokio::task::spawn_blocking(move || {
        result
            .effects
            .iter()
            .filter_map(|effect| file(&blobs, effect))
            .collect::<Vec<Value>>()
    })
    .await
    .map_err(|_| Refusal::INTERNAL)?;
    Ok(json!({ "files": files, "output": output }))
}

/// 一次压缩：替代到 `upto` 的那个检查点的摘要原文。没有的照没有这次调用拒。
async fn compaction(core: &Core, session: &SessionId, upto: u64) -> Result<Value, Refusal> {
    let (_, events) = super::events_of(core, session).await?;
    let summary = events
        .into_iter()
        .rev()
        .find_map(|event| match event.body {
            Body::ContextCompacted(compacted) if compacted.upto.get() == upto => {
                Some(compacted.summary)
            }
            _ => None,
        })
        .ok_or(Refusal::UNKNOWN_CALL)?;
    Ok(json!({ "summary": summary }))
}

/// 结果原文：给模型看的文字块，照先后接起来，块之间空一行；图片在条目的 `images` 里，不进这里。
fn output(blocks: &[Content]) -> String {
    blocks
        .iter()
        .filter_map(|block| match block {
            Content::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect::<Vec<&str>>()
        .join("\n\n")
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
