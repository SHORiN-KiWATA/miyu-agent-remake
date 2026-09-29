//! 差异排成行的内容（蓝图 `tui.md`「编辑、写入点开的差异」）：`行号  标记 内容`。
//!
//! 行号右对齐，宽度照最大的行号；删掉的红字暗红底，加上的绿字暗青底，底色只铺到字的末尾；不变的、行号暗。

use ratatui::text::Span;

use super::rows::clip;
use crate::diff::{Diff, Mark};
use crate::theme;

/// 一份差异排成的每一行：行号那一格、标记加内容。`width` 是一行能占几列，太长的截掉，不折行。
pub fn lines(diff: &Diff, width: u16) -> Vec<(Span<'static>, Span<'static>)> {
    let digits = diff
        .lines
        .iter()
        .filter_map(|l| l.number)
        .max()
        .map_or(1, |n| n.to_string().len());
    let room = usize::from(width).saturating_sub(digits + 2);
    diff.lines
        .iter()
        .map(|line| {
            let number = line.number.map_or_else(String::new, |n| n.to_string());
            let gutter = Span::styled(format!("{number:>digits$}  "), theme::dim());
            let (mark, style) = match line.mark {
                Mark::Removed => ("-", theme::diff_removed()),
                Mark::Added => ("+", theme::diff_added()),
                Mark::Keep => (" ", theme::dim()),
                Mark::Gap => return (gutter, Span::styled("⋯", theme::dim())),
            };
            let room = u16::try_from(room).unwrap_or(u16::MAX);
            let text = clip(&format!("{mark} {}", line.text), room);
            (gutter, Span::styled(text, style))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::lines;
    use crate::diff::compare;

    fn shown(before: &str, after: &str) -> Vec<String> {
        lines(&compare(before, after), 40)
            .into_iter()
            .map(|(gutter, content)| format!("{}{}", gutter.content, content.content))
            .collect()
    }

    #[test]
    fn numbers_line_up_and_marks_follow() {
        let before: String = (1..=12).map(|n| format!("l{n}\n")).collect();
        let after = before.replace("l10\n", "ten\n");
        assert_eq!(
            shown(&before, &after),
            vec![
                " 7    l7",
                " 8    l8",
                " 9    l9",
                "10  - l10",
                "10  + ten",
                "11    l11",
                "12    l12"
            ]
        );
    }
}
