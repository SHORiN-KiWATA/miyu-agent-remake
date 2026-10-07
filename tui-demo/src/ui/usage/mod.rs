//! `/usage` 框（蓝图 `tui.md`「配置与模型」第 5 条，2026-10-07 项目主人定照 Codex 做细）：顶上一行四个标签（当前的
//! 反色），右边暗色写全部的合计；下面是那一页：总览（四个数、热度图，`overview.rs`）、最近 30 天（柱状图加按天的表，
//! `charts.rs`）、按模型（占比条加用途）、按会话（排行，`tables.rs`）。没回来的写「正在读用量…」，拒了的写原因。

mod charts;
mod overview;
mod tables;

use std::collections::HashMap;

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use serde::Deserialize;
use unicode_width::UnicodeWidthStr;

use super::panel::Chrome;
use crate::app::usage::Loaded;
use crate::app::{App, UsageData};
use crate::core::{Bill, UsageRow};
use crate::{money, theme};

/// 用量框里的字。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Texts {
    /// 上边框写的。
    pub title: String,
    /// 四个标签：总览、最近 30 天、按模型、按会话。
    pub tabs: Vec<String>,
    /// 右上角的合计，`{amounts}`。
    pub total: String,
    /// 按天的表里今天那一行日期后面写的。
    pub today: String,
    /// 请求次数，`{n}`。
    pub requests: String,
    /// 行尾有几次没有价格，`{n}`。
    pub unpriced: String,
    /// 还没回来。
    pub loading: String,
    /// 一行都没有。
    pub empty: String,
    /// 核心拒了，`{reason}`。
    pub failed: String,
    /// 下边框的按键提示。
    pub hint: String,
    /// 总览四个数的名字：累计。
    pub lifetime: String,
    /// 单日最高。
    pub peak: String,
    /// 连续使用。
    pub streak: String,
    /// 连续几天，`{n}`。
    pub streak_days: String,
    /// 最长连过几天，`{n}`。
    pub streak_best: String,
    /// 最长的会话。
    pub longest: String,
    /// 热度图图例的两头。
    pub less: String,
    /// 见 `less`。
    pub more: String,
    /// 热度图照什么分档：token。
    pub by_tokens: String,
    /// 照花费。
    pub by_money: String,
    /// 热度图左边一天一行的名字，周一在最上；空的不写。
    pub weekdays: Vec<String>,
    /// 十二个月的名字。
    pub months: Vec<String>,
    /// 柱状图下面写最高那天，`{tokens}`。
    pub recent_peak: String,
    /// 用途：主请求、摘要请求（`purpose` 是 `null` 的）。
    pub main_purpose: String,
    /// 别的用途的名字；没有的照原样写。
    pub purposes: HashMap<String, String>,
}

/// 框里几样图的尺寸（`layout.json` 的 `usage`）。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Look {
    /// 「最近 N 天」那一页几天（连今天）。
    pub recent_days: u16,
    /// 热度图最多几周。
    pub weeks: u16,
    /// 柱状图几行高。
    pub bar_rows: u16,
    /// 占比条几格长。
    pub share_cells: u16,
}

/// 有几页：总览、最近 30 天、按模型、按会话。
const TABS: usize = 4;

/// 列和列之间空几格。
pub(super) const GAP: &str = "   ";

/// 框和露出来的行：标签那一行、空一行，再从第 `scroll` 行起最多露到 `max` 行。
pub fn lines(
    app: &App,
    (tab, scroll, by_money): (usize, usize, bool),
    width: u16,
    max: usize,
) -> (Chrome, Vec<Line<'static>>) {
    let texts = &app.config.text.usage;
    let chrome = Chrome::new(&texts.title, Vec::new()).hint(&texts.hint);
    let Some(data) = app.usage.as_ref() else {
        return (chrome, Vec::new());
    };
    let total = match &data.total {
        Some(Ok(rows)) => rows.first().and_then(|r| amounts(app, &r.bill)),
        _ => None,
    };
    let mut out = vec![tabs(texts, tab, total, width), Line::default()];
    // 换页不改框的高度：照四页里最高的那一页留地方，矮的下面空着（2026-10-07 项目主人）。
    let tallest = (0..TABS)
        .map(|page| body(app, data, (page, by_money), width).len())
        .max()
        .unwrap_or(0);
    let body = body(app, data, (tab, by_money), width);
    let room = max.saturating_sub(out.len()).max(1);
    let start = scroll.min(body.len().saturating_sub(room));
    out.extend(body.into_iter().skip(start).take(room));
    let high = 2 + tallest.min(room);
    out.resize(out.len().max(high), Line::default());
    (chrome, out)
}

/// 那一页标签以下有几行：按键滚到头照它停。
pub fn body_rows(app: &App, (tab, by_money): (usize, bool), width: u16) -> usize {
    app.usage
        .as_ref()
        .map_or(0, |data| body(app, data, (tab, by_money), width).len())
}

/// 一页的内容：要的那一样没回来、拒了、空的先说；都有了交给那一页排。
fn body(
    app: &App,
    data: &UsageData,
    (tab, by_money): (usize, bool),
    width: u16,
) -> Vec<Line<'static>> {
    let texts = &app.config.text.usage;
    let primary: &Loaded = match tab {
        0 | 1 => &data.days,
        2 => &data.models,
        _ => &data.sessions,
    };
    let rows = match primary {
        None => return vec![Line::styled(texts.loading.clone(), theme::dim())],
        Some(Err(reason)) => {
            let text = texts.failed.replace("{reason}", reason);
            return vec![Line::styled(text, theme::warn())];
        }
        Some(Ok(rows)) if rows.is_empty() => {
            return vec![Line::styled(texts.empty.clone(), theme::dim())];
        }
        Some(Ok(rows)) => rows,
    };
    match tab {
        0 => overview::page(app, data, rows, by_money, width),
        1 => charts::recent(app, data, rows, width),
        2 => tables::models(app, data, rows, width),
        _ => tables::sessions(app, rows, width),
    }
}

/// 标签那一行：当前的反色，右边暗色写合计（放得下才写）。
fn tabs(texts: &Texts, tab: usize, total: Option<String>, width: u16) -> Line<'static> {
    let mut spans = Vec::new();
    for (i, label) in texts.tabs.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        let style = if i == tab {
            Style::new().add_modifier(Modifier::REVERSED)
        } else {
            theme::dim()
        };
        spans.push(Span::styled(format!(" {label} "), style));
    }
    if let Some(total) = total {
        let total = texts.total.replace("{amounts}", &total);
        let used: usize = spans.iter().map(Span::width).sum();
        let gap = usize::from(width).saturating_sub(used + total.width());
        if gap >= 2 {
            spans.push(Span::raw(" ".repeat(gap)));
            spans.push(Span::styled(total, theme::dim()));
        }
    }
    Line::from(spans)
}

/// 一份账的金额（`$0.42 + ¥1.30`）；一笔都没有的是 `None`。
pub(super) fn amounts(app: &App, bill: &Bill) -> Option<String> {
    money::amounts(bill, &app.currency, &app.config.layout.currencies)
}

/// 一行行里一共多少 token。
pub(super) fn tokens(rows: &[UsageRow]) -> u64 {
    rows.iter().map(|r| r.tokens).sum()
}
