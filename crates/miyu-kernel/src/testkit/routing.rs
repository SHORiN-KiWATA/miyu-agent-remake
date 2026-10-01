//! 替身那一头的路由（施工 8-10）：回合开始时内核交来的引用照先后记下；测试说哪个引用解析不出了，照这一轮的 `models.chat`
//! 退回，交回 `replaced`，和真的路由一样（`docs/blueprint/models.md`「怎么走」第六条第 3、4 条）。`models.chat` 也解析不出
//! 的不交。

use super::Stage;
use crate::id::CommandId;
use crate::session::{Command, Replaced};

/// 替身的路由：交来的引用，解析不出的几个，`models.chat`。
#[derive(Debug, Default)]
pub(super) struct Routing {
    /// 回合开始时交来的引用，照先后。
    asked: Vec<Option<String>>,
    /// 解析不出的引用：配置里删掉了的。
    gone: Vec<String>,
    /// 这一轮的 `models.chat`；没配的没有。
    chat: Option<String>,
}

impl Routing {
    /// 回合开始：记下交来的引用 `model`；它解析不出、`models.chat` 解析得出的，退回 `models.chat`。
    pub(super) fn resolve(&mut self, model: Option<String>) -> Option<Replaced> {
        self.asked.push(model.clone());
        let from = model.filter(|model| self.gone.contains(model))?;
        let to = self.chat.clone().filter(|chat| !self.gone.contains(chat))?;
        Some(Replaced { from, to })
    }
}

impl Stage {
    /// 换模型（施工 8-10）。返回这个命令的编号。
    pub fn configure(&mut self, model: &str) -> CommandId {
        self.command(Command::Configure {
            model: model.to_string(),
        })
    }

    /// 从现在起，引用 `gone` 解析不出了（配置里删了）；这一轮的 `models.chat` 是 `chat`，没配的是 `None`。
    pub fn gone(&mut self, gone: &str, chat: Option<&str>) {
        self.routing.gone.push(gone.to_string());
        self.routing.chat = chat.map(str::to_string);
    }

    /// 会话现在的引用，内核说的（施工 8-10）。
    pub fn reference(&self) -> Option<&str> {
        self.session.reference()
    }

    /// 回合开始时内核交来的引用，照先后（施工 8-10）。
    pub fn asked_models(&self) -> &[Option<String>] {
        &self.routing.asked
    }
}
