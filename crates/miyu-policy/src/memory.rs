//! 记忆的范围（施工 R-3 下，`docs/blueprint/memory.md`「范围」，`docs/designs/17-记忆.md` L2、L3）：会话开局时定下、记进策略
//! 快照，之后不改。人格的 `persona.toml` 写默认的（`persona` 或 `session`），开会话时能换成三种里的哪一种。

use std::fmt;

use crate::Snapshot;

/// 一个会话的记忆的范围。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryScope {
    /// 跟着人格：用这个人格的会话都想得起来（默认）。
    Persona,
    /// 只在这个会话里：删掉会话，记忆一起没了。
    Session,
    /// 不召回、也不记：`miyu ask --no-memory`，子代理（17 第二节）。
    Off,
}

impl MemoryScope {
    /// 写法：`persona`、`session`、`off`。
    pub fn as_str(self) -> &'static str {
        match self {
            MemoryScope::Persona => "persona",
            MemoryScope::Session => "session",
            MemoryScope::Off => "off",
        }
    }

    /// 读写法；认不出的是 `None`。
    pub fn parse(text: &str) -> Option<MemoryScope> {
        match text {
            "persona" => Some(MemoryScope::Persona),
            "session" => Some(MemoryScope::Session),
            "off" => Some(MemoryScope::Off),
            _ => None,
        }
    }
}

impl fmt::Display for MemoryScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Snapshot {
    /// 记下记忆的范围。
    #[must_use]
    pub fn with_memory(mut self, scope: MemoryScope) -> Snapshot {
        self.memory = Some(scope.as_str().to_string());
        self
    }

    /// 这个会话的记忆的范围：没有的（以前造的）跟着人格；认不出的不记也不召回，不往不知道的地方写。
    pub fn memory_scope(&self) -> MemoryScope {
        match &self.memory {
            None => MemoryScope::Persona,
            Some(text) => MemoryScope::parse(text).unwrap_or(MemoryScope::Off),
        }
    }
}

#[cfg(test)]
mod tests;
