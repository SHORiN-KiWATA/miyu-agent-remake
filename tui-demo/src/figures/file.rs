//! 本机的图片文件（蓝图 `tui.md`「图片、公式和 mermaid 图」第 3 条）：读进来、缩到正文宽。地址的认法在 `local.rs`。

use image::{Rgba, RgbaImage};

use super::cells::{self, Cell, Fit};

/// 读图，缩到最多 `max_cols` 列宽，放到正好铺满格子的透明画布上。
///
/// # Errors
///
/// 文件读不出来、不是认得的图时返回原因。
pub fn draw(
    url: &str,
    cell: Cell,
    max_cols: u16,
    max_rows: u16,
) -> Result<(RgbaImage, Fit), String> {
    let file = crate::local::resolve(url).ok_or("读不出工作目录")?;
    let image = image::ImageReader::open(&file)
        .and_then(image::ImageReader::with_guessed_format)
        .map_err(|e| format!("{}：{e}", file.display()))?
        .decode()
        .map_err(|e| format!("{}：{e}", file.display()))?
        .to_rgba8();
    let fit = cells::fit(
        image.width() as f32,
        image.height() as f32,
        cell,
        max_cols,
        max_rows,
    );
    let image = cells::shrink(image, fit);
    Ok((
        cells::pad(&image, fit, cell, Rgba([0, 0, 0, 0]), false),
        fit,
    ))
}
