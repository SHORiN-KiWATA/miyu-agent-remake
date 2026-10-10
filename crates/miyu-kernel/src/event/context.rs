//! 上下文的事件（`docs/designs/03-事件模型.md` 第三节「上下文的事件怎么写」）。

use serde::{Deserialize, Serialize};

use crate::id::{ContentHash, FactKind, Seq};
use crate::text_enum::text_enum;

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
    /// 这一块带着哪几样东西的编号（施工 R-4 上：记忆的摘要带着的那几条，`m12` 这种）。内核不解读、不进请求，原样记、原样
    /// 交回挂接点（[`crate::facts::Present`]），给模块去重、算「用到没有」。以前的日志里没有、没带的是空的，不写这一格。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refs: Vec<String>,
}

/// `context.compacted`：压缩的检查点。三种压缩方式都写这一种（`09-压缩.md` 第三节）。
///
/// 代码写的几段、压完重读的文件（施工 6-5）也在这里；摘要外面那层包装、重读的文件那一块的头尾是投影的模板，重读的
/// 原文在 blob 里，都不存在这里。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextCompacted {
    /// 检查点替代到哪个序号为止，这一条也替代掉。之后的事件照常渲染在检查点后面
    /// （03 第七节）。
    pub upto: Seq,
    /// 摘要的正文，模型写的，草稿已经剥掉。内核不解读。清空上下文的是空的，只有它能空（施工 6-8 补，账本查）。
    pub summary: String,
    /// 为什么压：到线了、人要的、供应商报超长、人要清空。以前的日志里没有这一格，当作到线了（`compaction.md`）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<CompactTrigger>,
    /// 手动压缩时人附的要求，原样（`compaction.md` 第七条第 3 条，施工 6-8）。没附的、只有空白的没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    /// 代码写的几段：读过、改过的文件清单，取回指路，太大没重读的（`compaction.md` 第八条，施工 6-5）。写的时候拼好，
    /// 以后逐字节回放。以前的日志里没有，当作空的。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub notes: String,
    /// 压完重读的文件，照渲染的先后（施工 6-5）。以前的日志里没有，当作空的。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub restored: Vec<RestoredFile>,
    /// 压完很快又到线，连着的第几次（`compaction.md` 第十条第 5 条，施工 6-6 上）；不是的没有。是写下时的事实：撤销回到
    /// 前一个检查点时照它原样用，以后改了几个回合算快，旧检查点照旧。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refills: Option<u32>,
    /// 换上的是提前在后台压好的那一份（施工 6-11 三补，`compaction.md` 第十五条第 3 条）：头照它不画压缩的结果，翻页也认得出。
    /// 当场压的、以前的日志里没有的是假，不写这一格。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub prepared: bool,
}

/// `context.compaction_paused`：暂停了自动压缩（`compaction.md` 第十条，施工 6-6 上）。不进上下文；有效历史里、最近
/// 一次压缩那一条以后写下了它，就是暂停着。又压缩了一次、撤掉写着它的那一轮，暂停就不算了。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactionPaused {
    /// 为什么暂停。
    pub reason: PauseReason,
    /// 连续失败的：连着失败了几次。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failures: Option<u32>,
    /// 内容太大的：最近一个检查点以后估得最大的那一条。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry: Option<Seq>,
}

/// 压完重读的一个文件（`compaction.md` 第九条，施工 6-5）：路径照清单的写法，原文在 blob 里。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoredFile {
    /// 路径：在会话的工作目录里的写相对的，别的写绝对的。
    pub path: String,
    /// 原文的哈希。
    pub blob: ContentHash,
    /// 原文估出来的 token 数。
    pub tokens: u64,
}

text_enum!(
    /// 为什么暂停自动压缩（`compaction.md` 第十条）。不认识的原样留着，也算暂停。
    PauseReason {
        /// 连续失败到了次数。
        Failures = "failures",
        /// 压完很快又到线，连着到了次数：有异常巨大的内容（`09-压缩.md` Z9）。
        TooLarge = "too_large",
    }
);

text_enum!(
    /// 为什么压缩（`compaction.md`「对外的样子」）。
    CompactTrigger {
        /// 用量过了压缩线，自动压的。
        Auto = "auto",
        /// 人要的（6-8）。
        Manual = "manual",
        /// 供应商报上下文超长，被动压的（6-7）。
        Overflow = "overflow",
        /// 人要清空上下文（6-8 补）：压成一个空的检查点，不请求模型，渲染时什么都不出（`compaction.md` 第十四条）。
        Clear = "clear",
    }
);

#[cfg(test)]
mod tests;
