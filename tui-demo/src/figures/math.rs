//! 块级公式排成图（蓝图 `tui.md`「图片、公式和 mermaid 图」第 5、7 条）：RaTeX 排版，
//! 字号照一格的像素高，宽过正文的按比例重排一次，不缩放位图。

use image::{Rgba, RgbaImage};
use ratex_layout::{LayoutOptions, layout, to_display_list};
use ratex_parser::parser::parse;
use ratex_render::{RenderOptions, render_to_png};
use ratex_types::color::Color;
use ratex_types::math_style::MathStyle;

use super::cells::{self, Cell, Fit};

/// 公式四周留的像素。
const PADDING: f32 = 2.0;

/// 排一个公式：字色 `color`，字号是一格高的 `scale` 倍，最多 `max_cols` 列宽。
///
/// # Errors
///
/// RaTeX 读不懂、排不出来时返回原因。
pub fn draw(
    tex: &str,
    color: (u8, u8, u8),
    scale: f32,
    cell: Cell,
    max_cols: u16,
    max_rows: u16,
) -> Result<(RgbaImage, Fit), String> {
    quiet_fonts();
    let normalized = tex.split_whitespace().collect::<Vec<_>>().join(" ");
    let ast = parse(&normalized).map_err(|e| e.to_string())?;
    let (r, g, b) = color;
    let options = LayoutOptions::default()
        .with_style(MathStyle::Display)
        .with_color(Color {
            r: f32::from(r) / 255.0,
            g: f32::from(g) / 255.0,
            b: f32::from(b) / 255.0,
            a: 1.0,
        });
    let list = to_display_list(&layout(&ast, &options));
    let em = cell.height as f32 * scale;
    let first = png(&list, em)?;
    let fit = cells::fit(
        first.width() as f32,
        first.height() as f32,
        cell,
        max_cols,
        max_rows,
    );
    // 宽过正文的，字号照比例缩了重排，不去缩位图（缩位图字会糊）。
    let image = if fit.scale < 1.0 {
        png(&list, em * fit.scale)?
    } else {
        first
    };
    Ok((cells::pad(&image, fit, cell, Rgba([0, 0, 0, 0]), true), fit))
}

fn png(list: &ratex_types::display_item::DisplayList, em: f32) -> Result<RgbaImage, String> {
    let options = RenderOptions {
        font_size: em,
        padding: PADDING,
        background_color: Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        },
        font_dir: String::new(),
        device_pixel_ratio: 1.0,
    };
    let bytes = render_to_png(list, &options)?;
    Ok(image::load_from_memory(&bytes)
        .map_err(|e| e.to_string())?
        .to_rgba8())
}

/// RaTeX 第一次找中文字体时往 stderr 打一行，全屏界面会被搅乱：第一次排公式前，把 stderr 临时指到
/// 空设备，用一个带中文的小公式把字体找好，再指回来（蓝图第 7 条；照旧版 `silence_first_font_load`）。
/// 只这一次、只这一小段；字体找好以后缓存在进程里，不会再打。
fn quiet_fonts() {
    static ONCE: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    ONCE.get_or_init(|| {
        #[cfg(unix)]
        {
            use std::os::fd::AsFd;
            let null = std::fs::File::options().write(true).open("/dev/null");
            let saved = rustix::io::dup(std::io::stderr().as_fd());
            if let (Ok(null), Ok(saved)) = (null, saved)
                && rustix::stdio::dup2_stderr(&null).is_ok()
            {
                warm_up();
                if rustix::stdio::dup2_stderr(&saved).is_err() {
                    // 指不回来了：界面照画，只是之后的报错看不见。
                }
                return;
            }
        }
        warm_up();
    });
}

/// 排一个带中文的小公式，只为让 RaTeX 把回退字体找好；排出来的扔掉。
/// 必须带中文：纯 ASCII 的公式用不上回退字体，找字体那一行留到真公式来了才打（旧版踩过）。
fn warm_up() {
    if let Ok(ast) = parse(r"\text{中}") {
        let list = to_display_list(&layout(&ast, &LayoutOptions::default()));
        if png(&list, 12.0).is_err() {
            // 排不出来也没关系：真公式来了照常排，出错的写成一行字。
        }
    }
}

#[cfg(test)]
mod tests {
    use super::draw;
    use crate::figures::cells::Cell;

    const CELL: Cell = Cell {
        width: 10,
        height: 20,
    };

    #[test]
    fn a_formula_fills_whole_cells_and_bad_tex_says_why() {
        let (image, fit) =
            draw(r"\frac{a+1}{\sqrt{b}}", (192, 202, 245), 1.0, CELL, 80, 200).unwrap();
        assert_eq!(
            (image.width(), image.height()),
            (u32::from(fit.cols) * 10, u32::from(fit.rows) * 20)
        );
        assert!(fit.rows >= 2, "分式至少两行高：{fit:?}");
        assert!(draw(r"\frac{", (0, 0, 0), 1.0, CELL, 80, 200).is_err());
    }

    #[test]
    fn a_wide_formula_is_set_again_smaller_to_fit() {
        let long = r"a+b+c+d+e+f+g+h+i+j+k+l+m+n+o+p+q+r+s+t+u+v+w+x+y+z";
        let (image, fit) = draw(long, (0, 0, 0), 1.0, CELL, 20, 200).unwrap();
        assert!(fit.cols <= 20 && fit.scale < 1.0);
        assert_eq!(image.width(), u32::from(fit.cols) * 10);
    }
}
