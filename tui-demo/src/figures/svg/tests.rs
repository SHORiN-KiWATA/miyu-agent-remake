//! 回答里写的 `<svg>`（蓝图 `tui.md`「图片、公式和 mermaid 图」第 9 条）：照自然大小算格子；不读外面的东西。

use std::io::Cursor;

use image::{ImageFormat, Rgba, RgbaImage};

use super::draw;
use crate::figures::cells::Cell;

const CELL: Cell = Cell {
    width: 10,
    height: 20,
};

fn png() -> Vec<u8> {
    let mut out = Vec::new();
    RgbaImage::from_pixel(4, 4, Rgba([255, 0, 0, 255]))
        .write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
        .unwrap();
    out
}

fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().fold(0u32, |n, b| n << 8 | u32::from(*b)) << (8 * (3 - chunk.len()));
        for i in 0..4 {
            let c = if i <= chunk.len() {
                ABC[(n >> (18 - 6 * i) & 63) as usize] as char
            } else {
                '='
            };
            out.push(c);
        }
    }
    out
}

#[test]
fn an_svg_without_a_namespace_takes_its_natural_size() {
    // 模型在回答里写的 `<svg>` 多半不带 xmlns（HTML 里不用写）。
    let svg =
        r##"<svg width="120" height="80"><rect width="120" height="80" fill="#4fa3ff"/></svg>"##;
    let (image, fit) = draw(svg, &[], CELL, 100, 100).unwrap();
    assert_eq!((fit.cols, fit.rows), (12, 4));
    assert_eq!(image.dimensions(), (120, 80));
    assert!(image.pixels().all(|p| p.0[3] > 0), "铺满了");
    let (_, narrow) = draw(svg, &[], CELL, 6, 100).unwrap();
    assert_eq!(narrow.cols, 6, "宽过正文的等比缩");
}

#[test]
fn only_inline_svg_images_are_read() {
    let bytes = png();
    let path = std::env::temp_dir().join(format!("miyu-svg-{}.png", std::process::id()));
    std::fs::write(&path, &bytes).unwrap();
    let inner = br##"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect width="4" height="4" fill="red"/></svg>"##;
    let file = std::env::temp_dir().join(format!("miyu-svg-{}.svg", std::process::id()));
    std::fs::write(&file, inner).unwrap();
    let seen = |href: String| {
        let svg = format!(
            r#"<svg width="40" height="40"><image href="{href}" width="40" height="40"/></svg>"#
        );
        let (image, _) = draw(&svg, &[], CELL, 100, 100).unwrap();
        image.pixels().any(|p| p.0[3] > 0)
    };
    assert!(!seen(path.display().to_string()), "本机文件不读");
    assert!(
        !seen(format!("file://{}", path.display())),
        "file:// 也不读"
    );
    assert!(!seen(file.display().to_string()), "本机的 SVG 文件也不读");
    assert!(
        !seen(format!("data:image/png;base64,{}", base64(&bytes))),
        "位图不画"
    );
    assert!(
        seen(format!("data:image/svg+xml;base64,{}", base64(inner))),
        "内嵌的 SVG 照画"
    );
    std::fs::remove_file(&path).unwrap_or_default();
    std::fs::remove_file(&file).unwrap_or_default();
}
