//! 条目：头直接画的一条（`docs/blueprint/view.md`「对外的样子」）。格照图纸，写成 JSON 就是交给头的样子。

use serde::Serialize;

use miyu_kernel::event::{
    CallError, ChildReason, Decision, EndReason, IdleReason, JobKind, JobReason, Question,
    Response, ToolStatus, Usage,
};
use miyu_kernel::id::{CallId, ContentHash, JobId, ModelName, ProviderId, SessionId, TurnId};
use miyu_kernel::origin::By;
use miyu_kernel::raw::RawJson;
use miyu_kernel::time::Timestamp;
use miyu_kernel::tool::Access;

pub use crate::id::EntryId;
pub use crate::notice::Notice;

/// 一条。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Entry {
    /// 稳定编号：跨页、跨重连都不变。
    pub id: EntryId,
    /// 种类和它自己的格。
    #[serde(flatten)]
    pub body: Body,
    /// 属于哪一轮；不属于哪一轮的没有。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn: Option<TurnId>,
    /// 撤销藏起来的。
    #[serde(skip_serializing_if = "is_false")]
    pub hidden: bool,
    /// 开始的时刻：流式来的照第一段增量，翻页算的照引出它的那条事件。
    pub at: Timestamp,
}

/// 条目的种类，连同它自己的格。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[expect(
    clippy::large_enum_variant,
    reason = "条目一条一份、存在一个 Vec 里，工具那一种最常见，装箱只多一次分配"
)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Body {
    /// 一条消息：人说的，和别处来的话。
    User(User),
    /// 她的回答里的一块正文。
    Reply(Reply),
    /// 时间线的一段：她开口之前的思考、调工具。
    Group(Group),
    /// 一步思考。
    Thought(Thought),
    /// 一步工具。
    Tool(Tool),
    /// 一轮的收尾。
    End(End),
    /// 旁白。
    Notice(Notice),
}

/// 一条消息。
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct User {
    /// 字：几块文字照先后接起来。
    pub text: String,
    /// 附件，照先后。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<Attachment>,
    /// 别处来的话的来处；人在本机说的没有。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<By>,
    /// 发出去那一刻的权限级别：`workspace`、`full`、`read_only`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
    /// 排着、她还没听到。
    #[serde(skip_serializing_if = "is_false")]
    pub queued: bool,
    /// 排着的被退回了：不画。
    #[serde(skip_serializing_if = "is_false")]
    pub withdrawn: bool,
}

/// 一件附件。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Attachment {
    /// `image` 或 `file`。
    pub kind: &'static str,
    /// 名字；图可以没有。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 类型。
    pub media_type: String,
    /// 内容的哈希：头照 `blob.get` 取。
    pub blob: ContentHash,
    /// 图的宽。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    /// 图的高。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    /// 带了路径的附件的路径。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// 她的回答里的一块正文。
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Reply {
    /// Markdown 原文。
    pub text: String,
    /// 还在写。
    #[serde(skip_serializing_if = "is_false")]
    pub open: bool,
}

/// 时间线的一段。
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Group {
    /// 几步的编号，照先后。
    pub steps: Vec<EntryId>,
    /// 还在进行：她没开口、这一轮没结束、没插进旁白。
    #[serde(skip_serializing_if = "is_false")]
    pub open: bool,
    /// 收起那一行，照连接的语言。
    pub summary: Vec<Part>,
    /// 收起那一行，固定英文。
    pub summary_en: Vec<Part>,
    /// 这一段只有一条命令、它出错了：整行红。
    #[serde(skip_serializing_if = "is_false")]
    pub failed: bool,
    /// 从第一步开始到最后一步结束；还在进行的没有。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub took_ms: Option<u64>,
}

/// 收起那一行的一段字。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Part {
    /// 字。
    pub text: String,
    /// 颜色；照这一行的没有。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tone: Option<Tone>,
}

/// 收起那一行里单拎出来的颜色。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    /// 加的行数。
    Added,
    /// 删的行数。
    Removed,
    /// 出错的那一格。
    Error,
}

/// 一步思考。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Thought {
    /// 属于哪一段。
    pub group: EntryId,
    /// 想的字。
    pub text: String,
    /// 还在想。
    #[serde(skip_serializing_if = "is_false")]
    pub open: bool,
    /// 想了多久；还在想的没有。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub took_ms: Option<u64>,
}

/// 一步工具。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Tool {
    /// 属于哪一段。
    pub group: EntryId,
    /// 调用编号；流式时还没有。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call: Option<CallId>,
    /// 工具名。
    pub name: String,
    /// 标题那一句。
    pub title: Title,
    /// 进行到哪了。
    pub state: ToolState,
    /// 参数原文，流式时边收边接。
    pub args: String,
    /// 编辑、写入加减的行数。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff: Option<Diff>,
    /// 结果里的图。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<Picture>,
    /// 派出去的后台任务的编号。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job: Option<JobId>,
    /// 留言发给子代理的：那个子代理的标题。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_title: Option<String>,
    /// 要人确认的：问的什么、怎么定的。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval: Option<Approval>,
    /// 用了多久；还没结果的没有。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub took_ms: Option<u64>,
    /// 改了哪些文件（施工 9-8 三补）：照结果的效果，换成真实位置以后的绝对路径，照先后；读的不算。没有的不写。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<Touched>,
}

/// 一步改了的一个文件（施工 9-8 三补）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Touched {
    /// 换成真实位置以后的绝对路径。
    pub path: String,
    /// 改了还是移进了回收站。
    pub action: TouchKind,
}

/// 文件怎么了。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TouchKind {
    /// 新建：改之前没有这个文件（`file.changed` 没有 `before`）。
    Created,
    /// 覆盖、编辑已有的（`file.changed` 有 `before`）。
    Changed,
    /// 移进回收站（`file.trashed`）。
    Trashed,
}

/// 标题那一句，照连接的语言。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Title {
    /// 显示名；还在写参数的是「准备……」。
    pub name: String,
    /// 对象。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    /// 参数带着别的会话的编号：短编号那一格。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// 结果那一句。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub said: Option<String>,
}

/// 一步工具进行到哪了。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolState {
    /// 还在写参数。
    Preparing,
    /// 参数写完了、还没结果。
    Running,
    /// 成了。
    Ok,
    /// 出错。
    Error,
    /// 被拒。
    Denied,
    /// 打断了。
    Cancelled,
    /// 跳过了。
    Skipped,
}

impl ToolState {
    /// 结果的状态写成这一步的状态。
    #[must_use]
    pub fn of(status: &ToolStatus) -> ToolState {
        match status {
            ToolStatus::Ok => ToolState::Ok,
            ToolStatus::Error => ToolState::Error,
            ToolStatus::Denied => ToolState::Denied,
            ToolStatus::Skipped => ToolState::Skipped,
            ToolStatus::Cancelled | ToolStatus::Other(_) => ToolState::Cancelled,
        }
    }

    /// 出错、被拒：图标换成叉、整行红。
    #[must_use]
    pub fn failed(self) -> bool {
        matches!(self, ToolState::Error | ToolState::Denied)
    }

    /// 还没结果。
    #[must_use]
    pub fn busy(self) -> bool {
        matches!(self, ToolState::Preparing | ToolState::Running)
    }
}

/// 编辑、写入加减的行数。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Diff {
    /// 加了几行。
    pub added: u64,
    /// 删了几行。
    pub removed: u64,
    /// 照参数估的（结果还没到，或者算不出真数）。
    #[serde(skip_serializing_if = "is_false")]
    pub estimated: bool,
}

/// 结果里的一张图。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Picture {
    /// 内容的哈希。
    pub blob: ContentHash,
    /// 类型。
    pub media_type: String,
    /// 宽。
    pub width: u32,
    /// 高。
    pub height: u32,
}

/// 要人确认的那一次。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Approval {
    /// 要的访问。
    pub access: Access,
    /// 链提的规则，原样。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule: Option<RawJson>,
    /// 链给的细节，原样。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<RawJson>,
    /// 怎么定的；还在等的没有。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision: Option<Decision>,
    /// 不允许时说的原因。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// 谁定的。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub by: Option<By>,
}

/// 一轮的收尾。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct End {
    /// 怎么结束的。
    pub reason: EndReason,
    /// 最近一次说话的端点。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<ProviderId>,
    /// 最近一次说话的模型。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelName>,
    /// 这一轮用了多久。
    pub took_ms: u64,
    /// 这一轮每次主请求的用量加起来；一次都没调成的没有。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    /// 这一轮开始时的权限级别。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
    /// 出错结束的：最后一次请求的错。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<CallError>,
    /// 出错说明，照连接的语言。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explain: Option<String>,
}

/// 一组题：问的和答的。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Answered {
    /// 问的那一组。
    pub questions: Vec<Question>,
    /// 答的，照题目的先后。
    pub answers: Vec<Response>,
    /// 谁答的。
    pub by: By,
}

/// 一件后台任务了结了。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JobDone {
    /// 任务编号。
    pub job: JobId,
    /// 后台命令还是子代理（不叫 `what`：旁白的种类占了那个名字）。
    pub job_kind: JobKind,
    /// 标题：派它那一步的描述。
    pub title: String,
    /// 记号：`done`、`failed`、`stopped`。
    pub mark: Mark,
    /// 回报里的原因，原样。
    pub reason: JobEnd,
    /// 子代理的报告全文。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report: Option<String>,
    /// 后台命令：派它那次调用的命令原文。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// 后台命令的输出。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<Output>,
    /// 后台命令的退出码（施工 9-8 三补）：照回报，没有的不写。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// 杀掉后台命令的信号（施工 9-8 三补）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<u32>,
    /// 后台命令从起到结束的毫秒数（施工 9-8 三补）：照回报，核心崩了补的没有。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub took_ms: Option<u64>,
}

/// 后台任务了结的原因：后台命令的、子代理的。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum JobEnd {
    /// 后台命令。
    Command(JobReason),
    /// 子代理。
    Agent(ChildReason),
}

/// 后台任务了结的记号。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mark {
    /// 做完了。
    Done,
    /// 失败了。
    Failed,
    /// 停掉了。
    Stopped,
}

/// 后台命令的输出。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Output {
    /// 内容的哈希。
    pub blob: ContentHash,
    /// 多少个字。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chars: Option<u64>,
}

/// 等的会话怎么了。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Peer {
    /// 哪个会话。
    pub session: SessionId,
    /// 空下来了、到点了、没了。
    pub reason: IdleReason,
    /// 它最后一句的状态。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

/// 换了模型。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelSwap {
    /// 换成的端点。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// 换成的模型。
    pub model: String,
    /// 换之前的。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<Was>,
    /// 为什么：`failover`、`replaced`。
    pub why: &'static str,
    /// 出错换的：换之前最近一次出错的分类。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
}

/// 换之前的模型。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Was {
    /// 端点。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// 模型。
    pub model: String,
}

pub(crate) fn is_false(value: &bool) -> bool {
    !*value
}
