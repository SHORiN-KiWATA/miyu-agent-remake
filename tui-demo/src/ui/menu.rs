//! 斜杠命令列表（蓝图 `tui.md`「斜杠命令列表」第 3 条）：贴在输入框上面，左边和框里的字对齐。样子和输入历史列表
//! 同一套记号（`panel.rs`），选了轻的：标题写在横线上，一条一行，不空行、不写按键说明（每打一个 `/` 都弹，不要太高）。

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthStr;

use super::panel;
use crate::commands::Spec;
use crate::config::MenuTexts;
use crate::menu::window;
use crate::theme;

/// 列表的每一行：标题横线，下面露出来的那一段，最多 `rows` 条。名字对齐成一列，样子见 [`looks`]。
pub fn lines(
    matches: &[Spec],
    selected: usize,
    rows: usize,
    width: u16,
    words: &MenuTexts,
) -> Vec<Line<'static>> {
    let count = words.count.replace("{count}", &matches.len().to_string());
    let mut out = vec![panel::rule(
        &words.title,
        vec![Span::styled(count, theme::dim())],
        width,
    )];
    let top = window(selected, matches.len(), rows);
    let column = matches.iter().map(|s| label(s).width()).max().unwrap_or(0) + 3;
    for (i, spec) in matches.iter().enumerate().skip(top).take(rows) {
        let picked = i == selected;
        let name = format!("/{}", spec.name);
        let pad = " ".repeat(column.saturating_sub(label(spec).width()));
        let (name_style, summary_style) = looks(picked);
        let mut content = vec![Span::styled(name, name_style)];
        if !spec.aliases.is_empty() {
            let aliases = format!(" ({})", spec.aliases.join(", "));
            content.push(Span::styled(aliases, theme::dim()));
        }
        content.push(Span::raw(pad));
        content.push(Span::styled(spec.summary.clone(), summary_style));
        out.push(panel::item(picked, content, None, width));
    }
    out
}

/// 露几条：配置的条数，放不下时照列表那块的高度减去标题那一行，至少一条（「窗口小的时候」第 1 条）。
pub fn rows(configured: usize, height: u16) -> usize {
    configured.min(usize::from(height).saturating_sub(1)).max(1)
}

/// 名字那一列写的：`/名字`，有别名的跟上 ` (别名)`（2026-09-29 项目主人：原来在说明里写「也可以打」）。
fn label(spec: &Spec) -> String {
    if spec.aliases.is_empty() {
        format!("/{}", spec.name)
    } else {
        format!("/{} ({})", spec.name, spec.aliases.join(", "))
    }
}

/// 画列表。
pub fn draw(frame: &mut Frame, area: Rect, lines: Vec<Line<'static>>) {
    frame.render_widget(Paragraph::new(lines), area);
}

/// 屏幕上第 `y` 行点中的是第几条；标题那一行、空着的地方是 `None`。
pub fn index_at(area: Rect, count: usize, selected: usize, rows: usize, y: u16) -> Option<usize> {
    let row = usize::from(y.checked_sub(area.y)?).checked_sub(1)?;
    let index = window(selected, count, rows) + row;
    (row < rows && index < count).then_some(index)
}

/// 一条的名字、说明的样子：名字强调色、说明暗；选中的名字品红加粗、说明原色（`tui.md`「斜杠命令列表」第 3 条）。
fn looks(selected: bool) -> (Style, Style) {
    if selected {
        (theme::picked(), Style::new())
    } else {
        (theme::accent(), theme::dim())
    }
}

#[cfg(test)]
mod tests {
    use ratatui::layout::Rect;
    use ratatui::style::Style;

    use super::{index_at, lines, looks};
    use crate::config::Config;
    use crate::theme;

    fn lines_all(
        matches: &[crate::commands::Spec],
        config: &Config,
    ) -> Vec<ratatui::text::Line<'static>> {
        lines(matches, usize::MAX, matches.len(), 80, &config.text.menu)
    }

    #[test]
    fn names_stand_out_and_summaries_recede() {
        assert_eq!(looks(false), (theme::accent(), theme::dim()));
        assert_eq!(looks(true), (theme::picked(), Style::new()));
    }

    #[test]
    fn a_titled_rule_then_one_row_each_with_the_selected_shaded() {
        // 2026-09-29 项目主人：和输入历史列表同一套记号，选了轻的：标题写在横线上，不空行、不写按键提示。
        let _theme = theme::hold();
        let config = Config::builtin().unwrap();
        let matches: Vec<_> = config.commands.filter("").into_iter().cloned().collect();
        let rows = config.layout.menu_rows;
        let lines = lines(&matches, 1, rows, 60, &config.text.menu);
        let text: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
        let head = format!("── 命令 {} 条 ─", matches.len());
        assert!(text[0].starts_with(&head), "{}", text[0]);
        assert_eq!(lines[0].width(), 60, "横线铺满");
        assert_eq!(lines.len(), 1 + rows, "标题一行，下面最多露 {rows} 条");
        assert!(text[1].starts_with("  /"));
        assert!(text[2].starts_with("❯ /"), "{}", text[2]);
        assert_eq!(lines[2].style.bg, theme::shade().bg, "选中的整行铺底色");
        // 别名暗色跟在后面的括号里，说明里不再写（2026-09-29 项目主人）。
        let undo = matches.iter().position(|s| s.name == "undo").unwrap();
        let all = lines_all(&matches, &config);
        let row = &all[1 + undo];
        let text = row.to_string();
        assert!(
            text.contains("/undo (rewind)") && !text.contains("也可以打"),
            "{text}"
        );
        let alias = row
            .spans
            .iter()
            .find(|s| s.content.contains("(rewind)"))
            .unwrap();
        assert_eq!(alias.style, theme::dim());
        let area = Rect::new(0, 10, 60, 6);
        assert_eq!(
            index_at(area, matches.len(), 1, rows, 10),
            None,
            "点标题不算"
        );
        assert_eq!(index_at(area, matches.len(), 1, rows, 12), Some(1));
    }
}
