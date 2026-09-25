//! 事件的 `by`：这件事由谁引起（`docs/designs/03-事件模型.md` 第二节）。
//!
//! 内核从连接取，不从正文取：正文里自称是谁一概不作数（`01-架构.md` 第六节）。
//! 权限按「这一步是谁要求的」来判，看的就是这一格（`06-多用户与身份.md` 第五节）。

use serde::{Deserialize, Deserializer, Serialize};

use crate::id::{
    AccountId, CallId, ExternalId, ModelName, ModuleId, ProviderId, SessionId, VenueId,
};
use crate::raw::{self, RawJson};

/// 这件事由谁引起。JSON 里用 `kind` 分开七种；读到不认识的，整块原样留着。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum By {
    /// 有账号的人。
    Person(Person),
    /// 通讯平台上的人，没有账号，由桥担保（`06-多用户与身份.md` 第二节）。
    External(External),
    /// 模型：它的回复 `message.assistant`，连同里面的工具调用。
    Model(Model),
    /// 一次工具调用：这件事是它在执行时引起的。
    Tool(Tool),
    /// 模块，包括扩展，例如记忆模块注入的召回结果。
    Module(Module),
    /// 另一个会话，例如父会话给子代理留言。
    Session(Session),
    /// 内核自己，例如崩溃重启后给没走完的回合补上的「中断」（`02-内核.md` 不变量 8）。
    Kernel,
    /// 不认识的种类，新版本才有的：整块原样留着，写出去还是原样。
    #[serde(untagged)]
    Unknown(RawJson),
}

/// 有账号的人。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Person {
    /// 这个人的账号。
    pub account: AccountId,
}

/// 通讯平台上的人。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct External {
    /// 这个人是在哪个场所说的话，例如哪个群。
    pub venue: VenueId,
    /// 平台上的身份编号，由桥担保，例如 `qq:10086`。
    pub id: ExternalId,
}

/// 一个模型。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Model {
    /// 经哪个供应商调用的。
    pub endpoint: ProviderId,
    /// 模型的名字。和供应商的名字一样，内核只记不解读。
    pub model: ModelName,
}

/// 一次工具调用。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tool {
    /// 那一次调用的编号。
    pub call_id: CallId,
}

/// 一个模块，包括扩展。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Module {
    /// 模块的编号，就是它清单里的 `id`。
    pub id: ModuleId,
}

/// 另一个会话。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    /// 那个会话的编号。
    pub id: SessionId,
}

impl<'de> Deserialize<'de> for By {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        raw::read_tagged(
            d,
            "kind",
            |kind, json| {
                Some(match kind {
                    "person" => raw::parse(json).map(By::Person),
                    "external" => raw::parse(json).map(By::External),
                    "model" => raw::parse(json).map(By::Model),
                    "tool" => raw::parse(json).map(By::Tool),
                    "module" => raw::parse(json).map(By::Module),
                    "session" => raw::parse(json).map(By::Session),
                    "kernel" => Ok(By::Kernel),
                    _ => return None,
                })
            },
            By::Unknown,
        )
    }
}

#[cfg(test)]
mod tests;
