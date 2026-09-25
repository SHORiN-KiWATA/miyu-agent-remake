//! 上下文的事件（`docs/designs/03-事件模型.md` 第三节「上下文的事件怎么写」）。

use serde::{Deserialize, Serialize};

use crate::id::{FactKind, Seq};

/// `context.injected`：注入进上下文的一块事实，例如当前时间、记忆召回的结果、
/// 这一轮为什么叫她（`08-上下文投影.md` 第五节）。谁注入的写在事件的 `by` 里。
///
/// 存的是发给模型的原文，以后一字不改地回放（内核 K2）。放在请求里的哪个位置，
/// 由投影照日志的先后、回合的触发和类别推出来（`08-上下文投影.md` C2），事件里不写。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextInjected {
    /// 这一块的类别。环境和状态变了才注入，拿它找同一个模块的同类块来比（C10）。
    /// 一块里可以有几个标签，所以类别单写一格，不从标签里猜。
    pub kind: FactKind,
    /// 发给模型的原文：标签外壳、不可信字段的转义都已经做好（C7）。投影只管放，不再改写。
    pub text: String,
}

/// `context.compacted`：压缩的检查点。三种压缩方式都写这一种（`09-压缩.md` 第三节）。
///
/// 检查点里别的东西，例如代码补上的文件清单、取回原文的办法、压完重读的文件，
/// 做压缩的那一步再加。摘要外面那层包装是投影的模板，不存在这里。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextCompacted {
    /// 检查点替代到哪个序号为止，这一条也替代掉。之后的事件照常渲染在检查点后面
    /// （03 第七节）。
    pub upto: Seq,
    /// 摘要的正文，模型写的。内核不解读。
    pub summary: String,
}

#[cfg(test)]
mod tests;
