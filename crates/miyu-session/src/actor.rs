//! actor 本身（`docs/designs/02-内核.md` 第七节「会话 actor 怎么跑」）：一个会话一个异步任务。
//!
//! 人的命令、执行器的回报进收件箱，一条一条送进内核，内核交出的动作照 02 第四节「执行器怎么回动作」
//! 的表回。当场就能回的（落盘了、挂接点跑完了）放进本地的队列，先于收件箱里的送：和执行器替身
//! （`miyu-kernel` 的 `testkit`）一个先后。

use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use tokio::sync::{broadcast, mpsc, oneshot};
use tracing::Instrument;

use miyu_kernel::event::Purpose;
use miyu_kernel::id::{CommandId, Seq, SessionId};
use miyu_kernel::session::{Action, Input, Limits, Outcome, Session};
use miyu_kernel::time::Timestamp;

use crate::TARGET;
use crate::agents::Inherit;
use crate::blocking::blocking;
use crate::clock::Clock;
use crate::config::Turning;
use crate::guard::Guard;
use crate::handle::{Message, Pushed};
use crate::jobs::SessionJobs;
use crate::kinds;
use crate::lines::note;
use crate::peers::Watches;
use crate::port::{Back, ModelPort};
use crate::report::Reporter;
use crate::shown::{Next, Shown};
use crate::store::Store;
use crate::tools::{Dispatch, ToolKit, Tools};

mod back;
mod extract;
mod halt;
mod listing;
mod mail;
mod model;
pub(crate) mod persona;
mod stop;
mod store;
mod watchers;

use back::answer_back;
use mail::Mail;

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
    /// 还没说完的辅助请求（施工 3-8 四补的回顾、五补的起标题，`model.rs`）：用途、照到第几条、叫停它的那一头（拿着不用：
    /// actor 停了放下它，请求跟着停）、交给端口的那一刻。一种用途至多一个。
    asides: Vec<(Purpose, Seq, oneshot::Sender<()>, Instant)>,
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
    /// 配置（施工 8-4）：从哪取，和这一轮的那一份；回合开始时换，这一轮的请求都照它。
    config: Turning,
    /// 拿着订阅的头有几个（施工 7-9）：从没有到有、从有到没有时交内核 `Watched`。造会话、载入时是 0，和内核一样当没人
    /// 看着。
    watchers: usize,
    /// 这时有没有至少一个头订阅着，和 `Handle` 共用（施工 C-5，照 `busy` 的做法）：`watchers` 从 0 到有、从有到 0 时写一次。
    /// `send_message` 发给这个会话的时候，别的会话照它和这个会话是不是一次性的，决定说 `sent` 还是 `held`。
    watched: Arc<AtomicBool>,
    /// 「空了告诉我」等的这一边订过的（施工 C-6，`crate::peers`）。
    watches: Watches,
    /// 「空了告诉我」被等的这一边：上一批送完时这个会话还忙不忙（施工 C-6，2026-10-01 改，`watchers.rs`）。忙起来时
    /// 名单上的都上膛；记着忙过、现在闲着的那一刻清掉，顺手记下 `finished_at`。
    busy_seen: bool,
    /// 「空了告诉我」被等的这一边：最近一次从忙变空的时刻（施工 C-6，2026-10-01 改，`watchers.rs`）。订的起算时刻
    /// 不晚于它的，当场上膛：带话又订、这边手快先忙完了一轮的情形。
    finished_at: Option<Timestamp>,
    /// 「空了告诉我」被等的这一边：谁在等这个会话空下来（施工 C-6，`watchers.rs`）。
    waiters: watchers::Waiters,
    /// 这一批推过的、要报会话表的（施工 9-5、9-8 补下修，`listing.rs`）。
    marks: listing::Marks,
    /// 上一次交给内核的限额（施工 8-9，`model.rs`）：请求说完了、回合开始重新解析完和端口的比，变了再交。
    handed: Limits,
    /// 给头看的限额和会话接下来请求的模型，和 `Handle` 共用（施工 8-9、8-10）：交了新的限额、回合开始解析完写一次。
    shown: Arc<Mutex<Shown>>,
    /// 人格的文件改了，下一个回合换上（施工 P-1 再补，`persona.rs`）：造会话、载入时交，测试里造的没有。
    persona: Option<persona::Refresh>,
}

/// 会话停了：写不进去。
pub(crate) struct Stop;

/// 任务表里这个会话的那一份要的（施工 7-3；施工 7-4 挪到 `jobs.rs`，多了名册和会话表的端口）。
pub(crate) use crate::jobs::Kit as JobKit;

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

impl Actor {
    /// 一个 actor：会话的状态机、写盘的地方、请求模型的端口、工具目录和替工具写的两句、任务表、权限策略、收件箱、时钟、
    /// 配置。
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
        config: Turning,
    ) -> Actor {
        let (backs, back) = mpsc::unbounded_channel();
        let tools = Tools::new(tools, backs.clone());
        let jobs = SessionJobs::new(jobs, backs.clone());
        let (pushes, _) = broadcast::channel(PUSH_QUEUE);
        let busy = Arc::new(AtomicBool::new(!session.idle()));
        let busy_seen = !session.vacant();
        // 造会话、载入时已经把端口的限额交给了内核（`open.rs`）：记下交的是哪一份。
        let handed = model.limits();
        let shown = Arc::new(Mutex::new(Shown::new(
            session.context_limits(),
            Next::of(&*model),
            session.todos(),
        )));
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
            asides: Vec::new(),
            clock,
            tools,
            jobs,
            guard: Arc::new(guard),
            busy,
            reporter: None,
            config,
            watchers: 0,
            watched: Arc::new(AtomicBool::new(false)),
            watches: Watches::default(),
            busy_seen,
            finished_at: None,
            waiters: watchers::Waiters::new(),
            marks: listing::Marks::default(),
            handed,
            shown,
            persona: None,
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

    /// 给头看的限额和模型：交给 `Handle` 的那一份（施工 8-9、8-10）。
    pub(crate) fn shown(&self) -> Arc<Mutex<Shown>> {
        Arc::clone(&self.shown)
    }

    /// 这时有没有至少一个头订阅着：交给 `Handle` 的那一份（施工 C-5）。
    pub(crate) fn watched(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.watched)
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
                    Some(Mail::Halt(halt)) => match self.halt(halt).await {
                        Ok(()) => continue,
                        Err(Stop) => return,
                    },
                    Some(Mail::Delete(force, reply)) => match self.delete(force, reply).await {
                        true => return,
                        false => continue,
                    },
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
        // 「空了告诉我」（施工 C-6）：先订、先把通知交出去，再写没有在跑的回合。
        self.after_batch();
        self.settle_busy();
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
                self.show_todos(&transient);
                self.push(Pushed::Transient(transient));
                None
            }
            // 回合开始（`turn.started` 已经落了盘）：冻结这一轮的配置，重新解析会话的引用，问挂接点（施工 8-4、8-10、R-4 上，`model.rs`）。
            Action::RunTurnStartHooks {
                turn,
                model,
                present,
                said,
            } => Some(self.turn_start(turn, model, present, said).await),
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
            Action::Aside {
                purpose,
                upto,
                request,
            } => {
                self.aside(purpose, upto, request);
                None
            }
            Action::CancelModel { seen } => {
                self.cancel(seen);
                None
            }
            // 替看不了图的模型看图（施工 8-17，`model.rs`）：交给端口，结果另走一路送回。
            Action::Describe { blob, request } => {
                self.describe(blob, request);
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
                grants,
            } => {
                // 判要碰磁盘（换真实的位置、造边界表）：在阻塞线程里判，不占跑异步任务的线程，慢盘上只让这个会话
                // 自己等（施工 4-9 再补四下：原来当场在这里判）。
                let guard = Arc::clone(&self.guard);
                let verdict =
                    blocking(move || guard.judge(&name, args, cwd, &dirs, &permission, &grants))
                        .await;
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
                asked,
            } => {
                let at = self.clock.now();
                // 后台运行关着的会话不给任务端口（施工 T-1 上）：`shell` 要放到后台的照「这里不能放到后台」说。
                let foreground = self
                    .persona
                    .as_ref()
                    .is_some_and(|refresh| refresh.snapshot.foreground);
                let jobs = (!foreground).then(|| self.jobs.port(call_id, cause));
                let usage = crate::usage::asked(&name, &self.session, self.config.current());
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
                        subagents: self.session.subagents(),
                        inherit: Inherit::of(&*self.model),
                        usage,
                        turn: self.session.turn_cause().map(|(turn, _)| turn),
                        asked,
                        config: Arc::clone(self.config.current()),
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
            // 停掉撤掉的那几轮派出去的（施工 7-8，`halt.rs`）：不送回，回报照停好了的样子另外交来。
            Action::StopJobs {
                jobs,
                by,
                cause,
                undone,
            } => {
                self.stop_jobs(jobs, by, cause, undone).await;
                None
            }
            // 向上回报（施工 7-6）：交给交回报的那一头，不等。没有的（测试里自己造的子会话）交不出去，运行日志里的
            // `action` 那一行记着。
            Action::Report(upward) => {
                if let Some(reporter) = &self.reporter {
                    reporter.send(upward);
                }
                None
            }
            // 回答落了盘，送给在等的工具（施工 D-2，`tools/questions.rs`）。
            Action::AnswerTool { call_id, answers } => {
                self.tools.answer(call_id, answers);
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
}

/// 交回一声。等的那一头不等了，就没人收。
#[expect(
    clippy::let_underscore_must_use,
    reason = "等的那一头不等了：回的话没人要，丢掉"
)]
fn answer<T>(reply: oneshot::Sender<T>, value: T) {
    let _ = reply.send(value);
}

#[cfg(test)]
mod tests;
