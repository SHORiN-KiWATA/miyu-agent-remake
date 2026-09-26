//! 模型调用的事件（`docs/designs/03-事件模型.md` 第三节「模型调用怎么写」）。

use serde::{Deserialize, Serialize};

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
    /// 和这个会话上一次请求比，第一处不同在哪。只是接着加的、前面没有请求可比的，没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_difference: Option<FirstDifference>,
    /// 用量。供应商没报的，没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    /// 从请求发出去到第一段增量用了多少毫秒。没发出去的、一段增量都没来的，没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_token_ms: Option<u64>,
    /// 从请求发出去到说完用了多少毫秒。没发出去的，没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// 结果。
    pub result: CallResult,
    /// 出错的分类和原话，只在出错时有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<CallError>,
}

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
    }
);

/// 出错的分类和原话。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallError {
    /// 分类。
    pub class: ErrorClass,
    /// 原话，给查问题的人看，不进上下文。
    pub message: String,
}

text_enum!(
    /// 出错的分类：驱动分的六种（`05-内核接口.md` 第七节），加上内核自己查出来的两种。
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
    }
);

#[cfg(test)]
mod tests;
