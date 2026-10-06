//! 有效历史另记的一张表：写过的待办（施工 D-3，`docs/blueprint/kernel/session.md`「待办」）。
//!
//! 当前的清单是最近一份没撤掉的 `todo.written`。那一条会被压缩换掉，压缩以后头和检查点照样要看清单，所以另记一张表：压缩不
//! 丢，撤销、恢复只改「撤掉了没有」，和派出去过的任务（`jobs.rs`）一个办法。一次写一项，随写的次数长：一份清单几十个字，
//! 写几百次也不多。

use crate::event::{Body, Effect, Event, Todo};
use crate::id::TurnId;

/// 写过的每一份待办，照先后。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Todos {
    written: Vec<Written>,
}

/// 写过的一份。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Written {
    todos: Vec<Todo>,
    /// 写它的那一轮：撤销、恢复照它改 `undone`。
    turn: Option<TurnId>,
    /// 写它的那一轮撤掉了：还能恢复的、恢复不了的都算。恢复了就不算。
    undone: bool,
}

impl Todos {
    /// 记下这一条带来的变化：工具结果的效果里换上的待办记下；撤掉的那几轮里写的标成撤掉了，恢复的去掉这个标。
    pub(super) fn note(&mut self, event: &Event) {
        match &event.body {
            Body::ToolResult(result) => {
                for effect in &result.effects {
                    if let Effect::TodoWritten(written) = effect {
                        self.written.push(Written {
                            todos: written.todos.clone(),
                            turn: event.turn,
                            undone: false,
                        });
                    }
                }
            }
            Body::TurnReverted(reverted) => self.mark(&reverted.turns, true),
            Body::TurnUnreverted(unreverted) => self.mark(&unreverted.turns, false),
            _ => {}
        }
    }

    /// 当前的清单：最近一份没撤掉的。没写过的（或者写过的都撤掉了）没有；全部做完、清空了的是空的。
    pub(super) fn current(&self) -> Option<&[Todo]> {
        self.written
            .iter()
            .rev()
            .find(|written| !written.undone)
            .map(|written| written.todos.as_slice())
    }

    /// 在这几轮里写的，标成撤掉了没有。
    fn mark(&mut self, turns: &[TurnId], undone: bool) {
        for written in &mut self.written {
            if written.turn.is_some_and(|turn| turns.contains(&turn)) {
                written.undone = undone;
            }
        }
    }
}

#[cfg(test)]
mod tests;
