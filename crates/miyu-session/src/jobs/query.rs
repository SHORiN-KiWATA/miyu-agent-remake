//! 查和停（施工 7-4，`docs/blueprint/session/tools.md`「查和停」）：任务端口的 `list`、`output`、`stop`，和 actor 替人停、替
//! 父会话停下时用的几样。都照 [`super::Roster`] 查是什么、结束了没有。

use std::fs::File;
use std::future::Future;
use std::io::{Cursor, Read};
use std::sync::Arc;

use miyu_kernel::event::JobKind;
use miyu_kernel::id::{JobId, SessionId};
use miyu_store::jobs::output_path;
use miyu_tool::{JobError, Output};

use super::stop::{Who, Why, stop_agent, stopped_report};
use super::{Ended, SessionJobs, Shared};
use crate::TARGET;
use crate::blocking::blocking;
use crate::port::Back;

/// 要停的是什么。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Target {
    /// 后台命令。
    Command,
    /// 子代理：它的会话。
    Agent(SessionId),
}

impl Shared {
    /// 能停的 `job` 是什么：这个会话没派过的（不认识的种类也算）是 [`JobError::Unknown`]，已经有回报的是
    /// [`JobError::Ended`]。子代理报了 `done` 也算结束了：它那一轮完了（父子留言叫醒它随 7-7）。
    pub(super) fn target(&self, job: &JobId) -> Result<Target, JobError> {
        let roster = self.roster();
        let record = roster.get(job).ok_or(JobError::Unknown)?;
        if record.end.is_some() {
            return Err(JobError::Ended);
        }
        match (&record.what, &record.session) {
            (JobKind::Command, _) => Ok(Target::Command),
            (JobKind::Agent, Some(session)) => Ok(Target::Agent(session.clone())),
            _ => Err(JobError::Unknown),
        }
    }

    /// 停一个：后台命令在阻塞线程里杀、存，报给 actor；子代理经会话表停。她用 `jobs` 停的走这里。
    pub(super) async fn stop(self: &Arc<Shared>, job: JobId, who: Who) -> Result<(), JobError> {
        match self.target(&job)? {
            Target::Command => {
                let shared = Arc::clone(self);
                let ended = blocking(move || shared.stop_command(job, &who))
                    .await
                    .ok_or(JobError::Ended)?;
                self.back(ended);
                Ok(())
            }
            Target::Agent(child) => match &self.agents {
                Some(agents) => stop_agent(agents, job, child, who.why).await,
                None => Err(JobError::Unknown),
            },
        }
    }

    /// 读 `job` 的输出：后台命令结束了、输出存成了 blob 的读 blob，别的读会话目录下的输出文件（跑着的读到这时的，载入时补
    /// `aborted` 的读到崩的那一刻的）；子代理经会话表看它的日志。
    pub(super) async fn output(&self, job: JobId) -> Result<Output, JobError> {
        let (what, session, stored, running) = {
            let roster = self.roster();
            let record = roster.get(&job).ok_or(JobError::Unknown)?;
            (
                record.what.clone(),
                record.session.clone(),
                record.end.as_ref().and_then(|end| end.output.clone()),
                record.end.is_none(),
            )
        };
        match (&what, session) {
            (JobKind::Command, _) => {
                let path = match stored {
                    Some(hash) => self.blobs.path(&hash),
                    None => output_path(&self.dir, &job),
                };
                let text = blocking(move || File::open(path).ok()).await;
                Ok(Output {
                    what,
                    text: text.map(|file| Box::new(file) as Box<dyn Read + Send>),
                    running,
                    doing: Vec::new(),
                })
            }
            (JobKind::Agent, Some(child)) => {
                let agents = self.agents.as_ref().ok_or(JobError::Unknown)?;
                let peek = agents.port.peek(child).await.unwrap_or_else(|error| {
                    tracing::warn!(target: TARGET, job = job.to_string().as_str(), error = error.as_str(), "subagent not read");
                    super::Peek::default()
                });
                Ok(Output {
                    what,
                    text: peek
                        .reply
                        .map(|reply| Box::new(Cursor::new(reply)) as Box<dyn Read + Send>),
                    running: peek.working,
                    doing: peek.doing,
                })
            }
            _ => Err(JobError::Unknown),
        }
    }

    /// 停好的后台命令交给 actor 记下。actor 已经停了的送不进去：没人会落它的盘了，从表里拿掉。
    fn back(&self, ended: Ended) {
        let key = ended.key.clone();
        if self.backs.send(Back::Job(ended)).is_err() {
            self.table.lock().remove(&key);
        }
    }
}

impl SessionJobs {
    /// 能停的 `job` 是什么（人用 `job.stop` 停的）。
    pub(crate) fn target(&self, job: &JobId) -> Result<Target, JobError> {
        self.shared.target(job)
    }

    /// 还在跑的，照编号（父会话停下它时，连它派的一起停）。
    pub(crate) fn running(&self) -> Vec<(JobId, Target)> {
        self.shared
            .roster()
            .running()
            .into_iter()
            .filter_map(|(job, record)| match (record.what, record.session) {
                (JobKind::Command, _) => Some((job, Target::Command)),
                (JobKind::Agent, Some(session)) => Some((job, Target::Agent(session))),
                _ => None,
            })
            .collect()
    }

    /// 停掉后台命令 `job`，交回回报，由 actor 当场交进内核：人停的，回应排在它落盘之后。已经结束了的交回空的。
    pub(crate) async fn stop_command(&self, job: JobId, who: Who) -> Option<Ended> {
        let shared = Arc::clone(&self.shared);
        blocking(move || shared.stop_command(job, &who)).await
    }

    /// 子代理 `job`（子会话 `child`）已经停下了（人删了它，施工 7-8）：作为它交来的那一份回报（命令编号、`by`、命令），由
    /// actor 当场交进内核。没有会话表的端口的，没有。
    pub(crate) async fn stopped_report(
        &self,
        job: JobId,
        child: &SessionId,
        why: Why,
    ) -> Option<(
        miyu_kernel::id::CommandId,
        miyu_kernel::origin::By,
        miyu_kernel::session::Command,
    )> {
        let agents = self.shared.agents.clone()?;
        Some(stopped_report(&agents, job, child, why).await)
    }

    /// 停掉子代理 `job`（子会话 `child`）的 future：拿着自己要的，actor 另起一个任务跑它，不在收件箱里等。没有会话表的
    /// 端口的，交回 [`JobError::Unknown`]。
    pub(crate) fn stop_agent(
        &self,
        job: JobId,
        child: SessionId,
        why: Why,
    ) -> impl Future<Output = Result<(), JobError>> + Send + 'static {
        let agents = self.shared.agents.clone();
        async move {
            match agents {
                Some(agents) => stop_agent(&agents, job, child, why).await,
                None => Err(JobError::Unknown),
            }
        }
    }
}
