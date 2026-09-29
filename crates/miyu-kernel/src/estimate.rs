//! 用量和压缩线（`docs/blueprint/compaction.md`「怎么走」第一、二条，`docs/designs/09-压缩.md` Z3，施工 6-1）。
//!
//! 用量 = 锚 + 锚以后新增内容的本地估算。锚是最近一次主请求时供应商报的真实用量（[`anchor`]）；
//! 本地估算照 codex，UTF-8 字节数除以 4（[`message`]）。只估锚之后那一截，估偏了也落在余量里，
//! 下一次请求就被真值盖过去（Claude Code、codex、pi、dsh 都是这个结构）。
//!
//! 这里只有算法；什么时候查、到线怎么压，在会话的 `compaction.rs`（施工 6-2）。压缩留尾巴时，一条事件照同样的数法
//! 估（[`event`]，施工 6-2 下）。

use crate::block::Block;
use crate::event::{Body, CallResult, Event};
use crate::history::History;
use crate::id::Seq;
use crate::origin::Model;
use crate::request::{Message, Request, json};

/// 本地估算：几个 UTF-8 字节算一个 token（照 codex 的 `APPROX_BYTES_PER_TOKEN`，2026-09-29 项目主人定）。
/// 中文一个字三个字节，算成 0.75 个 token，偏多；英文偏少一点，只估锚之后那一截，落在余量里。
const BYTES_PER_TOKEN: u64 = 4;

/// 一张图、一个文件在请求里算多少 token。各家的算法不一样，是驱动的事（DeepSeek 的在 6-3 做）。
pub trait Price {
    /// 一张宽 `width`、高 `height` 像素的图。
    fn image(&self, width: u32, height: u32) -> u64;
    /// 一个文件。
    fn file(&self) -> u64;
}

/// 没有算法时用的：不管多大，一张图、一个文件都算一个固定的数。
///
/// 出厂是 2000（Claude Code 的做法）：各家从 765 到 2000 都有，取偏多的那一头；真值下一次请求时由
/// 供应商报回来。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flat {
    /// 一张图算多少。
    pub image: u64,
    /// 一个文件算多少。
    pub file: u64,
}

impl Price for Flat {
    fn image(&self, _: u32, _: u32) -> u64 {
        self.image
    }

    fn file(&self) -> u64 {
        self.file
    }
}

/// 锚：最近一次主请求时供应商报的真实用量，和那次请求的样子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchor {
    /// 那条 `model.called` 的序号。
    pub seq: Seq,
    /// 发给了哪个端点的哪个模型。
    pub model: Model,
    /// 那次请求有几条消息。它的回复是之后请求里的第 `messages` 条（从 0 数）。
    pub messages: u64,
    /// 供应商报的 `uncached + cache_read + cache_write + output`。
    pub reported: u64,
}

/// 从有效历史里挑锚：最近一次压缩那一条之后的 `model.called` 中最近的一条，说完了（`ok`）、带着用量、
/// 知道发给了谁。
///
/// 压缩那一条之前的不算：摘要请求自己那条、尾巴里压缩以前的请求，报的都是压缩前的大小。撤掉的回合里的
/// 已经不在有效历史里了。一条都没有的，没有锚。
pub fn anchor(history: &History) -> Option<Anchor> {
    let after = history.checkpoint().map(|checkpoint| checkpoint.seq);
    history
        .events()
        .iter()
        .rev()
        .take_while(|event| after.is_none_or(|after| event.seq > after))
        .find_map(|event| {
            let Body::ModelCalled(called) = &event.body else {
                return None;
            };
            if called.result != CallResult::Ok {
                return None;
            }
            let usage = called.usage.as_ref()?;
            Some(Anchor {
                seq: event.seq,
                model: Model {
                    endpoint: called.endpoint.clone()?,
                    model: called.model.clone()?,
                },
                messages: called.messages,
                reported: usage
                    .uncached
                    .saturating_add(usage.cache_read)
                    .saturating_add(usage.cache_write)
                    .saturating_add(usage.output),
            })
        })
}

/// 这一次请求发给 `model` 时，上下文用了多少 token：锚加上锚以后新增的那几条的估算。
///
/// - 锚盖住的是工具面、system 和前 `messages + 1` 条（锚那次请求加上它的回复）：请求只追加，它们和锚那一次
///   一样。本地对它们的估算比供应商报的还大的，取估算（照 dsh，宁多勿少）。
/// - 整份估：没有锚；锚发给的端点、模型和这一次不一样（换过模型，照 dsh）；这一次的消息没比锚多（不该出现，
///   兜底）。
pub fn usage(request: &Request, anchor: Option<&Anchor>, model: &Model, price: &dyn Price) -> u64 {
    let covered = anchor
        .filter(|anchor| &anchor.model == model)
        .and_then(|anchor| {
            let covered = usize::try_from(anchor.messages).ok()?.checked_add(1)?;
            (covered <= request.messages.len()).then_some((anchor.reported, covered))
        });
    let Some((reported, covered)) = covered else {
        return head(request, request.messages.len(), price);
    };
    let added = request.messages[covered..]
        .iter()
        .fold(0u64, |sum, m| sum.saturating_add(message(m, price)));
    reported
        .max(head(request, covered, price))
        .saturating_add(added)
}

/// 压缩线：用量超过它就该压（Z3）。= 窗口 − min(最大输出, `reserve_cap`) − `margin`。
///
/// 没报最大输出的，输出预留按 `reserve_cap`。没有窗口的、算出来不是正数的（窗口比预留加余量还小），
/// 没有线：不主动压，只在供应商报超长时被动压。出厂的 `reserve_cap` 是 20000、`margin` 是 13000，
/// 和 Claude Code 的「有效窗口减 13000」一样。
pub fn line(
    window: Option<u64>,
    max_output: Option<u64>,
    reserve_cap: u64,
    margin: u64,
) -> Option<u64> {
    let reserve = max_output.map_or(reserve_cap, |max| max.min(reserve_cap));
    window?
        .checked_sub(reserve)?
        .checked_sub(margin)
        .filter(|line| *line > 0)
}

/// 一条消息的本地估算：文字、思考、工具调用的名字和参数、驱动的私有数据、认不出的块，UTF-8 字节加起来除以 4，
/// 向上取整；图片、文件照 `price`。不另加每块、每条的结构开销。
pub fn message(message: &Message, price: &dyn Price) -> u64 {
    let blocks = match message {
        Message::User { blocks } | Message::Assistant { blocks } | Message::Tool { blocks, .. } => {
            blocks
        }
    };
    blocks_of(blocks, price)
}

/// 一条事件的本地估算，留尾巴时用（施工 6-2 下）：人的消息、回复、工具结果照它们的内容块，和 [`message`] 一样数；
/// 事实照它的原文；别的事件不进上下文，算 0。
pub fn event(event: &Event, price: &dyn Price) -> u64 {
    match &event.body {
        Body::MessageUser(message) => blocks_of(&message.blocks, price),
        Body::MessageAssistant(reply) => blocks_of(&reply.blocks, price),
        Body::ToolResult(result) => blocks_of(&result.blocks, price),
        Body::ContextInjected(fact) => tokens(fact.text.len()),
        _ => 0,
    }
}

/// 一串内容块的估算：字节加起来除以 4，向上取整；图片、文件照 `price`。
fn blocks_of(blocks: &[Block], price: &dyn Price) -> u64 {
    let mut bytes = 0usize;
    let mut media = 0u64;
    for block in blocks {
        match block {
            Block::Text(text) => bytes = bytes.saturating_add(text.text.len()),
            Block::Reasoning(reasoning) => {
                bytes = bytes.saturating_add(reasoning.text.len());
                if let Some(private) = &reasoning.private {
                    bytes = bytes.saturating_add(private.data.get().len());
                }
            }
            Block::ToolCall(call) => {
                bytes = bytes
                    .saturating_add(call.name.len())
                    .saturating_add(call.args.len());
                if let Some(private) = &call.private {
                    bytes = bytes.saturating_add(private.data.get().len());
                }
            }
            Block::Unknown(raw) => bytes = bytes.saturating_add(raw.get().len()),
            Block::Image(image) => {
                media = media.saturating_add(price.image(image.width, image.height))
            }
            Block::File(_) => media = media.saturating_add(price.file()),
        }
    }
    tokens(bytes).saturating_add(media)
}

/// 整份请求里工具面、system 和前 `messages` 条消息的估算。工具面照它规范的 JSON 字节；没有工具的，
/// 工具面不发，算 0。
fn head(request: &Request, messages: usize, price: &dyn Price) -> u64 {
    let face = if request.tools.is_empty() {
        0
    } else {
        tokens(json(&request.tools).len())
    };
    request.messages[..messages].iter().fold(
        face.saturating_add(tokens(request.system.len())),
        |sum, m| sum.saturating_add(message(m, price)),
    )
}

/// 几个字节算几个 token：除以 4，向上取整。
fn tokens(bytes: usize) -> u64 {
    u64::try_from(bytes)
        .unwrap_or(u64::MAX)
        .div_ceil(BYTES_PER_TOKEN)
}

#[cfg(test)]
mod tests;
