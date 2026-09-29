//! 替身交任务的回报（施工 7-2，`docs/blueprint/kernel/session.md`「回报」）：子会话交来的回报（命令 `Report`，发命令的
//! 是那个子会话），执行器交来的后台命令结束（输入 `JobEnded`），会话 actor 交的有没有头订阅着（输入 `Watched`）。派任务
//! 照剧本回：[`super::Play::starts_command`]、[`super::Play::starts_agent`]。

use super::Stage;
use crate::event::{ChildReason, ChildReported, JobReason, JobReported};
use crate::id::{CommandId, ContentHash, JobId, SessionId};
use crate::origin::{By, Session};
use crate::session::{Command, Input};

impl Stage {
    /// 子会话 `session` 交来子代理 `j<job>` 的回报：原因 `reason`，正文 `text`，截没截过、人插没插过话照写。返回这个命令的
    /// 编号。
    ///
    /// # Panics
    ///
    /// `job` 是 0，或者 `session` 不是会话编号的写法。
    pub fn child_reports(
        &mut self,
        job: u64,
        session: &str,
        reason: ChildReason,
        text: &str,
    ) -> CommandId {
        self.child_reports_with(job, session, reason, text, false, false)
    }

    /// 同上，写明截过（`truncated`）、人插过话（`person`）。
    ///
    /// # Panics
    ///
    /// 同上。
    pub fn child_reports_with(
        &mut self,
        job: u64,
        session: &str,
        reason: ChildReason,
        text: &str,
        truncated: bool,
        person: bool,
    ) -> CommandId {
        let session =
            SessionId::parse(session).unwrap_or_else(|e| panic!("会话编号的写法坏了：{e}"));
        let by = By::Session(Session {
            id: session.clone(),
        });
        let reported = ChildReported {
            job: job_id(job),
            session,
            reason,
            text: text.to_string(),
            truncated,
            person,
        };
        self.command_as(by, Command::Report(reported))
    }

    /// 执行器交来后台命令 `j<job>` 结束了，`by`、`cause` 照给的；自己退出的带退出码 0、用时和一份输出。
    ///
    /// # Panics
    ///
    /// `job` 是 0。
    pub fn job_ends(&mut self, job: u64, reason: JobReason, by: By, cause: Option<CommandId>) {
        let exited = reason == JobReason::Exited;
        let reported = JobReported {
            job: job_id(job),
            reason,
            exit_code: exited.then_some(0),
            signal: None,
            by_model: false,
            duration_ms: Some(81_234),
            output: exited.then(|| ContentHash::of(b"output")),
            chars: exited.then_some(48_213),
        };
        self.job_ends_with(by, cause, reported);
    }

    /// 执行器交来后台命令结束，`body` 照给的原样（施工 7-2）。
    pub fn job_ends_with(&mut self, by: By, cause: Option<CommandId>, reported: JobReported) {
        let at = self.tick();
        self.run(Input::JobEnded {
            at,
            by,
            cause,
            reported,
        });
    }

    /// 会话 actor 交来：有没有头订阅着（施工 7-2）。
    pub fn watched(&mut self, watched: bool) {
        self.run(Input::Watched { watched });
    }
}

/// `j<n>`。
fn job_id(n: u64) -> JobId {
    JobId::new(n).unwrap_or_else(|| panic!("任务编号从 1 数起"))
}
