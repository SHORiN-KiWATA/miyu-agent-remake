//! 出错那一句的原因（蓝图 `tui.md`「正文」第 4 条）：供应商的原话照样写，不加分类（各家的错误码不一样，原话大多
//! 已经说清了为什么）；几种 HTTP 状态码前面加一句人话；内核自己查出来的写分类的人话。收尾那一行、压缩失败那一行
//! 都照它。

use crate::config::Texts;
use crate::core::CallError;

/// 内核自己查出来的分类：原话不是供应商的，只写分类的人话（`03-事件模型.md`「出错」）。
const KERNEL_CLASSES: [&str; 5] = [
    "no_model",
    "bad_stream",
    "empty_reply",
    "bad_summary",
    "compaction_paused",
];

/// 限速的分类：核心还没给状态码的，照它当 429（核心把 429 都分成它）。
const RATE_LIMITED: (&str, u16) = ("rate_limited", 429);

/// 出错的原因。
pub(super) fn reason(error: &CallError, texts: &Texts) -> String {
    let said = error.message.trim();
    let class = error.class.as_str();
    // 内核自己查出来的：原话是给运行日志的英文诊断，只写人话（2026-10-01 项目主人：有中文了后面不要再接英文）。
    if KERNEL_CLASSES.contains(&class) {
        return class_name(class, texts);
    }
    let status = error
        .status
        .or((class == RATE_LIMITED.0).then_some(RATE_LIMITED.1));
    match status.and_then(|s| texts.status_hints.get(&s.to_string())) {
        Some(hint) => with(hint, said, texts),
        None if said.is_empty() => class_name(class, texts),
        None => said.to_string(),
    }
}

/// 人话后面接原话；没有原话的只写人话。
fn with(head: &str, said: &str, texts: &Texts) -> String {
    match said {
        "" => head.to_string(),
        said => texts
            .reason_with
            .replace("{head}", head)
            .replace("{message}", said),
    }
}

/// 出错的分类写成人话（`text/zh.json` 的 `error_classes`）；认不得的照原样。
pub(super) fn class_name(class: &str, texts: &Texts) -> String {
    texts
        .error_classes
        .get(class)
        .cloned()
        .unwrap_or_else(|| class.to_string())
}
