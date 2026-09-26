//! 给模型看的几句固定的字：检查点的包装、回合没走完的那一句（`docs/designs/08-上下文投影.md`
//! 第四节第 5 条，`26-提示词.md` 第八节）。
//!
//! 出厂的放在资源目录的 `core/` 下，由执行器读好交进来（施工 M3）；这里只管拿来拼，
//! 不读文件。文件的内容原样用，行尾的换行也算。

use miyu_kernel::event::EndReason;

/// 组装时要用的几句固定的字。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Texts {
    /// 检查点包装的开头，摘要紧接在它后面（`core/checkpoint-open.txt`）。
    pub checkpoint_open: String,
    /// 检查点包装的结尾，紧接在摘要后面（`core/checkpoint-close.txt`）。
    pub checkpoint_close: String,
    /// 回合没走完时，排在下一条人的消息前面的那一句。
    pub turn_ended: TurnEndedTexts,
}

/// 回合没走完的四句，一种原因一句（`core/turn-ended/<原因>.txt`）。正常走完的不说。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnEndedTexts {
    /// 被人打断（`interrupted.txt`）。
    pub interrupted: String,
    /// 出了错（`error.txt`）。
    pub error: String,
    /// 走到了步数上限（`step_limit.txt`）。
    pub step_limit: String,
    /// 程序重启，没走完（`aborted.txt`）。
    pub aborted: String,
}

impl TurnEndedTexts {
    /// 这个原因要说的那一句。正常走完的不说；不认识的原因是新版本才有的，不知道该怎么说，
    /// 也不说。
    pub(crate) fn for_reason(&self, reason: &EndReason) -> Option<&str> {
        match reason {
            EndReason::Interrupted => Some(&self.interrupted),
            EndReason::Error => Some(&self.error),
            EndReason::StepLimit => Some(&self.step_limit),
            EndReason::Aborted => Some(&self.aborted),
            EndReason::Completed | EndReason::Other(_) => None,
        }
    }
}
