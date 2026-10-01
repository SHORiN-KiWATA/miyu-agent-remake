//! 送进会话的输入，和输入里的命令（`docs/designs/02-内核.md` 第四节「输入、动作、命令怎么写」）。

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use crate::accumulate::Delta;
use crate::block::Block;
use crate::estimate::ImagePrice;
use crate::event::{
    CallError, ChildReported, ContextInjected, Decision, Effect, Event, IdleReason, JobReported,
    Level, Purpose, Question, Response, Restored, Said, Usage,
};
use crate::facts::Environment;
use crate::id::{CallId, CommandId, ContentHash, ModuleId, Seq, SessionId, TurnId};
use crate::origin::{By, Model};
use crate::raw::RawJson;
use crate::time::Timestamp;
use crate::tool::Access;

/// 送进会话的一件事。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// 收到一个命令。
    Command(Received),
    /// 追加的事件已经同步到磁盘，到第 `upto` 条为止（`07-存储.md` S4）。
    Stored {
        /// 落完盘的时刻：落了盘接着发请求时，熔断要写的事件照它记（施工 6-6 上）。
        at: Timestamp,
        /// 落了盘的最后一条的序号。
        upto: Seq,
    },
    /// 环境变了：时区、工作目录。不当场注入，到下一个边界再查（`08-上下文投影.md` C10）。
    Environment(Environment),
    /// 模型的限额：造会话、载入以后交一次，换了模型再交（施工 6-2 上）。没交过的不主动压缩。
    Limits(Limits),
    /// 回合开始的挂接点跑完了（`05-内核接口.md` 第五节第 2 条）。
    TurnStartHooksDone {
        /// 到的时刻，取自执行器的时钟。
        at: Timestamp,
        /// 哪个回合的。
        turn: TurnId,
        /// 各模块交回来的注入，照固定的先后：先按声明的优先级，再按模块编号。
        injected: Vec<Injection>,
    },
    /// 请求发出去了（`02-内核.md` 第六节「回复怎么收、回合怎么结束」）。
    RequestSent {
        /// 到的时刻，取自执行器的时钟。用时从这一刻算起。
        at: Timestamp,
        /// 哪一次请求。
        seen: Seq,
        /// 发给了哪个端点的哪个模型。
        model: Model,
        /// 驱动编码以后的请求字节的哈希（`05-内核接口.md` 第七节）。
        request: ContentHash,
    },
    /// 模型的一段增量（`03-事件模型.md` 第五节「增量和累积器怎么写」）。
    ModelDelta {
        /// 到的时刻，取自执行器的时钟。
        at: Timestamp,
        /// 哪一次请求的。
        seen: Seq,
        /// 这一段增量。
        delta: Delta,
    },
    /// 模型说完了：正常说完的，附上用量；出错的，附上分类和原话。没发出去就失败了的，
    /// 不报「发出去了」，直接报这一条。
    ModelEnded {
        /// 到的时刻，取自执行器的时钟。
        at: Timestamp,
        /// 哪一次请求的。
        seen: Seq,
        /// 用量。供应商没报的，没有。
        usage: Option<Usage>,
        /// 出错的分类和原话；正常说完的，没有。
        error: Option<CallError>,
        /// 出错时供应商说了要等多久再试，毫秒（施工 3-5 下）；没说的，没有。
        wait_ms: Option<u64>,
        /// 超长时超了多少 token，驱动从原话里解析的（施工 6-6 中）：摘要请求照它截短。解析不出来的、别的出错，没有。
        excess: Option<u64>,
    },
    /// 到点了：内核交出去的「到点叫醒」到时候了（`02-内核.md` 第四节）。回合已经不在等了的，不理。
    Woke {
        /// 到的时刻，取自执行器的时钟。
        at: Timestamp,
        /// 为哪一次请求等的：出错的那一次。
        seen: Seq,
    },
    /// 工具执行完了（`02-内核.md` 第六节「工具怎么调、下一步怎么走」）。成功、出错，或者叫它停以后停在了改之前
    /// （施工 4-9 再补一）：已取消、被拒绝、已跳过的那一句是内核写的。
    ToolDone {
        /// 到的时刻，取自执行器的时钟。
        at: Timestamp,
        /// 哪一次调用。
        call_id: CallId,
        /// 出错了没有：工具执行了，但是出了错。错在哪，写在内容块里。
        error: bool,
        /// 给模型看的内容。
        blocks: Vec<Block>,
        /// 执行用了多少毫秒，执行器量的。
        duration_ms: Option<u64>,
        /// 给人看的说法，工具交的（施工 4-5 上）；没交的是空的。
        human: Option<Said>,
        /// 效果，工具交的（施工 4-6 上）：改前改后的内容已经由执行器存成了 blob，这里是它们的哈希。
        effects: Vec<Effect>,
        /// 叫它停以后停在了改之前，什么都没改（施工 4-9 再补一）：记成已取消，那一句内核写。
        stopped: bool,
    },
    /// 工具执行中的一段输出，只推给头（`03-事件模型.md` 第五节）。
    ToolProgress {
        /// 到的时刻，取自执行器的时钟。
        at: Timestamp,
        /// 哪一次调用。
        call_id: CallId,
        /// 一段输出。
        text: String,
    },
    /// 在跑的调用问人一组题（`02-内核.md` 第六节「提问怎么走」）。
    ToolAsks {
        /// 到的时刻，取自执行器的时钟。
        at: Timestamp,
        /// 哪一次调用。
        call_id: CallId,
        /// 一组题，照先后。
        questions: Vec<Question>,
    },
    /// 改回文件做完了（施工 4-7 上）：[`super::Action::Restore`] 的每一步照先后，一步一项结局。
    Restored {
        /// 到的时刻，取自执行器的时钟。
        at: Timestamp,
        /// 每一步的结局，照交出去的先后。
        files: Vec<Restored>,
    },
    /// 压完要重读的文件读好了（施工 6-5）：[`super::Action::Reread`] 的每一个照先后，一个一项。内核记在那次摘要
    /// 请求上，什么都不出；不是在路上的那一次的，不理。
    Reread {
        /// 到的时刻，取自执行器的时钟。
        at: Timestamp,
        /// 哪一次摘要请求。
        seen: Seq,
        /// 每一个读得怎么样，照交出去的先后。
        files: Vec<Reread>,
    },
    /// 撤销撤掉压缩时读回的日志（施工 6-9）：[`super::Action::ReadBack`] 的回报，从第 `from` 条起，一条接一条连到
    /// 最后一条。对得上正在读回的那一次才记撤销；对不上的当过时的不理。
    ReadBack {
        /// 到的时刻，取自执行器的时钟：撤销那一条记这一刻。
        at: Timestamp,
        /// 从第几条起，照交出去的。
        from: Seq,
        /// 读回的事件，照先后。
        events: Vec<Event>,
    },
    /// 检查点里重读的文件的原文，照 blob 找（施工 6-5）：[`super::Action::Recall`] 的回报（施工 6-9）。放进有效历史，
    /// 什么都不出；不是现在这个检查点的，组装时找不到它。
    Recalled {
        /// 原文，照 blob 找。
        texts: BTreeMap<ContentHash, String>,
    },
    /// 要重启了：有计划的重启，关之前送进来（`02-内核.md` 第六节「载入、崩溃、重启」第 3 条）。
    Restarting {
        /// 到的时刻，取自执行器的时钟。
        at: Timestamp,
    },
    /// 后台命令结束了（施工 7-2，`docs/blueprint/kernel/session.md`「回报」）：执行器交来，记一条 `job.reported`，不带回合
    /// 编号。对不上一个还没结束的后台命令的，不理。
    JobEnded {
        /// 到的时刻，取自执行器的时钟。
        at: Timestamp,
        /// 谁让它结束的，照原因填（2026-09-30 定）：自己退出的是起它的那次调用，被停掉的是停它的人或者那次 `jobs` 调用，
        /// 撤销停掉的是撤销的人，重启、崩了的是内核。
        by: By,
        /// 哪个命令引起的：那次调用所在回合的、停它、撤销的那个命令的。
        cause: Option<CommandId>,
        /// 那一条的 `body`。
        reported: JobReported,
    },
    /// 辅助请求发出去了（施工 3-8 四补 `recap.rs`；五补起回顾、起标题共用这一路，`aside.rs`）：和主请求的
    /// [`Input::RequestSent`] 一样，名字是用途加它照到的那一条 `upto`。不是在路上的那一次的，不理。
    AsideSent {
        /// 到的时刻，取自执行器的时钟。用时从这一刻算起。
        at: Timestamp,
        /// 哪一种辅助请求。
        purpose: Purpose,
        /// 哪一次：照到的那一条。
        upto: Seq,
        /// 发给了哪个端点的哪个模型。
        model: Model,
        /// 驱动编码以后的请求字节的哈希。
        request: ContentHash,
    },
    /// 辅助请求的一段增量。不推给头：辅助请求只用说完了的那一句。
    AsideDelta {
        /// 到的时刻，取自执行器的时钟。
        at: Timestamp,
        /// 哪一种辅助请求。
        purpose: Purpose,
        /// 哪一次。
        upto: Seq,
        /// 这一段增量。
        delta: Delta,
    },
    /// 辅助请求说完了：正常说完的附上用量，出错的附上分类和原话。辅助请求出错不自动重试，要等多久、超了多少都用不上。
    /// 没发出去就失败了的，不报「发出去了」，直接报这一条。
    AsideEnded {
        /// 到的时刻，取自执行器的时钟。
        at: Timestamp,
        /// 哪一种辅助请求。
        purpose: Purpose,
        /// 哪一次。
        upto: Seq,
        /// 用量。供应商没报的，没有。
        usage: Option<Usage>,
        /// 出错的分类和原话；正常说完的，没有。
        error: Option<CallError>,
    },
    /// 订的「空了告诉我」等不到了（施工 C-6，`docs/blueprint/cross-session.md` 第六条第 8、9 款）：执行器交，到点了是
    /// `expired`，订的时候那个会话不在了是 `gone`。内核照账本还在等、`expired` 的确实到了点的，记一条 `peer.idle`，`by` 是
    /// 内核，只记下、不叫醒；别的不理。读回日志的时候到的先放着。
    WatchEnded {
        /// 到的时刻，取自执行器的时钟：`expired` 照它查到没到点。
        at: Timestamp,
        /// 等的是哪个会话。
        session: SessionId,
        /// `expired` 或者 `gone`。
        reason: IdleReason,
    },
    /// 有没有头订阅着这个会话（施工 7-2）：会话 actor 在订阅、退订时交。只在内存里，不进日志；造会话、载入以后当没人
    /// 看着。没人看着的一次性会话，回报只记下、不开轮（`agents.md` 第三条第 3 条）。
    Watched {
        /// 有头订阅着。
        watched: bool,
    },
    /// 执行前的链判完了（`02-内核.md` 第六节「确认怎么走」）。
    ToolGuarded {
        /// 到的时刻，取自执行器的时钟。
        at: Timestamp,
        /// 哪一次调用。
        call_id: CallId,
        /// 链的结论。
        verdict: Verdict,
    },
}

/// 执行前的链交回的结论：几个守卫合起来，最严者胜（`05-内核接口.md` 第五节第 1 条）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// 放行。
    Allow,
    /// 拒绝。
    Deny {
        /// 哪个模块拒的：写成被拒绝的结果的 `by`。
        module: ModuleId,
        /// 写给模型的那一句。
        text: String,
        /// 给人看的说法（施工 4-5 上）；模块没交的是空的。
        human: Option<Said>,
    },
    /// 要问人。请求的另外几格照写进 `tool.approval_requested`，调用编号由内核填。
    Ask {
        /// 哪个模块问的：写成请求的 `by`。
        module: ModuleId,
        /// 要的是哪一类访问。
        access: Access,
        /// 提的放行规则；没提就没有。
        rule: Option<RawJson>,
        /// 给头看的：为什么要问。
        detail: Option<RawJson>,
    },
}

/// 收到的一个命令，连同它从哪里来、什么时候到的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Received {
    /// 命令编号，发送方生成。同一个编号只生效一次（不变量 9）。
    pub id: CommandId,
    /// 谁发的，取自连接，不取自正文。
    pub by: By,
    /// 到的时刻，取自执行器的时钟。
    pub at: Timestamp,
    /// 命令本身。
    pub command: Command,
}

/// 发给会话的意图（`02-内核.md` 第三节）。结局只有两种：被接受并产生事件，或被拒绝并附原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// `session.send`：发一条消息。会话空闲时，还会开一个回合。
    Send {
        /// 消息的内容块。
        blocks: Vec<Block>,
        /// 急着插话：这一步还没跑的工具跳过，这句话马上进下一步（`02-内核.md` 第六节
        /// 「打断和急着插话」）。
        urgent: bool,
    },
    /// `session.set_permission_level`：开关只读，或者改常用的那一级，改哪样写哪样
    /// （`02-内核.md` 第六节「权限级别怎么切」）。
    SetPermission {
        /// 常用的那一级；不改就没有。
        level: Option<Level>,
        /// 只读开关；不改就没有。
        read_only: Option<bool>,
    },
    /// `session.set_meta`：改标题、置顶，改哪样写哪样（施工 3-8 三补，`meta.rs`）。
    SetMeta {
        /// 新的标题，照 `session.meta_changed` 的写法：空的是去掉标题；不改就没有。去掉前后空白、量长短是协议端点的事
        /// （`docs/blueprint/protocol.md` 的 `session.set_meta`），内核照原样比、照原样记。
        title: Option<String>,
        /// 置顶还是取消置顶；不改就没有。
        pinned: Option<bool>,
    },
    /// `session.interrupt`：打断正在进行的回合。
    Interrupt {
        /// 排着队的消息怎么办（`02-内核.md` 第六节「排队的消息」）。
        queued: Queued,
    },
    /// `session.answer`：回答一次权限确认，或者一组题（`02-内核.md` 第六节「确认怎么走」第 3 条、
    /// 「提问怎么走」第 3 条）。
    Answer {
        /// 回答的是哪一次调用的请求或者题目。
        call_id: CallId,
        /// 回答。
        answer: Answer,
    },
    /// `session.revert`：从这一轮起撤销，它和它以后的全撤（`02-内核.md` 第六节「撤销与恢复」）。
    Revert {
        /// 从哪一轮起。不写的，撤还在有效历史里的最后一轮（施工 4-7 下）。
        turn: Option<TurnId>,
    },
    /// `session.unrevert`：恢复最近一次撤销，在下一轮开始、压缩之前。
    Unrevert,
    /// `session.redo`：重做最后一轮（`docs/blueprint/kernel/history.md`「重做」，施工 4-7 再补）：撤掉它，把开它的那几句
    /// 人的话再发一次，开新的一轮。最后一轮不是人的话开的、一轮都没有的，拒绝，`not_redoable`。
    Redo {
        /// 开这一轮的那一句里的字换成这几块（原来的文字块全换掉，空的是不要字）；`None` 是字照原来的。
        text: Option<Vec<Block>>,
        /// 开这一轮的那一句里的附件换成这几块（原来文字以外的块全换掉，空的是不要附件）；`None` 是附件照原来的。两样都是
        /// `None` 的原样重发。
        attachments: Option<Vec<Block>>,
    },
    /// `session.compact`：手动压缩，空闲时才收，单开一轮只做压缩（`compaction.md` 第七条，施工 6-8）。
    Compact {
        /// 人附的要求，原样；`None` 是没附。只有空白的也当没附。
        instructions: Option<String>,
    },
    /// `session.clear`：清空上下文，空闲时才收，单开一轮压成一个空的检查点，不请求模型（`compaction.md` 第十四条，
    /// 施工 6-8 补）。
    Clear,
    /// `session.recap`：要一句回顾（施工 3-8 四补，`docs/blueprint/kernel/session.md`「回顾」）。有回合在进行时照收，照这一刻
    /// 落了盘的有效历史；不开回合、不进她的上下文。
    Recap,
    /// 子会话交来的回报（施工 7-2，`agents.md` 第二条第 5 条）：子会话的执行器经端口交，发命令的一方就是子会话。记一条
    /// `child.reported`，不带回合编号。对不上一个还会报的子代理的，拒绝，`unknown_job`。
    Report(ChildReported),
    /// 被等的会话交来的「空了」（施工 C-6，`docs/blueprint/cross-session.md` 第六条第 6、7 款）：发命令的一方就是它。这边
    /// 在等它的，记一条 `peer.idle`（`idle`），不带回合编号，照回报的规矩叫不叫醒她；不在等的拒绝，`unknown_watch`。
    PeerIdle {
        /// 它最近结束的那一轮最后一条有字的回复的第一行，被等的那一边截好的；一个字都没说的没有。
        status: Option<String>,
    },
}

/// 一次回答：回答确认的，或者回答一组题的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// 回答确认。
    Approval {
        /// 选了哪一项。
        decision: Decision,
        /// 拒绝的理由；只有拒绝能带，空的当没写。
        reason: Option<String>,
    },
    /// 回答一组题：照题目的先后，每道题选了哪几项、自己写了什么。
    Questions(Vec<Response>),
}

/// 打断时，排着队的消息怎么办。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Queued {
    /// 接着发：打断以后马上开一轮，由最后一条触发。
    Send,
    /// 退回：撤回来，交还给头，放回输入框。
    Return,
}

/// 一个模块在回合开始时交回来的一块注入。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Injection {
    /// 哪个模块注入的：写成事件的 `by`。
    pub module: ModuleId,
    /// 注入的那一块，原样追加。
    pub fact: ContextInjected,
}

/// 会话要发给的模型的限额（`compaction.md`「对外的样子」模型的资料）：压缩线照它算。
#[derive(Clone)]
pub struct Limits {
    /// 发给哪个端点的哪个模型：和锚比，换过模型锚作废。
    pub model: Model,
    /// 上下文窗口；没报的没有，不主动压。
    pub window: Option<u64>,
    /// 最大输出；没报的没有，输出预留按策略里的上限。
    pub max_output: Option<u64>,
    /// 一张图怎么算，驱动交的（施工 6-3 上）；没有的照策略里的固定数。
    pub images: Option<Arc<dyn ImagePrice>>,
}

/// 图片的算法是驱动交的对象，打印时只说有没有。
impl fmt::Debug for Limits {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Limits")
            .field("model", &self.model)
            .field("window", &self.window)
            .field("max_output", &self.max_output)
            .field("images", &self.images.is_some())
            .finish()
    }
}

/// 图片的算法比的是不是同一个对象。
impl PartialEq for Limits {
    fn eq(&self, other: &Limits) -> bool {
        self.model == other.model
            && self.window == other.window
            && self.max_output == other.max_output
            && match (&self.images, &other.images) {
                (None, None) => true,
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                _ => false,
            }
    }
}

impl Eq for Limits {}

/// 压完要重读的一个文件读得怎么样（`compaction.md` 第九条，施工 6-5）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reread {
    /// 读到了：原文存进了 blob。
    Read {
        /// 原文的哈希。
        blob: ContentHash,
        /// 原文。
        text: String,
    },
    /// 比上限大，没读完。
    TooLarge,
    /// 读不到：没有、读不了、不是普通文件、不是 UTF-8。
    Unreadable,
}
