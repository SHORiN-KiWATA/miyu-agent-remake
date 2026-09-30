//! 替身交任务的回报（施工 7-2，`docs/blueprint/kernel/session.md`「回报」）：子会话交来的回报（命令 `Report`，发命令的
//! 是那个子会话），执行器交来的后台命令结束（输入 `JobEnded`），会话 actor 交的有没有头订阅着（输入 `Watched`）。派任务
//! 照剧本回：[`super::Play::starts_command`]、[`super::Play::starts_agent`]。子会话交出来的向上回报记下来（施工 7-6）。子会话
//! 发来的留言（施工 7-7）。

use std::collections::BTreeMap;

use super::Stage;
use crate::block::{Block, Text};
use crate::event::{ChildReason, ChildReported, JobReason, JobReported};
use crate::id::{CommandId, ContentHash, JobId, SessionId};
use crate::origin::{By, Session};
use crate::session::{Command, Input, Received, Subagent, Upward};

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
            by_model: false,
        };
        self.command_as(by, Command::Report(reported))
    }

    /// 子会话 `session` 交来子代理 `j<job>` 被停掉的回报（施工 7-4）：`by_model` 是她自己用 `jobs` 停的，不是人停的。返回这个
    /// 命令的编号。
    ///
    /// # Panics
    ///
    /// 同上。
    pub fn child_stopped(&mut self, job: u64, session: &str, by_model: bool) -> CommandId {
        let session =
            SessionId::parse(session).unwrap_or_else(|e| panic!("会话编号的写法坏了：{e}"));
        let by = By::Session(Session {
            id: session.clone(),
        });
        let reported = ChildReported {
            job: job_id(job),
            session,
            reason: ChildReason::Stopped,
            text: String::new(),
            truncated: false,
            person: false,
            by_model,
        };
        self.command_as(by, Command::Report(reported))
    }

    /// 子会话再交一次同一份回报：命令编号是 `id`（施工 7-6：子会话载入时再交最后报的那一份）。
    ///
    /// # Panics
    ///
    /// `job` 是 0，或者 `session` 不是会话编号的写法。
    pub fn child_reports_again(
        &mut self,
        id: &CommandId,
        job: u64,
        session: &str,
        reason: ChildReason,
        text: &str,
    ) {
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
            truncated: false,
            person: false,
            by_model: false,
        };
        let at = self.tick();
        self.run(Input::Command(Received {
            id: id.clone(),
            by,
            at,
            command: Command::Report(reported),
        }));
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

    /// 子会话交出来的向上回报，照先后（施工 7-6）。
    pub fn reported_up(&self) -> &[Upward] {
        &self.upward
    }

    /// 人（alice）说一句：子会话的替身平常说话的是父会话，人切进来说的用这个（施工 7-6）。返回这个命令的编号。
    ///
    /// # Panics
    ///
    /// alice 的写法坏了才会：那是替身自己的 bug。
    pub fn person_says(&mut self, words: &str) -> CommandId {
        let alice: By = serde_json::from_str(r#"{"kind":"person","account":"alice"}"#)
            .unwrap_or_else(|e| panic!("alice 的写法坏了：{e}"));
        let blocks = vec![Block::Text(Text {
            text: words.to_string(),
        })];
        self.command_as(
            alice,
            Command::Send {
                blocks,
                urgent: false,
            },
        )
    }

    /// 子会话 `session` 发来一句留言（施工 7-7）：`by` 是它，一块字。返回这个命令的编号。
    ///
    /// # Panics
    ///
    /// `session` 不是会话编号的写法。
    pub fn child_says(&mut self, session: &str, words: &str) -> CommandId {
        let id = SessionId::parse(session).unwrap_or_else(|e| panic!("会话编号的写法坏了：{e}"));
        let blocks = vec![Block::Text(Text {
            text: words.to_string(),
        })];
        self.command_as(
            By::Session(Session { id }),
            Command::Send {
                blocks,
                urgent: false,
            },
        )
    }

    /// 这个会话派出去的子代理，照内核交给执行器的那一份（施工 7-7）。
    pub fn subagents(&self) -> BTreeMap<JobId, Subagent> {
        self.session.subagents()
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
