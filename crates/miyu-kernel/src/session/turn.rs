//! 正在进行的回合：怎么开、请求怎么发（`docs/designs/02-内核.md` 第六节「回合怎么开、请求
//! 怎么发」）。
//!
//! 开头那一批落了盘，叫执行器跑回合开始的挂接点；挂接点跑完了、追加过的事件都落了盘，
//! 组装请求，交给执行器去请求模型。收回复、结束回合是施工 2-3 下。

use super::Session;
use super::action::Action;
use super::input::Injection;
use crate::event::{Body, Event, TurnStarted};
use crate::facts::changed;
use crate::id::{CommandId, Seq, TurnId};
use crate::origin::{By, Module};
use crate::time::Timestamp;

/// 正在进行的回合。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Turn {
    /// 回合编号：它的 `turn.started` 的序号。
    pub(super) id: TurnId,
    /// 回合里内核造的事件的 `cause`：触发它的那条事件的 `cause`，一个命令引起的事一路
    /// 追得下去。
    cause: Option<CommandId>,
    /// 走到了哪一步。
    stage: Stage,
}

/// 回合走到了哪一步。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    /// 开头那一批还没全落盘：落到第 `opened` 条，就叫执行器跑回合开始的挂接点。
    Opening {
        /// 开头那一批的最后一条。
        opened: Seq,
    },
    /// 回合开始的挂接点在跑。
    Hooking,
    /// 挂接点跑完了：追加过的事件都落了盘，就发请求。
    Ready,
    /// 请求在路上。
    Asking,
}

impl Session {
    /// 由第 `trigger` 条开一个回合：追加 `turn.started`，和变了的环境、权限两块事实
    /// （`08-上下文投影.md` C10）。`cause` 是触发它的那条事件的 `cause`。返回追加的事件。
    pub(super) fn open_turn(
        &mut self,
        at: Timestamp,
        trigger: Seq,
        cause: Option<CommandId>,
    ) -> Vec<Event> {
        let body = Body::TurnStarted(TurnStarted { trigger });
        let started = self.record(at, By::Kernel, cause.clone(), body);
        self.turn = Some(Turn {
            id: TurnId::new(started.seq),
            cause: cause.clone(),
            stage: Stage::Opening {
                opened: started.seq,
            },
        });
        let facts = vec![
            self.policy.facts.env(at, &self.environment),
            self.policy.facts.permission(&self.permission),
        ];
        let mut events = vec![started];
        for fact in changed(&self.history, &By::Kernel, facts) {
            events.push(self.record(at, By::Kernel, cause.clone(), Body::ContextInjected(fact)));
        }
        if let (Some(turn), Some(last)) = (self.turn.as_mut(), events.last()) {
            turn.stage = Stage::Opening { opened: last.seq };
        }
        events
    }

    /// 回合开始的挂接点跑完了：照交回来的先后追加成 `context.injected`，`by` 是各自的模块，
    /// 然后回合往下走。回合对不上的、同一个回合第二次来的，不理：打断以后迟到的就是这种。
    pub(super) fn turn_start_hooked(
        &mut self,
        at: Timestamp,
        turn: TurnId,
        injected: Vec<Injection>,
    ) -> Vec<Action> {
        let Some(current) = self.turn.as_mut() else {
            return Vec::new();
        };
        if current.id != turn || current.stage != Stage::Hooking {
            return Vec::new();
        }
        current.stage = Stage::Ready;
        let cause = current.cause.clone();
        let events: Vec<Event> = injected
            .into_iter()
            .map(|injection| {
                let by = By::Module(Module {
                    id: injection.module,
                });
                self.record(at, by, cause.clone(), Body::ContextInjected(injection.fact))
            })
            .collect();
        let mut actions = Vec::new();
        if !events.is_empty() {
            actions.push(Action::Append(events));
        }
        actions.extend(self.advance());
        actions
    }

    /// 回合往下走：开头那一批落了盘，叫执行器跑回合开始的挂接点；挂接点跑完了、追加过的
    /// 事件都落了盘，拿有效历史组装请求，交给执行器去请求模型（`08-上下文投影.md` 第一节
    /// 第 2 条「先落盘，后请求」）。
    pub(super) fn advance(&mut self) -> Vec<Action> {
        let Some(turn) = self.turn.as_mut() else {
            return Vec::new();
        };
        match turn.stage {
            Stage::Opening { opened } if self.stored.is_some_and(|stored| stored >= opened) => {
                turn.stage = Stage::Hooking;
                vec![Action::RunTurnStartHooks { turn: turn.id }]
            }
            Stage::Ready if self.unstored.is_empty() => {
                let Some(seen) = self.stored else {
                    return Vec::new();
                };
                turn.stage = Stage::Asking;
                let request = self.policy.assembler.assemble(&self.history);
                vec![Action::CallModel { seen, request }]
            }
            _ => Vec::new(),
        }
    }
}
