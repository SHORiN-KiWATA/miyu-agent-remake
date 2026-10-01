//! 模型调用的事件（`docs/designs/03-事件模型.md` 第三节「模型调用怎么写」）。

use serde::{Deserialize, Serialize};

use crate::event::CompactTrigger;
use crate::id::{ContentHash, ModelName, ProviderId, Seq};
use crate::request::{Difference, Role};
use crate::text_enum::text_enum;

/// `model.called`：一次模型请求的记录，出错的也记（`08-上下文投影.md` 第七节）。
/// `by` 是内核：请求是内核发的。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelCalled {
    /// 这次请求看到了第几条为止，也是这次请求的名字；有回复的，和回复的 `seen` 一样。
    pub seen: Seq,
    /// 请求发给了哪个供应商。没发出去就失败了的，没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<ProviderId>,
    /// 请求发给了哪个模型。没发出去就失败了的，没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelName>,
    /// 驱动编码以后的请求字节的 SHA-256（`05-内核接口.md` 第七节）。没编码就失败了的，没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<ContentHash>,
    /// 统一的请求里有几条消息。
    pub messages: u64,
    /// 和这个会话上一次请求比，第一处不同在哪。只是接着加的、前面没有请求可比的，没有。装在盒子里（施工 3-8 四补）：多半
    /// 没有，放在事件里平白占地方；`purpose` 加进来以后，`model.called` 比别的种类大出两百字节，clippy 的
    /// `large_enum_variant` 拦下了。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_difference: Option<Box<FirstDifference>>,
    /// 用量。供应商没报的，没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    /// 从请求发出去到第一段增量用了多少毫秒。没发出去的、一段增量都没来的，没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_token_ms: Option<u64>,
    /// 从请求发出去到说完用了多少毫秒。没发出去的，没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// 回复里每一块的起止，照这次请求写成的 `message.assistant` 的块的先后，一块一项（施工 2-3 补，
    /// `kernel/events-bodies.md`）。头照它写「已思考 N 秒」，刷新、重开也算得出来。没写回复的没有；以前的日志没有
    /// 这一格，照读。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocks: Option<Vec<BlockSpan>>,
    /// 结果。
    pub result: CallResult,
    /// 出错的分类和原话，只在出错时有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<CallError>,
    /// 这是哪一种压缩的摘要请求（施工 6-6 上）；主请求没有。日志里靠它认出摘要请求，失败照它数（`compaction.md`
    /// 第十条第 3 条）。以前的日志没有这一格。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compaction: Option<CompactTrigger>,
    /// 辅助请求的用途（施工 3-8 四补，`26-提示词.md` J6）：回顾 `recap`、起标题 `title`（施工 3-8 五补）；主请求、摘要请求
    /// 没有。以前的日志没有这一格。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purpose: Option<Purpose>,
}

impl ModelCalled {
    /// 是一次辅助请求（施工 3-8 四补）：带着用途的，不认识的用途也算。它和主对话无关：她没在这次请求里听到什么、它报的用量
    /// 也不是主对话的大小，所以用量的锚、排队的消息听到没有、压缩的边界、渲染时回合开始的那几块，都不看它。
    pub fn aside(&self) -> bool {
        self.purpose.is_some()
    }
}

text_enum!(
    /// 辅助请求的用途（施工 3-8 四补）。
    Purpose {
        /// 回顾：`session.recap` 要的一句（`docs/blueprint/kernel/session.md`「回顾」）。
        Recap = "recap",
        /// 起标题：没起名的会话一轮答完以后内核自己要的（施工 3-8 五补，`docs/blueprint/kernel/session.md`「起标题」）。
        Title = "title",
    }
);

/// 第一处不同在哪：工具面、system，或者第几条消息。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstDifference {
    /// 哪一部分。
    pub part: Part,
    /// 第几条消息，从 0 数起。只有消息才有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<u64>,
    /// 那一条的角色；这一次少了的，是上一次那一条的角色。只有消息才有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<MessageRole>,
}

text_enum!(
    /// 请求的哪一部分。
    Part {
        /// 工具面。
        Tools = "tools",
        /// system。
        System = "system",
        /// 一条消息。
        Message = "message",
    }
);

text_enum!(
    /// 消息的角色，和统一的请求里的写法一样（`08-上下文投影.md` 第二节）。
    MessageRole {
        /// `user`。
        User = "user",
        /// `assistant`。
        Assistant = "assistant",
        /// `tool`。
        Tool = "tool",
    }
);

impl From<Difference> for FirstDifference {
    fn from(difference: Difference) -> FirstDifference {
        let (part, index, role) = match difference {
            Difference::Tools => (Part::Tools, None, None),
            Difference::System => (Part::System, None, None),
            Difference::Message { index, role } => {
                let role = match role {
                    Role::User => MessageRole::User,
                    Role::Assistant => MessageRole::Assistant,
                    Role::Tool => MessageRole::Tool,
                };
                (Part::Message, Some(index as u64), Some(role))
            }
        };
        FirstDifference { part, index, role }
    }
}

/// 回复里一块的起止（施工 2-3 补）：都是从请求发出去算起的毫秒数，和 `first_token_ms` 同一个起点。时钟往回拨了，
/// 早于发出去的算 0。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockSpan {
    /// 这一块第一段增量到的时刻。
    pub start_ms: u64,
    /// 这一块最后一段增量到的时刻，不早于 `start_ms`。收块的 `End` 不算：驱动流完了才一起收块，算上它，每一块都
    /// 收在流的末尾。
    pub end_ms: u64,
}

/// 用量，四项都是 token 数。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    /// 没命中缓存的输入。
    pub uncached: u64,
    /// 缓存读取。
    pub cache_read: u64,
    /// 缓存写入。
    pub cache_write: u64,
    /// 输出。
    pub output: u64,
}

text_enum!(
    /// 一次请求的结果。
    CallResult {
        /// 说完了。
        Ok = "ok",
        /// 出错。
        Error = "error",
        /// 被人打断：用量没有，用时算到打断为止。
        Interrupted = "interrupted",
    }
);

/// 出错的分类、原话，有的话还有 HTTP 状态码。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallError {
    /// 分类。
    pub class: ErrorClass,
    /// 原话，给查问题的人看，不进上下文。
    pub message: String,
    /// 供应商回的 HTTP 状态码（施工 3-5 三补）：头照它分 429、402、404 说人话，不从原话里抠。连不上的、流里报的、
    /// 内核自己查出来的没有；以前的日志没有这一格，照读。分类不看它：分类管的是内核怎么办。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
}

text_enum!(
    /// 出错的分类：驱动分的六种（`05-内核接口.md` 第七节），加上内核自己查出来的几种，和端口没发出去就说完的
    /// `no_model`（施工 8-6，`models.md`「事件」）。
    ErrorClass {
        /// 可重试。
        Retryable = "retryable",
        /// 限速。
        RateLimited = "rate_limited",
        /// 上下文超长。
        ContextTooLong = "context_too_long",
        /// 认证失败。
        Auth = "auth",
        /// 被内容策略拦截。
        ContentPolicy = "content_policy",
        /// 其他：驱动分不进上面五种的。变体不叫 `Other`，那个名字留给读到的不认识的分类。
        Unclassified = "other",
        /// 增量对不上，或者执行器的回报先后不对：驱动或执行器的错。
        BadStream = "bad_stream",
        /// 回复里一个块都没有。
        EmptyReply = "empty_reply",
        /// 摘要请求的回复里取不出摘要：是空的，或者调了工具（`compaction.md` 第三条第 6、7 条）。
        BadSummary = "bad_summary",
        /// 自动压缩暂停着，这一次请求明知放不下，没发（`compaction.md` 第二条第 5 条，施工 6-6 上）。
        CompactionPaused = "compaction_paused",
        /// 没有能用的模型：`models.chat` 没配、会话的引用解析不出（施工 8-6）。端口当场说完，没发出去；内核不再来。
        NoModel = "no_model",
    }
);

#[cfg(test)]
mod tests;
