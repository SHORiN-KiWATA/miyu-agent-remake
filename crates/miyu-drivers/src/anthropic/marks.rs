//! 缓存打点放在哪几块（`docs/blueprint/drivers/anthropic.md`「缓存打点」）：前缀即契约，打点只告诉这一家「到这里存一份」。
//!
//! 照这个先后算，同一块只打一次，最多 4 处：
//!
//! 1. 工具和 system 的末尾：system 那一块；system 是空的打在最后一件工具上；两样都没有的不打。
//! 2. 稳定区的末尾：前 `stable` 条消息（示范对话）写出的最后一块。
//! 3. 上一次请求的末尾：稳定区以后最后一条 assistant 前面那一条线上消息的最后一块；没有这样的 assistant 的不打。这一家
//!    每个打点最多往回找 20 块，这一次新加的块多了，第 4 处找不到上一次存的，第 3 处正好落在那里。
//! 4. 这一次请求的末尾：最后一条线上消息的最后一块。
//!
//! 打在一条消息里，落在指定的那一块或者它前面最后一个能打的块上：思考块不能打，往前找；整条都没有能打的，这一处不打。

use std::collections::BTreeSet;

use miyu_kernel::request::Request;

use super::messages::Written;
use super::wire::Tool;

/// 打在哪几处。
#[derive(Debug, Default)]
pub(super) struct Marks {
    /// system 那一块打了。
    pub(super) system: bool,
    /// 打在第几件工具上。
    pub(super) tool: Option<usize>,
    /// 打在第几条线上的消息的第几块上：几处落在同一块上的只打一次。
    pub(super) messages: BTreeSet<(usize, usize)>,
}

/// 照请求、工具面、写好的消息算打点。
pub(super) fn place(request: &Request, tools: Option<&[Tool<'_>]>, written: &Written) -> Marks {
    let mut marks = Marks::default();
    if !request.system.is_empty() {
        marks.system = true;
    } else if let Some(last) = tools.and_then(|tools| tools.len().checked_sub(1)) {
        marks.tool = Some(last);
    }
    let messages = &written.messages;
    let last_block = |m: usize| (m, messages[m].content.len().saturating_sub(1));
    let previous = messages
        .iter()
        .enumerate()
        .rposition(|(m, message)| {
            message.role == "assistant" && written.firsts[m] >= request.stable
        })
        .and_then(|assistant| assistant.checked_sub(1))
        .map(last_block);
    let latest = messages.len().checked_sub(1).map(last_block);
    for (m, upto) in [written.stable_end, previous, latest].into_iter().flatten() {
        let found = messages[m].content[..=upto]
            .iter()
            .rposition(super::wire::Part::markable);
        if let Some(b) = found {
            marks.messages.insert((m, b));
        }
    }
    marks
}
