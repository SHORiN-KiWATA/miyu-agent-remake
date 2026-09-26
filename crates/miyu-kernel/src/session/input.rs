//! 送进会话的输入，和输入里的命令（`docs/designs/02-内核.md` 第四节「输入、动作、命令怎么写」）。

use crate::block::Block;
use crate::event::ContextInjected;
use crate::facts::Environment;
use crate::id::{CommandId, ModuleId, Seq, TurnId};
use crate::origin::By;
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
