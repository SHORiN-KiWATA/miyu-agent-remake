//! 哪些算一条、一条的原文怎么写（`docs/blueprint/tools/history.md`「哪些算一条」「一条的原文」，施工 6-4）。
//!
//! 撤掉的、撤回的交给内核的 `History::whole()` 算过，这里只看剩下的：人说的话、她的回复、工具结果、以前的摘要。
//! 她自己翻记录的那几步（`history` 的调用和结果）不算：找的时候会找到自己这一次调用，翻出来的旧结果又和原文重复。
//!
//! 人切权限级别另算（[`switched`]，施工 2-7 补）：撤销不改现在的权限（`docs/blueprint/kernel/session.md`「载入和崩溃」
//! 第 2 条），所以撤掉的回合里切的也列，不经 `History::whole()`。

use std::collections::BTreeSet;

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, Event, PolicyChanged};
use miyu_kernel::facts::effective_level;
use miyu_kernel::history::History as Kept;
use miyu_kernel::id::CallId;
use miyu_kernel::origin::By;
use miyu_kernel::template::Template;
use miyu_kernel::time::Timestamp;

use crate::load::say;

/// 一条是谁说的：参数 `by` 的三种写法。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Who {
    /// 人说的话。
    User,
    /// 她的回复、以前的摘要。
    Assistant,
    /// 工具结果。
    Tool,
}

impl Who {
    /// 参数里、输出里的写法。
    pub(super) fn name(self) -> &'static str {
        match self {
            Who::User => "user",
            Who::Assistant => "assistant",
            Who::Tool => "tool",
        }
    }

    /// 读参数 `by`；不是这三种的没有。
    pub(super) fn parse(text: &str) -> Option<Who> {
        [Who::User, Who::Assistant, Who::Tool]
            .into_iter()
            .find(|who| who.name() == text)
    }
}

/// 一条：序号、时刻、谁说的、原文。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Entry {
    /// 日志的序号。
    pub(super) seq: u64,
    /// 写下的时刻。
    pub(super) at: Timestamp,
    /// 谁说的：筛的时候照它。
    pub(super) who: Who,
    /// 「谁」那一格另写的：别的 harness 发来的话写 `agent "<名字>"`（施工 7-10），别的会话发来的话写 `session <短编号>`
    /// （施工 C-2）；没有的照 `who` 写。
    pub(super) from: Option<String>,
    /// 「读」时这一条下面的原文；「找」也比它。
    pub(super) text: String,
}

impl Entry {
    /// 「谁」那一格：别的 harness 发来的写它的名字，别的会话发来的写它的短编号，别的写 `by` 的那三种写法。
    pub(super) fn said_by(&self) -> &str {
        self.from.as_deref().unwrap_or(self.who.name())
    }
}

/// 一条里代码写的几样：图片、文件的占位（`history/image.txt`、`history/file.txt`），别的 harness 发来的那一条的「谁」
/// （`history/agent.txt`，施工 7-10），别的会话发来的那一条的「谁」（`history/session.txt`，施工 C-2），人切了权限级别的
/// 那一条的原文（`history/permission.txt`，施工 2-7 补）。
#[derive(Clone)]
pub(super) struct Placeholders {
    pub(super) image: Template,
    pub(super) file: Template,
    pub(super) agent: Template,
    pub(super) session: Template,
    pub(super) permission: Template,
}

/// 还算数的事件里挑出算一条的，照日志的先后。一个字都没有的（例如只想了没说就被打断的回复）不算；末尾的空白去掉。
/// 工具名是 `own` 的调用和它们的结果不算：她自己翻记录的那几步。`kept` 是整份日志算出来的有效历史：谁是别的会话照它认。
pub(super) fn entries(kept: &Kept, placeholders: &Placeholders, own: &str) -> Vec<Entry> {
    let events = kept.events();
    let lookups: BTreeSet<CallId> = events
        .iter()
        .filter_map(|event| match &event.body {
            Body::MessageAssistant(reply) => Some(&reply.blocks),
            _ => None,
        })
        .flatten()
        .filter_map(|block| match block {
            Block::ToolCall(call) if call.name == own => Some(call.call_id),
            _ => None,
        })
        .collect();
    let written = |blocks: &[Block]| self::blocks(blocks, placeholders, own);
    events
        .iter()
        .filter_map(|event| {
            let (who, text) = match &event.body {
                Body::MessageUser(message) => (Who::User, written(&message.blocks)),
                Body::MessageAssistant(reply) => (Who::Assistant, written(&reply.blocks)),
                Body::ToolResult(result) if lookups.contains(&result.call_id) => return None,
                Body::ToolResult(result) => (Who::Tool, written(&result.blocks)),
                Body::ContextCompacted(compacted) => (Who::Assistant, compacted.summary.clone()),
                _ => return None,
            };
            let text = text.trim_end();
            (!text.trim_start().is_empty()).then(|| Entry {
                seq: event.seq.get(),
                at: event.at,
                who,
                from: from(kept, event, placeholders),
                text: text.to_string(),
            })
        })
        .collect()
}

/// 人切了权限级别（带 `permission` 的 `session.policy_changed`）算一条，「谁」是 `user`，原文写切成了哪一级，写法和
/// 事实里的一样（施工 2-7 补）。别的事件、只换了策略快照的没有。照日志原样读来的事件交给它，不管撤没撤掉：现在的权限就是
/// 这样算的，撤销不改它。
pub(super) fn switched(event: &Event, placeholders: &Placeholders) -> Option<Entry> {
    let Body::PolicyChanged(PolicyChanged {
        permission: Some(permission),
        ..
    }) = &event.body
    else {
        return None;
    };
    Some(Entry {
        seq: event.seq.get(),
        at: event.at,
        who: Who::User,
        from: None,
        text: inline(say(
            &placeholders.permission,
            &[("level", effective_level(permission))],
        )),
    })
}

/// 几块写成原文，一块一段：正文照原样；工具调用写成 `→ <工具名> <参数原文>`，工具名是 `own` 的不写；图片、文件写
/// 占位；思考不给。
fn blocks(blocks: &[Block], placeholders: &Placeholders, own: &str) -> String {
    let parts: Vec<String> = blocks
        .iter()
        .filter_map(|block| match block {
            Block::Text(text) => Some(text.text.clone()),
            Block::ToolCall(call) if call.name == own => None,
            Block::ToolCall(call) => Some(format!("→ {} {}", call.name, call.args)),
            Block::Image(_) => Some(inline(say(&placeholders.image, &[]))),
            Block::File(file) => Some(inline(say(
                &placeholders.file,
                &[("name", file.name.as_str())],
            ))),
            Block::Reasoning(_) | Block::Unknown(_) => None,
        })
        .collect();
    parts.join("\n")
}

/// 别的 harness 发来的话（`message.user`，`by` 是 `harness`，施工 7-10）：「谁」那一格照 `history/agent.txt` 写它的名字，
/// 名字照模板的规矩转义，和请求里那块标签是同一个名字。别的会话发来的话（`by` 是会话，既不是父会话、也不是派的子代理，
/// 施工 C-2）：照 `history/session.txt` 写它的短编号，和请求里那块标签是同一个编号。别的没有。
fn from(kept: &Kept, event: &Event, placeholders: &Placeholders) -> Option<String> {
    match (&event.body, &event.by) {
        (Body::MessageUser(_), By::Harness(harness)) => Some(inline(say(
            &placeholders.agent,
            &[("name", harness.name.as_str())],
        ))),
        (Body::MessageUser(_), By::Session(session)) if kept.is_peer(&session.id) => Some(inline(
            say(&placeholders.session, &[("id", session.id.short())]),
        )),
        _ => None,
    }
}

/// 占位写在一行里：去掉字的文件末尾那个换行。
fn inline(text: String) -> String {
    text.trim_end().to_string()
}
