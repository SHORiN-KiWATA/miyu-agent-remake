//! 统一的请求：投影交给驱动的那一份（`docs/designs/08-上下文投影.md` 第二节
//! 「统一的请求怎么写」）。
//!
//! 它和供应商无关，驱动再把它编码成各家的格式（`05-内核接口.md` 第七节）。同样的请求
//! 写成的字节一定一样（内核不变量 3），所以能算哈希，也能和上一次请求比出第一处不同。

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::block::Block;
use crate::id::{CallId, ContentHash};
use crate::raw::RawJson;

/// 一份请求：工具面、system、消息，加上缓存标记。
///
/// 用哪个供应商、哪个模型、输出的上限不在这里：同一份投影可以交给不同的端点，
/// 这些是发请求的时候才定的。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Request {
    /// 工具面，按名字排好。
    pub tools: Vec<ToolSpec>,
    /// 系统提示词，一整段，照 `26-提示词.md` 第四节的顺序拼好。
    pub system: String,
    /// 示范对话、检查点、历史、本轮，照先后排。
    pub messages: Vec<Message>,
    /// 稳定区有几条消息，也就是示范对话有几条，不超过 `messages` 的条数。缓存标记
    /// 「稳定区结束」就在它们后面；「请求结束」就是最后一条，不另外标（08 第六节）。
    pub stable: usize,
}

/// 工具面上的一件工具：名字、说明、参数格式。存根也是这三样，只是说明短、参数宽松
/// （`25-工具加载.md`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolSpec {
    /// 工具名，英文，稳定不变（`05-内核接口.md` 第六节）。
    pub name: String,
    /// 给模型看的说明。
    pub description: String,
    /// 参数的 JSON Schema，原样照抄：重新写一遍会改变字节。
    pub parameters: RawJson,
}

/// 请求里的一条消息。JSON 里用 `role` 分开三种。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum Message {
    /// 模型以外的一方说的：人的消息、事实块、压缩的检查点、子代理的回报。
    /// 事实块排在触发消息前面，是同一条消息里的几块（08 第五节）。
    User {
        /// 内容块，照先后排。
        blocks: Vec<Block>,
    },
    /// 模型的回复，工具调用也在里面。思考块连同驱动私有数据原样带着，发哪些由驱动定。
    Assistant {
        /// 内容块，照先后排。
        blocks: Vec<Block>,
    },
    /// 一次工具调用的结果，一次调用一条，按调用的先后排（`03-事件模型.md` 第六节）。
    /// 各家要把结果放在哪、要不要和别的合并，是驱动的事。
    Tool {
        /// 这是哪一次调用的结果。
        call_id: CallId,
        /// 算不算出错。
        error: bool,
        /// 给模型看的内容。
        blocks: Vec<Block>,
    },
}

/// 消息的角色。指纹报「第一处不同」时，说是哪一种消息。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// `user`。
    User,
    /// `assistant`。
    Assistant,
    /// `tool`。
    Tool,
}

impl Role {
    /// 在 JSON 里的写法。
    pub fn as_str(self) -> &'static str {
        match self {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Tool => "tool",
        }
    }
}

impl Message {
    /// 这条消息的角色。
    pub fn role(&self) -> Role {
        match self {
            Message::User { .. } => Role::User,
            Message::Assistant { .. } => Role::Assistant,
            Message::Tool { .. } => Role::Tool,
        }
    }
}

impl Request {
    /// 规范的字节：紧凑的 JSON，字段照结构体的顺序，参数格式原样照抄。同样的请求，
    /// 字节一定一样。
    pub fn canonical_bytes(&self) -> Vec<u8> {
        json(self)
    }

    /// 请求的哈希：规范字节的 SHA-256。
    pub fn hash(&self) -> ContentHash {
        ContentHash::of(&self.canonical_bytes())
    }

    /// 这份请求的指纹：工具面、system、每条消息，各算一个 SHA-256。
    pub fn fingerprint(&self) -> Fingerprint {
        Fingerprint {
            tools: digest(&json(&self.tools)),
            system: digest(&json(&self.system)),
            messages: self
                .messages
                .iter()
                .map(|message| (message.role(), digest(&json(message))))
                .collect(),
        }
    }
}

/// 一份请求的指纹：工具面、system、每条消息各一个 SHA-256。
///
/// 和上一次请求的指纹一比，就知道第一处不同在哪（08 第七节）。只留这些哈希，
/// 每条消息 32 个字节，不留上一次的请求本身。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fingerprint {
    tools: [u8; 32],
    system: [u8; 32],
    messages: Vec<(Role, [u8; 32])>,
}

/// 和上一次请求比，第一处不同在哪。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Difference {
    /// 工具面变了。
    Tools,
    /// system 变了。
    System,
    /// 第 `index` 条消息不同，从 0 数起。
    Message {
        /// 第几条，从 0 数起。
        index: usize,
        /// 那一条的角色。这一次少了的，是上一次那一条的角色。
        role: Role,
    },
}

impl Fingerprint {
    /// 和上一次请求的指纹比，第一处不同在哪。
    ///
    /// 这一次只是在上一次后面接着加的，就是 `None`：前缀一个字节没动，缓存照样命中。
    /// 上一次有、这一次少了的，从少了的那一条算不同。
    pub fn first_difference(&self, before: &Fingerprint) -> Option<Difference> {
        if self.tools != before.tools {
            return Some(Difference::Tools);
        }
        if self.system != before.system {
            return Some(Difference::System);
        }
        let changed = self
            .messages
            .iter()
            .zip(&before.messages)
            .position(|(now, then)| now != then);
        if let Some(index) = changed {
            let role = self.messages[index].0;
            return Some(Difference::Message { index, role });
        }
        let index = self.messages.len();
        before
            .messages
            .get(index)
            .map(|&(role, _)| Difference::Message { index, role })
    }
}

/// 写成紧凑的 JSON。请求里只有字符串、数字、布尔、原样的 JSON 和结构体，
/// 没有非字符串的键，也没有会报错的 `Serialize`，所以写不出来是不可能的。
fn json(value: &impl Serialize) -> Vec<u8> {
    serde_json::to_vec(value).expect("请求里没有写不成 JSON 的东西")
}

/// 一串字节的 SHA-256。
fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

#[cfg(test)]
mod tests;
