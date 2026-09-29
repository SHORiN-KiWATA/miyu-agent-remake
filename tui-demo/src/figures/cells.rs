//! 像素和格子的换算（蓝图 `tui.md`「图片、公式和 mermaid 图」第 3、4 条）：先算出图占几格，
//! 再照格子乘出像素渲，不再缩放；只限宽度，行数只防病态。

use image::{Rgba, RgbaImage, imageops};

/// 一格多少像素，启动时问终端问出来的。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    /// 宽。
    pub width: u32,
    /// 高。
    pub height: u32,
}

/// 一张图占的格子，和把自然大小的图缩到这些格子里要乘的比例（不放大，最大是 1）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fit {
    /// 几列。
    pub cols: u16,
    /// 几行。
    pub rows: u16,
    /// 缩放比例。
    pub scale: f32,
}

impl Fit {
    /// 渲出来的图该有多少像素：正好铺满格子。
    pub fn pixels(self, cell: Cell) -> (u32, u32) {
        (
            u32::from(self.cols) * cell.width,
            u32::from(self.rows) * cell.height,
        )
    }
}

/// 自然大小是 `width` × `height` 像素的图占几格：照自然大小，宽过 `max_cols` 的等比缩到那么宽；
/// 缩完还高过 `max_rows` 的再等比缩（只防病态）。
pub fn fit(width: f32, height: f32, cell: Cell, max_cols: u16, max_rows: u16) -> Fit {
    let (cw, ch) = (cell.width.max(1) as f32, cell.height.max(1) as f32);
    let (width, height) = (width.max(1.0), height.max(1.0));
    let mut scale = (f32::from(max_cols.max(1)) * cw / width).min(1.0);
    if height * scale / ch > f32::from(max_rows.max(1)) {
        scale = f32::from(max_rows.max(1)) * ch / height;
    }
    let cells = |px: f32, per: f32| (px * scale / per).ceil().clamp(1.0, f32::from(u16::MAX));
    Fit {
        cols: cells(width, cw) as u16,
        rows: cells(height, ch) as u16,
        scale,
    }
}

/// 放到正好铺满格子的画布上，多出来的地方是 `fill`：左对齐；`middle` 为真时上下居中（公式），不然贴顶。
pub fn pad(image: &RgbaImage, fit: Fit, cell: Cell, fill: Rgba<u8>, middle: bool) -> RgbaImage {
    let (w, h) = fit.pixels(cell);
    let mut canvas = RgbaImage::from_pixel(w, h, fill);
    let top = if middle {
        h.saturating_sub(image.height()) / 2
    } else {
        0
    };
    imageops::overlay(&mut canvas, image, 0, i64::from(top));
    canvas
}

#[cfg(test)]
mod tests {
    use super::{Cell, fit};

    const CELL: Cell = Cell {
        width: 10,
        height: 20,
    };

    #[test]
    fn natural_size_rounds_up_to_whole_cells() {
        let f = fit(95.0, 41.0, CELL, 80, 200);
        assert_eq!((f.cols, f.rows, f.scale), (10, 3, 1.0));
        assert_eq!(f.pixels(CELL), (100, 60));
    }

    #[test]
    fn too_wide_shrinks_to_the_width_and_keeps_the_ratio() {
        let f = fit(1600.0, 400.0, CELL, 80, 200);
        assert_eq!((f.cols, f.rows), (80, 10));
        assert!((f.scale - 0.5).abs() < 1e-6);
    }

    #[test]
    fn tall_is_not_squeezed_into_a_screen_only_capped() {
        // 800×4000 像素：80 列放得下，200 行是 4000/20，照原样，不为一屏缩小。
        let f = fit(800.0, 4000.0, CELL, 80, 200);
        assert_eq!((f.cols, f.rows), (80, 200));
        // 高过上限的才缩。
        let f = fit(800.0, 8000.0, CELL, 80, 200);
        assert_eq!((f.cols, f.rows), (40, 200));
    }
}
