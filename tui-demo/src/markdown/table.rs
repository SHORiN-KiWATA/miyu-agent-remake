//! 表格（蓝图 `tui.md`「她的回答：Markdown」第 8 条，规则照旧版 `render/table.rs`）：实线画满格子，
//! 每两行之间一条横线；列宽照显示宽度，放不下时收最宽的列，保底宽度照旧版；格子里的字按列宽折行。

use pulldown_cmark::Alignment;
use ratatui::text::Span;
use unicode_width::UnicodeWidthStr;

use super::inline::{Folded, Piece, fold, plain};
use crate::theme;

/// 一格：片段。
pub type Cell = Vec<Piece>;

/// 画一张表，交回折好的行。`width` 是整张表最多几列宽。
pub fn render(head: &[Cell], rows: &[Vec<Cell>], aligns: &[Alignment], width: u16) -> Vec<Folded> {
    let columns = head.len().max(rows.iter().map(Vec::len).max().unwrap_or(0));
    if columns == 0 {
        return Vec::new();
    }
    let natural: Vec<usize> = (0..columns)
        .map(|c| {
            std::iter::once(head)
                .chain(rows.iter().map(Vec::as_slice))
                .filter_map(|row| row.get(c))
                .map(cell_width)
                .max()
                .unwrap_or(1)
                .max(1)
        })
        .collect();
    let widths = bounded(natural, usize::from(width));
    let rule = |left: &str, mid: &str, right: &str| {
        let body: Vec<String> = widths.iter().map(|w| "─".repeat(w + 2)).collect();
        line(vec![Span::styled(
            format!("{left}{}{right}", body.join(mid)),
            theme::dim(),
        )])
    };
    let mut out = vec![rule("┌", "┬", "┐")];
    out.extend(row_lines(head, &widths, aligns, true));
    for row in rows {
        out.push(rule("├", "┼", "┤"));
        out.extend(row_lines(row, &widths, aligns, false));
    }
    out.push(rule("└", "┴", "┘"));
    out
}

/// 放不下时，每次把最宽的一列收窄一格，收到保底宽度为止（照旧版的 `bounded_table_widths`）。
fn bounded(mut widths: Vec<usize>, room: usize) -> Vec<usize> {
    // 每列左右各空一格、一条竖线，最右边再一条。
    let frame = widths.len() * 3 + 1;
    let floor = match widths.len() {
        1 => 16,
        2 => 14,
        3 | 4 => 10,
        _ => 8,
    };
    while widths.iter().sum::<usize>() + frame > room {
        let Some((i, w)) = widths.iter().copied().enumerate().max_by_key(|(_, w)| *w) else {
            break;
        };
        if w <= floor {
            break;
        }
        widths[i] -= 1;
    }
    widths
}

fn cell_width(cell: &Cell) -> usize {
    let text: String = cell.iter().map(|p| p.text.as_str()).collect();
    text.split('\n')
        .map(UnicodeWidthStr::width)
        .max()
        .unwrap_or(0)
}

/// 一行格子：每格按列宽折好，行高照最高的那格，矮的补空。
fn row_lines(row: &[Cell], widths: &[usize], aligns: &[Alignment], head: bool) -> Vec<Folded> {
    let empty = Cell::new();
    let cells: Vec<Vec<Folded>> = widths
        .iter()
        .enumerate()
        .map(|(c, w)| {
            let pieces: Vec<Piece> = row
                .get(c)
                .unwrap_or(&empty)
                .iter()
                .map(|p| {
                    if head {
                        Piece {
                            style: p.style.patch(theme::md_table_head()),
                            ..p.clone()
                        }
                    } else {
                        p.clone()
                    }
                })
                .collect();
            fold(&pieces, u16::try_from(*w).unwrap_or(u16::MAX))
        })
        .collect();
    let height = cells.iter().map(Vec::len).max().unwrap_or(1);
    (0..height)
        .map(|r| {
            let mut spans = Vec::new();
            let mut links = Vec::new();
            let mut col = 0u16;
            for (c, w) in widths.iter().enumerate() {
                let bar = if c == 0 { "│ " } else { " │ " };
                spans.push(Span::styled(bar, theme::dim()));
                col += u16::try_from(bar.width()).unwrap_or(0);
                let folded = cells[c].get(r);
                let used = folded.map_or(0, |f| plain(f).width());
                let gap = w.saturating_sub(used);
                let (before, after) = match aligns.get(c) {
                    Some(Alignment::Right) => (gap, 0),
                    Some(Alignment::Center) => (gap / 2, gap - gap / 2),
                    _ => (0, gap),
                };
                spans.push(Span::raw(" ".repeat(before)));
                col += u16::try_from(before).unwrap_or(0);
                if let Some(f) = folded {
                    links.extend(
                        f.links
                            .iter()
                            .map(|(a, b, u)| (a + col, b + col, u.clone())),
                    );
                    spans.extend(f.spans.iter().cloned());
                }
                spans.push(Span::raw(" ".repeat(after)));
                col += u16::try_from(used + after).unwrap_or(0);
            }
            spans.push(Span::styled(" │", theme::dim()));
            Folded {
                spans,
                links,
                joined: false,
            }
        })
        .collect()
}

fn line(spans: Vec<Span<'static>>) -> Folded {
    Folded {
        spans,
        links: Vec::new(),
        joined: false,
    }
}

#[cfg(test)]
mod tests {
    use pulldown_cmark::Alignment;
    use ratatui::style::Style;
    use unicode_width::UnicodeWidthStr;

    use super::{bounded, render};
    use crate::markdown::inline::{Piece, plain};

    fn cell(text: &str) -> Vec<Piece> {
        vec![Piece::new(text, Style::new())]
    }

    #[test]
    fn a_table_has_rules_between_every_row_and_aligns_cells() {
        let head = vec![cell("左"), cell("右")];
        let rows = vec![vec![cell("a"), cell("b")], vec![cell("长一点"), cell("c")]];
        let lines: Vec<String> = render(&head, &rows, &[Alignment::Left, Alignment::Right], 40)
            .iter()
            .map(plain)
            .collect();
        assert_eq!(
            lines,
            vec![
                "┌────────┬────┐",
                "│ 左     │ 右 │",
                "├────────┼────┤",
                "│ a      │  b │",
                "├────────┼────┤",
                "│ 长一点 │  c │",
                "└────────┴────┘",
            ]
        );
        assert!(
            lines.iter().all(|l| l.width() == lines[0].width()),
            "每行一样宽"
        );
    }

    #[test]
    fn too_wide_tables_shrink_the_widest_column_down_to_the_floor() {
        assert_eq!(bounded(vec![30, 5], 30), vec![18, 5]);
        assert_eq!(bounded(vec![30, 30], 20), vec![14, 14], "保底两列各 14");
    }
}
