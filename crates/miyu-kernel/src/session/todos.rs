//! 待办（施工 D-3，`docs/blueprint/kernel/session.md`「待办」）：当前的清单由有效历史另记的那张表算（`history/todos.rs`），
//! 这里管交给头的那一份。
//!
//! - 变了才推：落了盘、推完事件以后，当前的清单和上次告诉头的那份不一样，推一条瞬时的 `todos.changed`。写了、撤销、恢复都
//!   走这一条。
//! - 先落盘，后推送：还没落盘的事件里有换待办、撤销、恢复的，等它们落了盘再比，头看到的清单一定是磁盘上有的。
//! - 造会话时告诉过头的是空的；载入时当已经告诉过当前那份（头订阅时从回应里拿，`protocol.md` 的 `subscribe`）。

use super::Session;
use super::action::Action;
use crate::event::{Body, Effect, Event, Todo, TodosChanged, Transient, TransientBody};
use crate::origin::By;
use crate::time::Timestamp;

impl Session {
    /// 当前的待办，照先后：没写过、清空了、写过的都撤掉了的是空的。协议照它回 `subscribe`。
    pub fn todos(&self) -> Vec<Todo> {
        self.history
            .todos()
            .map(<[Todo]>::to_vec)
            .unwrap_or_default()
    }

    /// 落了盘以后：当前的待办和上次告诉头的不一样，推一条 `todos.changed`，记下这一份。还有没落盘的事件会动待办的，先不推。
    pub(super) fn todos_changed(&mut self, at: Timestamp) -> Option<Action> {
        if self.unstored.iter().any(touches_todos) {
            return None;
        }
        let todos = self.todos();
        if todos == self.told_todos {
            return None;
        }
        self.told_todos = todos.clone();
        Some(Action::PushTransient(Transient {
            at,
            turn: self.turn.as_ref().map(|turn| turn.id),
            by: By::Kernel,
            cause: None,
            body: TransientBody::TodosChanged(TodosChanged { todos }),
        }))
    }
}

/// 这一条会不会动当前的待办：换了一份的工具结果、撤销、恢复。
fn touches_todos(event: &Event) -> bool {
    match &event.body {
        Body::ToolResult(result) => result
            .effects
            .iter()
            .any(|effect| matches!(effect, Effect::TodoWritten(_))),
        Body::TurnReverted(_) | Body::TurnUnreverted(_) => true,
        _ => false,
    }
}
