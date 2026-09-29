//! 斜杠命令列表（蓝图 `tui.md`「斜杠命令列表」第 3 条）：贴在输入框上面，和输入历史列表、后台面板一个框（`panel.rs`）：
//! 标题嵌在上边框，一条一行，不写按键说明（每打一个 `/` 都弹，不要太高）。

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use super::panel::{self, Chrome};
use crate::commands::Spec;
use crate::config::MenuTexts;
use crate::menu::window;
use crate::theme;

/// 列表的框（上边框写标题、条数）和露出来的那一段，最多 `rows` 条。名字对齐成一列，样子见 [`looks`]。
pub fn lines(
    matches: &[Spec],
    selected: usize,
    rows: usize,
    width: u16,
    words: &MenuTexts,
) -> (Chrome, Vec<Line<'static>>) {
    let count = words.count.replace("{count}", &matches.len().to_string());
    let chrome = Chrome::new(&words.title, vec![Span::styled(count, theme::dim())]);
    let mut out = Vec::new();
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
            content.push(Span::styled(aliases, theme::faint()));
        }
        content.push(Span::raw(pad));
        content.push(Span::styled(spec.summary.clone(), summary_style));
        out.push(panel::item(picked, content, None, width));
    }
    (chrome, out)
}

/// 露几条：配置的条数，放不下时照框里剩下的行数（`room`），至少一条（「窗口小的时候」第 1 条）。
pub fn rows(configured: usize, room: u16) -> usize {
    configured.min(usize::from(room)).max(1)
}

/// 名字那一列写的：`/名字`，有别名的跟上 ` (别名)`（2026-09-29 项目主人：原来在说明里写「也可以打」）。
fn label(spec: &Spec) -> String {
    if spec.aliases.is_empty() {
        format!("/{}", spec.name)
    } else {
        format!("/{} ({})", spec.name, spec.aliases.join(", "))
    }
}

/// 画列表：`outer` 连框，`text` 是框里放字的那一块。
pub fn draw(frame: &mut Frame, outer: Rect, text: Rect, chrome: Chrome, lines: Vec<Line<'static>>) {
    panel::draw(frame, outer, text, chrome, lines);
}

/// 屏幕上第 `y` 行点中的是第几条；`text` 是框里放字的那一块，框的边、空着的地方是 `None`。
pub fn index_at(text: Rect, count: usize, selected: usize, rows: usize, y: u16) -> Option<usize> {
    let row = usize::from(y.checked_sub(text.y)?);
    let index = window(selected, count, rows) + row;
    (row < rows && index < count).then_some(index)
}

/// 一条的名字、说明的样子：名字暗、说明 `faint`；选中的名字加粗，颜色由选中的那一条的底色定（`tui.md`
/// 「斜杠命令列表」第 3 条，`panel::item`）。
fn looks(selected: bool) -> (Style, Style) {
    if selected {
        (Style::new().add_modifier(Modifier::BOLD), Style::new())
    } else {
        (theme::dim(), theme::faint())
    }
}

#[cfg(test)]
mod tests {
    use ratatui::layout::Rect;
    use ratatui::style::{Modifier, Style};

    use super::{index_at, lines, looks};
    use crate::config::Config;
    use crate::theme;

    fn lines_all(
        matches: &[crate::commands::Spec],
        config: &Config,
    ) -> Vec<ratatui::text::Line<'static>> {
        lines(matches, usize::MAX, matches.len(), 80, &config.text.menu).1
    }

    #[test]
    fn names_are_dim_and_summaries_fainter_the_picked_name_is_bold() {
        // 2026-09-30 项目主人看过对比页定的 A：没选中的整体暗一档；选中的颜色由那一条的底色定。
        assert_eq!(looks(false), (theme::dim(), theme::faint()));
        assert_eq!(
            looks(true),
            (Style::new().add_modifier(Modifier::BOLD), Style::new())
        );
    }

    #[test]
    fn a_titled_frame_then_one_row_each_with_the_picked_on_a_blue_bar() {
        // 标题嵌在上边框，框里一条一行，不空行、不写按键提示（每打一个 `/` 都弹，不要太高）。
        let _theme = theme::hold();
        let config = Config::builtin().unwrap();
        let matches: Vec<_> = config.commands.filter("").into_iter().cloned().collect();
        let rows = config.layout.menu_rows;
        let (chrome, lines) = lines(&matches, 1, rows, 60, &config.text.menu);
        let title: String = chrome.title.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(title, format!("命令 {} 条", matches.len()));
        assert!(chrome.hint.is_none(), "没有按键提示");
        let text: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
        assert_eq!(lines.len(), rows, "框里最多露 {rows} 条");
        assert!(text[0].starts_with("  /"));
        assert!(text[1].starts_with("❯ /"), "{}", text[1]);
        assert_eq!(
            lines[1].style,
            theme::picked_bar(),
            "选中的铺强调色的底、字深色"
        );
        // 别名 `faint` 跟在后面的括号里，说明里不再写（2026-09-29 项目主人）。
        let undo = matches.iter().position(|s| s.name == "undo").unwrap();
        let all = lines_all(&matches, &config);
        let row = &all[undo];
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
        assert_eq!(alias.style, theme::faint());
        // 框里放字的那一块从第 11 行起：点框的上边不算。
        let text_area = Rect::new(2, 11, 56, 5);
        assert_eq!(
            index_at(text_area, matches.len(), 1, rows, 10),
            None,
            "点框的边不算"
        );
        assert_eq!(index_at(text_area, matches.len(), 1, rows, 12), Some(1));
    }
}
