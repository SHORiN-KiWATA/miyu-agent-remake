//! 瞬时事件：不落库，只推给正在连接的头，用于实时显示（`docs/designs/03-事件模型.md`
//! 第五节「瞬时事件的外壳」）。
//!
//! 外壳和持久事件同一种写法，只少了 `seq`：它不进日志。`cause` 留着，一个命令引起的事，
//! 从持久的到瞬时的都能一路追下去。内核只推不读，所以这里只写出去；读回来的那一半，
//! 头读它们的时候再写（M8）。

use serde::{Serialize, Serializer};

use crate::accumulate::Kind;
use crate::event::{CompactTrigger, ErrorClass, Usage};
use crate::id::{CallId, CommandId, ModelName, ProviderId, Seq, TurnId};
use crate::origin::By;
use crate::session::ContextLimits;
use crate::text_enum::text_enum;
use crate::time::Timestamp;

/// 一条瞬时事件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transient {
    /// 发生的时刻，取自执行器送进来的时钟输入。
    pub at: Timestamp,
    /// 所属回合；不属于任何回合时没有。
    pub turn: Option<TurnId>,
    /// 由谁引起。
    pub by: By,
    /// 引起它的命令。
    pub cause: Option<CommandId>,
    /// 种类，连同它自己的内容。
    pub body: TransientBody,
}

/// 瞬时事件的种类，连同它自己的内容。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransientBody {
    /// `model.delta`：模型输出的一段增量。
    ModelDelta(ModelDelta),
    /// `tool.progress`：工具执行中的一段输出。
    ToolProgress(ToolProgress),
    /// `status`：会话在干什么（施工 3-5 下）。现在只有一种：出了错，等着重试。
    Status(Status),
    /// `compaction.progress`：摘要写到哪了（施工 6-2 上）。
    CompactionProgress(CompactionProgress),
    /// `compaction.started`：在后台提前压的那一次发出去了（施工 6-11 再补）。当场压的不推它，推的是 0 字的进度。
    CompactionStarted(CompactionStarted),
    /// `compaction.done`：压好了，压前、压后的用量（施工 6-3 下）。
    CompactionDone(CompactionDone),
    /// `model.changed`：会话接下来请求的模型、限额变了（施工 8-9，`models.md`「瞬时事件」）。会话 actor 造，内核不推。装在
    /// 盒子里：它比别的种类大出一截，推送的队列里每一份都照最大的那一种占地方（clippy 的 `large_enum_variant`）。
    ModelChanged(Box<ModelChanged>),
    /// `todos.changed`：当前的待办变了（施工 D-3，`docs/blueprint/kernel/session.md`「待办」）：写了、撤销、恢复。
    TodosChanged(TodosChanged),
}

/// `model.changed` 的 `body`（施工 8-9）：会话接下来请求的模型、限额变了，头照它换底栏、限额，`why` 是 `failover` 的在
/// 时间线上出一条通知。写出去的格照这个先后，没有的不写。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelChanged {
    /// 会话的引用：模型或 `@池`。
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    /// 接下来发给哪一家。轮换的池没有（每次都换）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<ProviderId>,
    /// 接下来发给哪个模型。轮换的池没有。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelName>,
    /// 接下来那个模型真用的思考强度（施工 8-18）：一档和从哪来。请求里什么都不带的、轮换的池没有。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<EffortInUse>,
    /// 和 `subscribe` 回应里的一样：窗口、压缩线，没有的不写。
    pub limits: ContextLimits,
    /// 为什么变。
    pub why: ChangeWhy,
}

/// 一次请求用的思考强度和它从哪来（施工 8-18，8-18（补）起从哪来是配置的哪一层；`models.md`「怎么走」第十一条第 4
/// 条）：`model.changed`、`subscribe` 回应的 `effort`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EffortInUse {
    /// 那一档：规整过的名字（`off`、`on`，或者目录里的档位名）。
    pub level: String,
    /// 从配置的哪一层来。
    pub from: EffortSource,
}

text_enum!(
    /// 思考强度从配置的哪一层来（施工 8-18；8-18（补）起不再有会话那一层，只剩配置的两层）：`providers.<id>.models.<model>
    /// .effort` 写在系统配置还是个人设置里，照 `config.get` 说的来源。
    EffortSource {
        /// 系统配置 `system/config.toml`。
        System = "system",
        /// 个人设置 `home/<账号>/settings.toml`。
        Personal = "personal",
    }
);

text_enum!(
    /// `model.changed` 为什么推（施工 8-9；回合开始时重新解析的 `turn`，施工 8-10）。
    ChangeWhy {
        /// 回合开始时照这一轮的配置重新解析，头看得到的变了：换了模型、钉着的没了、配置改了（`models.md`「怎么走」第六条
        /// 第 3 条，施工 8-10）。
        Turn = "turn",
        /// 出错换到了池里别的模型：成了才钉过去（`models.md`「怎么走」第四条第 5 条、第五条第 7 条）。
        Failover = "failover",
    }
);

/// `compaction.done` 的 `body`：压好了。压前、压后都是本地估算，和压缩线同一个算法；摘要请求的用量、用时取自它的
/// `model.called`（施工 6-3 下，`compaction.md` 第十三条）。头照它印「上下文已压缩」，核心照它记度量。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompactionDone {
    /// 哪一次摘要请求：它替代到的那一条。
    pub seen: Seq,
    /// 哪一种压缩，和那一条 `context.compacted` 一样（施工 6-8）：运行日志 `compacted` 那一行照它写。
    pub trigger: CompactTrigger,
    /// 压之前的用量。
    pub before: u64,
    /// 压完这一步接着要发的请求的用量。
    pub after: u64,
    /// 摘要请求的用量；供应商没报的没有。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    /// 摘要请求从发出去到说完的毫秒数；没发出去的没有。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// 换上的是提前压好的那一份（施工 6-11 上，`compaction.md` 第十五条）：前面没推过进度，用量、用时是提前那一次摘要请求的。
    /// 当场压的不写。
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub prepared: bool,
}

/// `todos.changed` 的 `body`：现在的整份待办（施工 D-3）。全部做完清空了、写过的都撤掉了的，是空列表。头照它换掉手里的那份，
/// 不自己翻效果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TodosChanged {
    /// 照先后的每一项。
    pub todos: Vec<crate::event::Todo>,
    /// 这一次是因为全部做完而清空的（施工 D-3 补）：做完的那一份，`todo.written` 的 `done`。撤销、恢复退回的、她写空的不带。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub done: Vec<crate::event::Todo>,
}

/// `compaction.progress` 的 `body`：摘要请求收到了多少字，估计要写多少字，头照它画进度（`compaction.md`
/// 第三条第 8 条）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompactionProgress {
    /// 哪一次摘要请求：它替代到的那一条。
    pub seen: Seq,
    /// 哪一种压缩，和压好了写的 `context.compacted` 一样（施工 6-11 再补）：头照它分，自动压的停下等只写一行，手动的画进度。
    pub trigger: CompactTrigger,
    /// 到这时收到的正文字数，草稿加摘要，照 Unicode 字符数。
    pub written: u64,
    /// 估计要写多少字。
    pub expected: u64,
}

/// `compaction.started` 的 `body`：在后台提前压的那一次摘要请求发出去了（施工 6-11 再补，`compaction.md` 第十五条）。主请求
/// 照发，头照它弹一句提示，正文里不画。压失败、作废的不另推；换上时推带 `prepared` 的 `compaction.done`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompactionStarted {
    /// 哪一次摘要请求：它替代到的那一条，和换上时的 `compaction.done` 一样。
    pub seen: Seq,
    /// 是提前压的：现在只有这一种，总是真。
    pub prepared: bool,
}

/// `status` 的 `body`：哪一次请求出了错，等着重试（`03-事件模型.md` 第五节）。以后别的状态
/// （等第一个字时的心跳）也用这一种，换一格。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Status {
    /// 哪一次请求出了错。
    pub seen: Seq,
    /// 等多久再试第几次。
    pub retry: Retry,
}

/// 等着重试：第几次、一共最多几次、等多久，出错的分类、原话和 HTTP 状态码。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Retry {
    /// 这是第几次重试，从 1 数起。
    pub attempt: u32,
    /// 一共最多几次。
    pub limit: u32,
    /// 等多久，毫秒。
    pub wait_ms: u64,
    /// 出错的分类。
    pub class: ErrorClass,
    /// 出错的原话，给人看。
    pub message: String,
    /// 出错的 HTTP 状态码，照那一次的 `model.called` 带过来（施工 3-5 三补）；没有的不写。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    /// 换了端点当场再来（施工 8-9，`models.md`「怎么走」第五条第 5 条）：端口说的。不是的不写。
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub failover: bool,
}

/// `tool.progress` 的 `body`：哪一次调用、一段输出（`03-事件模型.md` 第五节）。结果以
/// `tool.result` 为准，这些只是给人看着它在跑。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolProgress {
    /// 哪一次调用。
    pub call_id: CallId,
    /// 一段输出。
    pub text: String,
}

/// `model.delta` 的 `body`：哪次请求的、第几块、这一段增量。私有数据不推，头用不着。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelDelta {
    /// 这次请求看到了第几条为止，和这次响应最后写成的回复的 `seen` 一样。
    pub seen: Seq,
    /// 第几块，从 0 数起。
    pub index: usize,
    /// 这一段增量。
    pub piece: Piece,
}

/// 推给头的一段增量：累积器收的四种里，除了私有数据的三种（03 第五节）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    /// 一块开始了，是什么块。
    Start(Kind),
    /// 这一块的一段字。
    Text(String),
    /// 这一块收全了。
    End,
}

impl TransientBody {
    /// 外壳里 `kind` 那一格写的名字。
    pub fn kind(&self) -> &'static str {
        match self {
            TransientBody::ModelDelta(_) => "model.delta",
            TransientBody::ToolProgress(_) => "tool.progress",
            TransientBody::Status(_) => "status",
            TransientBody::CompactionProgress(_) => "compaction.progress",
            TransientBody::CompactionStarted(_) => "compaction.started",
            TransientBody::CompactionDone(_) => "compaction.done",
            TransientBody::ModelChanged(_) => "model.changed",
            TransientBody::TodosChanged(_) => "todos.changed",
        }
    }
}

impl Transient {
    /// 写成推给头的一行：紧凑的 JSON，字段照图纸的顺序，不带换行。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：里面只有字符串、数字和原样的 JSON，写成 JSON 不会失败。
    pub fn to_line(&self) -> String {
        serde_json::to_string(self).expect("瞬时事件里只有字符串、数字和原样的 JSON")
    }
}

/// 写出去的样子。字段的顺序是图纸上的：`at`、`kind`、`turn`、`by`、`cause`、`body`。
#[derive(Serialize)]
struct LineOut<'a> {
    at: Timestamp,
    kind: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    turn: Option<TurnId>,
    by: &'a By,
    #[serde(skip_serializing_if = "Option::is_none")]
    cause: Option<&'a CommandId>,
    body: &'a TransientBody,
}

impl Serialize for Transient {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        LineOut {
            at: self.at,
            kind: self.body.kind(),
            turn: self.turn,
            by: &self.by,
            cause: self.cause.as_ref(),
            body: &self.body,
        }
        .serialize(s)
    }
}

impl Serialize for TransientBody {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            TransientBody::ModelDelta(delta) => delta.serialize(s),
            TransientBody::ToolProgress(progress) => progress.serialize(s),
            TransientBody::Status(status) => status.serialize(s),
            TransientBody::CompactionProgress(progress) => progress.serialize(s),
            TransientBody::CompactionStarted(started) => started.serialize(s),
            TransientBody::CompactionDone(done) => done.serialize(s),
            TransientBody::ModelChanged(changed) => changed.serialize(s),
            TransientBody::TodosChanged(changed) => changed.serialize(s),
        }
    }
}

/// `model.delta` 的 `body` 写出去的样子：一块开始写 `start`（工具调用再带 `name`），
/// 一段字写 `text`，收全了写 `"end":true`。
#[derive(Serialize)]
struct DeltaOut<'a> {
    seen: Seq,
    index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    start: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<&'a str>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    end: bool,
}

impl Serialize for ModelDelta {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut out = DeltaOut {
            seen: self.seen,
            index: self.index,
            start: None,
            name: None,
            text: None,
            end: false,
        };
        match &self.piece {
            Piece::Start(Kind::Text) => out.start = Some("text"),
            Piece::Start(Kind::Reasoning) => out.start = Some("reasoning"),
            Piece::Start(Kind::ToolCall { name }) => {
                out.start = Some("tool_call");
                out.name = Some(name);
            }
            Piece::Text(text) => out.text = Some(text),
            Piece::End => out.end = true,
        }
        out.serialize(s)
    }
}

#[cfg(test)]
mod tests;
