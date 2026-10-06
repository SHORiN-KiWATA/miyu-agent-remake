//! 画抽屉（蓝图 `tui.md`「确认和提问的抽屉」第 2、3 条，样子照旧版的提问面板）：画在输入框里，框往上长；
//! 放不下时问题钉在上面、按键提示钉在下面，中间的选项跟着选中的那一项滚。选项的写法在 `items.rs`，
//! 右边的文字画在 `preview.rs`。

mod items;
mod preview;

#[cfg(test)]
mod tests;

use ratatui::Frame;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::rows::clip;
use crate::caret::Caret;
use crate::drawer::{Drawer, Edit, Texts};
use crate::input::pieces;
use crate::theme;

/// 排好的抽屉：每一行、每一行是第几项、选中那一项从第几行起、打字的光标在哪（行、列）。
#[derive(Debug, Default)]
pub struct View {
    /// 每一行。
    pub lines: Vec<Line<'static>>,
    /// 每一行是第几项；不是项的是 `None`。
    pub items: Vec<Option<usize>>,
    /// 选中那一项的第一行、最后一行。
    pub focus: (usize, usize),
    /// 编辑时光标的行、列。
    pub cursor: Option<(usize, u16)>,
    /// 选项前面有几行（谁在问、标签、问题、空行）：放不下时钉在上面。
    pub head: usize,
}

impl View {
    fn push(&mut self, line: Line<'static>, item: Option<usize>) {
        self.lines.push(line);
        self.items.push(item);
    }
}

/// 排成的全部行（还没按高度裁）。
pub fn view(d: &Drawer, texts: &Texts, width: u16) -> View {
    let mut v = View::default();
    if let Some(who) = &d.who {
        let who = texts.asking.replace("{who}", who);
        v.push(Line::styled(clip(&who, width), theme::dim()), None);
    }
    if d.has_review() {
        v.push(tabs(d, texts), None);
        v.push(Line::default(), None);
    }
    if !d.on_review() {
        head(d, texts, width, &mut v);
        v.push(Line::default(), None);
    }
    v.head = v.lines.len();
    v.focus = (v.head, v.head);
    if d.on_review() {
        items::review(d, texts, width, &mut v);
    } else {
        preview::body(d, texts, width, &mut v);
        items::notes(d, texts, width, &mut v);
    }
    v.push(Line::default(), None);
    // 按过一下 `Esc`：这一行换成黄色的「再按一次 Esc 取消」（第 3、5 条）。
    let last = if d.armed.is_some() {
        Line::styled(clip(&texts.cancel_hint, width), theme::warn())
    } else {
        Line::styled(clip(&keys(d, texts), width), theme::dim())
    };
    v.push(last, None);
    v
}

/// 最后一行按键提示：照这一页能做什么写；编辑时只写保存、退出编辑。
fn keys(d: &Drawer, texts: &Texts) -> String {
    // 写「不允许」的理由不换提示：方向键、两下 Esc 照旧（第 4 条）。
    if matches!(d.editing, Some(Edit::Other | Edit::Notes)) {
        return texts.keys_edit.clone();
    }
    let mut keys = if d.on_review() {
        texts.keys_review.clone()
    } else if !d.is_question() {
        texts.keys_approve.clone()
    } else if d.multiple(d.tab) {
        texts.keys_multi.clone()
    } else {
        texts.keys.clone()
    };
    if d.has_review() {
        keys.push_str(&texts.keys_tabs);
    }
    keys
}

/// 顶上那一排标签，最后一个是「确认」：当前那个反色，答过的后面加 `✓`，别的暗。
fn tabs(d: &Drawer, texts: &Texts) -> Line<'static> {
    let mut spans = Vec::new();
    for page in 0..=d.pages() {
        if page > 0 {
            spans.push(Span::raw("  "));
        }
        let mut label = if page == d.pages() {
            texts.review_tab.clone()
        } else {
            d.label(page)
        };
        if d.answers.get(page).is_some_and(Option::is_some) {
            label.push_str(&texts.answered_tab);
        }
        if page == d.tab {
            let on = Style::new().add_modifier(Modifier::REVERSED);
            spans.push(Span::styled(format!(" {label} "), on));
        } else {
            spans.push(Span::styled(label, theme::dim()));
        }
    }
    Line::from(spans)
}

/// 问题：提问的加粗、放不下折行；确认的问题行加粗，下面每个路径一行暗色，工作区外的标出来。
fn head(d: &Drawer, texts: &Texts, width: u16, v: &mut View) {
    let bold = Style::new().add_modifier(Modifier::BOLD);
    let title = match d.question_at(d.tab) {
        Some(q) => q.question.trim().to_string(),
        None => d.title(texts),
    };
    for (piece, _) in pieces(&title, width) {
        v.push(Line::styled(piece, bold), None);
    }
    // 要跑的命令原文：`$ ` 打头、暗色，一行行写，长的折行（2026-10-07 项目主人：确认时看不到具体的命令）。
    if let Some((_, command)) = &d.command {
        for line in command.lines() {
            for (piece, _) in pieces(&format!("$ {line}"), width) {
                v.push(Line::styled(piece, theme::dim()), None);
            }
        }
    }
    for (path, mark) in d.paths(texts) {
        let mut spans = vec![Span::styled(clip(&path, width), theme::dim())];
        if let Some(mark) = mark {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(mark, theme::warn()));
        }
        v.push(Line::from(spans), None);
    }
}

/// 抽屉要几行：排出来的行数，最多 `max`。
pub fn rows(d: &Drawer, texts: &Texts, width: u16, max: u16) -> u16 {
    let n = view(d, texts, width).lines.len();
    u16::try_from(n).unwrap_or(u16::MAX).min(max).max(1)
}

/// 按高度裁：放得下全画。放不下时最后两行（空行、按键提示）钉在下面，问题那几行钉在上面，中间的选项跟着
/// 选中那一项滚（照旧版 `panel_layout`）：选项至少留三行（不超过剩下的一半），问题太长留它的后半截。
/// 矮到四行以下（「窗口小的时候」第 3 条）：先去按键提示上面那一行空行，再去问题，最后去按键提示，选项至少留一行；
/// 问题被裁时不留它后面那行空行。交回裁好的样子，顺手把选项滚到哪记进 `scroll`。
pub fn fit(v: View, height: usize, scroll: &mut usize) -> View {
    let n = v.lines.len();
    if n <= height {
        *scroll = 0;
        return v;
    }
    let tail = 2.min(n - v.head);
    let body_len = n - tail - v.head;
    let foot = match height {
        0 | 1 => 0,
        2 | 3 => 1,
        _ => 2,
    }
    .min(tail);
    let avail = height - foot;
    let reserved = body_len
        .min(3)
        .min(avail / 2)
        .max(body_len.min(1))
        .min(avail);
    let top = v.head.min(avail - reserved);
    let room = avail - top;
    // 问题被裁时，留下的是问题的字，不是它后面那行空行。
    let blank = |k: usize| v.lines[k].width() == 0;
    let head_end = if top < v.head {
        (0..v.head).rev().find(|&k| !blank(k)).map_or(0, |k| k + 1)
    } else {
        v.head
    };
    let (first, last) = (v.focus.0 - v.head, v.focus.1 - v.head);
    let mut start = (*scroll).min(body_len.saturating_sub(room));
    if first < start || last - first >= room {
        start = first;
    } else if last >= start + room {
        start = (last + 1).saturating_sub(room);
    }
    *scroll = start;
    // 留下的行在原来的第几行：问题的后半截、选项的一段、按键提示。
    let kept: Vec<usize> = (head_end.saturating_sub(top)..head_end)
        .chain(v.head + start..(v.head + start + room).min(n - tail))
        .chain(n - foot..n)
        .collect();
    let at = |row: usize| kept.iter().position(|&k| k == row);
    let cursor = v.cursor.and_then(|(row, col)| at(row).map(|r| (r, col)));
    View {
        lines: kept.iter().map(|&k| v.lines[k].clone()).collect(),
        items: kept.iter().map(|&k| v.items[k]).collect(),
        focus: v.focus,
        head: top,
        cursor,
    }
}

/// 画在输入框的字的地方（`area`）；交回每一行是第几项，鼠标照它认。
pub fn draw(
    frame: &mut Frame,
    area: Rect,
    d: &mut Drawer,
    texts: &Texts,
    caret: &mut Caret,
) -> Vec<Option<usize>> {
    let v = view(d, texts, area.width);
    let v = fit(v, usize::from(area.height), &mut d.scroll);
    if let Some((row, col)) = v.cursor {
        let x = area.x + col.min(area.width.saturating_sub(1));
        caret.put(
            Position::new(x, area.y + u16::try_from(row).unwrap_or(0)),
            true,
        );
    }
    frame.render_widget(Paragraph::new(v.lines), area);
    v.items
}
