//! 选项带文字画时，画选中那一项的（蓝图「确认和提问的抽屉」第 3 条，2026-10-07 项目主人定）：并排时竖线落在抽屉的
//! 中线上，左半是选项、右半是预览，和选项顶上对齐；第一行暗色写「预览」，下面是图，不描边、不铺底色。图占的高度照这一页最高的那张，换选项时
//! 抽屉不跳；抽屉的字少于 `side_min` 列时挪到选项下面。

use ratatui::text::{Line, Span};
use serde::Deserialize;
use unicode_width::UnicodeWidthStr;

use super::View;
use super::items::{self, Block};
use crate::drawer::{Drawer, Item, Texts};
use crate::theme;

/// 预览面板怎么摆（`layout.json` 的 `drawer_preview`）。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Look {
    /// 抽屉的字够这么多列才左右并排，不够的面板放在选项下面。
    pub side_min: u16,
    /// 并排时选项和预览之间空几列，竖线画在正中。
    pub gap: u16,
    /// 中间那条竖线的字。
    pub divider: String,
    /// 「预览」和图离这一块左边几列。
    pub pad: u16,
}

/// 选项那一块：这一页有文字画时带上选中那一项的预览。
pub fn body(d: &Drawer, texts: &Texts, look: &Look, width: u16, v: &mut View) {
    let pictures: Vec<&str> = d
        .question_at(d.tab)
        .map(|q| {
            q.options
                .iter()
                .filter_map(|o| o.preview.as_deref())
                .collect()
        })
        .unwrap_or_default();
    if pictures.is_empty() {
        items::choices(d, texts, width).append_to(v);
        return;
    }
    let tall = pictures
        .iter()
        .map(|p| p.lines().count())
        .max()
        .unwrap_or(0);
    let shown = match d.current() {
        Some(Item::Choice(i)) => d
            .question_at(d.tab)
            .and_then(|q| q.options[i].preview.as_deref())
            .unwrap_or(""),
        _ => "",
    };
    if width < look.side_min {
        items::choices(d, texts, width).append_to(v);
        v.lines.push(Line::default());
        v.items.push(None);
        let panel = panel(shown, texts, look, width, tall + 1);
        v.items.extend(panel.iter().map(|_| None));
        v.lines.extend(panel);
        return;
    }
    // 竖线在中线上：选项排在它左边留出的地方，预览从它右边空出的地方起。
    let half = look.gap.saturating_sub(1) / 2;
    let left = (width / 2).saturating_sub(half);
    let right = width.saturating_sub(left + look.gap);
    let list = items::choices(d, texts, left);
    let rows = tall + 1;
    let panel = panel(shown, texts, look, right, rows);
    side_by_side(list, panel, left, look).append_to(v);
}

/// 左边的选项和右边的预览并排成一块：左边的每一行补空格补到 `left` 列，空隙正中一条暗色竖线，再接预览。
fn side_by_side(list: Block, panel: Vec<Line<'static>>, left: u16, look: &Look) -> Block {
    let rows = list.lines.len().max(panel.len());
    let mut out = Block {
        focus: list.focus,
        cursor: list.cursor,
        ..Block::default()
    };
    let mut lines = list.lines.into_iter();
    let mut marks = list.items.into_iter();
    let mut panel = panel.into_iter();
    for _ in 0..rows {
        let mut spans = lines.next().map(|l| l.spans).unwrap_or_default();
        let used: usize = spans.iter().map(Span::width).sum();
        let half = usize::from(look.gap.saturating_sub(1) / 2);
        let pad = usize::from(left).saturating_sub(used) + half;
        spans.push(Span::raw(" ".repeat(pad)));
        spans.push(Span::styled(look.divider.clone(), theme::dim()));
        let rest = usize::from(look.gap).saturating_sub(half + look.divider.width());
        spans.push(Span::raw(" ".repeat(rest)));
        spans.extend(panel.next().map(|l| l.spans).unwrap_or_default());
        out.lines.push(Line::from(spans));
        out.items.push(marks.next().flatten());
    }
    out
}

/// 预览排成 `rows` 行：第一行暗色「预览」，接着是图（不够的补空行，换选项时高度不变），每行放不下 `width` 列的截掉。
fn panel(picture: &str, texts: &Texts, look: &Look, width: u16, rows: usize) -> Vec<Line<'static>> {
    let inner = width.saturating_sub(look.pad.saturating_mul(2));
    let mut pictures = picture.lines();
    (0..rows)
        .map(|row| {
            let (text, style) = if row == 0 {
                (texts.preview.as_str(), theme::dim())
            } else {
                (pictures.next().unwrap_or(""), ratatui::style::Style::new())
            };
            Line::from(vec![
                Span::raw(" ".repeat(usize::from(look.pad))),
                Span::styled(super::clip(text, inner), style),
            ])
        })
        .collect()
}
