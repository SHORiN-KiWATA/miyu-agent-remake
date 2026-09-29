//! 旁白里的几句话怎么拼：撤销点开以后的那一行、收尾行后面的本轮用量。字在配置里，这里只管拼。

use crate::config::Texts;
use crate::core::{Report, Usage};

/// 撤销点开以后的那一行：`改回 2 个文件 · 1 条命令的改动撤不回`。是 0 的那几格不写，都是 0 的是 `None`。
pub fn undo_counts(report: &Report, texts: &Texts) -> Option<String> {
    let counts = [
        (report.restored, &texts.restored),
        (report.untouched, &texts.untouched),
        (report.commands, &texts.commands),
    ];
    let parts: Vec<String> = counts
        .into_iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, template)| template.replace("{count}", &n.to_string()))
        .collect();
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// 收尾行后面那一截：本轮用量（输入加输出）和命中率（命中 ÷ 输入）；这一轮一次模型都没调成的是空的
/// （`tui.md`「正文」第 4 条）。
pub(super) fn turn_usage(usage: &Usage, texts: &Texts) -> String {
    let input = usage.input();
    if input == 0 {
        return String::new();
    }
    texts
        .done_usage
        .replace("{tokens}", &crate::meter::short(input + usage.output))
        .replace(
            "{percent}",
            &crate::meter::hit_rate(usage.cache_read, input),
        )
}
