//! 本机的图片文件（蓝图 `tui.md`「图片、公式和 mermaid 图」第 3、8 条）：读进来、照写的宽高定大小、缩到正文宽。
//! 地址的认法在 `local.rs`。

use image::imageops::{self, FilterType};
use image::{Rgba, RgbaImage};

use super::cells::{self, Cell, Fit};
use crate::markdown::Size;

/// 读图，缩到最多 `max_cols` 列宽，放到正好铺满格子的透明画布上。`size` 是 `<img>` 写的宽高（像素）：
/// 写了的照它，只写一个的另一个照原图的比例算，没写的照图本身。
///
/// # Errors
///
/// 文件读不出来、不是认得的图时返回原因。
pub fn draw(
    url: &str,
    size: Size,
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
    let (iw, ih) = (image.width().max(1) as f32, image.height().max(1) as f32);
    let (w, h) = match size {
        (Some(w), Some(h)) => (w as f32, h as f32),
        (Some(w), None) => (w as f32, w as f32 * ih / iw),
        (None, Some(h)) => (h as f32 * iw / ih, h as f32),
        (None, None) => (iw, ih),
    };
    let fit = cells::fit(w, h, cell, max_cols, max_rows);
    let target = (
        ((w * fit.scale).round() as u32).max(1),
        ((h * fit.scale).round() as u32).max(1),
    );
    let image = if (image.width(), image.height()) == target {
        image
    } else {
        imageops::resize(&image, target.0, target.1, FilterType::Triangle)
    };
    Ok((
        cells::pad(&image, fit, cell, Rgba([0, 0, 0, 0]), false),
        fit,
    ))
}

#[cfg(test)]
mod tests;
