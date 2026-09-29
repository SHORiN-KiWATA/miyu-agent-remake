//! 宽字后面那一格清成空白（蓝图 `tui.md`「每一帧」）：画完、交给终端之前过一遍。
//!
//! ratatui 比对两帧时认定宽字后面那格是空的；`Paragraph` 画宽字却不清后面那格，列表、面板盖在别的东西上时，
//! 缓冲区里就成了「宽字 + 横线」。等宽字被盖掉，比对只重写前半格，终端上留下半格空白或残色。

use ratatui::buffer::{Buffer, CellDiffOption};
use unicode_width::UnicodeWidthStr;

/// 每个宽字后面那几格不是空白的，照 ratatui 自己写宽字时的做法清掉。图占着的格子不碰。
pub fn tidy(buf: &mut Buffer) {
    let area = buf.area;
    for y in area.top()..area.bottom() {
        let mut x = area.left();
        while x < area.right() {
            let width = u16::try_from(buf[(x, y)].symbol().width()).unwrap_or(1);
            for k in 1..width {
                let Some(next) = buf.cell_mut((x + k, y)) else {
                    break;
                };
                if next.diff_option != CellDiffOption::Skip && next.symbol() != " " {
                    next.reset();
                }
            }
            x += width.max(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::{Color, Style};
    use ratatui::text::Line;
    use ratatui::widgets::{Paragraph, Widget};

    use super::tidy;

    #[test]
    fn a_wide_character_drawn_over_a_line_owns_the_next_cell() {
        let area = Rect::new(0, 0, 6, 1);
        let mut buf = Buffer::empty(area);
        buf.set_string(0, 0, "──────", Style::new().fg(Color::Blue));
        // Paragraph 画宽字不清后面那格：盖在横线上以后，「令」后面还是横线。
        Paragraph::new(Line::raw("命令")).render(Rect::new(1, 0, 4, 1), &mut buf);
        assert_eq!(
            buf[(2, 0)].symbol(),
            "─",
            "先证明 Paragraph 会留下这种缓冲区"
        );
        tidy(&mut buf);
        let row: Vec<&str> = (0..6).map(|x| buf[(x, 0)].symbol()).collect();
        assert_eq!(row, ["─", "命", " ", "令", " ", "─"]);
        // 照这样比对，盖回横线时两格都重写。
        let mut next = Buffer::empty(area);
        next.set_string(0, 0, "──────", Style::new().fg(Color::Blue));
        let written: Vec<u16> = buf.diff(&next).iter().map(|(x, _, _)| *x).collect();
        assert_eq!(written, [1, 2, 3, 4]);
    }

    #[test]
    fn cells_held_by_a_picture_are_left_alone() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 3, 1));
        buf.set_string(0, 0, "图", Style::new());
        buf[(1, 0)]
            .set_symbol("x")
            .set_diff_option(ratatui::buffer::CellDiffOption::Skip);
        tidy(&mut buf);
        assert_eq!(buf[(1, 0)].symbol(), "x");
    }
}
