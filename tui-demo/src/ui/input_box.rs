//! 输入框这一格：圆角框里画输入的字、提示符、暂存标记，编辑上一句时上边框写一句，切进子会话时上边框右边写它是谁；抽屉开着时框里画抽屉（蓝图 `tui.md`「输入框」、
//! 「确认和提问的抽屉」第 2 条）。

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType, Paragraph};
use unicode_width::UnicodeWidthStr;

use super::{Areas, drawer};
use crate::app::App;
use crate::config::Config;
use crate::core::Level;
use crate::input::{InputBox, VisualLine};
use crate::theme;

/// 输入框：抽屉开着时框里画抽屉，别的时候画输入的字。
pub(super) fn draw_box(frame: &mut Frame, areas: Areas, app: &mut App, home: bool) {
    let texts = &app.config.text.drawer;
    if let Some(d) = app.drawers.current.as_mut() {
        frame.render_widget(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(theme::dim()),
            areas.frame,
        );
        app.drawer_rows = drawer::draw(frame, areas.text, d, texts);
        return;
    }
    app.drawer_rows.clear();
    let child = app.viewing_title();
    // 焦点移到框下面（按钮、子代理状态行）、后台面板开着时，框里不放光标：打的字不进这里（2026-09-30 项目主人）。
    let typing = app.focus() == crate::focus::Focus::Input && app.panel.is_none();
    let look = Look {
        level: app.transcript.level,
        placeholder: placeholder(&app.config, home, app.tips.at()),
        child,
        typing,
    };
    draw_input(frame, areas, &mut app.input, &app.config, look);
}

/// 框里框外要照的几样。
struct Look<'a> {
    /// 权限级别：提示符的颜色。
    level: Level,
    /// 空着时写的提示。
    placeholder: &'a str,
    /// 切进了哪个子代理的会话：上边框右边写它（「切进子会话」第 2 条）。
    child: Option<String>,
    /// 焦点在输入框、没开后台面板：放光标。
    typing: bool,
}

fn draw_input(frame: &mut Frame, areas: Areas, input: &mut InputBox, config: &Config, look: Look) {
    let Look {
        level,
        placeholder,
        child,
        typing,
    } = look;
    let mut border = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme::dim());
    // 编辑上一句：上边框写一句，框线的颜色（「输入框」第 13 条）。
    if input.editing() {
        border = border.title(Line::styled(config.text.editing.as_str(), theme::dim()));
    }
    // 切进了子会话：上边框右边写它是谁（「切进子会话」第 2 条，2026-09-30 项目主人照 Claude Code 定）。
    if let Some(title) = child {
        let tag = config.text.jobs.child_tag.replace("{title}", &title);
        border = border.title_top(Line::styled(tag, theme::dim()).right_aligned());
    }
    frame.render_widget(border, areas.frame);
    let scroll = input.place(areas.text);
    let editor = &input.editor;
    let blocks = editor.blocks();
    let body = if editor.is_empty() {
        vec![Line::styled(placeholder, theme::dim())]
    } else {
        let lines = input.lines(areas.text.width);
        lines
            .iter()
            .skip(scroll)
            .take(usize::from(areas.text.height))
            .map(|l| {
                let hover = input.hovered_attachment();
                styled_line(editor.text(), *l, editor.selection(), &blocks, hover)
            })
            .collect()
    };
    frame.render_widget(Paragraph::new(body), areas.text);
    // 暂存着东西：第一行最右边暗色写一个标记；和第一行的字挤不下就不写。
    if input.stashed() {
        let mark = config.text.stashed.as_str();
        let first = if editor.is_empty() {
            placeholder.width()
        } else {
            input
                .lines(areas.text.width)
                .get(scroll)
                .map_or(0, |l| editor.text()[l.start..l.end].width())
        };
        // 标记和字之间至少空一列。
        if first + mark.width() < usize::from(areas.text.width) {
            let row = Rect::new(areas.text.x, areas.text.y, areas.text.width, 1);
            frame.render_widget(
                Paragraph::new(Line::styled(mark, theme::dim()).right_aligned()),
                row,
            );
        }
    }
    // 提示符画在第一行文字的左边，住在 pad_left 那几列里，颜色跟着权限级别；放不下就不画。
    // 暂存着东西换成这一套图标的 `stash`（`tui.md`「输入框」第 6 条、「图标」）。
    let prompt = if input.stashed() {
        config.icons.stash.as_str()
    } else {
        config.layout.prompt.as_str()
    };
    let prompt_style = theme::level(level);
    let width = u16::try_from(prompt.width()).unwrap_or(u16::MAX);
    // 框里左边那几列（边线和字之间）放得下提示符才画；紧凑版面正好放下（`tui.md`「输入框」第 10 条）。
    let room = areas.text.x.saturating_sub(areas.frame.x + 1);
    if scroll == 0 && room >= width {
        let at = Rect::new(areas.text.x - width, areas.text.y, width, 1);
        frame.render_widget(Paragraph::new(Line::styled(prompt, prompt_style)), at);
    }
    if let Some(pos) = input.cursor_position().filter(|_| typing) {
        frame.set_cursor_position(pos);
    }
}

/// 输入框空着时写的提示（`tui.md`「输入框」第 9 条）：首页固定写怎么切权限级别，别处照轮到的那一条。
pub(super) fn placeholder(config: &Config, home: bool, tip: usize) -> &str {
    if home {
        return &config.text.home_placeholder;
    }
    config.text.tips.get(tip).map_or("", String::as_str)
}

/// 一行字，选中的那一段反色；鼠标悬停着的附件块（`hover`）加下划线（「输入框」第 12 条）。
fn styled_line<'a>(
    text: &'a str,
    line: VisualLine,
    selection: Option<(usize, usize)>,
    blocks: &[(usize, usize)],
    hover: Option<(usize, usize)>,
) -> Line<'a> {
    use ratatui::text::Span;
    // 这一行里样子变的地方：选区、粘贴块的两头。
    let mut cuts = vec![line.start, line.end];
    for &(s, e) in selection.iter().chain(blocks) {
        cuts.extend([s, e].map(|at| at.clamp(line.start, line.end)));
    }
    cuts.sort_unstable();
    cuts.dedup();
    let inside =
        |range: Option<&(usize, usize)>, at: usize| range.is_some_and(|&(s, e)| s <= at && at < e);
    let spans = cuts
        .windows(2)
        .map(|w| {
            let style = if inside(selection.as_ref(), w[0]) {
                theme::selected()
            } else if let Some(b) = blocks.iter().find(|b| inside(Some(b), w[0])) {
                let chip = theme::chip();
                if hover == Some(*b) {
                    chip.add_modifier(ratatui::style::Modifier::UNDERLINED)
                } else {
                    chip
                }
            } else {
                ratatui::style::Style::new()
            };
            Span::styled(&text[w[0]..w[1]], style)
        })
        .collect::<Vec<_>>();
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::styled_line;
    use crate::input::VisualLine;
    use crate::theme;

    #[test]
    fn a_paste_block_is_a_chip_and_the_selection_wins() {
        // 输入框里的粘贴块品红（`tui.md`「输入框」第 11 条）；选中的照旧反色。
        let text = "看[已粘贴 12 行]吗";
        let line = VisualLine {
            start: 0,
            end: text.len(),
        };
        let block = ("看".len(), text.len() - "吗".len());
        let got = styled_line(text, line, None, &[block], None);
        let label = got
            .spans
            .iter()
            .find(|s| s.content == "[已粘贴 12 行]")
            .unwrap();
        assert_eq!(label.style, theme::chip(), "品红字、暗紫底");
        assert!(theme::chip().bg.is_some());
        let all: String = got.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(all, text);
        let chosen = styled_line(text, line, Some((0, text.len())), &[block], None);
        assert!(
            chosen
                .spans
                .iter()
                .all(|s| s.content.is_empty() || s.style == theme::selected())
        );
    }
}
