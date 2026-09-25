//! 事件的 `by`：这件事由谁引起（`docs/designs/03-事件模型.md` 第二节）。
//! 内核从连接取，不从正文取。

use serde::{Deserialize, Deserializer, Serialize};

use crate::id::{
    AccountId, CallId, ExternalId, ModelName, ModuleId, ProviderId, SessionId, VenueId,
};
use crate::raw::{self, RawJson};

/// JSON 里用 `kind` 分开七种；读到不认识的，整块原样留着。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum By {
    /// 有账号的人。
    Person(Person),
    /// 通讯平台上的人，由桥担保。
    External(External),
    Model(Model),
    /// 一次工具调用。
    Tool(Tool),
    /// 模块，包括扩展。
    Module(Module),
    /// 另一个会话，例如父会话给子代理留言。
    Session(Session),
    Kernel,
    #[serde(untagged)]
    Unknown(RawJson),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Person {
    pub account: AccountId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct External {
    pub venue: VenueId,
    pub id: ExternalId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Model {
    pub endpoint: ProviderId,
    pub model: ModelName,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tool {
    pub call_id: CallId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Module {
    pub id: ModuleId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
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
