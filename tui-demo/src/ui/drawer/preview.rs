//! 选项带文字画时，画选中那一项的，不加框，从抽屉的中线开始（蓝图「确认和提问的抽屉」第 3 条，照 Claude Code）。
//! 图占的高度照这一页最高的那张，换选项时抽屉不跳；抽屉的字少于 `SIDE_BY_SIDE` 列时图画在选项下面。

use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use super::View;
use super::items::{self, Block};
use crate::drawer::{Drawer, Item, Texts};

/// 字够这么多列才左右分栏。
const SIDE_BY_SIDE: u16 = 70;
/// 选项和图之间至少空几列。
const GAP: u16 = 2;

/// 选项那一块：这一页有文字画时带上选中那一项的图。
pub fn body(d: &Drawer, texts: &Texts, width: u16, v: &mut View) {
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
    let wide = pictures
        .iter()
        .flat_map(|p| p.lines())
        .map(UnicodeWidthStr::width)
        .max()
        .unwrap_or(0);
    let shown = match d.current() {
        Some(Item::Choice(i)) => d
            .question_at(d.tab)
            .and_then(|q| q.options[i].preview.as_deref())
            .unwrap_or(""),
        _ => "",
    };
    if width < SIDE_BY_SIDE {
        items::choices(d, texts, width).append_to(v);
        v.lines.push(Line::default());
        v.items.push(None);
        let picture = drawing(shown, tall, width);
        v.items.extend(picture.iter().map(|_| None));
        v.lines.extend(picture);
        return;
    }
    let right = u16::try_from(wide).unwrap_or(u16::MAX).min(width / 2);
    // 图从抽屉的中线开始；选项比半宽还宽时接在选项后面。「输入其他答案」写的字不算宽（不然边打字图边往右跑）。
    let room = width - right - GAP;
    let options = d.question_at(d.tab).map_or(0, |q| q.options.len());
    let block = items::choices(d, texts, room);
    let widest = block
        .lines
        .iter()
        .zip(&block.items)
        .filter(|(_, item)| item.is_some_and(|i| i < options))
        .map(|(line, _)| line.width())
        .max()
        .unwrap_or(0);
    let half = (width / 2).saturating_sub(GAP);
    let left = u16::try_from(widest).unwrap_or(room).max(half).min(room);
    let list = items::choices(d, texts, left);
    let picture = drawing(shown, tall, right);
    side_by_side(list, picture, left).append_to(v);
}

/// 左边的选项和右边的图并排成一块：左边的每一行补空格补到 `left` 列，再空两列接图。
fn side_by_side(list: Block, picture: Vec<Line<'static>>, left: u16) -> Block {
    let rows = list.lines.len().max(picture.len());
    let mut out = Block {
        focus: list.focus,
        cursor: list.cursor,
        ..Block::default()
    };
    let mut lines = list.lines.into_iter();
    let mut marks = list.items.into_iter();
    let mut picture = picture.into_iter();
    for _ in 0..rows {
        let mut spans = lines.next().map(|l| l.spans).unwrap_or_default();
        let used: usize = spans.iter().map(Span::width).sum();
        let pad = usize::from(left + GAP).saturating_sub(used);
        spans.push(Span::raw(" ".repeat(pad)));
        spans.extend(picture.next().map(|l| l.spans).unwrap_or_default());
        out.lines.push(Line::from(spans));
        out.items.push(marks.next().flatten());
    }
    out
}

/// 一张文字画排成 `tall` 行（不够的补空行，换选项时高度不变），每行放不下 `width` 列的截掉。
fn drawing(picture: &str, tall: usize, width: u16) -> Vec<Line<'static>> {
    let mut rows = picture.lines();
    (0..tall)
        .map(|_| Line::raw(super::clip(rows.next().unwrap_or(""), width)))
        .collect()
}
