//! 输入框的留白，按窗口宽度定（蓝图 `tui.md`「输入框」第 1、2、10 条）：够宽时两边留白、框里左右留几列；
//! 少于 `compact_below` 列（手机上 ssh 连过来）换紧凑版面，框贴着两边，框里只留提示符和光标的地方。

use crate::config::Layout;

/// 这一帧的留白。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Margins {
    /// 框外面两边各留几列。
    pub side_gap: u16,
    /// 框里左边几列：提示符住在这里。
    pub pad_left: u16,
    /// 框里右边几列：给行尾的光标。
    pub pad_right: u16,
    /// 紧凑版面：框占满整行。
    pub compact: bool,
}

/// 窗口 `width` 列时的留白。
pub fn margins(layout: &Layout, width: u16) -> Margins {
    if width < layout.compact_below {
        Margins {
            side_gap: 0,
            pad_left: layout.compact_pad_left,
            pad_right: layout.compact_pad_right,
            compact: true,
        }
    } else {
        Margins {
            side_gap: layout.side_gap,
            pad_left: layout.pad_left,
            pad_right: layout.pad_right,
            compact: false,
        }
    }
}
