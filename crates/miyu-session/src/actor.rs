//! actor 本身（`docs/designs/02-内核.md` 第七节「会话 actor 怎么跑」）：一个会话一个异步任务。
//!
//! 人的命令、执行器的回报进收件箱，一条一条送进内核，内核交出的动作照 02 第四节「执行器怎么回动作」
//! 的表回。当场就能回的（落盘了、挂接点跑完了）放进本地的队列，先于收件箱里的送：和执行器替身
//! （`miyu-kernel` 的 `testkit`）一个先后。

use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use tokio::sync::{broadcast, mpsc, oneshot};
use tracing::Instrument;

use miyu_kernel::id::{CommandId, Seq, SessionId};
use miyu_kernel::session::{Action, Input, Outcome, Received, Session};
use miyu_kernel::time::Timestamp;

use crate::TARGET;
use crate::blocking::blocking;
use crate::clock::Clock;
use crate::guard::Guard;
use crate::handle::{Message, Pushed};
use crate::jobs::{Jobs, SessionJobs};
use crate::kinds;
use crate::lines::note;
use crate::port::{Back, ModelPort, Report};
use crate::report::Reporter;
use crate::store::Store;
use crate::tools::{Dispatch, ToolKit, Tools};

mod model;
mod stop;
mod store;

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
    /// 执行工具的端口（施工 4-2）。
    tools: Tools,
    /// 这个会话的后台命令（施工 7-3）：丢掉它就整组杀掉。
    jobs: SessionJobs,
    /// 执行前的链：权限策略（施工 4-3 下）。在阻塞线程里判，所以放在 `Arc` 里交过去（施工 4-9 再补四下）。
    guard: Arc<Guard>,
    /// 有没有在跑的回合，和 `Handle` 共用：每送完一批输入写一次；actor 退出了写成没有（施工 3-9 上）。
    busy: Arc<AtomicBool>,
    /// 向上回报交给谁（施工 7-6，`report.rs`）：子会话、有会话表的端口才有。
    reporter: Option<Reporter>,
}

/// 会话停了：写不进去。
pub(crate) struct Stop;

/// 任务表里这个会话的那一份要的（施工 7-3）：核心里的那张表、会话目录（输出写在它下面的 `jobs/`）、属主的 blob、这个
/// 会话的任务编号（和派子代理的共用一个，施工 7-5）。
pub(crate) struct JobKit {
    /// 核心里的那张表。
    pub(crate) table: Arc<Jobs>,
    /// 会话目录。
    pub(crate) dir: std::path::PathBuf,
    /// 属主的 blob。
    pub(crate) blobs: miyu_store::blob::Blobs,
    /// 这个会话的任务编号：从内核照日志算的用过的最大编号往后数。
    pub(crate) ids: Arc<crate::job_ids::JobIds>,
}

/// 会话的 span：开在 `ERROR` 级。span 也照级别筛，开在 `INFO` 的话，调到 `WARN` 它就被筛掉了，底下的
/// 行就没了会话编号（`miyu-log` 的说明，施工 3-7 上）。
pub(crate) fn span(id: &SessionId) -> tracing::Span {
    tracing::error_span!(target: TARGET, "session", session = id.as_str())
}

/// 起一个 actor 的任务，外面再套一个看着它的：它 panic 了（内核自己的 bug、端口的 bug），记一条
/// `ERROR`，别的会话照常（`28-运行日志.md` 第三节：`ERROR` 一定是 bug）。
pub(crate) fn spawn(actor: Actor, first: Vec<Action>, span: tracing::Span) {
    let busy = actor.busy();
    let task = tokio::spawn(actor.run(first).instrument(span.clone()));
    tokio::spawn(
        async move {
            if let Err(error) = task.await
                && error.is_panic()
            {
                tracing::error!(target: TARGET, "panicked, stopped");
            }
            // 停了的会话不算在跑：核心不为它不肯空闲退出。
            busy.store(false, Ordering::Release);
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
    /// 一个 actor：会话的状态机、写盘的地方、请求模型的端口、工具目录和替工具写的两句、任务表、权限策略、收件箱、时钟。
    #[expect(
        clippy::too_many_arguments,
        reason = "造 actor 的几样各不相干，拼成一个结构体也只是换个地方列"
    )]
    pub(crate) fn new(
        session: Session,
        store: Box<dyn Store>,
        model: Arc<dyn ModelPort>,
        tools: ToolKit,
        jobs: JobKit,
        guard: Guard,
        inbox: mpsc::UnboundedReceiver<Message>,
        clock: Clock,
    ) -> Actor {
        let (backs, back) = mpsc::unbounded_channel();
        let tools = Tools::new(tools, backs.clone());
        let jobs = SessionJobs::new(&jobs.table, jobs.dir, jobs.blobs, jobs.ids, backs.clone());
        let (pushes, _) = broadcast::channel(PUSH_QUEUE);
        let busy = Arc::new(AtomicBool::new(!session.idle()));
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
            tools,
            jobs,
            guard: Arc::new(guard),
            busy,
            reporter: None,
        }
    }

    /// 向上回报交给 `reporter`（施工 7-6）：子会话造好、载入时交。
    pub(crate) fn report_to(&mut self, reporter: Reporter) {
        self.reporter = Some(reporter);
    }

    /// 有没有在跑的回合：交给 `Handle` 的那一份。
    pub(crate) fn busy(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.busy)
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
                Some(back) = self.back.recv() => match self.back(back) {
                    Some(input) => input,
                    None => continue,
                },
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
            Message::Environment(environment) => {
                self.tools.locate(environment.offset);
                Mail::Input(Input::Environment(environment))
            }
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
        // 交进去的后台命令结束都落了盘，才从任务表里拿掉：核心看表空了才空闲退出（施工 7-3）。
        self.jobs.land();
        self.busy.store(!self.session.idle(), Ordering::Release);
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
                note(&transient);
                self.push(Pushed::Transient(transient));
                None
            }
            Action::RunTurnStartHooks { turn } => Some(Input::TurnStartHooksDone {
                at: self.clock.now(),
                turn,
                injected: Vec::new(),
            }),
            Action::CallModel {
                seen,
                request,
                changed,
            } => {
                self.call(seen, request, changed);
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
            Action::GuardTool {
                call_id,
                name,
                args,
                cwd,
                dirs,
                permission,
            } => {
                // 判要碰磁盘（换真实的位置、造边界表）：在阻塞线程里判，不占跑异步任务的线程，慢盘上只让这个会话
                // 自己等（施工 4-9 再补四下：原来当场在这里判）。
                let guard = Arc::clone(&self.guard);
                let verdict =
                    blocking(move || guard.judge(&name, args, cwd, &dirs, &permission)).await;
                Some(Input::ToolGuarded {
                    at: self.clock.now(),
                    call_id,
                    verdict,
                })
            }
            Action::RunTool {
                call_id,
                name,
                args,
                cwd,
                dirs,
                permission,
                cause,
            } => {
                let at = self.clock.now();
                let jobs = self.jobs.port(call_id, cause);
                self.tools.run(
                    at,
                    Dispatch {
                        call_id,
                        name,
                        args,
                        cwd,
                        dirs,
                        permission,
                        jobs,
                    },
                )
            }
            // 叫它停（施工 4-9 再补一）：只举旗，工具交回来照常送回。
            Action::StopTool { call_id } => {
                self.tools.stop(call_id);
                None
            }
            Action::CancelTool { call_id } => {
                self.tools.cancel(call_id);
                None
            }
            // 改回文件（施工 4-7 上）：当场在阻塞线程里做完，结局排在收件箱里别的命令前面送回去。
            Action::Restore { steps } => {
                let files = self.tools.restore(steps).await;
                Some(Input::Restored {
                    at: self.clock.now(),
                    files,
                })
            }
            // 压完重读（施工 6-5）：当场在阻塞线程里读完、存成 blob，再做下一个动作（它后面紧跟着摘要请求）。
            Action::Reread { seen, paths, limit } => {
                let files = self.tools.reread(paths, limit).await;
                Some(Input::Reread {
                    at: self.clock.now(),
                    seen,
                    files,
                })
            }
            // 撤掉压缩时读回日志（施工 6-9）：当场在阻塞线程里读完再收收件箱；读不了的，会话停下。
            Action::ReadBack { from } => Some(self.read_back(from).await?),
            // 取回原文（施工 6-9）：当场照 blob 读完，排在收件箱里别的前面送回去。
            Action::Recall { blobs } => Some(Input::Recalled {
                texts: self.tools.recall(blobs).await,
            }),
            // 向上回报（施工 7-6）：交给交回报的那一头，不等。没有的（测试里自己造的子会话）交不出去，运行日志里的
            // `action` 那一行记着。
            Action::Report(upward) => {
                if let Some(reporter) = &self.reporter {
                    reporter.send(upward);
                }
                None
            }
            // 工具执行中问人随施工 4-9：这之前没有工具会问。
            Action::AnswerTool { .. } => {
                tracing::error!(target: TARGET, action = kind, "answer without a question");
                None
            }
        })
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

    /// 执行器送回来的，照 actor 的时钟记下到的时刻，写成内核的输入；已经不要了的工具回报，不理。
    fn back(&mut self, back: Back) -> Option<Input> {
        let at = self.clock.now();
        Some(match back {
            Back::Woke { seen } => Input::Woke { at, seen },
            Back::Tool(back) => return self.tools.back(at, back),
            Back::Job(ended) => self.jobs.arrived(at, ended),
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
                    excess,
                } => {
                    self.ended(seen, usage.as_ref(), error.as_ref());
                    Input::ModelEnded {
                        at,
                        seen,
                        usage,
                        error,
                        wait_ms,
                        excess,
                    }
                }
            },
        })
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

#[cfg(test)]
mod tests;
