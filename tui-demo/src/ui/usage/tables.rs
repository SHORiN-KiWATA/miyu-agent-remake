//! 几张表（蓝图「配置与模型」第 5 条 `/usage`）：按天、按模型（带占比条，最下面一行用途）、按会话。几列对齐，数靠右，
//! 列之间空三格，放不下的截掉。

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use super::{GAP, amounts, tokens};
use crate::app::{App, UsageData};
use crate::core::UsageRow;
use crate::{meter, theme};

/// 表里的一格。
pub(super) struct Cell {
    /// 写的字。
    pub text: String,
    /// 颜色。
    pub style: Style,
    /// 靠右（数）。
    pub right: bool,
    /// 已经排好的几段（占比条这类一格里几种颜色的）；有的不看 `text`、`style`。
    pub spans: Option<Vec<Span<'static>>>,
}

impl Cell {
    fn left(text: String) -> Cell {
        Cell {
            text,
            style: Style::new(),
            right: false,
            spans: None,
        }
    }
    fn right(text: String) -> Cell {
        Cell {
            text,
            style: Style::new(),
            right: true,
            spans: None,
        }
    }
    fn dim(text: String) -> Cell {
        Cell {
            text,
            style: theme::dim(),
            right: false,
            spans: None,
        }
    }
    fn width(&self) -> usize {
        match &self.spans {
            Some(spans) => spans.iter().map(Span::width).sum(),
            None => self.text.width(),
        }
    }
}

/// 一行行排成对齐的几列：整列空着的不占地方，第一列（名字）最宽到一半、长了截掉加 `…`；最后一列不补空格，放不下的截掉。
pub(super) fn table(mut rows: Vec<Vec<Cell>>, width: u16) -> Vec<Line<'static>> {
    let half = u16::try_from(usize::from(width) / 2).unwrap_or(u16::MAX);
    for row in &mut rows {
        if let Some(name) = row.first_mut()
            && name.spans.is_none()
            && name.text.width() > usize::from(half)
        {
            name.text = crate::ui::rows::clip(&name.text, half);
        }
    }
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    let widths: Vec<usize> = (0..columns)
        .map(|i| {
            rows.iter()
                .filter_map(|r| r.get(i))
                .map(Cell::width)
                .max()
                .unwrap_or(0)
        })
        .collect();
    rows.into_iter()
        .map(|row| {
            let last = row.iter().rposition(|c| c.width() > 0).unwrap_or(0);
            let mut spans = Vec::new();
            for (i, cell) in row.into_iter().enumerate().take(last + 1) {
                if widths[i] == 0 {
                    continue;
                }
                if !spans.is_empty() {
                    spans.push(Span::raw(GAP));
                }
                let pad = " ".repeat(widths[i].saturating_sub(cell.width()));
                if cell.right {
                    spans.push(Span::raw(pad.clone()));
                }
                let right = cell.right;
                match cell.spans {
                    Some(parts) => spans.extend(parts),
                    None => spans.push(Span::styled(cell.text, cell.style)),
                }
                if !right && i < last {
                    spans.push(Span::raw(pad));
                }
            }
            clip(spans, usize::from(width))
        })
        .collect()
}

/// 一行截到 `width` 列：照显示的宽度一段段放，放不下的那段截掉。
fn clip(spans: Vec<Span<'static>>, width: usize) -> Line<'static> {
    let mut used = 0;
    let mut out = Vec::new();
    for span in spans {
        let w = span.width();
        if used + w <= width {
            used += w;
            out.push(span);
            continue;
        }
        let text = crate::ui::rows::clip(&span.content, u16::try_from(width - used).unwrap_or(0));
        out.push(Span::styled(text, span.style));
        break;
    }
    Line::from(out)
}

/// 次数、token、金额、缺价那几格（每张表的后半截一样）。
fn tail(app: &App, row: &UsageRow) -> Vec<Cell> {
    let texts = &app.config.text.usage;
    let unpriced = if row.bill.unpriced > 0 {
        texts
            .unpriced
            .replace("{n}", &row.bill.unpriced.to_string())
    } else {
        String::new()
    };
    vec![
        Cell::right(texts.requests.replace("{n}", &row.requests.to_string())),
        Cell::right(meter::short(row.tokens)),
        Cell::left(amounts(app, &row.bill).unwrap_or_default()),
        Cell::dim(unpriced),
    ]
}

/// 按天的表：今天在最上，今天那一行标「今天」，没用的日子不列。
pub(super) fn days(
    app: &App,
    data: &UsageData,
    rows: &[UsageRow],
    width: u16,
) -> Vec<Line<'static>> {
    let texts = &app.config.text.usage;
    let today = data.today.to_string();
    let rows: Vec<Vec<Cell>> = rows
        .iter()
        .rev()
        .filter(|r| r.day.is_some())
        .map(|r| {
            let day = r.day.clone().unwrap_or_default();
            let mark = if day == today {
                texts.today.clone()
            } else {
                String::new()
            };
            let mut row = vec![Cell::left(day), Cell::dim(mark)];
            row.extend(tail(app, r));
            row
        })
        .collect();
    table(rows, width)
}

/// 按模型：照 token 多少排，一个模型一行，名字后面接占比条和百分比；最下面一行用途占比。
pub(super) fn models(
    app: &App,
    data: &UsageData,
    rows: &[UsageRow],
    width: u16,
) -> Vec<Line<'static>> {
    let all = tokens(rows).max(1);
    let cells = usize::from(app.config.layout.usage.share_cells);
    let mut sorted: Vec<&UsageRow> = rows.iter().collect();
    sorted.sort_by_key(|r| std::cmp::Reverse(r.tokens));
    let table_rows: Vec<Vec<Cell>> = sorted
        .iter()
        .map(|r| {
            let share = r.tokens as f64 / all as f64;
            let mut row = vec![
                Cell::left(r.model.clone().unwrap_or_else(|| "—".to_string())),
                bar(share, cells),
                Cell::right(percent(share)),
            ];
            row.extend(tail(app, r));
            row
        })
        .collect();
    let mut out = table(table_rows, width);
    if let Some(Ok(purposes)) = &data.purposes
        && !purposes.is_empty()
    {
        out.push(Line::default());
        out.push(purpose_line(app, purposes, width));
    }
    out
}

/// 按会话：照 token 多少排，一个会话一行；标题照会话列表，没有的写短编号。
pub(super) fn sessions(app: &App, rows: &[UsageRow], width: u16) -> Vec<Line<'static>> {
    let mut sorted: Vec<&UsageRow> = rows.iter().collect();
    sorted.sort_by_key(|r| std::cmp::Reverse(r.tokens));
    let table_rows: Vec<Vec<Cell>> = sorted
        .iter()
        .map(|r| {
            let id = r.session.clone().unwrap_or_default();
            let mut row = vec![Cell::left(app.usage_name(&id))];
            row.extend(tail(app, r));
            row
        })
        .collect();
    table(table_rows, width)
}

/// 用途占比一行：「主对话 91% · 起标题 2% · 回顾 1%」，照 token 多少排。
fn purpose_line(app: &App, rows: &[UsageRow], width: u16) -> Line<'static> {
    let texts = &app.config.text.usage;
    let all = tokens(rows).max(1);
    let mut sorted: Vec<&UsageRow> = rows.iter().collect();
    sorted.sort_by_key(|r| std::cmp::Reverse(r.tokens));
    let mut spans = Vec::new();
    for (i, r) in sorted.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" · ", theme::dim()));
        }
        let name = match &r.purpose {
            None => texts.main_purpose.clone(),
            Some(p) => texts.purposes.get(p).cloned().unwrap_or_else(|| p.clone()),
        };
        spans.push(Span::raw(format!("{name} ")));
        spans.push(Span::styled(
            percent(r.tokens as f64 / all as f64),
            theme::picked(),
        ));
    }
    clip(spans, usize::from(width))
}

/// 占比条：用了的那几格强调色，剩下的最暗那档。
pub(super) fn bar(share: f64, cells: usize) -> Cell {
    let full = ((share * cells as f64).round() as usize).min(cells);
    let full = if share > 0.0 { full.max(1) } else { 0 };
    Cell {
        text: String::new(),
        style: Style::new(),
        right: false,
        spans: Some(vec![
            Span::styled("█".repeat(full), theme::accent()),
            Span::styled("█".repeat(cells - full), theme::heat(0)),
        ]),
    }
}

/// 百分比：整数，用了一点点的写 `<1%`。
pub(super) fn percent(share: f64) -> String {
    let p = (share * 100.0).round();
    if share > 0.0 && p < 1.0 {
        "<1%".to_string()
    } else {
        format!("{p:.0}%")
    }
}

#[cfg(test)]
mod tests {
    use super::{Cell, percent, table};

    fn text(line: &ratatui::text::Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn empty_columns_take_no_room_and_long_names_are_cut_at_half_the_width() {
        let rows = vec![
            vec![
                Cell::left("一个很长很长很长很长的会话标题".into()),
                Cell::right("4 次".into()),
                Cell::left(String::new()),
                Cell::dim("2 次没有价格".into()),
            ],
            vec![
                Cell::left("短".into()),
                Cell::right("12 次".into()),
                Cell::left(String::new()),
                Cell::dim(String::new()),
            ],
        ];
        let got: Vec<String> = table(rows, 50).iter().map(text).collect();
        assert_eq!(
            got[0], "一个很长很长很长很长的会…    4 次   2 次没有价格",
            "空的那一列不留间隔"
        );
        assert_eq!(got[1], "短                          12 次");
        assert_eq!(percent(0.004), "<1%");
        assert_eq!(percent(0.72), "72%");
    }
}
