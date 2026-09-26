//! 正在进行的回合：怎么开、请求怎么发、怎么结束（`docs/designs/02-内核.md` 第六节「回合怎么开、
//! 请求怎么发」「回复怎么收、回合怎么结束」）。
//!
//! 开头那一批落了盘，叫执行器跑回合开始的挂接点；挂接点跑完了、追加过的事件都落了盘，
//! 组装请求，交给执行器去请求模型。请求在路上时的事在 [`super::call`]，调工具在 [`super::tools`]。

use super::Session;
use super::action::Action;
use super::call::Call;
use super::input::Injection;
use super::step::Step;
use crate::event::{Body, EndReason, Event, TurnEnded, TurnStarted};
use crate::facts::changed;
use crate::id::{CommandId, Seq, TurnId};
use crate::origin::{By, Module};
use crate::time::Timestamp;

/// 正在进行的回合。
#[derive(Debug)]
pub(super) struct Turn {
    /// 回合编号：它的 `turn.started` 的序号。
    pub(super) id: TurnId,
    /// 回合里内核造的事件的 `cause`：触发它的那条事件的 `cause`，一个命令引起的事一路
    /// 追得下去。
    pub(super) cause: Option<CommandId>,
    /// 走到了哪一步。
    pub(super) stage: Stage,
    /// 这一轮的工作目录：回合开始时的那一个，派工具时带上。
    pub(super) cwd: String,
    /// 这一轮请求过几次模型，比步数上限用。重试的不算（施工 3-5 下）。
    pub(super) requests: u32,
    /// 这一步连着出了几次可以重试的错：说完了一次就清零（`retry.rs`）。
    pub(super) retries: u32,
    /// 下一次请求是重试：不算步数。
    pub(super) retrying: bool,
    /// 急着插话的那句话是谁说的、哪个命令：下一次请求之前，还没跑的调用都跳过。
    pub(super) interjected: Option<Interjection>,
    /// 排着队的消息：回合进行中来的，还没被请求看到过。序号和它的命令，照先后。
    pub(super) queued: Vec<(Seq, Option<CommandId>)>,
    /// 上一次请求以后切过权限级别：下一次请求之前把事实查一遍。
    pub(super) refresh: bool,
}

/// 急着插话：谁说的，哪个命令。跳过的结果 `by` 是说话的人，`cause` 是这个命令。
#[derive(Debug)]
pub(super) struct Interjection {
    pub(super) by: By,
    pub(super) cause: CommandId,
}

/// 回合走到了哪一步。
#[derive(Debug)]
pub(super) enum Stage {
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
    Asking(Call),
    /// 出了可以重试的错，等着再来（施工 3-5 下）：到点了回到「准备好」，照有效历史再组装一次。
    Waiting {
        /// 为哪一次请求等的：出错的那一次。「到点了」照它对上。
        after: Seq,
    },
    /// 这次请求刚说完，正在收拾：回复写好以后，要么结束回合，要么换成调工具。只在处理
    /// 一条输入的当中出现，什么输入都不收。
    Settling,
    /// 回复里有工具调用：这一步的调用走到了哪。
    Tools(Step),
}

impl Session {
    /// 由第 `trigger` 条开一个回合：追加 `turn.started`，和变了的环境、权限两块事实
    /// （`08-上下文投影.md` C10）；空闲时放宽的，这时生效。`cause` 是触发它的那条事件的
    /// `cause`。返回追加的事件。
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
            cwd: self.environment.cwd.clone(),
            requests: 0,
            retries: 0,
            retrying: false,
            interjected: None,
            queued: Vec::new(),
            refresh: false,
        });
        self.effective = self.permission.clone();
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
        if current.id != turn || !matches!(current.stage, Stage::Hooking) {
            return Vec::new();
        }
        current.stage = Stage::Ready;
        let cause = current.cause.clone();
        let mut events: Vec<Event> = injected
            .into_iter()
            .map(|injection| {
                let by = By::Module(Module {
                    id: injection.module,
                });
                self.record(at, by, cause.clone(), Body::ContextInjected(injection.fact))
            })
            .collect();
        events.extend(self.refresh_facts(at));
        let mut actions = Vec::new();
        if !events.is_empty() {
            actions.push(Action::Append(events));
        }
        actions.extend(self.advance());
        actions
    }

    /// 回合往下走：开头那一批落了盘，叫执行器跑回合开始的挂接点；挂接点跑完了、追加过的
    /// 事件都落了盘，拿有效历史组装请求，交给执行器去请求模型（`08-上下文投影.md` 第一节
    /// 第 2 条「先落盘，后请求」）。发请求时算出指纹，和上一次请求的比出第一处不同。
    /// 回复里有工具调用的，回复落了盘就派。
    pub(super) fn advance(&mut self) -> Vec<Action> {
        let dispatched = self.dispatch();
        if !dispatched.is_empty() {
            return dispatched;
        }
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
                let request = self.policy.assembler.assemble(&self.history);
                let fingerprint = request.fingerprint();
                let difference = self
                    .last_request
                    .as_ref()
                    .and_then(|before| fingerprint.first_difference(before));
                self.last_request = Some(fingerprint);
                if !std::mem::take(&mut turn.retrying) {
                    turn.requests += 1;
                }
                turn.interjected = None;
                turn.queued.clear();
                turn.stage = Stage::Asking(Call::new(seen, request.messages.len(), difference));
                vec![Action::CallModel { seen, request }]
            }
            _ => Vec::new(),
        }
    }

    /// 结束正在进行的回合：追加 `turn.ended`，会话空闲。等它落了盘，再跑回合结束的挂接点。
    /// `by` 是结束它的一方：自己走完的、出错的是内核，被打断的是打断的人。
    pub(super) fn end_turn(
        &mut self,
        at: Timestamp,
        by: By,
        cause: Option<CommandId>,
        reason: EndReason,
    ) -> Event {
        let ended = self.record(at, by, cause, Body::TurnEnded(TurnEnded { reason }));
        if let Some(turn) = self.turn.take() {
            self.closing.push((turn.id, ended.seq));
        }
        ended
    }

    /// `turn.ended` 落了盘的回合：叫执行器跑回合结束的挂接点（广播，不等结果）。
    pub(super) fn closed(&mut self) -> Vec<Action> {
        let Some(stored) = self.stored else {
            return Vec::new();
        };
        let (done, waiting): (Vec<_>, Vec<_>) = std::mem::take(&mut self.closing)
            .into_iter()
            .partition(|&(_, ended)| ended <= stored);
        self.closing = waiting;
        done.into_iter()
            .map(|(turn, _)| Action::RunTurnEndHooks { turn })
            .collect()
    }
}
