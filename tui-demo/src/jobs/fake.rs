//! 演示用的假数据源（蓝图 `tui.md`「后台命令、子代理和侧边栏」）：核心还没有的几样照 `resources/fake.json` 的脚本推——
//! 待办（隔一阵推进一项），确认和提问的抽屉的题。后台命令、子代理已经接核心，不在这里。

use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::drawer::{Approval, Asked, Fake};

use super::{Board, Todo, TodoState};

/// 一份假的待办。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TodoScript {
    /// 各项。
    pub items: Vec<String>,
    /// 隔多久推进一项。
    pub every_ms: u64,
}

/// 整个脚本（`resources/fake.json`）。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Script {
    /// 待办，`/demo-todo` 推。
    pub todos: TodoScript,
    /// 提问，`/demo-ask` 轮着出（蓝图「确认和提问的抽屉」）。
    pub asks: Vec<Fake<Asked>>,
    /// 权限确认，`/demo-approve` 轮着出。
    pub approvals: Vec<Fake<Approval>>,
}

/// 假数据源：待办推到第几项、下一步的时刻。
#[derive(Debug, Default)]
pub struct Feed {
    todo: Option<(usize, Instant)>,
}

impl Feed {
    /// 推一份待办：第一项在做，别的没做。
    pub fn start_todo(&mut self, script: &Script, board: &mut Board, now: Instant) {
        board.todos = script
            .todos
            .items
            .iter()
            .enumerate()
            .map(|(i, text)| Todo {
                text: text.clone(),
                state: if i == 0 {
                    TodoState::Active
                } else {
                    TodoState::Pending
                },
            })
            .collect();
        self.todo = Some((0, now + Duration::from_millis(script.todos.every_ms)));
    }

    /// 走到 `now`：到点的推一项。一次只推一项，落后了下一帧接着推。
    pub fn advance(&mut self, script: &Script, board: &mut Board, now: Instant) {
        let Some((step, at)) = self.todo.filter(|(_, at)| *at <= now) else {
            return;
        };
        let todos = &mut board.todos;
        if let Some(t) = todos.get_mut(step) {
            t.state = TodoState::Done;
        }
        self.todo = todos.get_mut(step + 1).map(|t| {
            t.state = TodoState::Active;
            (step + 1, at + Duration::from_millis(script.todos.every_ms))
        });
    }

    /// 下一步的时刻；没有在推的是 `None`。
    pub fn next_at(&self) -> Option<Instant> {
        self.todo.map(|(_, at)| at)
    }
}
