//! 抽屉里的选项、补充那一行、「确认」页（蓝图「确认和提问的抽屉」第 3 条）。

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::View;
use crate::drawer::{Decision, Drawer, Edit, Item, Texts, inline, said};
use crate::input::pieces;
use crate::theme;

/// 一块排好的行，行号从这一块的第一行数：选项那一块要和右边的文字画并排，先单独排。
#[derive(Debug, Default)]
pub struct Block {
    /// 每一行。
    pub lines: Vec<Line<'static>>,
    /// 每一行是第几项。
    pub items: Vec<Option<usize>>,
    /// 选中那一项的第一行、最后一行。
    pub focus: (usize, usize),
    /// 编辑时光标的行、列。
    pub cursor: Option<(usize, u16)>,
}

impl Block {
    fn push(&mut self, line: Line<'static>, item: Option<usize>) {
        self.lines.push(line);
        self.items.push(item);
    }

    /// 接到 `v` 后面，行号照接上的位置挪。
    pub fn append_to(self, v: &mut View) {
        let at = v.lines.len();
        v.focus = (at + self.focus.0, at + self.focus.1);
        if let Some((row, col)) = self.cursor {
            v.cursor = Some((at + row, col));
        }
        v.lines.extend(self.lines);
        v.items.extend(self.items);
    }
}

/// 选项：一项一行标题（加粗；选中的行首 `› `、品红；能多选的前面 `[ ]`），下面暗色说明。
pub fn choices(d: &Drawer, texts: &Texts, width: u16) -> Block {
    let mut b = Block::default();
    for (index, item) in d.items(d.tab).into_iter().enumerate() {
        let first = b.lines.len();
        item_lines(d, texts, width, index, item, &mut b);
        if index == d.cursor[d.tab] {
            b.focus = (first, b.lines.len() - 1);
        }
    }
    b
}

fn item_lines(d: &Drawer, texts: &Texts, width: u16, index: usize, item: Item, b: &mut Block) {
    let tab = d.tab;
    let active = index == d.cursor[tab];
    let typed = &d.typed[tab];
    let pointer = if active {
        texts.pointer.clone()
    } else {
        " ".repeat(texts.pointer.width())
    };
    let checked = match item {
        Item::Choice(i) => d.checked[tab].get(i) == Some(&true),
        Item::Other => !typed.trim().is_empty(),
        Item::Decision(_) => false,
    };
    let mark = if checked {
        &texts.checked
    } else {
        &texts.unchecked
    };
    let marker = (d.multiple(tab) && matches!(item, Item::Choice(_) | Item::Other)).then_some(mark);
    let lit = active || (checked && d.multiple(tab));
    let style = if lit { theme::picked() } else { Style::new() };
    let label_style = style.add_modifier(Modifier::BOLD);
    let mut spans = vec![Span::styled(pointer.clone(), theme::picked())];
    if let Some(marker) = marker {
        spans.push(Span::styled(marker.clone(), style));
    }
    let lead = pointer.width() + marker.map_or(0, |m| m.width());
    let room = usize::from(width).saturating_sub(lead);
    // 在写的那一项：标题换成写的字（「其他」），或者标题后面空两格接写的字（理由）；光标跟在字后面。
    let editing = active
        && matches!(
            (d.editing, item),
            (Some(Edit::Other), Item::Other) | (Some(Edit::Reason), Item::Decision(Decision::Deny))
        );
    let (label, description) = match item {
        Item::Choice(i) => {
            let option = d.question_at(tab).map(|q| &q.options[i]);
            (
                option.map_or(String::new(), |o| o.label.clone()),
                option.and_then(|o| o.description.clone()),
            )
        }
        Item::Other if editing => (String::new(), None),
        Item::Other if !typed.trim().is_empty() => {
            (texts.custom.replace("{text}", &inline(typed)), None)
        }
        Item::Other => (texts.other.clone(), None),
        Item::Decision(decision) => {
            let at = Decision::ALL
                .iter()
                .position(|x| *x == decision)
                .unwrap_or(0);
            (texts.decisions[at].clone(), None)
        }
    };
    let label = super::clip(&label, u16::try_from(room).unwrap_or(u16::MAX));
    let mut used = lead + label.width();
    spans.push(Span::styled(label, label_style));
    if editing {
        let (gap, hint) = match item {
            Item::Other => ("", &texts.other),
            _ => ("  ", &texts.reason_hint),
        };
        spans.push(Span::raw(gap));
        used += gap.width();
        let room = usize::from(width).saturating_sub(used + 1);
        let shown = tail(&inline_raw(typed), room);
        b.cursor = Some((b.lines.len(), col(used + shown.width())));
        if shown.is_empty() {
            spans.push(Span::styled(tail(hint, room), theme::dim()));
        } else {
            spans.push(Span::raw(shown));
        }
    }
    b.push(Line::from(spans), Some(index));
    if let Some(description) = description.filter(|s| !s.trim().is_empty()) {
        let indent = " ".repeat(lead);
        let room = col(usize::from(width).saturating_sub(lead)).max(1);
        for (piece, _) in pieces(description.trim(), room) {
            let spans = vec![Span::raw(indent.clone()), Span::styled(piece, theme::dim())];
            b.push(Line::from(spans), Some(index));
        }
    }
}

/// 按 `n` 写的补充：有字或者正在写时，选项下面一行 `补充  字`。
pub fn notes(d: &Drawer, texts: &Texts, width: u16, v: &mut View) {
    let text = &d.notes[d.tab];
    let editing = d.editing == Some(Edit::Notes);
    if text.trim().is_empty() && !editing {
        return;
    }
    let lead = format!(
        "{}{}  ",
        " ".repeat(texts.pointer.width()),
        texts.notes_label
    );
    let room = usize::from(width).saturating_sub(lead.width() + 1);
    let shown = tail(&inline_raw(text), room);
    if editing {
        v.cursor = Some((v.lines.len(), col(lead.width() + shown.width())));
    }
    v.push(
        Line::from(vec![Span::styled(lead, theme::dim()), Span::raw(shown)]),
        None,
    );
}

/// 「确认」页：一道一行 `短名：回答`，短名加粗、回答暗，没答的红色「未回答」。
pub fn review(d: &Drawer, texts: &Texts, width: u16, v: &mut View) {
    let bold = Style::new().add_modifier(Modifier::BOLD);
    for page in 0..d.pages() {
        let label = format!("{}：", d.label(page));
        let used = label.width();
        let room = col(usize::from(width).saturating_sub(used));
        let (answer, style) = match d.answers[page].as_ref() {
            Some(a) => (said(a), theme::dim()),
            None => (texts.unanswered.clone(), theme::error()),
        };
        v.push(
            Line::from(vec![
                Span::styled(label, bold),
                Span::styled(super::clip(&answer, room), style),
            ]),
            None,
        );
    }
}

/// 写的字排成一行：换行写成 `↵`，前后的空白照留（光标跟着）。
fn inline_raw(text: &str) -> String {
    text.replace(['\r', '\n'], "↵")
}

/// 放进 `width` 列：放不下的留后面那截（看得到最新写的）。
fn tail(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_string();
    }
    let mut kept = Vec::new();
    let mut used = 0;
    for c in text.chars().rev() {
        let w = c.width().unwrap_or(0);
        if used + w > width {
            break;
        }
        used += w;
        kept.push(c);
    }
    kept.into_iter().rev().collect()
}

fn col(n: usize) -> u16 {
    u16::try_from(n).unwrap_or(u16::MAX)
}
