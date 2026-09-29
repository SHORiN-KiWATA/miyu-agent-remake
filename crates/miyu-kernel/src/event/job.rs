//! 任务的两种回报（施工 7-1，`docs/blueprint/agents.md`「对外的样子」，`03-事件模型.md` 第三节）：后台命令结束了
//! `job.reported`，子会话的回报 `child.reported`。任务开始记在派它的那次调用的效果 `job.started` 里（`effect.rs`）。
//!
//! 回报对不对得上派出去的任务、子代理还会不会再报，由账本查（`ledger/jobs.rs`）。谁写、到了开不开一轮、渲染成什么样，
//! 随 7-2。

use serde::{Deserialize, Serialize};

use crate::id::{ContentHash, JobId, SessionId};
use crate::text_enum::text_enum;

/// `job.reported`：后台命令结束了。哪一种 `reason` 都算结束，之后这个任务不再报。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobReported {
    /// 哪一个后台命令。
    pub job: JobId,
    /// 为什么结束。
    pub reason: JobReason,
    /// 退出码，照系统交回的原样（`ExitStatus::code()`）：带符号，Windows 上的 NTSTATUS 是负的。被信号杀掉的没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// 杀掉它的信号的编号（Unix），和前台 `shell` 写的一样。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<u32>,
    /// 是她自己用 `jobs` 停的：只跟着 `stopped`，她知道自己停了，不叫醒她（`agents.md` 第三条第 3 条）。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub by_model: bool,
    /// 从起进程到结束的毫秒数。载入时补的 `aborted` 没有：进程什么时候没的不知道。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// 整份输出存成的 blob。没存下来的没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<ContentHash>,
    /// 整份输出有多少个字（Unicode 字符），和 `output` 一起有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chars: Option<u64>,
}

text_enum!(
    /// 后台命令为什么结束。
    JobReason {
        /// 自己退出了，被信号杀掉的也算。
        Exited = "exited",
        /// 被停掉了：人停的，或者她自己用 `jobs` 停的。
        Stopped = "stopped",
        /// 撤销派它的那一轮时停掉的。
        Undone = "undone",
        /// 有计划的重启停掉的。
        Restarted = "restarted",
        /// 核心崩了，进程跟着没了：载入时补（`02-内核.md` 不变量 8）。
        Aborted = "aborted",
    }
);

/// `child.reported`：子会话的回报。`by` 是那个子会话。一个子代理可以报好几次：父会话留言叫醒它，它那一轮结束时再报
/// （`agents.md` 第六条）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildReported {
    /// 哪一个子代理。
    pub job: JobId,
    /// 子会话，和派它的 `job.started` 记的一样。
    pub session: SessionId,
    /// 为什么报。
    pub reason: ChildReason,
    /// 那一轮最后的回答，超过上限的留头尾各一半；一个字都没说就结束的是空的。
    pub text: String,
    /// 正文截过。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
    /// 那一轮里人插过话，或者那一轮是人开的、里面进过父会话的留言：渲染时注明，免得父会话对不上自己派的活
    /// （`agents.md` 第二条第 4 条）。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub person: bool,
}

text_enum!(
    /// 子会话为什么报。
    ChildReason {
        /// 那一轮结束了：走完的，出错、到了步数上限结束的也算。
        Done = "done",
        /// 被停掉了：之后不再报。
        Stopped = "stopped",
        /// 撤销父会话派它的那一轮时停掉的：之后不再报。
        Undone = "undone",
        /// 核心崩了，它那一轮没走完：载入时补。以后还能再报（`agents.md` 第八条）。
        Aborted = "aborted",
    }
);

#[cfg(test)]
mod tests;
