//! 停掉派出去的任务（施工 7-4，`docs/blueprint/session/actor.md`「停掉任务」，`agents.md` 第五条）：人用 `job.stop` 停一个，
//! 父会话停下这个会话时连它派的全停。她自己用 `jobs` 停的不经这里，走任务端口（`jobs/query.rs`）。撤销停掉那几轮派出去的
//! 也在这里（施工 7-8，`agents.md` 第七条第 1 条）：回报记 `undone`，撤销的回应不等它们。
//!
//! 后台命令当场在阻塞线程里杀、存，回报当场交进内核：人停的，回应排在回报落盘之后（先见结果，后见回应）。子代理要经会话
//! 表停它的会话、再把回报送回这个会话，另起一个任务跑：在收件箱里等，回报就送不进来了。

use std::collections::VecDeque;

use tokio::sync::oneshot;

use miyu_kernel::id::{CommandId, JobId, SessionId};
use miyu_kernel::origin::By;
use miyu_kernel::session::{Input, Outcome, Received};
use miyu_tool::JobError;

use super::{Actor, Stop, answer, answer_back};
use crate::handle::Halt;
use crate::jobs::{Target, Who, Why};
use crate::port::Back;

impl Actor {
    /// 办一件停的事。写不进去的，会话停下。
    pub(super) async fn halt(&mut self, halt: Halt) -> Result<(), Stop> {
        match halt {
            Halt::One {
                job,
                by,
                cause,
                reply,
            } => {
                let who = Who {
                    by,
                    cause: Some(cause),
                    why: Why::Stopped { by_model: false },
                };
                match self.jobs.target(&job) {
                    Err(error) => answer(reply, Err(error)),
                    Ok(Target::Command) => {
                        let result = self.stop_command(job, who).await?;
                        answer(reply, result);
                    }
                    Ok(Target::Agent(child)) => {
                        let stopping =
                            self.jobs
                                .stop_agent(job, child, Why::Stopped { by_model: false });
                        tokio::spawn(async move { answer(reply, stopping.await) });
                    }
                }
            }
            Halt::All { by, cause, reply } => {
                let mut agents = Vec::new();
                for (job, target) in self.jobs.running() {
                    let who = Who {
                        by: by.clone(),
                        cause: Some(cause.clone()),
                        why: Why::Stopped { by_model: true },
                    };
                    match target {
                        // 已经自己结束了的不要紧：只认先到的那一个。
                        Target::Command => match self.stop_command(job, who).await? {
                            Ok(()) | Err(_) => {}
                        },
                        Target::Agent(child) => agents.push(self.jobs.stop_agent(
                            job,
                            child,
                            Why::Stopped { by_model: true },
                        )),
                    }
                }
                tokio::spawn(async move {
                    for stopping in agents {
                        // 已经有人停了它的，不要紧。
                        match stopping.await {
                            Ok(()) | Err(_) => {}
                        }
                    }
                    answer(reply, ());
                });
            }
            Halt::Deleted { job, reply } => {
                let result = match self.jobs.target(&job) {
                    Ok(Target::Agent(child)) => self.deleted(job, child).await?,
                    Ok(Target::Command) => Err(JobError::Unknown),
                    Err(error) => Err(error),
                };
                answer(reply, result);
            }
        }
        Ok(())
    }

    /// 人删了子代理 `job`（子会话 `child`），它已经停下了（施工 7-8）：照人停它的那一份回报当场交进内核，落了盘交回。没有
    /// 会话表的端口的交回没有这个任务；内核不收的（已经有人停了它、撤销停了它）交回已经结束了。写不进去的，会话停下。
    async fn deleted(
        &mut self,
        job: JobId,
        child: SessionId,
    ) -> Result<Result<(), JobError>, Stop> {
        let why = Why::Stopped { by_model: false };
        let Some((id, by, command)) = self.jobs.stopped_report(job, &child, why).await else {
            return Ok(Err(JobError::Unknown));
        };
        let (reply, mut outcome) = oneshot::channel();
        self.wait_for(id.clone(), reply);
        let at = self.clock.now();
        let input = Input::Command(Received {
            id,
            by,
            at,
            command,
        });
        self.drain(VecDeque::from([input])).await?;
        // 接受的落了盘才回应，拒绝的当场回应：都在这一批里回过了。
        Ok(match outcome.try_recv() {
            Ok(Outcome::Accepted { .. }) => Ok(()),
            _ => Err(JobError::Ended),
        })
    }

    /// 撤销停掉那几轮派出去、还在跑的（施工 7-8，动作 `StopJobs`）：后台命令当场在阻塞线程里整组杀掉、存好输出，回报
    /// （`undone`，`by`、`cause` 是撤销的人和命令）经收件箱交回，排在这一批后面：撤销的回应不等它落盘，可回应之前命令已经
    /// 杀了。子代理另起一个任务经会话表停，连它派的一起停，回报由子会话交来。已经结束了的（正好自己退出了、已经报过）不要紧。
    pub(super) async fn undo_jobs(&mut self, jobs: Vec<JobId>, by: By, cause: CommandId) {
        let mut agents = Vec::new();
        for job in jobs {
            match self.jobs.target(&job) {
                Ok(Target::Command) => {
                    let who = Who {
                        by: by.clone(),
                        cause: Some(cause.clone()),
                        why: Why::Undone,
                    };
                    if let Some(ended) = self.jobs.stop_command(job, who).await {
                        answer_back(&self.backs, Back::Job(ended));
                    }
                }
                Ok(Target::Agent(child)) => {
                    agents.push(self.jobs.stop_agent(job, child, Why::Undone))
                }
                Err(_) => {}
            }
        }
        if !agents.is_empty() {
            tokio::spawn(async move {
                for stopping in agents {
                    // 已经有人停了它、它正好报完了的，不要紧。
                    match stopping.await {
                        Ok(()) | Err(_) => {}
                    }
                }
            });
        }
    }

    /// 停一条后台命令，回报当场交进内核、落了盘再交回。已经结束了的交回 [`JobError::Ended`]。
    async fn stop_command(&mut self, job: JobId, who: Who) -> Result<Result<(), JobError>, Stop> {
        let Some(ended) = self.jobs.stop_command(job, who).await else {
            return Ok(Err(JobError::Ended));
        };
        let at = self.clock.now();
        let input = self.jobs.arrived(at, ended);
        self.drain(VecDeque::from([input])).await?;
        Ok(Ok(()))
    }
}
