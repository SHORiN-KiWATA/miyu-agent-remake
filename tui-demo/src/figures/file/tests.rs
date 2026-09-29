//! `<img>` 写的宽高定大小（蓝图 `tui.md`「图片、公式和 mermaid 图」第 8 条）。

use image::{Rgba, RgbaImage};

use super::draw;
use crate::figures::cells::Cell;

#[test]
fn written_width_and_height_decide_the_cells() {
    let path = std::env::temp_dir().join(format!("miyu-img-{}.png", std::process::id()));
    RgbaImage::from_pixel(10, 10, Rgba([0, 128, 255, 255]))
        .save(&path)
        .unwrap();
    let url = path.display().to_string();
    let cell = Cell {
        width: 10,
        height: 20,
    };
    let cells = |size| {
        let (_, fit) = draw(&url, size, cell, 100, 100).unwrap();
        (fit.cols, fit.rows)
    };
    assert_eq!(cells((None, None)), (1, 1), "没写的照图本身");
    assert_eq!(cells((Some(40), Some(20))), (4, 1), "写了宽高照写的");
    assert_eq!(cells((Some(40), None)), (4, 2), "只写宽：高照原图比例");
    assert_eq!(cells((None, Some(40))), (4, 2), "只写高：宽照原图比例");
    let (_, fit) = draw(&url, (Some(400), None), cell, 10, 100).unwrap();
    assert_eq!(fit.cols, 10, "宽过正文的等比缩");
    std::fs::remove_file(&path).unwrap_or_default();
}
