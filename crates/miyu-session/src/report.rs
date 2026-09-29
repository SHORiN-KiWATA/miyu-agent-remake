//! 向上回报，执行器这一头（施工 7-6，`docs/blueprint/agents.md` 第二条第 5 条，`session/actor.md`「向上回报」）：内核交出的
//! 回报（[`Upward`]）补上任务编号、子会话，经会话表的端口（[`SessionPort`]）交给父会话，命令 `Report`，发命令的是这个
//! 子会话。
//!
//! 一个子会话一份 [`Reporter`]，造会话、载入时定：父会话、它在父会话里的任务编号（照 `session.created` 的 `cause`
//! `<父会话>/<编号>` 读回，[`crate::agents::job_in`]）。回报照先后一个一个交，不挡着 actor：父会话落了盘才回应，父会话
//! 正忙、正在载入都要等。命令编号照回报的那一轮定（`<子会话>/report/<回合>`）：载入时再交同一份，父会话认得出是重的。
//!
//! 父会话说对不上任务（`unknown_job`）的，退避着再交同一份：子代理做得快，回报可能赶在父会话记下派它的那次调用之前，
//! 那一条一落盘就对得上了（`agents.md` 第二条第 5 条）。等满了还对不上的，和别的拒绝、交不到的（父会话没了、核心正在停）
//! 一样丢掉，记一行运行日志。

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tracing::Instrument;

use miyu_kernel::event::ChildReported;
use miyu_kernel::id::{CommandId, JobId, SessionId};
use miyu_kernel::origin::{By, Session};
use miyu_kernel::session::{Command, Outcome, Reason, Upward};

use crate::TARGET;
use crate::agents::job_in;
use crate::spawn::SessionPort;

/// 父会话说对不上任务时，第一次等多久再交，之后每次翻倍（毫秒）。
const RETRY_FIRST_MS: u64 = 100;

/// 父会话说对不上任务时，一共最多等多久（毫秒）：派它的那次调用早该落盘了，还对不上的是真对不上。
const RETRY_TOTAL_MS: u64 = 30_000;

/// 往哪儿报：父会话、这个子会话在它那里的任务编号、这个子会话，和会话表的端口。
pub(crate) struct Upstream {
    /// 会话表交进来的端口。
    pub(crate) port: Arc<dyn SessionPort>,
    /// 父会话。
    pub(crate) parent: SessionId,
    /// 这个子会话在父会话里的任务编号。
    pub(crate) job: JobId,
    /// 这个子会话。
    pub(crate) session: SessionId,
}

/// 交回报的那一头：交进来的照先后由一个任务一个一个交给父会话。actor 退出放下它以后，已经交进来的照样交完。
pub(crate) struct Reporter(mpsc::UnboundedSender<Upward>);

impl Reporter {
    /// 起交回报的任务，带着会话的 span。
    pub(crate) fn start(upstream: Upstream, span: tracing::Span) -> Reporter {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        tokio::spawn(
            async move {
                while let Some(upward) = receiver.recv().await {
                    upstream.hand(upward).await;
                }
            }
            .instrument(span),
        );
        Reporter(sender)
    }

    /// 交一份回报，不等。
    #[expect(
        clippy::let_underscore_must_use,
        reason = "交回报的任务只在运行时关掉时才没了：那时也交不出去，丢掉"
    )]
    pub(crate) fn send(&self, upward: Upward) {
        let _ = self.0.send(upward);
    }
}

impl Upstream {
    /// 子会话往哪儿报：有父会话、有会话表的端口、造它的命令编号读得出任务编号才有。有父会话、读不出任务编号的，记一行
    /// `WARN`：它的回报交不出去。
    pub(crate) fn of(
        port: Option<&Arc<dyn SessionPort>>,
        parent: Option<&SessionId>,
        command: Option<&CommandId>,
        session: &SessionId,
    ) -> Option<Upstream> {
        let (port, parent) = (port?, parent?);
        let Some(job) = command.and_then(|command| job_in(parent, command)) else {
            tracing::warn!(target: TARGET, parent = parent.as_str(), "subagent without a job id");
            return None;
        };
        Some(Upstream {
            port: Arc::clone(port),
            parent: parent.clone(),
            job,
            session: session.clone(),
        })
    }

    /// 交给父会话，等它回应，记一行运行日志。
    async fn hand(&self, upward: Upward) {
        let id = format!("{}/report/{}", self.session, upward.turn);
        let job = self.job.to_string();
        let Ok(id) = CommandId::parse(&id) else {
            tracing::warn!(target: TARGET, job = job.as_str(), "report has no command id");
            return;
        };
        let by = By::Session(Session {
            id: self.session.clone(),
        });
        let reported = ChildReported {
            job: self.job,
            session: self.session.clone(),
            reason: upward.reason,
            text: upward.text,
            truncated: upward.truncated,
            person: upward.person,
        };
        let parent = self.parent.as_str();
        let (mut wait, mut waited) = (RETRY_FIRST_MS, 0);
        let handed = loop {
            let handed = self
                .port
                .command(
                    self.parent.clone(),
                    id.clone(),
                    by.clone(),
                    Command::Report(reported.clone()),
                )
                .await;
            let unknown = matches!(
                handed,
                Ok(Outcome::Rejected {
                    reason: Reason::UnknownJob
                })
            );
            if !unknown || waited >= RETRY_TOTAL_MS {
                break handed;
            }
            let now = wait.min(RETRY_TOTAL_MS - waited);
            tokio::time::sleep(Duration::from_millis(now)).await;
            waited += now;
            wait *= 2;
        };
        match handed {
            Ok(Outcome::Accepted { .. }) => {
                tracing::info!(target: TARGET, job = job.as_str(), parent, "reported");
            }
            Ok(Outcome::Rejected { reason }) => {
                tracing::warn!(target: TARGET, job = job.as_str(), parent, reason = reason.code(), "report refused");
            }
            Err(error) => {
                tracing::warn!(target: TARGET, job = job.as_str(), parent, error = error.as_str(), "report not delivered");
            }
        }
    }
}

/// 父会话载入以后叫起还没回报的子会话（施工 7-6，`agents.md` 第八条）：崩了的由它们自己补报、重启打断的接着干。一个一个
/// 起任务叫，不等：会话表正在载入父会话、拿着表的锁，等它载入完才轮得到。叫不起来的记一行 `WARN`。
pub(crate) fn wake_children(
    port: &Arc<dyn SessionPort>,
    children: Vec<SessionId>,
    span: &tracing::Span,
) {
    for child in children {
        let port = Arc::clone(port);
        tokio::spawn(
            async move {
                if let Err(error) = port.open(child.clone()).await {
                    tracing::warn!(target: TARGET, child = child.as_str(), error = error.as_str(), "subagent not woken");
                }
            }
            .instrument(span.clone()),
        );
    }
}
