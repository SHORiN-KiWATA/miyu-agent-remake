//! 「最近 30 天」那一页（蓝图「配置与模型」第 5 条 `/usage`）：上面一天一列的柱状图（`▁▂▃▄▅▆▇█`，用得最多的那天
//! 强调色），下面一行写头一天、今天的日期和最高那天的数，再下面是按天的表。

use jiff::ToSpan;
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use super::tables;
use crate::app::{App, UsageData};
use crate::core::UsageRow;
use crate::{meter, theme};

/// 一格里八分之一到八分之八高。
const EIGHTHS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// 柱状图加按天的表。
pub(super) fn recent(
    app: &App,
    data: &UsageData,
    rows: &[UsageRow],
    width: u16,
) -> Vec<Line<'static>> {
    let look = &app.config.layout.usage;
    let days = crate::usage::days(rows, &app.currency);
    // 放得下几天画几天，一天占两格（柱子和一格空）。
    let count = usize::from(look.recent_days)
        .min(usize::from(width) / 2)
        .max(1);
    let first = data.today - i64::try_from(count - 1).unwrap_or(0).days();
    let values: Vec<u64> = (0..count)
        .map(|i| {
            let date = first + i64::try_from(i).unwrap_or(0).days();
            days.iter().find(|d| d.date == date).map_or(0, |d| d.tokens)
        })
        .collect();
    let mut out = bars(&values, usize::from(look.bar_rows).max(1));
    // 下面一行：头一天、今天的日期，最高那天的数接在今天后面。
    let texts = &app.config.text.usage;
    let left = first.strftime("%m-%d").to_string();
    let right = data.today.strftime("%m-%d").to_string();
    let top = values.iter().copied().max().unwrap_or(0);
    let peak = texts.recent_peak.replace("{tokens}", &meter::short(top));
    let span = count * 2 - 1;
    let gap = span.saturating_sub(left.width() + right.width());
    out.push(Line::from(vec![
        Span::styled(format!("{left}{}{right}", " ".repeat(gap)), theme::dim()),
        Span::styled(format!("   {peak}"), theme::dim()),
    ]));
    out.push(Line::default());
    out.extend(tables::days(app, data, rows, width));
    out
}

/// 柱状图排成 `height` 行，从上往下；每天一列，柱子之间空一格。
fn bars(values: &[u64], height: usize) -> Vec<Line<'static>> {
    let top = values.iter().copied().max().unwrap_or(0).max(1);
    (0..height)
        .rev()
        .map(|row| {
            let mut spans = Vec::new();
            for (i, &v) in values.iter().enumerate() {
                if i > 0 {
                    spans.push(Span::raw(" "));
                }
                // 这一列有几个八分之一高。
                let eighths = (v as f64 / top as f64 * (height * 8) as f64).round() as usize;
                let eighths = if v > 0 { eighths.max(1) } else { 0 };
                let here = eighths.saturating_sub(row * 8).min(8);
                let style = if v == top {
                    theme::accent()
                } else {
                    theme::heat(3)
                };
                let cell = if here == 0 { ' ' } else { EIGHTHS[here - 1] };
                spans.push(Span::styled(cell.to_string(), style));
            }
            Line::from(spans)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::bars;

    fn text(line: &ratatui::text::Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn bars_are_drawn_to_scale_in_eighths() {
        let got: Vec<String> = bars(&[0, 4, 8, 1], 2).iter().map(text).collect();
        assert_eq!(
            got,
            ["    █  ", "  █ █ ▂"],
            "最高的满格，一半高的满一格，小的照比例"
        );
    }
}
