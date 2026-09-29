//! 输入框这一格：圆角框里画输入的字、提示符、暂存标记；抽屉开着时框里画抽屉（蓝图 `tui.md`「输入框」、
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
    draw_input(
        frame,
        areas,
        &mut app.input,
        &app.config,
        app.transcript.level,
        placeholder(&app.config, home, app.tips.at()),
    );
}

fn draw_input(
    frame: &mut Frame,
    areas: Areas,
    input: &mut InputBox,
    config: &Config,
    level: Level,
    placeholder: &str,
) {
    frame.render_widget(
        Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(theme::dim()),
        areas.frame,
    );
    let scroll = input.place(areas.text);
    let editor = &input.editor;
    let body = if editor.is_empty() {
        vec![Line::styled(placeholder, theme::dim())]
    } else {
        let lines = input.lines(areas.text.width);
        lines
            .iter()
            .skip(scroll)
            .take(usize::from(areas.text.height))
            .map(|l| styled_line(editor.text(), *l, editor.selection()))
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
    // 暂存着东西换成软盘（`tui.md`「输入框」第 6 条）。
    let prompt = if input.stashed() {
        config.layout.stash_prompt.as_str()
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
    if let Some(pos) = input.cursor_position() {
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

/// 一行字，选中的那一段反色。
fn styled_line(text: &str, line: VisualLine, selection: Option<(usize, usize)>) -> Line<'_> {
    use ratatui::text::Span;
    let Some((s, e)) = selection else {
        return Line::raw(&text[line.start..line.end]);
    };
    let s = s.clamp(line.start, line.end);
    let e = e.clamp(line.start, line.end);
    Line::from(vec![
        Span::raw(&text[line.start..s]),
        Span::styled(&text[s..e], theme::selected()),
        Span::raw(&text[e..line.end]),
    ])
}
