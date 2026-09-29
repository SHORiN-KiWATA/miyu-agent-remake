//! 停掉一个任务（施工 7-4，`docs/blueprint/session/tools.md`「查和停」，`agents.md` 第五条）：她用 `jobs` 停、人用 `job.stop`
//! 停、父会话被停下时连它派的一起停，都走这里。
//!
//! - 后台命令：还在表里、还没人报过结束的，拿着表的锁记成报了（之后它自己退出了也不再报：只认先到的那一个），整组杀掉，
//!   关上输出、存成 blob，写好 `job.reported`（`stopped`）；
//! - 子代理：经会话表停下它的会话（打断它这一轮，再停掉它派的），再把 `child.reported`（`stopped`）作为它交来的回报送进
//!   这个会话：`by` 是子会话，账本照旧查得上。

use std::time::Instant;

use miyu_kernel::event::{ChildReason, ChildReported, JobReason, JobReported};
use miyu_kernel::id::{CommandId, JobId, SessionId};
use miyu_kernel::origin::{By, Session};
use miyu_kernel::session::{Command, Outcome, Reason};
use miyu_tool::JobError;

use super::{Ended, Shared};
use crate::TARGET;
use crate::agents::{Agents, command_id};
use crate::lines::millis;

/// 谁停的：记进回报的几样（`kernel/session.md`「回报」第 2 条）。
#[derive(Debug, Clone)]
pub(crate) struct Who {
    /// 后台命令那条 `job.reported` 的 `by`：她那次 `jobs` 调用、停它的人，连着这个会话一起停的是父会话。
    pub(crate) by: By,
    /// 那条的 `cause`：那次调用所在那一轮的、停它的命令。
    pub(crate) cause: Option<CommandId>,
    /// 不是人停的：她自己用 `jobs` 停的，或者这个会话被父会话停下、连它派的一起停的。只记下，不叫醒。
    pub(crate) by_model: bool,
}

impl Shared {
    /// 停掉这个会话的后台命令 `job`：还在表里、还没人报过的，记成报了，整组杀掉，关上输出、存成 blob，交回 `stopped` 的
    /// 回报（到这时的用时、输出，没有退出码、信号：杀的时候还没等到）。已经报过、不在表里的（自己退出了、停下时报了
    /// `restarted`）交回空的。杀进程、碰磁盘，在阻塞线程里调。
    pub(super) fn stop_command(&self, job: JobId, who: &Who) -> Option<Ended> {
        let key = (self.owner, job.clone());
        let (process, output, started) = {
            let mut table = self.table.lock();
            let entry = table.get_mut(&key).filter(|entry| !entry.reported)?;
            entry.reported = true;
            (
                std::sync::Arc::clone(&entry.process),
                std::sync::Arc::clone(&entry.output),
                entry.started,
            )
        };
        process.kill();
        output.close();
        let (output, chars) = output.stored(&self.blobs);
        tracing::info!(target: TARGET, job = job.to_string().as_str(), "job stopped");
        Some(Ended {
            key,
            by: who.by.clone(),
            cause: who.cause.clone(),
            reported: JobReported {
                job,
                reason: JobReason::Stopped,
                exit_code: None,
                signal: None,
                by_model: who.by_model,
                duration_ms: Some(millis(Instant::now().duration_since(started))),
                output,
                chars,
            },
        })
    }
}

/// 停掉这个会话派的子代理 `job`（子会话 `child`）：经会话表停下子会话（打断它这一轮，连它派的一起停，记成这个会话发的，
/// 编号 `<这个会话>/<编号>/stop`），再把 `stopped` 的回报作为子会话交来的送进这个会话（编号 `<这个会话>/<编号>/stopped`）。
/// 正文是它这一轮最后的回答，照快照里的 `jobs.report_chars` 截头尾，和子会话自己向上回报的截法是同一份（`Reports::cut`）。
/// 停不下来的（子会话载入不了）记一行照样报：它不会再干活了。
///
/// # Errors
///
/// 回报没收下：已经有人停了它、撤销停了它（`unknown_job`），或者这个会话停了。
pub(super) async fn stop_agent(
    agents: &Agents,
    job: JobId,
    child: SessionId,
    by_model: bool,
) -> Result<(), JobError> {
    let parent = &agents.session;
    let by = By::Session(Session { id: parent.clone() });
    let job_text = job.to_string();
    if let Err(error) = agents
        .port
        .stop(child.clone(), command_id(parent, &job, "/stop"), by)
        .await
    {
        tracing::warn!(target: TARGET, job = job_text.as_str(), child = child.as_str(), error = error.as_str(), "subagent not stopped");
    }
    let (text, truncated) = match agents.port.peek(child.clone()).await {
        Ok(peek) if peek.this_turn => agents.reports.cut(&peek.reply.unwrap_or_default()),
        _ => (String::new(), false),
    };
    let report = Command::Report(ChildReported {
        job: job.clone(),
        session: child.clone(),
        reason: ChildReason::Stopped,
        text,
        truncated,
        person: false,
        by_model,
    });
    let from_child = By::Session(Session { id: child.clone() });
    let id = command_id(parent, &job, "/stopped");
    match agents
        .port
        .command(parent.clone(), id, from_child, report)
        .await
    {
        Ok(Outcome::Accepted { .. }) => {
            tracing::info!(target: TARGET, job = job_text.as_str(), child = child.as_str(), "subagent stopped");
            Ok(())
        }
        Ok(Outcome::Rejected {
            reason: Reason::UnknownJob,
        }) => Err(JobError::Ended),
        Ok(Outcome::Rejected { reason }) => {
            tracing::warn!(target: TARGET, job = job_text.as_str(), reason = reason.code(), "subagent stop not recorded");
            Err(JobError::Ended)
        }
        Err(error) => {
            tracing::warn!(target: TARGET, job = job_text.as_str(), error = error.as_str(), "subagent stop not recorded");
            Err(JobError::Ended)
        }
    }
}
