//! 问终端能不能显示图（蓝图 `tui.md`「图片、公式和 mermaid 图」第 1 条）。

use ratatui_image::picker::{Picker, ProtocolType};

use super::cells::Cell;

/// 终端能显示图时，编码用的那一份，和一格多少像素。
#[derive(Clone)]
pub struct Graphics {
    /// ratatui-image 的挑选结果：认哪种协议、一格多少像素。
    pub picker: Picker,
}

impl Graphics {
    /// 一格多少像素。
    pub fn cell(&self) -> Cell {
        let size = self.picker.font_size();
        Cell {
            width: u32::from(size.width.max(1)),
            height: u32::from(size.height.max(1)),
        }
    }
}

/// 问一次终端。认得 kitty、sixel、iTerm2 之一的交回 `Some`；只能半格拼图的、没回话的交回 `None`
/// （设计：三种都不认的终端显示源码，不用半格字符拼图）。
///
/// 要在进了全屏、开始读按键之前问：它自己读终端的回话，最多等 2 秒。
pub fn probe() -> Option<Graphics> {
    let picker = Picker::from_query_stdio().ok()?;
    match picker.protocol_type() {
        ProtocolType::Halfblocks => None,
        ProtocolType::Sixel | ProtocolType::Kitty | ProtocolType::Iterm2 => {
            Some(Graphics { picker })
        }
    }
}
