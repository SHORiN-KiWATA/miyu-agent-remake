//! 会话状态里投影算得出的那一半（施工 9-8 补上，`docs/blueprint/view.md`「会话状态」）：在跑没跑、在等什么、正在做什么、
//! 上下文用了多少、最近一次的速度、都在冷却到几时、派出去的任务。用量、模型、权限这些要向会话要，由端点拼上。

use serde::Serialize;

use miyu_kernel::event::{ErrorClass, JobKind};
use miyu_kernel::id::{CallId, JobId, SessionId};
use miyu_kernel::time::Timestamp;

use crate::entry::EntryId;

/// 做完的任务最多留几个：最近的。
pub const FINISHED_KEPT: usize = 20;

/// 投影算得出的会话状态。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Status {
    /// 在跑没跑、在等人。
    pub state: State,
    /// 没了结的确认、提问，照先后。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub waiting: Vec<Waiting>,
    /// 这一轮开始的时刻；没在跑的没有。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<Timestamp>,
    /// 正在做什么；没在跑的没有。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doing: Option<Doing>,
    /// 上下文用了多少 token：最近一次主请求的输入，压完的照压完的；还不知道的没有。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used: Option<u64>,
    /// 最近一次主请求的速度。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<Speed>,
    /// 候选都在冷却，最早恢复的时刻；没在冷却的没有。只认订阅以后看到的。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cooling_until: Option<Timestamp>,
    /// 这个会话直接派出去的任务：在跑的，和最近做完的 [`FINISHED_KEPT`] 个，照派出的先后。
    pub jobs: Vec<JobRow>,
}

/// 在跑没跑。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// 闲着。
    Idle,
    /// 在跑。
    Running,
    /// 在等人确认、等人回答。
    Waiting,
}

/// 一件在等人的事。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Waiting {
    /// 等什么。
    pub what: Wait,
    /// 哪一步（工具那一条）。
    pub entry: EntryId,
    /// 哪一次调用。
    pub call: CallId,
}

/// 等什么。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Wait {
    /// 等人确认。
    Approve,
    /// 等人回答。
    Ask,
}

/// 正在做什么。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "what", rename_all = "snake_case")]
pub enum Doing {
    /// 在想：那一块思考。
    Thinking {
        /// 哪一条。
        entry: EntryId,
    },
    /// 在写回答：那一块正文。
    Writing {
        /// 哪一条。
        entry: EntryId,
    },
    /// 在调工具：那一步。
    Tool {
        /// 哪一条。
        entry: EntryId,
    },
    /// 出错了，等着再试。
    Retrying {
        /// 第几次，从 1 数起。
        attempt: u32,
        /// 最多几次。
        limit: u32,
        /// 再试的时刻。
        at: Timestamp,
        /// 出错的分类。
        class: ErrorClass,
        /// 出错的原话。
        message: String,
        /// HTTP 状态码。
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<u16>,
        /// 换了端点再来。
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        failover: bool,
    },
    /// 在压缩：写到哪了。
    Compacting {
        /// 压缩那一条。
        entry: EntryId,
        /// 摘要写了多少 token。
        written: u64,
        /// 估计一共多少。
        expected: u64,
    },
}

/// 一次主请求的速度：输出多少 token、首字到结束多少毫秒，每秒多少由头算。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Speed {
    /// 输出的 token。
    pub output: u64,
    /// 首字到结束的毫秒数；没报首字的照整次。
    pub ms: u64,
}

/// 任务表的一行。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JobRow {
    /// 任务编号。
    pub job: JobId,
    /// 后台命令还是子代理。
    pub what: JobKind,
    /// 派它时的描述。
    pub title: String,
    /// 在跑、做完、失败、停了、断了。
    pub state: JobState,
    /// 派出的时刻。
    pub started: Timestamp,
    /// 结束的时刻；在跑的没有。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended: Option<Timestamp>,
    /// 子代理的会话。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<SessionId>,
    /// 后台命令的命令原文。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// 后台命令的退出码。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// 杀掉后台命令的信号。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<u32>,
    /// 停了的为什么：`stopped`（人停或她自己停的）、`undone`（撤销时停的）、`restarted`（有计划的重启停的）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub why: Option<&'static str>,
}

/// 任务的状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    /// 在跑。
    Running,
    /// 做完了：后台命令退出码是 0；子代理那一轮结束了（还能留言叫醒）。
    Done,
    /// 失败了：后台命令退出码不是 0、被信号杀了。
    Failed,
    /// 停了。
    Stopped,
    /// 核心崩了，断了。
    Aborted,
}

impl JobState {
    /// 还在跑。
    #[must_use]
    pub fn running(self) -> bool {
        self == JobState::Running
    }
}
