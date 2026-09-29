//! 停掉派出去的任务（施工 7-4，`docs/blueprint/session/actor.md`「停掉任务」，`agents.md` 第五条）：人用 `job.stop` 停一个，
//! 父会话停下这个会话时连它派的全停。她自己用 `jobs` 停的不经这里，走任务端口（`jobs/query.rs`）。
//!
//! 后台命令当场在阻塞线程里杀、存，回报当场交进内核：人停的，回应排在回报落盘之后（先见结果，后见回应）。子代理要经会话
//! 表停它的会话、再把回报送回这个会话，另起一个任务跑：在收件箱里等，回报就送不进来了。

use std::collections::VecDeque;

use miyu_kernel::id::JobId;
use miyu_tool::JobError;

use super::{Actor, Stop, answer};
use crate::handle::Halt;
use crate::jobs::{Target, Who};

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
                    by_model: false,
                };
                match self.jobs.target(job) {
                    Err(error) => answer(reply, Err(error)),
                    Ok(Target::Command) => {
                        let result = self.stop_command(job, who).await?;
                        answer(reply, result);
                    }
                    Ok(Target::Agent(child)) => {
                        let stopping = self.jobs.stop_agent(job, child, false);
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
                        by_model: true,
                    };
                    match target {
                        // 已经自己结束了的不要紧：只认先到的那一个。
                        Target::Command => match self.stop_command(job, who).await? {
                            Ok(()) | Err(_) => {}
                        },
                        Target::Agent(child) => agents.push(self.jobs.stop_agent(job, child, true)),
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
        }
        Ok(())
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
