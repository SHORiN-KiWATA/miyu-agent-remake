//! 正文里的图（`resources/figures.json`，蓝图 `tui.md`「图片、公式和 mermaid 图」第 3 条）：多大、记几张、用什么字体。

use serde::Deserialize;

/// 正文里的图怎么画。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FigureLook {
    /// 一张图最多占几行：只防病态，不为塞进一屏。
    pub max_rows: u16,
    /// mermaid、公式、回答里写的 `<svg>` 最多多大：里面是字，缩多了看不清。
    pub room: Room,
    /// 图片（本机的文件、`<img>`）最多多大。
    pub picture_room: Room,
    /// mermaid 图里的字体，照先后找；都没有的由 resvg 从系统里找。
    pub fonts: Vec<String>,
    /// 块级公式的字号是一格高的几倍。
    pub math_scale: f32,
    /// 最多记着几张做好的图。
    pub keep: usize,
    /// 点开看的 mermaid 大图，缓存目录里最多留几张。
    pub zoom_keep: usize,
}

/// 一张图最多占多大：窗口高度的几分之几、正文宽度的几分之几，写成 `[分子, 分母]`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Room {
    /// 最多窗口高度的几分之几。
    pub height: [u16; 2],
    /// 最多正文宽度的几分之几。
    pub width: [u16; 2],
    /// 窗口再矮也至少给几行。
    pub min_rows: u16,
}

impl Room {
    /// 正文 `cols` 列宽、窗口 `screen_rows` 行高时，最多几列宽、几行高。
    pub fn fit(&self, cols: u16, screen_rows: u16) -> (u16, u16) {
        let part = |whole: u16, [num, den]: [u16; 2]| {
            let got = u32::from(whole) * u32::from(num) / u32::from(den.max(1));
            u16::try_from(got).unwrap_or(whole)
        };
        (
            part(cols, self.width).max(1),
            part(screen_rows, self.height).max(self.min_rows),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::Room;

    #[test]
    fn a_room_is_a_share_of_the_width_and_the_window() {
        let third = Room {
            height: [1, 3],
            width: [3, 5],
            min_rows: 3,
        };
        assert_eq!(third.fit(100, 45), (60, 15));
        assert_eq!(third.fit(1, 6), (1, 3), "至少一列、至少 3 行");
    }
}
