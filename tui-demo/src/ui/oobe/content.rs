//! 引导中间几步右边那一块的一行行（「第一次打开的引导」第 6、7、13–26 条）：标题、说明、选一行、打字的格、红字黄字。
//! 每一步照它拼，画在哪、滑到哪由 `mod.rs` 管。

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::oobe::field::Field;
pub use crate::oobe::wrap::wrap;
use crate::theme;

/// 拼好的一块。
#[derive(Debug, Default)]
pub struct Content {
    /// 一行行。
    pub lines: Vec<Line<'static>>,
    /// 输入光标在哪：(行, 列)。
    pub caret: Option<(u16, u16)>,
    /// 光标停在哪一行（吉祥物看它、太高时照它滚）。
    pub focus: Option<u16>,
    /// 这一块有多宽。
    pub width: u16,
    /// 这一块最多多高；`0` 是不管（只有长的列表照它开窗）。
    pub height: u16,
    /// 光标停着、没在编辑的格子右端暗色写的（「Enter 编辑」，「第一次打开的引导」第 7 条）；空的不写。
    pub cue: String,
    /// 画在浮窗里：浮窗的底就是面板色，格子一律浅一档，不然看不出格子。
    pub on_panel: bool,
    /// 要画的一张小图（建人格的头像）：占着的几行是空行（`ui/oobe/picture.rs` 画）。
    pub picture: Option<Picture>,
}

/// 一张小图：从第几行起、往右挪几列，哪个文件，最多几列几行。
#[derive(Debug, Clone)]
pub struct Picture {
    /// 从这一块的第几行起。
    pub line: u16,
    /// 往右挪几列（和格子对齐）。
    pub x: u16,
    /// 图片文件。
    pub path: String,
    /// 最多几列。
    pub cols: u16,
    /// 最多几行（占着这么多空行）。
    pub rows: u16,
}

/// 光标停着的那一行前面的记号（「第一次打开的引导」第 7 条，2026-10-10 项目主人：「改成箭头加变颜色吧，列表里面可以是
/// 箭头加背景」）。
const POINTER: &str = "❯ ";
/// 没选中的行前面空两格。
const GAP: &str = "  ";
/// 几行的格最多露几行。
const FIELD_ROWS: usize = 5;

impl Content {
    /// `width` 列宽的一块。
    pub fn new(width: u16) -> Self {
        Self {
            width,
            ..Self::default()
        }
    }

    /// `width` 列宽、最多 `height` 行高的一块：长的列表照它开窗（「第一次打开的引导」第 19 条）。
    pub fn sized(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            ..Self::default()
        }
    }

    /// 已经有几行。
    pub fn len(&self) -> u16 {
        self.at()
    }

    fn at(&self) -> u16 {
        u16::try_from(self.lines.len()).unwrap_or(u16::MAX)
    }

    /// 标题（加粗）、说明（暗），下面空一行。
    pub fn heading(&mut self, title: &str, sub: &str) {
        self.lines.push(Line::styled(
            title.to_string(),
            Style::new().add_modifier(Modifier::BOLD),
        ));
        if !sub.is_empty() {
            self.lines.push(Line::styled(sub.to_string(), theme::dim()));
        }
        self.blank();
    }

    /// 空一行。
    pub fn blank(&mut self) {
        self.lines.push(Line::default());
    }

    /// 一段字，照块宽折行（核心的原话常常很长）。
    pub fn note(&mut self, text: &str, style: Style) {
        let (rows, _) = wrap(text, 0, usize::from(self.width).max(1));
        for row in rows {
            self.lines.push(Line::styled(row, style));
        }
    }

    /// 选一行的列表的一行：选着的前面 `❯`、铺底色到整块宽；右边那一截靠右。
    pub fn row(&mut self, selected: bool, left: Vec<Span<'static>>, right: Option<Span<'static>>) {
        if selected {
            self.focus = Some(self.at());
        }
        let lead = if selected {
            Span::styled(POINTER, theme::accent())
        } else {
            Span::raw(GAP)
        };
        let mut spans = vec![lead];
        spans.extend(left);
        if let Some(right) = right {
            let used: usize = spans.iter().map(|s| s.content.width()).sum();
            let pad = usize::from(self.width).saturating_sub(used + right.content.width());
            spans.push(Span::raw(" ".repeat(pad.max(1))));
            spans.push(right);
        }
        self.lines
            .push(fill(spans, self.width, selected.then(theme::row_focus)));
    }

    /// 段名（暗、加粗），不能选。
    pub fn section(&mut self, name: &str) {
        self.lines.push(Line::styled(
            name.to_string(),
            theme::dim().add_modifier(Modifier::BOLD),
        ));
    }

    /// 打字的一格：左边暗色的名字（照 `label_w` 对齐），右边铺底色的字；空着的暗色写 `hint`。光标在的那一格前面 `❯`、
    /// 名字变强调色，格子浅一档；在编辑的记下输入光标，没在编辑的格子右端写 [`Content::cue`]。几行的格折行，最多露
    /// [`FIELD_ROWS`] 行，照光标滚。
    pub fn field(&mut self, label: &str, label_w: usize, field: &Field, hint: &str, focused: bool) {
        let cue = if focused {
            self.cue.clone()
        } else {
            String::new()
        };
        self.boxed_field((label, label_w), field, hint, focused, &cue);
    }

    /// 搜索那一格（搜模型）：在搜的时候才算光标停着；没在搜时右端写 `cue`（「/ 搜索」，「第一次打开的引导」第 7 条）。
    pub fn search(&mut self, label: &str, label_w: usize, field: &Field, cue: &str) {
        self.boxed_field((label, label_w), field, "", field.editing(), cue);
    }

    /// 打字的格子照 [`Content::field`] 画，没在编辑时右端写 `cue`。
    fn boxed_field(
        &mut self,
        (label, label_w): (&str, usize),
        field: &Field,
        hint: &str,
        focused: bool,
        cue: &str,
    ) {
        let lead = lead(label, label_w);
        let room = usize::from(self.width)
            .saturating_sub(lead.width() + 1)
            .max(4);
        let ground = ground(focused || self.on_panel);
        if focused {
            self.focus = Some(self.at());
        }
        let shown = if field.unlinked && !field.editing() {
            crate::oobe::field::unlink(&field.shown())
        } else {
            field.shown()
        };
        let hint = if field.unlinked {
            crate::oobe::field::unlink(hint)
        } else {
            hint.to_string()
        };
        let cursor = if field.secret {
            field.text()[..field.editor.cursor().min(field.text().len())]
                .chars()
                .count()
                * '•'.len_utf8()
        } else {
            field.editor.cursor()
        };
        // 密钥不折行：太长的只露后面那一截（光标多半在最后）。
        let (rows, (row, col)) = if field.secret {
            let dots = shown.chars().count().min(room.saturating_sub(1));
            let tail = "•".repeat(dots);
            let at = (cursor / '•'.len_utf8()).min(dots);
            (vec![tail], (0, at))
        } else {
            wrap(&shown, cursor.min(shown.len()), room)
        };
        let first = row.saturating_sub(FIELD_ROWS - 1);
        let blank_lead = " ".repeat(lead.width());
        for (i, text) in rows.iter().enumerate().skip(first).take(FIELD_ROWS) {
            let head = if i == first {
                labelled(label, label_w, focused)
            } else {
                vec![Span::raw(blank_lead.clone())]
            };
            let body = if shown.is_empty() {
                Span::styled(hint.clone(), ground.patch(theme::faint()))
            } else {
                Span::styled(text.clone(), ground)
            };
            // 光标只在编辑时出现：停上去还没按 `Enter` 的不画光标，格子右端写「Enter 编辑」（「第一次打开的引导」第 7 条）。
            if focused && field.editing() && i == row {
                let line = u16::try_from(self.lines.len()).unwrap_or(u16::MAX);
                let x = u16::try_from(lead.width() + col).unwrap_or(u16::MAX);
                self.caret = Some((line, x));
            }
            let cue = if !field.editing() && i == first {
                cue
            } else {
                ""
            };
            let mut spans = head;
            spans.extend(boxed(body, room, cue, ground));
            self.lines.push(Line::from(spans));
        }
    }

    /// 一个动作行（「测试连接 →」「下一步 →」「创建 →」）：停着的前面 `❯`、字变强调色，没停的照常色，都加粗、不铺底。
    pub fn action(&mut self, focused: bool, text: &str) {
        if focused {
            self.focus = Some(self.at());
        }
        let (lead, style) = if focused {
            (Span::styled(POINTER, theme::accent()), theme::accent())
        } else {
            (Span::raw(GAP), Style::new())
        };
        self.lines.push(Line::from(vec![
            lead,
            Span::styled(text.to_string(), style.add_modifier(Modifier::BOLD)),
        ]));
    }

    /// 不打字的一格（人格提示词、人设提醒短语写第一行，示范对话写几轮），和打字的格一样宽、一样的底、光标停着时一样写
    /// [`Content::cue`]；空着的暗色写 `hint`。回车开浮窗（「第一次打开的引导」第 21 条）。
    pub fn chip(&mut self, label: &str, label_w: usize, text: &str, hint: &str, focused: bool) {
        if focused {
            self.focus = Some(self.at());
        }
        let room = self.room(label_w);
        let ground = ground(focused || self.on_panel);
        let body = if text.is_empty() {
            Span::styled(hint.to_string(), ground.patch(theme::faint()))
        } else {
            Span::styled(text.to_string(), ground)
        };
        let cue = if focused {
            self.cue.clone()
        } else {
            String::new()
        };
        let mut spans = labelled(label, label_w, focused);
        spans.extend(boxed(body, room, &cue, ground));
        self.lines.push(Line::from(spans));
    }

    /// 左右换的一格（自定义的接口）：只写选着的那一个，两边 `‹` `›`，不铺底色；光标在的时候箭头用强调色
    /// （「第一次打开的引导」第 18 条，2026-10-09 项目主人：「那三个接口可以是左右切换的那种」，2026-10-10：「似乎不需要
    /// 有背景颜色」）。
    pub fn switch(&mut self, label: &str, label_w: usize, name: &str, focused: bool) {
        if focused {
            self.focus = Some(self.at());
        }
        let arrow = if focused {
            theme::accent()
        } else {
            theme::faint()
        };
        let mut spans = labelled(label, label_w, focused);
        spans.push(Span::styled("‹ ", arrow));
        spans.push(Span::raw(name.to_string()));
        spans.push(Span::styled(" ›", arrow));
        self.lines.push(Line::from(spans));
    }

    /// 一长串选一行的（试通了的模型）：比剩下的高度多的开一个窗，照光标滚；上面、下面还有的那一头暗色写一行「还有 N 个」
    /// （第 19 条）。`below` 是这串下面还要留几行（红字、转轮）。
    pub fn window(
        &mut self,
        rows: Vec<Vec<Span<'static>>>,
        cursor: usize,
        below: u16,
        more: (&str, &str),
    ) {
        let total = rows.len();
        let room = if self.height == 0 {
            total
        } else {
            usize::from(self.height.saturating_sub(self.at() + below)).max(3)
        };
        let (start, end) = if total <= room {
            (0, total)
        } else {
            let shown = room.saturating_sub(2).max(1);
            let start = cursor.saturating_sub(shown / 2).min(total - shown);
            (start, start + shown)
        };
        let note = |c: &mut Self, text: &str, n: usize| {
            c.lines.push(Line::styled(
                format!("{GAP}{}", text.replace("{n}", &n.to_string())),
                theme::faint(),
            ));
        };
        if total > room {
            if start > 0 {
                note(self, more.0, start);
            } else {
                self.blank();
            }
        }
        for (i, spans) in rows.into_iter().enumerate().take(end).skip(start) {
            self.row(i == cursor, spans, None);
        }
        if total > room && end < total {
            note(self, more.1, total - end);
        }
    }

    /// 名字后面放得下几列。
    fn room(&self, label_w: usize) -> usize {
        usize::from(self.width)
            .saturating_sub(GAP.width() + label_w + 2 + 1)
            .max(4)
    }
}

/// 一格前面的名字（照 `label_w` 对齐）：光标在的那一格前面 `❯`、名字强调色加粗，别的暗色（「第一次打开的引导」第 7 条）。
fn labelled(label: &str, label_w: usize, focused: bool) -> Vec<Span<'static>> {
    let (lead, style) = if focused {
        (
            Span::styled(POINTER, theme::accent()),
            theme::accent().add_modifier(Modifier::BOLD),
        )
    } else {
        (Span::raw(GAP), theme::dim())
    };
    vec![
        lead,
        Span::styled(label.to_string(), style),
        Span::raw(" ".repeat(label_w.saturating_sub(label.width()) + 2)),
    ]
}

/// 格子里的一行，补到 `room + 1` 列宽：`cue` 不是空的暗色写在右端，字太长的截掉写「…」。
fn boxed(body: Span<'static>, room: usize, cue: &str, ground: Style) -> Vec<Span<'static>> {
    let total = room + 1;
    if cue.is_empty() {
        let pad = total.saturating_sub(body.content.width());
        return vec![body, Span::styled(" ".repeat(pad), ground)];
    }
    let cue_w = cue.width();
    let room_for_text = u16::try_from(total.saturating_sub(cue_w + 2)).unwrap_or(u16::MAX);
    let text = crate::ui::rows::clip(&body.content, room_for_text);
    let pad = total.saturating_sub(text.width() + cue_w + 1);
    vec![
        Span::styled(text, body.style),
        Span::styled(" ".repeat(pad), ground),
        Span::styled(cue.to_string(), ground.patch(theme::faint())),
        Span::styled(" ", ground),
    ]
}

/// 名字那一截有多宽的空白。
fn lead(label: &str, label_w: usize) -> String {
    format!(
        "{GAP}{label}{}  ",
        " ".repeat(label_w.saturating_sub(label.width()))
    )
}

/// 格子的底：光标在的那一格（浮窗里的都算）浅一档（选中一行用的深底留给列表），别的铺面板的底。
fn ground(focused: bool) -> Style {
    if focused {
        theme::row_dim()
    } else {
        theme::panel()
    }
}

/// 一行补到 `width` 列宽，铺上 `ground` 的底色（没有的不补）。
pub fn fill(mut spans: Vec<Span<'static>>, width: u16, ground: Option<Style>) -> Line<'static> {
    let Some(ground) = ground else {
        return Line::from(spans);
    };
    let used: usize = spans.iter().map(|s| s.content.width()).sum();
    spans.push(Span::raw(
        " ".repeat(usize::from(width).saturating_sub(used)),
    ));
    for span in &mut spans {
        span.style = ground.patch(span.style);
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests;
