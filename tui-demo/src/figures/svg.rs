//! SVG 栅格化（蓝图 `tui.md`「图片、公式和 mermaid 图」第 4、9 条）：照 SVG 的自然大小算格子，一次渲到那个尺寸，
//! 底透明。mermaid 出的 SVG、回答里写的 `<svg>` 都走这里。
//!
//! 回答里写的 `<svg>` 不引用外面的东西：`<image>` 只画 `data:` 开头内嵌的 SVG，本机文件、网址都不读；
//! 位图不画（没开 resvg 的位图解码）。

use std::sync::{Arc, OnceLock};

use image::RgbaImage;
use resvg::tiny_skia;
use resvg::usvg::{self, ImageHrefResolver, fontdb};

use super::cells::{self, Cell, Fit};

/// 回答里写的 `<svg>`：照它自己的颜色画，不读外面的东西。`fonts` 是中文字体，照先后找。
///
/// # Errors
///
/// SVG 读不懂、尺寸是 0 时返回原因。
pub fn draw(
    source: &str,
    fonts: &[String],
    cell: Cell,
    max_cols: u16,
    max_rows: u16,
) -> Result<(RgbaImage, Fit), String> {
    let options = usvg::Options {
        font_family: fonts.first().cloned().unwrap_or_default(),
        fontdb: font_db(),
        resources_dir: None,
        image_href_resolver: ImageHrefResolver {
            resolve_data: ImageHrefResolver::default_data_resolver(),
            resolve_string: Box::new(|_, _| None),
        },
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_str(source, &options).map_err(|e| e.to_string())?;
    raster(&tree, cell, max_cols, max_rows)
}

/// 照 SVG 的自然大小算格子（宽过 `max_cols` 的等比缩），一次渲到那个尺寸，底透明。
///
/// # Errors
///
/// 尺寸是 0、像素数对不上时返回原因。
pub fn raster(
    tree: &usvg::Tree,
    cell: Cell,
    max_cols: u16,
    max_rows: u16,
) -> Result<(RgbaImage, Fit), String> {
    let size = tree.size();
    let fit = cells::fit(size.width(), size.height(), cell, max_cols, max_rows);
    let (w, h) = fit.pixels(cell);
    let mut pixmap = tiny_skia::Pixmap::new(w, h).ok_or("图的尺寸是 0")?;
    let scale = tiny_skia::Transform::from_scale(fit.scale, fit.scale);
    resvg::render(tree, scale, &mut pixmap.as_mut());
    let rgba: Vec<u8> = pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect();
    let image = RgbaImage::from_raw(w, h, rgba).ok_or("像素数对不上")?;
    Ok((image, fit))
}

/// 系统字体库，每个进程只扫一遍（字体多的机器上冷启动要一秒多，放在后台线程里第一次用时扫）。
pub fn font_db() -> Arc<fontdb::Database> {
    static DB: OnceLock<Arc<fontdb::Database>> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        Arc::new(db)
    })
    .clone()
}

#[cfg(test)]
mod tests;
