//! 侧边栏的字能拖选（蓝图 `tui.md`「后台命令、子代理和侧边栏」第 7 条）：按下、拖动定选区，松开由外面复制并保留选区；
//! 只点不拖的算点。画的时候把写了字的行记下来，复制时照屏幕的列切。

use unicode_width::UnicodeWidthStr;

/// 侧边栏上写了字的一行：在屏幕的哪一行、从哪一列起、写的字。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextRow {
    /// 屏幕的第几行。
    pub y: u16,
    /// 从屏幕的第几列起。
    pub x: u16,
    /// 写的字。
    pub text: String,
}

/// 选区和上一帧写了字的行。
#[derive(Debug, Default)]
pub struct SideSelect {
    rows: Vec<TextRow>,
    /// 按下的地方。
    anchor: Option<(u16, u16)>,
    /// 拖到的地方。
    head: Option<(u16, u16)>,
    /// 按下以后动过。
    moved: bool,
    /// 正在拖选，松开后仍留选区但不再抓鼠标事件。
    pressed: bool,
}

impl SideSelect {
    /// 记下这一帧写了字的行。
    pub fn set_rows(&mut self, rows: Vec<TextRow>) {
        self.rows = rows;
    }

    /// 按下：从这里开始，之前的选区作废。
    pub fn press(&mut self, x: u16, y: u16) {
        self.anchor = Some((x, y));
        self.head = Some((x, y));
        self.moved = false;
        self.pressed = true;
    }

    /// 拖到这里。
    pub fn drag(&mut self, x: u16, y: u16) {
        if self.anchor.is_none() {
            return;
        }
        self.moved |= self.anchor != Some((x, y));
        self.head = Some((x, y));
    }

    /// 松开：只点没拖的算点一下，交回 `true`，选区不留。
    pub fn release(&mut self) -> bool {
        self.pressed = false;
        let click = self.anchor.is_some() && !self.moved;
        if click {
            self.clear();
        }
        click
    }

    /// 取消选区。
    pub fn clear(&mut self) {
        self.pressed = false;
        self.anchor = None;
        self.head = None;
        self.moved = false;
    }

    /// 按着还没松开。
    pub fn pressing(&self) -> bool {
        self.pressed
    }

    /// 有选区。
    pub fn active(&self) -> bool {
        self.moved && self.anchor.is_some()
    }

    /// 起、止两头，照先上后下、先左后右（行、列）。
    fn ends(&self) -> Option<((u16, u16), (u16, u16))> {
        let (a, h) = (self.anchor?, self.head?);
        if !self.moved {
            return None;
        }
        let key = |(x, y): (u16, u16)| (y, x);
        Some(if key(a) <= key(h) { (a, h) } else { (h, a) })
    }

    /// 第 `y` 行选中的是哪几列（起、止，不含止）；这一行没选中、没写字是 `None`。
    pub fn cols(&self, y: u16) -> Option<(u16, u16)> {
        let ((sx, sy), (ex, ey)) = self.ends()?;
        let row = self.rows.iter().find(|r| r.y == y)?;
        if y < sy || y > ey {
            return None;
        }
        let end_of_row = row.x + u16::try_from(row.text.width()).unwrap_or(u16::MAX);
        let from = if y == sy { sx } else { row.x };
        let to = if y == ey { ex } else { end_of_row };
        (from < to).then_some((from, to.min(end_of_row)))
    }

    /// 选中的字：一行一行，照屏幕的样子，行首的缩进、行尾的空格不要。
    pub fn text(&self) -> String {
        self.rows
            .iter()
            .filter_map(|row| {
                let (from, to) = self.cols(row.y)?;
                let piece =
                    crate::body_view::slice(&row.text, from - row.x.min(from), to - row.x.min(to));
                Some(piece.trim().to_string())
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::{SideSelect, TextRow};

    fn select() -> SideSelect {
        let mut s = SideSelect::default();
        s.set_rows(vec![
            TextRow {
                y: 10,
                x: 100,
                text: "工作目录".into(),
            },
            TextRow {
                y: 11,
                x: 100,
                text: "  ~/Documents/github/".into(),
            },
            TextRow {
                y: 12,
                x: 100,
                text: "  miyu-agent-remake".into(),
            },
        ]);
        s
    }

    #[test]
    fn dragging_selects_by_screen_columns_across_rows() {
        let mut s = select();
        // 从第 11 行的「~」拖到第 12 行「miyu」的末尾。
        s.press(102, 11);
        s.drag(106, 12);
        assert!(s.pressing());
        assert!(!s.release(), "拖过了，不算点");
        assert!(!s.pressing(), "松开后不再抓后续的鼠标事件");
        assert!(s.active());
        assert_eq!(s.text(), "~/Documents/github/\nmiyu");
        assert_eq!(
            s.cols(11),
            Some((102, 121)),
            "起头那一行选到行尾（这一行的最后一个字）"
        );
        assert_eq!(s.cols(12), Some((100, 106)));
        assert_eq!(s.cols(10), None);
    }

    #[test]
    fn dragging_backwards_and_over_wide_characters() {
        let mut s = select();
        // 从第 10 行「目」的后面往回拖到开头：两个汉字四列。
        s.press(104, 10);
        s.drag(100, 10);
        s.release();
        assert_eq!(s.text(), "工作");
    }

    #[test]
    fn a_click_is_not_a_selection() {
        let mut s = select();
        s.press(103, 11);
        assert!(s.release(), "只点不拖：算点一下");
        assert!(!s.active());
        // 再拖一次，Esc 取消。
        s.press(102, 11);
        s.drag(110, 11);
        s.release();
        s.clear();
        assert!(!s.active());
        assert_eq!(s.text(), "");
    }
}
