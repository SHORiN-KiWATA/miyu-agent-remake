//! 旁白（`docs/blueprint/view.md`「`notice` 的 `what`」）：只给结构，字由头写；出错说明照连接的语言给。

use serde::Serialize;

use miyu_kernel::event::{CallError, CompactTrigger, PauseReason, RestoreOutcome, Usage};
use miyu_kernel::id::{JobId, Seq, TurnId};

use crate::entry::{Answered, JobDone, ModelSwap, Peer, is_false};

/// 一条旁白。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "what", rename_all = "snake_case")]
pub enum Notice {
    /// 压缩。
    Compaction(Compaction),
    /// 上下文清空了。
    Cleared {},
    /// 暂停了自动压缩。
    Paused {
        /// 为什么。
        reason: PauseReason,
        /// 连着失败了几次。
        #[serde(skip_serializing_if = "Option::is_none")]
        failures: Option<u32>,
        /// 太大压不下的那一条。
        #[serde(skip_serializing_if = "Option::is_none")]
        entry: Option<Seq>,
    },
    /// 撤销了几轮。
    Reverted {
        /// 撤掉的几轮。
        turns: Vec<TurnId>,
        /// 撤掉的第一句的头一行。
        #[serde(skip_serializing_if = "Option::is_none")]
        said: Option<String>,
        /// 改回了哪些文件。
        #[serde(skip_serializing_if = "Vec::is_empty")]
        files: Vec<RevertedFile>,
        /// 停掉的后台任务。
        #[serde(skip_serializing_if = "Vec::is_empty")]
        jobs: Vec<JobId>,
    },
    /// 一件后台任务了结了。
    Job(JobDone),
    /// 一组题答了。
    Answered(Answered),
    /// 一段回顾。
    Recap {
        /// 回顾的字。
        text: String,
        /// 讲到的最后一轮：撤了那一轮跟着藏。
        #[serde(skip_serializing_if = "Option::is_none")]
        covers: Option<TurnId>,
    },
    /// 换了模型。
    Model(ModelSwap),
    /// 换了工作区。
    Workspace {
        /// 新的工作目录，原样。
        cwd: String,
        /// 另外几个能干活的目录，原样。
        #[serde(skip_serializing_if = "Option::is_none")]
        dirs: Option<Vec<String>>,
    },
    /// 等的会话怎么了。
    Peer(Peer),
}

/// 改回的一个文件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RevertedFile {
    /// 路径。
    pub path: String,
    /// 改回的结局。
    pub outcome: RestoreOutcome,
}

/// 压缩那一条。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Compaction {
    /// 自动的还是手动的。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger: Option<CompactTrigger>,
    /// 手动压缩附的要求。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    /// 在压、压好了、失败了。
    pub state: CompactionState,
    /// 摘要写了多少字：只在在压时有。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub written: Option<u64>,
    /// 估计要写多少字：只在在压时有。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<u64>,
    /// 哪一次摘要请求：只在在压时有。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seen: Option<u64>,
    /// 换上的是提前压好的：不出进度。
    #[serde(skip_serializing_if = "is_false")]
    pub prepared: bool,
    /// 压之前的用量。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<u64>,
    /// 压完的用量。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<u64>,
    /// 用了多久。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub took_ms: Option<u64>,
    /// 摘要请求的用量；手动压缩那一轮是整轮的。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    /// 失败的：那一次请求的错。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<CallError>,
    /// 失败的出错说明，照连接的语言。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explain: Option<String>,
}

/// 压缩到哪了。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionState {
    /// 在压。
    Running,
    /// 压好了。
    Done,
    /// 失败了。
    Failed,
}
