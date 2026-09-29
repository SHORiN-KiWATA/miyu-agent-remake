//! 摘要请求说完了（`docs/blueprint/compaction.md` 第三条第 6、7 条）：取出摘要交给压缩那一步；取不出来的、调了工具的怎么
//! 记。

use super::Session;
use super::input::Reread;
use crate::block::Block;
use crate::event::{CallError, CompactTrigger, ErrorClass, Usage};
use crate::id::Seq;

/// 摘要请求取到了摘要：替代到哪、摘要、压之前的用量，和这次摘要请求的用量、用时（施工 6-3 下：推 `compaction.done`）。
pub(super) struct Summarized {
    pub(super) upto: Seq,
    /// 哪一种压缩、人附的要求（施工 6-8）、压完很快又到线连着的第几次（施工 6-6 上）。
    pub(super) trigger: CompactTrigger,
    pub(super) instructions: Option<String>,
    pub(super) refills: Option<u32>,
    /// 截短重试截到第几条（施工 6-6 中）：写压缩时照它写摘要没看到的那一段。
    pub(super) cut: Option<Seq>,
    pub(super) summary: String,
    pub(super) before: u64,
    pub(super) usage: Option<Usage>,
    pub(super) duration_ms: Option<u64>,
    /// 交给执行器重读的候选、送回的结果（施工 6-5）。
    pub(super) paths: Vec<String>,
    pub(super) reread: Option<Vec<Reread>>,
}

impl Session {
    /// 摘要请求的回复里取出摘要：一个块都没有的，照回复是空的算，可以重试；调了工具的、取不出来的，是
    /// `bad_summary`（`compaction.md` 第三条第 6、7 条）。调了工具、接着改走隔离式的（`isolates`，施工 6-6 下），原话写明。
    pub(super) fn summary_of(&self, blocks: &[Block], isolates: bool) -> Result<String, CallError> {
        if blocks.is_empty() {
            return Err(CallError {
                class: ErrorClass::EmptyReply,
                message: "回复里一个块都没有".to_string(),
                status: None,
            });
        }
        if called_tool(blocks) {
            let message = match isolates {
                true => "the summary reply called a tool; trying again without tools",
                false => "the summary reply called a tool",
            };
            return Err(CallError {
                class: ErrorClass::BadSummary,
                message: message.to_string(),
                status: None,
            });
        }
        self.policy
            .assembler
            .summary(blocks)
            .ok_or_else(|| CallError {
                class: ErrorClass::BadSummary,
                message: "no summary in the reply".to_string(),
                status: None,
            })
    }
}

/// 回复里有工具调用。
pub(super) fn called_tool(blocks: &[Block]) -> bool {
    blocks
        .iter()
        .any(|block| matches!(block, Block::ToolCall(_)))
}
