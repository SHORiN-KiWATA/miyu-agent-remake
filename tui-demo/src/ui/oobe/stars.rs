//! 画星点（「第一次打开的引导」第 8、11 条）：位置是窗口的几成，换成格子；越亮挑越亮的字、越亮的颜色。

use ratatui::Frame;
use ratatui::layout::{Position, Rect};

use crate::oobe::motion::Star;
use crate::theme;

/// 在 `area` 里画这一帧的星。
pub fn draw(frame: &mut Frame, area: Rect, stars: &[Star], marks: &[String]) {
    if marks.is_empty() || area.width == 0 || area.height == 0 {
        return;
    }
    let top = marks.len() - 1;
    let buf = frame.buffer_mut();
    for star in stars {
        let col = (star.x * f64::from(area.width - 1)).round() as u16;
        let row = (star.y * f64::from(area.height - 1)).round() as u16;
        let at = Position::new(area.x + col, area.y + row);
        let pick = ((star.level * top as f64).round() as usize).min(top);
        let style = if star.level < 0.4 {
            theme::faint()
        } else if star.level < 0.75 {
            theme::dim()
        } else {
            theme::accent()
        };
        if let Some(cell) = buf.cell_mut(at) {
            cell.set_symbol(&marks[pick]).set_style(style);
        }
    }
}

/// `rect` 的中心在 `area` 里的几成：星点往这里聚。
pub fn center(area: Rect, rect: Rect) -> (f64, f64) {
    let x = f64::from(rect.x) + f64::from(rect.width) / 2.0 - f64::from(area.x);
    let y = f64::from(rect.y) + f64::from(rect.height) / 2.0 - f64::from(area.y);
    (
        (x / f64::from(area.width.max(1))).clamp(0.0, 1.0),
        (y / f64::from(area.height.max(1))).clamp(0.0, 1.0),
    )
}
