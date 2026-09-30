//! 会话要做的事，和命令的结局（`docs/designs/02-内核.md` 第四节「输入、动作、命令怎么写」）。
//!
//! 会话自己不做 I/O：要追加的事件、要回应的命令、要推送的事件，都写成动作交给执行器。

use crate::event::{Event, Permission, Response, Transient};
use crate::id::{CallId, CommandId, ContentHash, JobId, Seq, TurnId};
use crate::origin::By;
use crate::request::{Difference, Request};
use crate::time::Timestamp;

use super::report::Upward;
use super::restore::Step;

/// 会话要执行器做的一件事。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// 追加这几条事件：一次写入、一次同步（`07-存储.md` 第三节）。落了盘，执行器送一条
    /// 「落盘了」回来。
    Append(Vec<Event>),
    /// 回应一个命令：接受的等它的事件落了盘才回，拒绝的当场回。
    Reply {
        /// 回应的是哪个命令。
        id: CommandId,
        /// 结局。
        outcome: Outcome,
    },
    /// 把这几条落了盘的事件推给头（S4：先落盘，后推送）。
    Push(Vec<Event>),
    /// 跑回合开始的挂接点：叫各模块，等它们都回来或者超时，把各自的注入照固定的先后交回来
    /// （`05-内核接口.md` 第五节第 2 条）。一个模块都没挂，也照样回一次，交回空的。
    RunTurnStartHooks {
        /// 哪个回合。
        turn: TurnId,
    },
    /// 请求模型：把这份请求交给驱动编码、发出去（`05-内核接口.md` 第七节）。发出去了、
    /// 每一段增量、说完了，都带着 `seen` 回报（`02-内核.md` 第六节「回复怎么收、回合怎么结束」）。
    CallModel {
        /// 这次请求看到了第几条为止，也是这次请求的名字。
        seen: Seq,
        /// 统一的请求。
        request: Request,
        /// 和这个会话上一次请求比，第一处不同在哪；只是接着加的是 `None`。和记进 `model.called` 的是同一份，
        /// 执行器照它写运行日志：缓存没命中时，一看就知道是不是前缀变了（施工 3-9 下）。
        changed: Option<Difference>,
    },
    /// 把一条瞬时事件推给头：不落盘，不等（`03-事件模型.md` 第五节）。
    PushTransient(Transient),
    /// 不要这次请求了：停下来，别再发它的增量。之后还来的回报，都当过时的不理。
    CancelModel {
        /// 哪一次请求。
        seen: Seq,
    },
    /// 到点叫醒：到了 `at` 这一刻，送一条「到点了」回来（`02-内核.md` 第四节，施工 3-5 下：重试前
    /// 等一会儿）。内核是纯逻辑，自己不睡。
    Wake {
        /// 什么时候叫醒。
        at: Timestamp,
        /// 为哪一次请求等的：出错的那一次，「到点了」照它对上。
        seen: Seq,
    },
    /// 跑回合结束的挂接点：广播给各模块，不等结果（`05-内核接口.md` 第五节）。
    RunTurnEndHooks {
        /// 哪个回合。
        turn: TurnId,
    },
    /// 掐掉一次在跑的工具调用：回合被打断了，或者不等停着的了、要重启了。之后还来的结果和输出，都当过时的不理。
    CancelTool {
        /// 哪一次调用。
        call_id: CallId,
    },
    /// 叫一次改文件的调用停（施工 4-9 再补一）：打断时它还在跑。工具停在改之前，或者做完；照常交回结果，停在改之前
    /// 的状态是已取消。内核等它交回来才结束这一轮，最多等 10 秒。
    StopTool {
        /// 哪一次调用。
        call_id: CallId,
    },
    /// 过执行前的链：权限策略和各扩展的守卫，最严者胜，交回一个结论（`02-内核.md` 第六节
    /// 「确认怎么走」）。守卫超时的按拒绝交回。
    GuardTool {
        /// 哪一次调用。
        call_id: CallId,
        /// 工具名。
        name: String,
        /// 修正过的参数：一个 JSON 对象的原文。
        args: String,
        /// 这一轮的工作目录：回合开始时的那一个。
        cwd: String,
        /// 这一轮加进来的目录：权限策略照工作区算（施工 5-10 上）。
        dirs: Vec<String>,
        /// 实际生效的那一级：权限策略照它判。
        permission: Permission,
    },
    /// 把人的回答交给在等的那个调用：它问的那组题答完了，回答已经落了盘（`02-内核.md` 第六节
    /// 「提问怎么走」第 3 条）。之后照常等它执行完。
    AnswerTool {
        /// 哪一次调用。
        call_id: CallId,
        /// 照题目的先后，每道题的回答。
        answers: Vec<Response>,
    },
    /// 撤销、恢复时照效果改回文件（`10-自带软件.md` 第七节「改回文件的细则」，施工 4-7 上）：一步一步先核对再动手，
    /// 做完了一步一项交回结局。这时不开新的一轮，做完才接下一个命令。
    Restore {
        /// 改回的几步，照先后。
        steps: Vec<Step>,
    },
    /// 停掉撤掉的那几轮派出去、还在跑的任务（施工 7-8，`agents.md` 第七条第 1 条）：后台命令整组杀掉，子代理连它派的一起
    /// 停，回报都记 `undone`、不叫醒她。后台命令的回报交回 [`super::Input::JobEnded`]（`by`、`cause` 照这里的），子代理的
    /// 由子会话交来（命令 `Report`）。不送回、不等：撤销照常回应。排在撤销那一条的 `Append` 后面、改回文件前面：停下的
    /// 命令不会再动文件。
    StopJobs {
        /// 停哪几个，照编号。
        jobs: Vec<JobId>,
        /// 撤销的人：后台命令那几条 `job.reported` 的 `by`。
        by: By,
        /// 撤销的命令：那几条的 `cause`。
        cause: CommandId,
    },
    /// 压完要重读的文件（`compaction.md` 第九条，施工 6-5）：排在摘要请求的「请求模型」前面，执行器读完、存成
    /// blob，送回 [`super::Input::Reread`]，再做下一个动作。
    Reread {
        /// 哪一次摘要请求。
        seen: Seq,
        /// 真实的位置，照先后。
        paths: Vec<String>,
        /// 单个最多多少字节：超了的不读完，报太大。
        limit: u64,
    },
    /// 撤销撤掉压缩时读回日志（`compaction.md` 第十一条，施工 6-9）：只读地读这个会话的日志，从第 `from` 条到最后
    /// 一条，送回 [`super::Input::ReadBack`]，读完才收收件箱。读不了的，会话停下。
    ReadBack {
        /// 从第几条起：撤完以后还算数的最近一次压缩替代到的下一条，一次都没有的是第 1 条。
        from: Seq,
    },
    /// 取回检查点里重读的文件的原文（`compaction.md` 第九条，施工 6-9）：检查点换了、原文不在内存里的时候（载入以后、
    /// 撤掉了压缩、恢复了压缩）。照 blob 读出原文，送回 [`super::Input::Recalled`]，读不出来的不交；读完才收收件箱。
    Recall {
        /// 新检查点里重读的文件的 blob，照先后。
        blobs: Vec<ContentHash>,
    },
    /// 向上回报（施工 7-6，`report.rs`）：子会话交给父会话的一份。执行器补上任务编号、子会话，经端口交给父会话（命令
    /// `Report`，发命令的是这个子会话），不送回；父会话落了盘就算送到，父会话没了的丢掉。
    Report(Upward),
    /// 发回顾的请求（施工 3-8 四补，`recap.rs`）：单独的一次辅助请求，交给这个会话的模型，和主请求不相干。发出去了、每一段
    /// 增量、说完了，都带着 `upto` 回报（[`super::Input::RecapSent`]、`RecapDelta`、`RecapEnded`）。不叫停：会话停了，执行器
    /// 放下它就停了。
    Recap {
        /// 照到第几条，也是这一次回顾的名字。
        upto: Seq,
        /// 统一的请求：一条 user，没有 system、工具面。
        request: Request,
    },
    /// 执行一次工具调用。执行中的输出、执行完了，都带着调用编号回报
    /// （`02-内核.md` 第六节「工具怎么调、下一步怎么走」）。
    RunTool {
        /// 哪一次调用。
        call_id: CallId,
        /// 工具名。
        name: String,
        /// 修正过的参数：一个 JSON 对象的原文。
        args: String,
        /// 这一轮的工作目录：回合开始时的那一个。
        cwd: String,
        /// 这一轮加进来的目录：执行器写沙盒的规格时照工作区放行（施工 5-10 上）。
        dirs: Vec<String>,
        /// 派出去那一刻实际生效的那一级（施工 5-4 上）：执行器照它给这次调用写沙盒的规格。
        permission: Permission,
        /// 这一轮的 `cause`，也是这次调用的结果的（施工 7-3）：它起的后台命令自己退出了，执行器照它填 `job.reported` 的
        /// `cause`（`kernel/session.md`「回报」第 2 条）。
        cause: Option<CommandId>,
    },
}

/// 一个命令的结局：被接受并产生事件，或被拒绝并附原因（不变量 7）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// 接受了。
    Accepted {
        /// 它产生的事件的序号，照先后。
        events: Vec<Seq>,
    },
    /// 回顾好了（施工 3-8 四补，`recap.rs`）：那一句、照到第几条、是不是交回的上一句。那条 `session.recapped` 落了盘才回。
    Recapped {
        /// 那一句。
        text: String,
        /// 照到第几条。
        upto: Seq,
        /// 是交回的上一句：上一次回顾以后没有新内容，没有请求。
        cached: bool,
    },
    /// 拒绝了，什么都没产生。
    Rejected {
        /// 为什么拒绝。
        reason: Reason,
    },
}

/// 拒绝的原因。给程序看的原因码是稳定的英文（[`Reason::code`]）；给人看的话，由核心照头的
/// 语言配上（`04-核心协议.md` 第六节第 4 条）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// 发来的消息一块内容都没有。
    EmptyMessage,
    /// 没有回合在进行，打断不了。
    NotRunning,
    /// 要切到的级别不认识。
    UnknownLevel,
    /// 这个调用不在等人回答：没问过、已经回答过、已经有了结果，或者等的是另一种回答（对着确认
    /// 答提问，对着提问答确认）。
    NotAsking,
    /// 确认的选项不认识。
    UnknownDecision,
    /// 请求没提放行规则，选不了本会话都允许、这个工作区以后都允许。
    NoRule,
    /// 只有拒绝能带理由。
    UnexpectedReason,
    /// 回答对不上题目：题数不对、选了没有的选项、单选的选了几项、同一项选了两次。
    BadAnswer,
    /// 有回合在进行，撤销、手动压缩（施工 6-8）、清空（施工 6-8 补）不了：头先打断，或者等它做完（`02-内核.md` 第六节
    /// 「撤销与恢复」）。
    TurnRunning,
    /// 要撤的那一轮没有，或者已经撤掉了。压缩以前的回合照样能撤（施工 6-9）。
    UnknownTurn,
    /// 没有能恢复的撤销：没撤过，或者撤了以后开过回合、压缩过。
    NothingToUnrevert,
    /// 撤销、恢复还没做完：正在读回更早的日志（施工 6-9）、正在改回文件（施工 4-7 上）。等它做完再来。
    Restoring,
    /// 撤最后一轮（不写回合编号）时一轮都没有：没说过话、都撤掉了、都压缩进了摘要（施工 4-7 下）。
    NothingToRevert,
    /// 手动压缩时没有能压的：上一次压缩以后没有新的消息、回复、工具结果，或者全在压完要原样留着的尾巴里（施工 6-8，
    /// `compaction.md` 第七条第 2 条）。
    NothingToCompact,
    /// 清空时上下文本来就是空的：有效历史里没有摘要，最近的检查点后面也没有人的消息、回复、工具结果、回报（施工 6-8 补，
    /// `compaction.md` 第十四条第 2 条）。
    NothingToClear,
    /// 子会话交来的回报对不上一个还会报的子代理（施工 7-2）：没有这个任务、不是子代理、会话不对、不是那个子会话发的、
    /// 被停掉过（账本的几条，`docs/blueprint/kernel/history.md`）。
    UnknownJob,
    /// 重做不了（施工 4-7 再补，`docs/blueprint/kernel/history.md`「重做」）：最后一轮不是人说的话开的（回报叫醒的、
    /// 手动压缩、清空、重启以后接着干的），或者一轮都没有。一个原因码管两种（2026-09-30 项目主人定）。
    NotRedoable,
    /// 没有能回顾的（施工 3-8 四补，`recap.rs`）：有效历史里她一个带正文的回复都没有；以前造的快照没有回顾的字的也是它，只在
    /// 开发时的旧会话里有，不另开原因码（照 `NothingToCompact` 的先例）。
    NothingToRecap,
    /// 回顾没写成（施工 3-8 四补）：请求出了错，或者回复里没有正文。原因记在那一次的 `model.called` 里；不再来，头要再要一次
    /// 就是。
    RecapFailed,
}

impl Reason {
    /// 原因码。
    pub fn code(self) -> &'static str {
        match self {
            Reason::EmptyMessage => "empty_message",
            Reason::NotRunning => "not_running",
            Reason::UnknownLevel => "unknown_level",
            Reason::NotAsking => "not_asking",
            Reason::UnknownDecision => "unknown_decision",
            Reason::NoRule => "no_rule",
            Reason::UnexpectedReason => "unexpected_reason",
            Reason::BadAnswer => "bad_answer",
            Reason::TurnRunning => "turn_running",
            Reason::UnknownTurn => "unknown_turn",
            Reason::NothingToUnrevert => "nothing_to_unrevert",
            Reason::Restoring => "restoring",
            Reason::NothingToRevert => "nothing_to_revert",
            Reason::NothingToCompact => "nothing_to_compact",
            Reason::NothingToClear => "nothing_to_clear",
            Reason::UnknownJob => "unknown_job",
            Reason::NotRedoable => "not_redoable",
            Reason::NothingToRecap => "nothing_to_recap",
            Reason::RecapFailed => "recap_failed",
        }
    }
}
