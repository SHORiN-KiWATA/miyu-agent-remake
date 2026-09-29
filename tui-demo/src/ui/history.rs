//! 输入历史列表（蓝图 `tui.md`「输入历史列表」）：第一行「历史：」加打的字，下面对得上的几条，
//! 最新的贴着输入框，越早越往上；选中的停在正中间，到头才往边上走（照命令列表的 [`window`]）。
//! `Tab` 展开着时，选中的那一条写全文。
//!
//! 列表排成哪几行、每一行是哪一条，都由 [`lines`] 定：占几行、画什么、鼠标点的是哪一条，照同一份。

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::rows::clip;
use crate::config::Config;
use crate::history::History;
use crate::input::pieces;
use crate::menu::window;
use crate::theme;

/// 排好的一行：是对得上的第几条（「历史：」那一行、「没有对得上的」是 `None`；展开的一条连「还有几行」都算它），
/// 和画出来的样子。
pub type Row = (Option<usize>, Line<'static>);

/// 列表从上往下的每一行。`matches` 是对得上的几条，最新的在前；`width` 是能写几列。
pub fn lines(history: &History, matches: &[&str], width: u16, config: &Config) -> Vec<Row> {
    let text = &config.text;
    let mut out = vec![(
        None,
        Line::from(vec![
            Span::styled(text.history_search.clone(), theme::dim()),
            Span::raw(history.query.clone()),
        ]),
    )];
    if matches.is_empty() {
        let empty = Span::styled(text.history_empty.clone(), theme::dim());
        out.push((None, Line::from(empty)));
        return out;
    }
    let rows = config.layout.history_rows.max(1);
    let top = window(history.selected, matches.len(), rows);
    let end = (top + rows).min(matches.len());
    for i in (top..end).rev() {
        let selected = i == history.selected;
        if selected && history.expanded {
            out.extend(full(i, matches[i], width, config));
            continue;
        }
        let one_line = matches[i].split_whitespace().collect::<Vec<_>>().join(" ");
        let style = if selected {
            theme::picked()
        } else {
            Style::new()
        };
        out.push((
            Some(i),
            Line::from(Span::styled(clip(&one_line, width), style)),
        ));
    }
    out
}

/// 展开的一条：原来的换行照留，太长的折行；最多 `history_preview_rows` 行，再长的最后一行写还有几行。
fn full(index: usize, text: &str, width: u16, config: &Config) -> Vec<Row> {
    let wrapped = pieces(text.trim_end(), width);
    let cap = config.layout.history_preview_rows.max(2);
    let shown = if wrapped.len() > cap {
        cap - 1
    } else {
        wrapped.len()
    };
    let mut out: Vec<Row> = wrapped
        .iter()
        .take(shown)
        .map(|(piece, _)| {
            let line = Line::from(Span::styled(piece.clone(), theme::picked()));
            (Some(index), line)
        })
        .collect();
    let more = wrapped.len() - shown;
    if more > 0 {
        let note = config
            .text
            .history_more
            .replace("{count}", &more.to_string());
        out.push((Some(index), Line::from(Span::styled(note, theme::dim()))));
    }
    out
}

/// 画列表。
pub fn draw(frame: &mut Frame, area: Rect, rows: Vec<Row>) {
    let lines: Vec<Line> = rows.into_iter().map(|(_, line)| line).collect();
    frame.render_widget(Paragraph::new(lines), area);
}

/// 屏幕上第 `y` 行是对得上的第几条；「历史：」那一行、空着的地方是 `None`。
pub fn index_at(area: Rect, rows: &[Row], y: u16) -> Option<usize> {
    rows.get(usize::from(y.checked_sub(area.y)?))?.0
}

#[cfg(test)]
mod tests {
    use ratatui::layout::Rect;

    use super::{index_at, lines};
    use crate::config::Config;
    use crate::history::History;

    fn plain(rows: &[super::Row]) -> Vec<String> {
        rows.iter().map(|(_, l)| l.to_string()).collect()
    }

    #[test]
    fn the_bottom_row_is_the_newest_and_clicks_find_their_entry() {
        let config = Config::builtin().unwrap();
        let history = History::default();
        let rows = lines(&history, &["新的", "中间", "早的"], 40, &config);
        assert_eq!(plain(&rows), ["历史：", "早的", "中间", "新的"]);
        let area = Rect::new(0, 10, 40, 4);
        assert_eq!(index_at(area, &rows, 13), Some(0));
        assert_eq!(index_at(area, &rows, 11), Some(2));
        assert_eq!(index_at(area, &rows, 10), None);
    }

    #[test]
    fn tab_shows_the_selected_one_in_full_and_caps_long_ones() {
        let config = Config::builtin().unwrap();
        let mut history = History::default();
        history.toggle_full();
        let rows = lines(&history, &["第一行\n第二行", "早的"], 40, &config);
        assert_eq!(plain(&rows), ["历史：", "早的", "第一行", "第二行"]);
        // 展开的两行都算这一条：点哪一行都是它。
        assert_eq!(rows[2].0, Some(0));
        assert_eq!(rows[3].0, Some(0));
        // 太长的：最多 history_preview_rows 行，最后一行写还有几行。
        let long: String = (1..=20).map(|n| format!("第 {n} 行\n")).collect();
        let rows = lines(&history, &[long.as_str()], 40, &config);
        let cap = config.layout.history_preview_rows;
        assert_eq!(rows.len(), 1 + cap);
        assert_eq!(
            rows.last().unwrap().1.to_string(),
            format!("⋮ 还有 {} 行", 20 - (cap - 1))
        );
    }
}
