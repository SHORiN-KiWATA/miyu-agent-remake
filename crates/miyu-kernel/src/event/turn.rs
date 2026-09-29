//! 回合的事件（`docs/designs/03-事件模型.md` 第三节「会话与回合的事件怎么写」）。

use serde::{Deserialize, Serialize};

use crate::id::{Seq, TurnId};
use crate::text_enum::text_enum;

/// `turn.started`：回合开始。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnStarted {
    /// 引起这一轮的那条事件的序号：人发来的消息、子代理的回报、后台命令结束。
    /// 是什么引起的，看那条事件的种类。人要的压缩单开的那一轮不是哪一条引起的，没有（`compaction.md` 第七条，
    /// 施工 6-8）：它只做压缩，载入时不接着干，渲染时不进上下文。以前的日志里都有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<Seq>,
    /// 这一轮开始时会话的工作目录，照会话的环境，人看到的那种写法（施工 4-9 再补三上）。核心重启以后载入会话，
    /// 照它找回会话在哪个目录里干活：日志是真相，不另放文件。之前的日志没有这一格。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// 这一轮加进来的目录，照头报的原样（施工 5-10 上）：权限策略、沙盒照它放行。没有就不写：原来的日志一个字节不变。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dirs: Vec<String>,
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
        /// 核心崩了，没走完（`02-内核.md` 不变量 8）。载入时补上。
        Aborted = "aborted",
        /// 被有计划的重启打断（`02-内核.md` 第六节「载入、崩溃、重启」）。再起来时接着干。
        Restarted = "restarted",
    }
);

/// `turn.reverted`：撤销哪几个回合。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnReverted {
    /// 被撤销的回合：某一轮，和它以后还在有效历史里的每一轮，照先后（`02-内核.md` 第六节
    /// 「撤销与恢复」）。撤销以后，投影里不再有它们（`03-事件模型.md` 第七节）。
    pub turns: Vec<TurnId>,
}

/// `turn.unreverted`：恢复最近一次撤销的那几轮（`02-内核.md` 第六节「撤销与恢复」）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnUnreverted {
    /// 恢复的回合：照那一条 `turn.reverted` 原样写。投影里它们回到原来的位置。
    pub turns: Vec<TurnId>,
}

#[cfg(test)]
mod tests;
