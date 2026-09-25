//! 回合的事件（`docs/designs/03-事件模型.md` 第三节「会话与回合的事件怎么写」）。

use serde::{Deserialize, Serialize};

use crate::id::{Seq, TurnId};
use crate::text_enum::text_enum;

/// `turn.started`：回合开始。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnStarted {
    /// 引起这一轮的那条事件的序号：人发来的消息、子代理的回报、后台命令结束。
    /// 是什么引起的，看那条事件的种类。
    pub trigger: Seq,
}

/// `turn.ended`：回合结束。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnEnded {
    /// 为什么结束。
    pub reason: EndReason,
}

text_enum!(
    /// 回合结束的原因。
    EndReason {
        /// 走完了：模型说完了，没有要执行的工具。
        Completed = "completed",
        /// 被人打断。
        Interrupted = "interrupted",
        /// 出错。细节在那一次模型请求的 `model.called` 里。
        Error = "error",
        /// 走到了步数上限。
        StepLimit = "step_limit",
        /// 核心崩溃或重启时没走完（`02-内核.md` 不变量 8）。
        Aborted = "aborted",
    }
);

/// `turn.reverted`：撤销哪几个回合。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnReverted {
    /// 被撤销的回合。撤销以后，投影里不再有它们（`03-事件模型.md` 第七节）。
    pub turns: Vec<TurnId>,
}

#[cfg(test)]
mod tests;
