//! 出错说明（终端蓝图 `tui.md`「正文」第 4 条，原样搬进核心）：供应商的原话照样写；几种 HTTP 状态码前面加一句人话；内核
//! 自己查出来的只写分类的人话。收尾那一行、压缩失败那一条都照它。

use miyu_kernel::event::CallError;

use crate::words::{self, Words, keys};

/// 内核自己查出来的分类：原话是给运行日志的英文诊断，只写人话。
const KERNEL_CLASSES: [&str; 6] = [
    "no_model",
    "cooling",
    "bad_stream",
    "empty_reply",
    "bad_summary",
    "compaction_paused",
];

/// 限速的分类：没给状态码的当 429。
const RATE_LIMITED: (&str, u16) = ("rate_limited", 429);

/// 出错说明，照 `words` 的语言。
pub(crate) fn explain(error: &CallError, words: &dyn Words) -> String {
    let said = error.message.trim();
    let class = error.class.as_str();
    if KERNEL_CLASSES.contains(&class) {
        return class_name(class, words);
    }
    let status = error
        .status
        .or((class == RATE_LIMITED.0).then_some(RATE_LIMITED.1));
    let hint = status.and_then(|s| words::say(words, &format!("{}/{s}", keys::STATUS), &[]));
    match hint {
        Some(head) if said.is_empty() => head,
        Some(head) => words::say(
            words,
            keys::REASON_WITH,
            &[("head", head.clone()), ("message", said.to_string())],
        )
        .unwrap_or(head),
        None if said.is_empty() => class_name(class, words),
        None => said.to_string(),
    }
}

/// 出错的分类写成人话；认不得的照原样。
fn class_name(class: &str, words: &dyn Words) -> String {
    words::say(words, &format!("{}/{class}", keys::CLASS), &[]).unwrap_or_else(|| class.to_string())
}
