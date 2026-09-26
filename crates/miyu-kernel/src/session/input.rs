//! 送进会话的输入，和输入里的命令（`docs/designs/02-内核.md` 第四节「输入、动作、命令怎么写」）。

use crate::accumulate::Delta;
use crate::block::Block;
use crate::event::{CallError, ContextInjected, Usage};
use crate::facts::Environment;
use crate::id::{CallId, CommandId, ContentHash, ModuleId, Seq, TurnId};
use crate::origin::{By, Model};
use crate::time::Timestamp;

/// 送进会话的一件事。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// 收到一个命令。
    Command(Received),
    /// 追加的事件已经同步到磁盘，到第 `upto` 条为止（`07-存储.md` S4）。
    Stored {
        /// 落了盘的最后一条的序号。
        upto: Seq,
    },
    /// 环境变了：时区、工作目录。不当场注入，到下一个边界再查（`08-上下文投影.md` C10）。
    Environment(Environment),
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
    },
    /// 工具执行完了（`02-内核.md` 第六节「工具怎么调、下一步怎么走」）。只有成功和出错两种：
    /// 已取消、被拒绝、已跳过是内核写的。
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
    },
}

/// 一个模块在回合开始时交回来的一块注入。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Injection {
    /// 哪个模块注入的：写成事件的 `by`。
    pub module: ModuleId,
    /// 注入的那一块，原样追加。
    pub fact: ContextInjected,
}
