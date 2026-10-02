//! 旁白里的几句话怎么拼：撤销点开以后的那一行、收尾行后面的本轮用量。字在配置里，这里只管拼。

use crate::config::Texts;
use crate::core::{Report, Usage};

/// 撤销点开以后，文件清单下面那一行：`停掉了 1 个任务`。没有的是 `None`（改回的文件一个一行列在上面，命令的改动撤不回
/// 那一句不要：2026-10-02 项目主人）。
pub fn undo_counts(report: &Report, texts: &Texts) -> Option<String> {
    (report.jobs > 0).then(|| {
        texts
            .stopped_jobs
            .replace("{count}", &report.jobs.to_string())
    })
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
