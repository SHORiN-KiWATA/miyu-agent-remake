//! 终端光标（蓝图 `tui.md`「每一帧」）：界面自己管，不交给 ratatui。每一帧画的时候记下光标该在哪、显不显示，
//! 画完由 [`place`] 一次写出去：先挪到那里，要显示时再显示。
//!
//! 不显示时也挪到同一个地方（输入框的插入点）：不挪的话光标停在这一帧最后写的格子，转轮、token 数、吉祥物轮流变，
//! 藏着的光标跟着跳，开了拖尾的终端（kitty 的 `cursor_trail`）照样给它画拖尾。

use std::io::{self, Write};

use ratatui::crossterm::{cursor, queue};
use ratatui::layout::Position;

/// 这一帧的光标。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Caret {
    /// 停在哪一格：输入框的插入点，抽屉里有搜索框时在搜索框里。输入框没画出来就留着上一帧的。
    pub at: Position,
    /// 显不显示：焦点在输入框、或者抽屉的搜索框在等字。
    pub shown: bool,
}

impl Caret {
    /// 新的一帧：先当不显示，停的地方留着上一帧的，画到输入框、抽屉时再改。
    pub fn begin(&mut self) {
        self.shown = false;
    }

    /// 停在 `at`，`shown` 说显不显示。
    pub fn put(&mut self, at: Position, shown: bool) {
        self.at = at;
        self.shown = shown;
    }
}

/// 画完一帧、同步输出结尾之前写：挪过去，要显示时再显示。ratatui 那边这一帧没设光标，已经藏起来了。
///
/// # Errors
///
/// 写不出去。
pub fn place(caret: Caret, out: &mut impl Write) -> io::Result<()> {
    queue!(out, cursor::MoveTo(caret.at.x, caret.at.y))?;
    if caret.shown {
        queue!(out, cursor::Show)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use ratatui::layout::Position;

    use super::{Caret, place};

    #[test]
    fn it_moves_before_it_shows() {
        let mut out = Vec::new();
        place(
            Caret {
                at: Position::new(4, 2),
                shown: true,
            },
            &mut out,
        )
        .unwrap();
        assert_eq!(
            out, b"\x1b[3;5H\x1b[?25h",
            "先挪后显示：不认同步输出的终端也不在最后写的格子闪一下"
        );
    }

    #[test]
    fn hidden_it_still_parks_at_the_same_place() {
        let mut out = Vec::new();
        place(
            Caret {
                at: Position::new(4, 2),
                shown: false,
            },
            &mut out,
        )
        .unwrap();
        assert_eq!(out, b"\x1b[3;5H", "不显示也挪回插入点，不留在最后写的格子");
    }

    #[test]
    fn a_new_frame_keeps_the_place_but_hides() {
        let mut caret = Caret::default();
        caret.put(Position::new(7, 9), true);
        caret.begin();
        assert_eq!(
            caret,
            Caret {
                at: Position::new(7, 9),
                shown: false
            }
        );
    }
}
