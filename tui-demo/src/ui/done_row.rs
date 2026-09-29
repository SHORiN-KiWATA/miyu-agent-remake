//! 一轮做完的收尾行（蓝图 `tui.md`「正文」第 4 条、「窗口小的时候」第 5 条）：打头的图标、多空的，
//! 放不下时只在 ` · ` 处折。

use unicode_width::UnicodeWidthStr;

use super::rows::clip;
use crate::config::Layout;
use crate::core::Level;

/// 收尾行：图标，这个级别要多空的（`done_gap`，只有 `▣` 多空一格），再接字（`tui.md`「正文」第 4 条）。
pub fn line(level: Option<Level>, text: &str, layout: &Layout) -> String {
    let gap = level
        .and_then(|l| layout.done_gap.get(&l))
        .map_or("", String::as_str);
    format!("{}{gap}{text}", mark(level, layout))
}

/// 收尾行打头的符号：这一轮开始时的权限级别的图标；没记着级别的用兜底的 `✻`（`tui.md`「正文」第 4 条）。
fn mark(level: Option<Level>, layout: &Layout) -> &str {
    level
        .and_then(|l| layout.level_icons.get(&l))
        .unwrap_or(&layout.done_icon)
}

/// 按 ` · ` 分成一格一格排进 `width` 列：放不下的那一格折到下一行，以 `· ` 打头；一格比一行还宽的截掉加 `…`。
/// 交回每一行和「是不是上一行折下来的」，复制时折下来的接回去（上一行末尾留着空格，接回去和原文一样）。
pub fn pieces(text: &str, width: u16) -> Vec<(String, bool)> {
    const SEP: &str = " · ";
    let room = usize::from(width).max(1);
    let mut out: Vec<(String, bool)> = Vec::new();
    let mut line = String::new();
    for (k, cell) in text.split(SEP).enumerate() {
        if k > 0 && !line.is_empty() && line.width() + SEP.width() + cell.width() > room {
            // 折在 ` · ` 的空格后面：上一行留着那个空格，下一行从 `· ` 起。
            line.push(' ');
            let joined = !out.is_empty();
            out.push((std::mem::take(&mut line), joined));
        }
        let lead = match (k, line.is_empty()) {
            (0, _) => "",
            (_, true) => "· ",
            (_, false) => SEP,
        };
        let piece = format!("{lead}{cell}");
        if line.is_empty() && piece.width() > room {
            line = clip(&piece, width.max(1));
        } else {
            line.push_str(&piece);
        }
    }
    let joined = !out.is_empty();
    out.push((line, joined));
    out
}

#[cfg(test)]
mod tests {
    use super::{line, mark, pieces};
    use crate::config::Config;
    use crate::core::Level;

    #[test]
    fn the_done_line_starts_with_its_level_icon() {
        let layout = Config::builtin().unwrap().layout;
        assert_eq!(mark(Some(Level::ReadOnly), &layout), "⏸ ");
        assert_eq!(mark(Some(Level::Workspace), &layout), "▣ ");
        assert_eq!(mark(None, &layout), "✻ ", "没记着级别的兜底");
        // 只有 `▣` 后面空两格，别的空一格。
        let line = |level| line(level, "03:44", &layout);
        assert_eq!(line(Some(Level::Workspace)), "▣  03:44");
        assert_eq!(line(Some(Level::Full)), "⏵⏵ 03:44");
        assert_eq!(line(Some(Level::ReadOnly)), "⏸ 03:44");
        assert_eq!(line(None), "✻ 03:44");
    }

    #[test]
    fn a_narrow_done_line_breaks_only_between_segments() {
        // 2026-09-29 28 列实测：照字折把模型名从中间劈开。
        let text = "▣  20:39 · dev/deepseek-v4.1-flash · 2.8s · 3.8k(C83%)";
        let got = pieces(text, 25);
        let lines: Vec<&str> = got.iter().map(|(l, _)| l.trim_end()).collect();
        assert_eq!(
            lines,
            [
                "▣  20:39",
                "· dev/deepseek-v4.1-flash",
                "· 2.8s · 3.8k(C83%)"
            ]
        );
        assert_eq!(
            got.iter().map(|(_, j)| *j).collect::<Vec<_>>(),
            [false, true, true]
        );
        let copied: String = got.iter().map(|(l, _)| l.as_str()).collect();
        assert_eq!(copied, text, "复制时接回去和原文一样");
        assert_eq!(
            pieces(text, 80),
            [(text.to_string(), false)],
            "放得下：一行"
        );
        // 一格比一行还宽：截掉加 …。
        let narrow = pieces(text, 12);
        assert!(
            narrow
                .iter()
                .all(|(l, _)| unicode_width::UnicodeWidthStr::width(l.trim_end()) <= 12),
            "{narrow:?}"
        );
        assert!(
            narrow.iter().any(|(l, _)| l.trim_end().ends_with('…')),
            "{narrow:?}"
        );
    }
}
