//! 正文区的视口和鼠标：滚到哪了、悬在哪、选了哪一段、点了哪一行。
//!
//! - 没滚过时跟着最新的走；滚过以后钉在那里，新来的字不会把看着的地方挤走。
//! - 点开、收起时，被点的那一行留在原来的屏幕位置，展开的内容往下长（设计稿「思考的输出」）。
//! - 拖着选字，松开复制并留着选区，Ctrl+C 仍可复制；复制的字不带行首的竖线和缩进（`13-终端界面.md` 第六节）。

use std::collections::HashSet;

use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use unicode_width::UnicodeWidthChar;

use crate::ui::row_cache::Rows;
use crate::ui::rows::Target;

/// 滚轮一格翻几行。
const WHEEL: usize = 3;

/// 选区的一头：第几行（全部行里的）、正文区里第几列。
pub type Point = (usize, u16);

/// 鼠标在正文上做完一件事以后，要外面做的。
#[derive(Debug, PartialEq, Eq)]
pub enum BodyAction {
    /// 什么都不用做。
    None,
    /// 选区完成，请外面复制已展开的原文。
    Copy(String),
    /// 展开或收起这一样。
    Toggle(Target),
    /// 用系统的打开方式开这个地址。
    Open(String),
}

/// 正文区的状态。
#[derive(Debug, Default)]
pub struct BodyView {
    /// 第一行露出的是第几行；`None` 是跟着最新的。
    pub top: Option<usize>,
    /// 下一帧要把这一样的第一行放在屏幕的这一行（点开、收起时不让它跳）。
    pub anchor: Option<(Target, u16)>,
    /// 鼠标悬在哪一样上。
    pub hover: Option<Target>,
    /// 鼠标悬在哪个链接上。
    pub hover_link: Option<String>,
    /// 上一帧第一行露出的是第几行。
    pub first: usize,
    /// 按过 Ctrl+L：清屏那一刻一共有几行。最底下至少滚到这里，所以清过的视口是空的，往回滚内容还在。
    pub cleared_at: Option<usize>,
    /// 跟着最新的时，视口底边至少露到第几行（不含）：只往下走、不往回退，内容变短时看着的行不掉下来；
    /// 记底边不记第一行，视口变高变矮时贴着底边走，不留空白（`tui.md`「正文」第 1 条）。
    pub floor_end: usize,
    /// 下一帧视口长高（一轮结束，运行状态行那一块收起）时不往下落，见 [`BodyView::hold`]。
    pub hold: bool,
    /// 还算在看的那段正文：编号、第一行是第几行。画之前由正文区填上；长过视口时停在它的开头（`tui.md`「正文」第 1 条）。
    pub reading: Option<(u64, usize)>,
    /// 上一帧最新的那段回答（在写的、写完了后面只有收尾那一行的）：编号、第一行是第几行。写完了的那段，上一帧开头还露着
    /// 才算在看：后到的图、卡片把它撑高时停住，一口气整段到的、补发来的照旧跟着最新的。
    pub latest: Option<(u64, usize)>,
    /// 已经停过的那段正文的编号：一段只停一次，人滚回底以后不再停。
    pub paused: Option<u64>,
    /// 现在钉着的地方是替人停的（不是人滚的）：她开始下一步时放开，见 [`BodyView::resume`]。
    pub auto: bool,
    /// 最后一次跟着最新的时一共有几行：离开底部以后多出来的，回到底部的按钮变成强调色（`ui/bottom_button.rs`）。
    pub seen_end: usize,
    /// 上一帧的全部行（引用按条缓存的那一份，不复制）。
    pub rows: Rows,
    /// 上一帧正文区的位置。
    pub area: Rect,
    /// 选中的一段，两头照拖的先后；画和复制时再排前后。
    pub select: Option<(Point, Point)>,
    /// 按下左键的地方；松开时没拖过就算点了一下。
    press: Option<Point>,
}

impl BodyView {
    /// 处理一个落在正文区上（或者从正文区开始拖）的鼠标事件。
    pub fn mouse(&mut self, event: MouseEvent) -> BodyAction {
        let at = self.point(event.column, event.row);
        match event.kind {
            MouseEventKind::Moved => {
                self.hover = at.and_then(|(r, _)| self.rows.get(r)?.target);
                self.hover_link = at.and_then(|p| self.link_at(p));
            }
            MouseEventKind::Down(MouseButton::Left) => {
                self.press = at;
                self.select = None;
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                // 拖出正文区的上下边，跟着滚一行。
                if event.row < self.area.y {
                    self.scroll_to(self.first.saturating_sub(1));
                } else if event.row >= self.area.bottom() {
                    self.scroll_to(self.first + 1);
                }
                if let (Some(press), Some(head)) =
                    (self.press, self.clamped(event.column, event.row))
                {
                    self.select = Some((press, head)).filter(|(a, b)| a != b);
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                let press = self.press.take();
                // 拖过的：松开复制原文，保留选区；空白和装饰不覆盖剪贴板。
                if press.is_some() && self.select.is_some() {
                    let text = self.selected_text();
                    return if text.is_empty() {
                        BodyAction::None
                    } else {
                        BodyAction::Copy(text)
                    };
                }
                // 点在链接上：开它，不展开收起。
                if let Some(url) = press.and_then(|p| self.link_at(p)) {
                    return BodyAction::Open(url);
                }
                let target = press.and_then(|(r, _)| self.rows.get(r)?.target);
                if let Some(target) = target {
                    // 这一样的第一行在屏幕上的位置；滚出上边的，算在第一行。
                    let above = self.first_row_of(target).saturating_sub(self.first);
                    let y = self.area.y + u16::try_from(above).unwrap_or(0);
                    self.anchor = Some((target, y));
                    return BodyAction::Toggle(target);
                }
            }
            MouseEventKind::ScrollUp => self.scroll_to(self.first.saturating_sub(WHEEL)),
            MouseEventKind::ScrollDown => self.scroll_to(self.first + WHEEL),
            _ => {}
        }
        BodyAction::None
    }

    /// 选中的字：不带行首的槽和引子；折下来的行接回上一行，只在原文换行的地方换行；布局行尾空白去掉，展开原文的空白保留。
    pub fn selected_text(&self) -> String {
        let Some((a, b)) = self.select else {
            return String::new();
        };
        let (start, end) = if a <= b { (a, b) } else { (b, a) };
        // 每行记住最后一次展开的原文到哪里，不能把原文末尾的空格、换行当布局空白删掉。
        let mut lines: Vec<(String, usize)> = Vec::new();
        let mut copied = HashSet::new();
        for r in start.0..=end.0 {
            let Some(row) = self.rows.get(r).filter(|row| row.copy) else {
                continue;
            };
            let from = if r == start.0 { start.1 } else { 0 };
            let to = if r == end.0 { end.1 + 1 } else { u16::MAX };
            let from = from.saturating_sub(row.content_x);
            let to = to.saturating_sub(row.content_x);
            let mut piece = String::new();
            let mut protected = 0;
            let mut at = from;
            for block in &row.copy_blocks {
                let (s, e) = block.cols;
                if from >= e || to <= s {
                    continue;
                }
                piece.push_str(&slice(&row.plain, at, s.max(at)));
                if copied.insert(block.id) {
                    piece.push_str(&block.text);
                    protected = piece.len();
                }
                at = e.min(to);
            }
            piece.push_str(&slice(&row.plain, at, to));
            match lines.last_mut() {
                Some((last, end)) if row.joined => {
                    if protected > 0 {
                        *end = last.len() + protected;
                    }
                    last.push_str(&piece);
                }
                _ => lines.push((piece, protected)),
            }
        }
        lines
            .iter()
            .map(|(line, end)| format!("{}{}", &line[..*end], line[*end..].trim_end()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 第 `row` 行里选中的列 `[起, 止)`，正文区里的列；这一行没选中是 `None`。
    pub fn selected_cols(&self, row: usize) -> Option<(u16, u16)> {
        let (a, b) = self.select?;
        let (start, end) = if a <= b { (a, b) } else { (b, a) };
        if row < start.0 || row > end.0 {
            return None;
        }
        let content_x = self.rows.get(row)?.content_x;
        let from = if row == start.0 { start.1 } else { 0 }.max(content_x);
        let to = if row == end.0 {
            end.1 + 1
        } else {
            self.area.width
        };
        (from < to).then_some((from, to))
    }

    /// 发出一句话、执行一条命令时：滚上去看过的回到最底下、跟着最新的。不动 `floor_end`：本来就跟着的照旧只往下走，
    /// 不因为发了一句话往回跳（撤销后底下空着的那一截，由新的字填上）。
    pub fn follow(&mut self) {
        self.top = None;
        self.auto = false;
    }

    /// 她开始下一步（思考、调工具）：长正文替人停着的，回到最底下接着跟；人自己滚上去的不动（`tui.md`「正文」第 1 条）。
    pub fn resume(&mut self) {
        if self.auto {
            self.follow();
        }
    }

    /// 一轮结束：放开「只往下走」一次，思考、工具收起留下的空白由上面的行补满（`tui.md`「正文」第 1 条）。
    pub fn settle(&mut self) {
        self.floor_end = 0;
    }

    /// 一轮结束、开着「限制工具时间线滚动区域」：运行状态行那一块收起、视口长高的那几行空着，
    /// 不贴着底边往下落，顶出屏幕的行不回来（`tui.md`「正文」第 1 条）。只管下一帧。
    pub fn hold(&mut self) {
        self.hold = true;
    }

    /// Ctrl+L：把视口顶空，往回滚内容还在（照旧版）。新的字从空着的视口顶上往下长：
    /// 清屏那一行就是新的顶（画的时候照 `cleared_at` 定），不用再记底边。
    pub fn clear(&mut self) {
        self.top = None;
        self.floor_end = 0;
        self.cleared_at = Some(self.rows.len());
    }

    /// PgUp、PgDn：往上、往下翻半屏（`tui.md`「按键」）。翻到底由画的时候改回跟着最新的。
    pub fn page(&mut self, down: bool) {
        let step = usize::from((self.area.height / 2).max(1));
        let first = if down {
            self.first + step
        } else {
            self.first.saturating_sub(step)
        };
        self.scroll_to(first);
    }

    fn scroll_to(&mut self, first: usize) {
        self.top = Some(first);
        self.auto = false;
    }

    /// 这一格上的链接。
    fn link_at(&self, (r, col): Point) -> Option<String> {
        let row = self.rows.get(r)?;
        let x = col.checked_sub(row.content_x)?;
        row.links
            .iter()
            .find(|(from, to, _)| (*from..*to).contains(&x))
            .map(|(_, _, url)| url.clone())
    }

    fn first_row_of(&self, target: Target) -> usize {
        self.rows
            .iter()
            .position(|r| r.target == Some(target))
            .unwrap_or(self.first)
    }

    /// 屏幕坐标对应的行和列；不在正文区里的是 `None`。
    fn point(&self, column: u16, row: u16) -> Option<Point> {
        let a = self.area;
        let inside = column >= a.x && column < a.right() && row >= a.y && row < a.bottom();
        inside
            .then(|| (self.first + usize::from(row - a.y), column - a.x))
            .filter(|(r, _)| *r < self.rows.len())
    }

    /// 同上，出了正文区的贴到最近的边上（拖选时用）。
    fn clamped(&self, column: u16, row: u16) -> Option<Point> {
        let a = self.area;
        if a.height == 0 || self.rows.is_empty() {
            return None;
        }
        let row = row.clamp(a.y, a.bottom() - 1);
        let column = column.clamp(a.x, a.right().saturating_sub(1));
        let r = (self.first + usize::from(row - a.y)).min(self.rows.len() - 1);
        Some((r, column - a.x))
    }
}

/// 一行字里第 `[from, to)` 列的那一段，按显示宽度；宽字只要起点落在里面就算。
pub(crate) fn slice(text: &str, from: u16, to: u16) -> String {
    let mut col = 0u16;
    let mut out = String::new();
    for c in text.chars() {
        if col >= from && col < to {
            out.push(c);
        }
        col = col.saturating_add(u16::try_from(c.width().unwrap_or(0)).unwrap_or(0));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{BodyView, slice};
    use crate::ui::rows::Row;
    use ratatui::layout::Rect;
    use ratatui::text::Line;

    fn row(plain: &str) -> Row {
        Row {
            line: Line::default(),
            target: None,
            shade: false,
            plain: plain.into(),
            copy_blocks: Vec::new(),
            content_x: 4,
            joined: false,
            links: Vec::new(),
            copy: true,
            figure: None,
            icon: None,
            figure_pending: false,
        }
    }

    fn view_of(rows: Vec<Row>) -> BodyView {
        BodyView {
            rows: rows.into(),
            area: Rect::new(0, 0, 40, 10),
            ..BodyView::default()
        }
    }

    fn view() -> BodyView {
        view_of(vec![row("你好世界"), row("second line")])
    }

    #[test]
    fn slicing_counts_display_columns() {
        assert_eq!(slice("你好世界", 2, 6), "好世");
        assert_eq!(slice("abc", 1, 99), "bc");
    }

    #[test]
    fn copying_skips_the_gutter_and_spans_rows() {
        let mut v = view();
        // 从第 0 行的第 6 列（「好」）拖到第 1 行的第 9 列（"second"）。
        v.select = Some(((0, 6), (1, 9)));
        assert_eq!(v.selected_text(), "好世界\nsecond");
        // 从槽里开始选的，也不带槽：末列算在里面，选到内容的第 1 列。
        v.select = Some(((1, 0), (1, 5)));
        assert_eq!(v.selected_text(), "se");
    }

    #[test]
    fn releasing_a_drag_copies_and_keeps_the_selection() {
        use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
        let mut v = view();
        let at = |kind, column| MouseEvent {
            kind,
            column,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        v.mouse(at(MouseEventKind::Down(MouseButton::Left), 4));
        v.mouse(at(MouseEventKind::Drag(MouseButton::Left), 7));
        let up = v.mouse(at(MouseEventKind::Up(MouseButton::Left), 7));
        assert_eq!(up, super::BodyAction::Copy("你好".into()));
        assert_eq!(v.selected_text(), "你好");
    }

    #[test]
    fn folded_rows_copy_back_as_one_line() {
        // 第二行是第一行折下来的：复制出来是一行，折行处的空格留着。
        let mut folded = row("world");
        folded.joined = true;
        let mut v = view_of(vec![row("hello "), folded]);
        v.select = Some(((0, 4), (1, 20)));
        assert_eq!(v.selected_text(), "hello world");
    }
}
