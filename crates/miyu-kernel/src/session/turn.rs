//! 正在进行的回合：怎么开、请求怎么发、怎么结束（`docs/designs/02-内核.md` 第六节「回合怎么开、
//! 请求怎么发」「回复怎么收、回合怎么结束」）。
//!
//! 开头那一批落了盘，叫执行器跑回合开始的挂接点；挂接点跑完了、追加过的事件都落了盘，
//! 组装请求，交给执行器去请求模型。请求在路上时的事在 [`super::call`]，调工具在 [`super::tools`]。

use super::Session;
use super::action::Action;
use super::breaker::Before;
use super::call::Call;
use super::input::Injection;
use super::manual::Manual;
use super::overflow::Passive;
use super::step::Step;
use crate::event::{Body, EndReason, Event, TurnEnded, TurnStarted};
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
    /// 这一轮加进来的目录：回合开始时的那些，判权限、派工具时带上（施工 5-10 上）。
    pub(super) dirs: Vec<String>,
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
    /// 这一轮里到的、会叫醒她的回报，还没被请求看到过，照先后（施工 7-2，`jobs.rs`）。和排着队的消息一样下一次请求就
    /// 听到了；回合结束时还没听到的接着开下一轮，打断的不开。打断退回时不撤回：不是人说的话。
    pub(super) reports: Vec<super::jobs::Arrived>,
    /// 上一次请求以后切过权限级别：下一次请求之前把事实查一遍。
    pub(super) refresh: bool,
    /// 这一步压过了（施工 6-2 下）：压完照常发这一步本来要发的请求，不再压第二次（`compaction.md` 第三条第 1 条）。
    /// 发了主请求就清掉。
    pub(super) compacted: bool,
    /// 打断了，在等停着的改文件的调用交回来（施工 4-9 再补一）：等齐了才收尾。
    pub(super) interrupting: Option<Interrupting>,
    /// 摘要请求换个样子再发（截短重试、隔离式回退，施工 6-6 中、下，`shorten.rs`）：落了盘再发时照它发。发出去就取走。
    pub(super) again: Option<super::shorten::Again>,
    /// 主请求报了超长，落了盘先压（被动压缩，施工 6-7，`overflow.rs`）。发出去就取走。
    pub(super) passive: Option<super::overflow::Passive>,
    /// 这一步被动压过了：重发的这一次再报超长，照一次失败算，不再压。主请求说完了就清掉。
    pub(super) overflowed: bool,
    /// 手动压缩单开的这一轮：替代到哪、人附的要求（施工 6-8，`manual.rs`）。平常的回合没有。
    pub(super) manual: Option<Manual>,
}

/// 打断以后在等停着的调用：谁打断的、哪个命令、排着队的怎么办，等的那一次 `Wake` 的记号。
#[derive(Debug)]
pub(super) struct Interrupting {
    pub(super) by: By,
    pub(super) cause: CommandId,
    pub(super) queued: super::input::Queued,
    /// `Wake` 带的记号：那一步回复的序号，到点送回的 `Woke` 照它对上。
    pub(super) wake: Seq,
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
    /// 刚开的回合：编号是它的 `turn.started` 的序号 `started`，`cause` 照交的，走到 `stage`；工作目录、加进来的目录取会话
    /// 现在的环境，别的都是还没开始的样子。平常的回合、手动压缩和清空单开的那一轮、载入时收拾的那一轮都从这里造（施工 6-8 补）。
    pub(super) fn new_turn(&self, started: Seq, cause: Option<CommandId>, stage: Stage) -> Turn {
        Turn {
            id: TurnId::new(started),
            cause,
            stage,
            cwd: self.environment.cwd.clone(),
            dirs: self.environment.dirs.clone(),
            requests: 0,
            retries: 0,
            retrying: false,
            interjected: None,
            queued: Vec::new(),
            reports: Vec::new(),
            refresh: false,
            compacted: false,
            interrupting: None,
            again: None,
            passive: None,
            overflowed: false,
            manual: None,
        }
    }

    /// 由第 `trigger` 条开一个回合：追加 `turn.started`（带着会话现在的工作目录），和变了的环境、权限、会话编号几块事实
    /// （`08-上下文投影.md` C10）；空闲时放宽的，这时生效。`cause` 是触发它的那条事件的
    /// `cause`。返回追加的事件。
    pub(super) fn open_turn(
        &mut self,
        at: Timestamp,
        trigger: Seq,
        cause: Option<CommandId>,
    ) -> Vec<Event> {
        let body = Body::TurnStarted(TurnStarted {
            trigger: Some(trigger),
            cwd: Some(self.environment.cwd.clone()),
            dirs: self.environment.dirs.clone(),
        });
        let started = self.record(at, By::Kernel, cause.clone(), body);
        let opened = Stage::Opening {
            opened: started.seq,
        };
        self.turn = Some(self.new_turn(started.seq, cause.clone(), opened));
        self.effective = self.permission.clone();
        // 记在一边的回报这一轮就听到了（施工 7-2）：不再由它们另开一轮。
        self.deferred.clear();
        let facts = self.policy.facts.boundary(
            &self.history,
            at,
            &self.environment,
            &self.permission,
            &self.id,
        );
        let mut events = vec![started];
        for fact in facts {
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
        actions.extend(self.advance(at));
        actions
    }

    /// 回合往下走：开头那一批落了盘，叫执行器跑回合开始的挂接点；挂接点跑完了、追加过的
    /// 事件都落了盘，拿有效历史组装请求，交给执行器去请求模型（到了压缩线的先压）（`08-上下文投影.md` 第一节
    /// 第 2 条「先落盘，后请求」）。发请求时算出指纹，和上一次请求的比出第一处不同。
    /// 回复里有工具调用的，回复落了盘就派。`at` 是这一刻：熔断要写事件时照它记（施工 6-6 上）。
    pub(super) fn advance(&mut self, at: Timestamp) -> Vec<Action> {
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
            Stage::Ready if self.unstored.is_empty() => self.ask(at),
            _ => Vec::new(),
        }
    }

    /// 发这一步的请求：拿有效历史组装，先问熔断（`breaker.rs`）：暂停着放不下的不发，压完很快又到线到了次数的写暂停；
    /// 用量过了压缩线的先压（`compaction.rs`），没过的交给执行器去请求模型。
    fn ask(&mut self, at: Timestamp) -> Vec<Action> {
        let Some(seen) = self.stored else {
            return Vec::new();
        };
        // 主请求报过超长的，先压（被动压缩，施工 6-7）。
        match self.turn.as_mut().and_then(|turn| turn.passive.take()) {
            Some(Passive::Due) => return self.passive_compaction(at),
            Some(Passive::Again(due)) => return self.start_compaction(due),
            None => {}
        }
        let request = self.policy.assembler.assemble(&self.history);
        // 手动压缩单开的那一轮：不问熔断，发摘要请求（施工 6-8，`manual.rs`）。
        if let Some(due) = self.manual_due(&request) {
            return self.start_compaction(due);
        }
        match self.before_asking(&request) {
            Before::Send => {}
            Before::Compact(due) => return self.start_compaction(due),
            Before::Pause(paused) => return self.pause(at, paused),
            Before::Refuse(error) => return self.refuse(at, seen, &request, error),
        }
        let fingerprint = request.fingerprint();
        let difference = self
            .last_request
            .as_ref()
            .and_then(|before| fingerprint.first_difference(before));
        self.last_request = Some(fingerprint);
        let Some(turn) = self.turn.as_mut() else {
            return Vec::new();
        };
        if !std::mem::take(&mut turn.retrying) {
            turn.requests += 1;
        }
        turn.compacted = false;
        turn.interjected = None;
        turn.queued.clear();
        turn.reports.clear();
        turn.stage = Stage::Asking(Call::new(seen, request.messages.len(), difference));
        vec![Action::CallModel {
            seen,
            request,
            changed: difference,
        }]
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
