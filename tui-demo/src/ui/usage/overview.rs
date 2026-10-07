//! 总览那一页（蓝图「配置与模型」第 5 条 `/usage`）：四个数排一行（累计、单日最高、连续使用、最长的会话），每个三行；
//! 下面是热度图：一周一列、一天一行，周一在最上，最右一列是这一周，五档颜色，顶上标月份，右下角图例。

use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use super::amounts;
use crate::app::{App, UsageData};
use crate::core::UsageRow;
use crate::usage::{self, Day};
use crate::{meter, theme};

/// 热度图左边写星期几的那一列有几格（名字两格加两格空）。
const LABEL: usize = 4;

/// 总览：四个数、空一行、热度图。
pub(super) fn page(
    app: &App,
    data: &UsageData,
    rows: &[UsageRow],
    by_money: bool,
    width: u16,
) -> Vec<Line<'static>> {
    let days = usage::days(rows, &app.currency);
    let mut out = stats(app, data, &days, width);
    out.push(Line::default());
    out.extend(heat(app, data, &days, by_money, width));
    out
}

/// 四个数，每个三行：暗色的名字、加粗的数、暗色的补充；一个占四分之一宽。
fn stats(app: &App, data: &UsageData, days: &[Day], width: u16) -> Vec<Line<'static>> {
    let texts = &app.config.text.usage;
    let missing = || "—".to_string();
    let total = match &data.total {
        Some(Ok(rows)) => rows.first(),
        _ => None,
    };
    let lifetime = (
        total.map_or_else(missing, |r| {
            format!("{} {}", meter::short(r.tokens), texts.by_tokens)
        }),
        total
            .and_then(|r| amounts(app, &r.bill))
            .unwrap_or_default(),
    );
    let peak = usage::peak(days).map_or_else(
        || (missing(), String::new()),
        |d| (meter::short(d.tokens), d.date.strftime("%m-%d").to_string()),
    );
    let (current, longest) = usage::streaks(days, data.today);
    let streak = (
        texts.streak_days.replace("{n}", &current.to_string()),
        texts.streak_best.replace("{n}", &longest.to_string()),
    );
    let sessions = match &data.sessions {
        Some(Ok(rows)) => rows.iter().max_by_key(|r| r.tokens),
        _ => None,
    };
    let longest_session = sessions.map_or_else(
        || (missing(), String::new()),
        |r| {
            let id = r.session.clone().unwrap_or_default();
            (meter::short(r.tokens), app.usage_name(&id))
        },
    );
    let cells = [
        (&texts.lifetime, lifetime),
        (&texts.peak, peak),
        (&texts.streak, streak),
        (&texts.longest, longest_session),
    ];
    let column = (usize::from(width) / cells.len()).max(1);
    let fit = |s: &str| {
        let s = crate::ui::rows::clip(s, u16::try_from(column.saturating_sub(2)).unwrap_or(0));
        let pad = column.saturating_sub(s.width());
        format!("{s}{}", " ".repeat(pad))
    };
    let mut lines = vec![Vec::new(), Vec::new(), Vec::new()];
    for (name, (value, more)) in cells {
        lines[0].push(Span::styled(fit(name), theme::dim()));
        lines[1].push(Span::styled(
            fit(&value),
            theme::model().add_modifier(Modifier::BOLD),
        ));
        lines[2].push(Span::styled(fit(&more), theme::dim()));
    }
    lines.into_iter().map(Line::from).collect()
}

/// 热度图：月份一行、七天七行、图例一行。照花费的那种币一笔都没有的照 token。
fn heat(
    app: &App,
    data: &UsageData,
    days: &[Day],
    by_money: bool,
    width: u16,
) -> Vec<Line<'static>> {
    let texts = &app.config.text.usage;
    let by_money = by_money && days.iter().any(|d| d.amount > 0.0);
    let weeks = usize::from(app.config.layout.usage.weeks)
        .min(usize::from(width).saturating_sub(LABEL) / 2)
        .max(1);
    let value = |d: &Day| if by_money { d.amount } else { d.tokens as f64 };
    let grid = usage::grid(days, data.today, weeks, value);
    let cuts = usage::thresholds(grid.weeks.iter().flatten().flatten().copied());
    let mut out = vec![months(texts, &grid.months, weeks)];
    for row in 0..7 {
        let name = texts.weekdays.get(row).cloned().unwrap_or_default();
        let mut spans = vec![Span::styled(
            format!("{name}{}", " ".repeat(LABEL.saturating_sub(name.width()))),
            theme::dim(),
        )];
        for column in &grid.weeks {
            spans.push(match column[row] {
                Some(v) => Span::styled("■ ", theme::heat(usage::level(v, &cuts))),
                None => Span::raw("  "),
            });
        }
        out.push(Line::from(spans));
    }
    out.push(legend(app, by_money, LABEL + weeks * 2));
    out
}

/// 月份那一行：每个月开头那一列写月份，挨得太近放不下的跳过。
fn months(texts: &super::Texts, marks: &[(usize, i8)], weeks: usize) -> Line<'static> {
    let mut line = " ".repeat(LABEL);
    let mut free = LABEL;
    for &(column, month) in marks {
        let at = LABEL + column * 2;
        let name = texts
            .months
            .get(usize::try_from(month - 1).unwrap_or(0))
            .cloned()
            .unwrap_or_default();
        if at < free || at + name.width() > LABEL + weeks * 2 {
            continue;
        }
        line.push_str(&" ".repeat(at - line.width()));
        line.push_str(&name);
        free = at + name.width() + 1;
    }
    Line::styled(line, theme::dim())
}

/// 图例：右对齐到热度图的右边，「token   少 ■ ■ ■ ■ ■ 多」（照花费的写「花费（$）」）。
fn legend(app: &App, by_money: bool, right: usize) -> Line<'static> {
    let texts = &app.config.text.usage;
    let metric = if by_money {
        let symbol = app
            .config
            .layout
            .currencies
            .get(&app.currency)
            .cloned()
            .unwrap_or_else(|| app.currency.clone());
        format!("{}（{symbol}）", texts.by_money)
    } else {
        texts.by_tokens.clone()
    };
    let mut spans = vec![Span::styled(
        format!("{metric}   {} ", texts.less),
        theme::dim(),
    )];
    for level in 0..5 {
        spans.push(Span::styled("■ ", theme::heat(level)));
    }
    spans.push(Span::styled(texts.more.clone(), theme::dim()));
    let used: usize = spans.iter().map(Span::width).sum();
    spans.insert(0, Span::raw(" ".repeat(right.saturating_sub(used))));
    Line::from(spans)
}
