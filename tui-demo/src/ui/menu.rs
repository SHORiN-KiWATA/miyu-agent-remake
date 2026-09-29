//! 斜杠命令列表：贴在输入框上面，左边和框里的字对齐。不加竖线、不写按键说明（`13-终端界面.md` 第十节）。

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthStr;

use crate::commands::Spec;
use crate::menu::window;
use crate::theme;

/// 画露出来的那一段。名字对齐成一列，样子见 [`looks`]。
pub fn draw(frame: &mut Frame, area: Rect, matches: &[Spec], selected: usize) {
    let rows = usize::from(area.height);
    let top = window(selected, matches.len(), rows);
    let column = matches.iter().map(|s| s.name.width()).max().unwrap_or(0) + 3;
    let lines: Vec<Line> = matches
        .iter()
        .enumerate()
        .skip(top)
        .take(rows)
        .map(|(i, spec)| {
            let name = format!("/{}", spec.name);
            let pad = " ".repeat(column.saturating_sub(name.width()));
            let (name_style, summary_style) = looks(i == selected);
            Line::from(vec![
                Span::styled(name, name_style),
                Span::raw(pad),
                Span::styled(spec.summary.as_str(), summary_style),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), area);
}

/// 一条的名字、说明的样子：名字原色、说明暗；选中的名字品红加粗、说明原色（`tui.md`「样子」）。
fn looks(selected: bool) -> (Style, Style) {
    if selected {
        (theme::picked(), Style::new())
    } else {
        (Style::new(), theme::dim())
    }
}

#[cfg(test)]
mod tests {
    use ratatui::style::Style;

    use super::looks;
    use crate::theme;

    #[test]
    fn names_stand_out_and_summaries_recede() {
        assert_eq!(looks(false), (Style::new(), theme::dim()));
        assert_eq!(looks(true), (theme::picked(), Style::new()));
    }
}
