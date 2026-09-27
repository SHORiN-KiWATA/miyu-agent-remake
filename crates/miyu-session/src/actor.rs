//! actor 本身（`docs/designs/02-内核.md` 第七节「会话 actor 怎么跑」）：一个会话一个异步任务。
//!
//! 人的命令、执行器的回报进收件箱，一条一条送进内核，内核交出的动作照 02 第四节「执行器怎么回动作」
//! 的表回。当场就能回的（落盘了、挂接点跑完了）放进本地的队列，先于收件箱里的送：和执行器替身
//! （`miyu-kernel` 的 `testkit`）一个先后。

use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{broadcast, mpsc, oneshot};
use tracing::Instrument;

use miyu_kernel::event::{CallError, Event, Transient, TransientBody, Usage};
use miyu_kernel::id::{CommandId, Seq, SessionId};
use miyu_kernel::request::Request;
use miyu_kernel::session::{Action, Input, Outcome, Received, Session};
use miyu_kernel::time::Timestamp;

use crate::TARGET;
use crate::clock::Clock;
use crate::handle::{Message, Pushed};
use crate::kinds;
use crate::port::{Back, Cancel, ModelPort, Report, Reports};
use crate::store::Store;

/// 推送的队列：一个会话最多攒这么多份还没被读走的。读得慢的订阅者被挤掉，掉了队
/// （`04-核心协议.md` 第七节）。
pub(crate) const PUSH_QUEUE: usize = 1024;

/// 一个会话的 actor：内核的状态机，和回它的动作要用的。
pub(crate) struct Actor {
    session: Session,
    /// 写盘的地方：写的时候挪进阻塞线程，写完拿回来。
    store: Option<Box<dyn Store>>,
    model: Arc<dyn ModelPort>,
    /// 人的命令：拿着 `Handle` 的都放下了，它就关了。
    inbox: mpsc::UnboundedReceiver<Message>,
    /// 执行器的回报：请求的回报、到点了。
    back: mpsc::UnboundedReceiver<Back>,
    /// 交给执行器、往 `back` 里送的那一头。actor 自己拿着一份，`back` 不会自己关。
    backs: mpsc::UnboundedSender<Back>,
    pushes: broadcast::Sender<Arc<Pushed>>,
    /// 等着回应的命令：同一个编号来了几次，照先后排着。
    replies: BTreeMap<CommandId, VecDeque<oneshot::Sender<Outcome>>>,
    /// 还没说完的请求：叫停它的那一头，和交给端口的那一刻（算用时）。
    calls: BTreeMap<Seq, (oneshot::Sender<()>, Instant)>,
    clock: Clock,
}

/// 会话停了：写不进去。
pub(crate) struct Stop;

/// 会话的 span：开在 `ERROR` 级。span 也照级别筛，开在 `INFO` 的话，调到 `WARN` 它就被筛掉了，底下的
/// 行就没了会话编号（`miyu-log` 的说明，施工 3-7 上）。
pub(crate) fn span(id: &SessionId) -> tracing::Span {
    tracing::error_span!(target: TARGET, "session", session = id.as_str())
}

/// 起一个 actor 的任务，外面再套一个看着它的：它 panic 了（内核自己的 bug、端口的 bug），记一条
/// `ERROR`，别的会话照常（`28-运行日志.md` 第三节：`ERROR` 一定是 bug）。
pub(crate) fn spawn(actor: Actor, first: Vec<Action>, span: tracing::Span) {
    let task = tokio::spawn(actor.run(first).instrument(span.clone()));
    tokio::spawn(
        async move {
            if let Err(error) = task.await
                && error.is_panic()
            {
                tracing::error!(target: TARGET, "panicked, stopped");
            }
        }
        .instrument(span),
    );
}

/// 收件箱里的一封怎么办。
enum Mail {
    /// 送进内核。
    Input(Input),
    /// 当场办完了。
    Done,
    /// 有计划地停下，停好了回这一头。
    Stop(oneshot::Sender<()>),
}

impl Actor {
    /// 一个 actor：会话的状态机、写盘的地方、请求模型的端口、收件箱、时钟。
    pub(crate) fn new(
        session: Session,
        store: Box<dyn Store>,
        model: Arc<dyn ModelPort>,
        inbox: mpsc::UnboundedReceiver<Message>,
        clock: Clock,
    ) -> Actor {
        let (backs, back) = mpsc::unbounded_channel();
        let (pushes, _) = broadcast::channel(PUSH_QUEUE);
        Actor {
            session,
            store: Some(store),
            model,
            inbox,
            back,
            backs,
            pushes,
            replies: BTreeMap::new(),
            calls: BTreeMap::new(),
            clock,
        }
    }

    /// 命令 `id` 在等回应：造会话的那一个，在 actor 跑起来之前就在等。
    pub(crate) fn wait_for(&mut self, id: CommandId, reply: oneshot::Sender<Outcome>) {
        self.replies.entry(id).or_default().push_back(reply);
    }

    /// 跑：先回造会话、载入吐出来的动作，再一封封收。执行器的回报先收，读流不断。
    pub(crate) async fn run(mut self, first: Vec<Action>) {
        if self.settle(first).await.is_err() {
            return;
        }
        loop {
            let input = tokio::select! {
                biased;
                Some(back) = self.back.recv() => self.back(back),
                message = self.inbox.recv() => match message.map(|message| self.mail(message)) {
                    Some(Mail::Input(input)) => input,
                    Some(Mail::Done) => continue,
                    Some(Mail::Stop(reply)) => return self.stop(reply).await,
                    None => {
                        tracing::info!(target: TARGET, "closed");
                        return;
                    }
                },
            };
            if self.drain(VecDeque::from([input])).await.is_err() {
                return;
            }
        }
    }

    /// 收件箱里的一封：命令照 actor 的时钟记下到的时刻，订阅当场办。
    fn mail(&mut self, message: Message) -> Mail {
        match message {
            Message::Command {
                id,
                by,
                command,
                reply,
            } => {
                self.wait_for(id.clone(), reply);
                let at = self.clock.now();
                Mail::Input(Input::Command(Received {
                    id,
                    by,
                    at,
                    command,
                }))
            }
            Message::Subscribe(reply) => {
                answer(reply, self.pushes.subscribe());
                Mail::Done
            }
            Message::Stop(reply) => Mail::Stop(reply),
            Message::Environment(environment) => Mail::Input(Input::Environment(environment)),
        }
    }

    /// 有计划地停下：送进「要重启了」，它产生的事件落了盘，回一声。
    async fn stop(&mut self, reply: oneshot::Sender<()>) {
        let at = self.clock.now();
        if self
            .drain(VecDeque::from([Input::Restarting { at }]))
            .await
            .is_ok()
        {
            tracing::info!(target: TARGET, "stopped");
            answer(reply, ());
        }
    }

    /// 回一串动作，再把当场回的输入送进去。
    async fn settle(&mut self, actions: Vec<Action>) -> Result<(), Stop> {
        let mut inputs = VecDeque::new();
        for action in actions {
            inputs.extend(self.act(action).await?);
        }
        self.drain(inputs).await
    }

    /// 一条条送进内核，回每个动作；当场回的输入排在后面接着送，直到没有。
    async fn drain(&mut self, mut inputs: VecDeque<Input>) -> Result<(), Stop> {
        while let Some(input) = inputs.pop_front() {
            let kind = kinds::input(&input);
            match kinds::chatty_input(&input) {
                true => tracing::trace!(target: TARGET, kind, "input"),
                false => tracing::debug!(target: TARGET, kind, "input"),
            }
            for action in self.session.handle(input) {
                inputs.extend(self.act(action).await?);
            }
        }
        Ok(())
    }

    /// 照 02 第四节的表回一个动作；当场就能回的，交回要送进内核的输入。
    async fn act(&mut self, action: Action) -> Result<Option<Input>, Stop> {
        let kind = kinds::action(&action);
        match kinds::chatty_action(&action) {
            true => tracing::trace!(target: TARGET, kind, "action"),
            false => tracing::debug!(target: TARGET, kind, "action"),
        }
        Ok(match action {
            Action::Append(events) => self.append(events).await?,
            Action::Reply { id, outcome } => {
                self.reply(id, outcome);
                None
            }
            Action::Push(events) => {
                self.push(Pushed::Events(events));
                None
            }
            Action::PushTransient(transient) => {
                retrying(&transient);
                self.push(Pushed::Transient(transient));
                None
            }
            Action::RunTurnStartHooks { turn } => Some(Input::TurnStartHooksDone {
                at: self.clock.now(),
                turn,
                injected: Vec::new(),
            }),
            Action::CallModel { seen, request } => {
                self.call(seen, request);
                None
            }
            Action::Wake { at, seen } => {
                self.wake(at, seen);
                None
            }
            Action::CancelModel { seen } => {
                self.cancel(seen);
                None
            }
            Action::RunTurnEndHooks { .. } => None,
            Action::GuardTool { .. }
            | Action::RunTool { .. }
            | Action::AnswerTool { .. }
            | Action::CancelTool { .. } => {
                tracing::error!(target: TARGET, action = kind, "tool action without tools");
                None
            }
        })
    }

    /// 在阻塞线程里写、同步，写完交回「落盘了」。写不进去就停下。
    async fn append(&mut self, events: Vec<Event>) -> Result<Option<Input>, Stop> {
        let Some(upto) = events.last().map(|event| event.seq) else {
            return Ok(None);
        };
        let mut store = self.store.take().ok_or(Stop)?;
        let written = tokio::task::spawn_blocking(move || {
            let result = store.append(&events);
            (store, result)
        })
        .await;
        match written {
            Ok((store, Ok(()))) => {
                self.store = Some(store);
                Ok(Some(Input::Stored { upto }))
            }
            Ok((_, Err(error))) => {
                tracing::warn!(target: TARGET, kind = ?error.kind(), "write failed, stopped");
                Err(Stop)
            }
            Err(error) => {
                if error.is_panic() {
                    tracing::error!(target: TARGET, "panicked, stopped");
                }
                Err(Stop)
            }
        }
    }

    /// 回应命令 `id`：交给等着它的最早的那一头。
    fn reply(&mut self, id: CommandId, outcome: Outcome) {
        let Some(waiting) = self.replies.get_mut(&id) else {
            return;
        };
        let first = waiting.pop_front();
        if waiting.is_empty() {
            self.replies.remove(&id);
        }
        if let Some(reply) = first {
            answer(reply, outcome);
        }
    }

    /// 推给订阅了的头。
    #[expect(
        clippy::let_underscore_must_use,
        reason = "没有订阅者就没人收，照常往下走"
    )]
    fn push(&self, pushed: Pushed) {
        let _ = self.pushes.send(Arc::new(pushed));
    }

    /// 请求模型：交给端口，记下叫停它的那一头和这一刻。
    fn call(&mut self, seen: Seq, request: Request) {
        let model = self.model.model();
        tracing::info!(
            target: TARGET,
            seen = seen.get(),
            endpoint = model.endpoint.as_str(),
            model = model.model.as_str(),
            "request"
        );
        let (stop, cancel) = oneshot::channel();
        self.calls.insert(seen, (stop, Instant::now()));
        let reports = Reports::new(seen, self.backs.clone());
        self.model.call(seen, request, reports, Cancel::new(cancel));
    }

    /// 不要请求 `seen` 了：叫端口停下。
    fn cancel(&mut self, seen: Seq) {
        let Some((stop, asked)) = self.calls.remove(&seen) else {
            return;
        };
        answer(stop, ());
        tracing::info!(
            target: TARGET,
            seen = seen.get(),
            took_ms = millis(asked.elapsed()),
            "cancelled"
        );
    }

    /// 到点叫醒：起一个定时的任务，到 `at` 这一刻送回「到点了」。
    fn wake(&mut self, at: Timestamp, seen: Seq) {
        let wait = at
            .unix_millis()
            .saturating_sub(self.clock.now().unix_millis());
        let wait = Duration::from_millis(u64::try_from(wait).unwrap_or(0));
        let backs = self.backs.clone();
        tokio::spawn(async move {
            tokio::time::sleep(wait).await;
            answer_back(&backs, Back::Woke { seen });
        });
    }

    /// 执行器送回来的，照 actor 的时钟记下到的时刻，写成内核的输入。
    fn back(&mut self, back: Back) -> Input {
        let at = self.clock.now();
        match back {
            Back::Woke { seen } => Input::Woke { at, seen },
            Back::Report { seen, report } => match report {
                Report::Sent { model, request } => Input::RequestSent {
                    at,
                    seen,
                    model,
                    request,
                },
                Report::Delta(delta) => Input::ModelDelta { at, seen, delta },
                Report::Ended {
                    usage,
                    error,
                    wait_ms,
                } => {
                    self.ended(seen, usage.as_ref(), error.as_ref());
                    Input::ModelEnded {
                        at,
                        seen,
                        usage,
                        error,
                        wait_ms,
                    }
                }
            },
        }
    }

    /// 请求 `seen` 说完了：不用再叫停它了；记一行收场（`28-运行日志.md` 第三节）。
    fn ended(&mut self, seen: Seq, usage: Option<&Usage>, error: Option<&CallError>) {
        let Some((_, asked)) = self.calls.remove(&seen) else {
            return;
        };
        let took_ms = millis(asked.elapsed());
        match error {
            Some(error) => tracing::info!(
                target: TARGET,
                seen = seen.get(),
                took_ms,
                class = error.class.as_str(),
                "failed"
            ),
            None => tracing::info!(
                target: TARGET,
                seen = seen.get(),
                took_ms,
                "in" = usage.map(|usage| usage
                    .uncached
                    .saturating_add(usage.cache_read)
                    .saturating_add(usage.cache_write)),
                hit = usage.map(|usage| usage.cache_read),
                write = usage.map(|usage| usage.cache_write).filter(|written| *written > 0),
                out = usage.map(|usage| usage.output),
                "ended"
            ),
        }
    }
}

/// 等着重试的状态提示，记一条 `WARN`：第几次、一共几次、等多久、出错的分类。原话不写：供应商的
/// 出错信息里可能回显请求里的字。
fn retrying(transient: &Transient) {
    if let TransientBody::Status(status) = &transient.body {
        let retry = &status.retry;
        tracing::warn!(
            target: TARGET,
            seen = status.seen.get(),
            attempt = retry.attempt,
            limit = retry.limit,
            wait_ms = retry.wait_ms,
            class = retry.class.as_str(),
            "retrying"
        );
    }
}

/// 交回一声。等的那一头不等了，就没人收。
#[expect(
    clippy::let_underscore_must_use,
    reason = "等的那一头不等了：回的话没人要，丢掉"
)]
fn answer<T>(reply: oneshot::Sender<T>, value: T) {
    let _ = reply.send(value);
}

/// 送回 actor。会话停了就送不进去，丢掉。
#[expect(
    clippy::let_underscore_must_use,
    reason = "会话停了：到点了也没人要，丢掉"
)]
fn answer_back(backs: &mpsc::UnboundedSender<Back>, back: Back) {
    let _ = backs.send(back);
}

/// 一段时间，毫秒。
fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests;
