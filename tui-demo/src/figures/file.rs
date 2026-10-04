//! 本机的图片文件（蓝图 `tui.md`「图片、公式和 mermaid 图」第 3、8 条）：读进来、照写的宽高定大小、缩到正文宽；
//! `.svg` 文件照第 9 条画。
//! 地址的认法在 `local.rs`。

use image::imageops::{self, FilterType};
use image::{Rgba, RgbaImage};

use super::cells::{self, Cell, Fit};
use crate::markdown::Size;

/// 读图，缩到最多 `max_cols` 列宽，放到正好铺满格子的透明画布上。`size` 是 `<img>` 写的宽高（像素）：
/// 写了的照它，只写一个的另一个照原图的比例算，没写的照图本身。`.svg` 文件照回答里写的 `<svg>` 画
/// （不读它引用的外部东西），`fonts` 是它用的中文字体。
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
    fonts: &[String],
) -> Result<(RgbaImage, Fit), String> {
    let file = crate::local::resolve(url).ok_or("读不出工作目录")?;
    let failed = |e: &dyn std::fmt::Display| format!("{}：{e}", file.display());
    if is_svg(&file) {
        let text = std::fs::read_to_string(&file).map_err(|e| failed(&e))?;
        let tree = super::svg::tree(&text, fonts).map_err(|e| failed(&e))?;
        let natural = tree.size();
        let wanted = sized(size, (natural.width(), natural.height()));
        return super::svg::raster_at(&tree, wanted, cell, max_cols, max_rows);
    }
    let image = image::ImageReader::open(&file)
        .and_then(image::ImageReader::with_guessed_format)
        .map_err(|e| failed(&e))?
        .decode()
        .map_err(|e| failed(&e))?
        .to_rgba8();
    let (w, h) = sized(
        size,
        (image.width().max(1) as f32, image.height().max(1) as f32),
    );
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

/// 是不是 SVG 文件：照扩展名认。
fn is_svg(file: &std::path::Path) -> bool {
    file.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("svg"))
}

/// 照 `<img>` 写的宽高定多大（像素）：写了的照它，只写一个的另一个照原图 `(iw, ih)` 的比例算，没写的照原图。
fn sized(size: Size, (iw, ih): (f32, f32)) -> (f32, f32) {
    let (iw, ih) = (iw.max(1.0), ih.max(1.0));
    match size {
        (Some(w), Some(h)) => (w as f32, h as f32),
        (Some(w), None) => (w as f32, w as f32 * ih / iw),
        (None, Some(h)) => (h as f32 * iw / ih, h as f32),
        (None, None) => (iw, ih),
    }
}

#[cfg(test)]
mod tests;
