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
        let (_, fit) = draw(&url, size, cell, 100, 100, &[]).unwrap();
        (fit.cols, fit.rows)
    };
    assert_eq!(cells((None, None)), (1, 1), "没写的照图本身");
    assert_eq!(cells((Some(40), Some(20))), (4, 1), "写了宽高照写的");
    assert_eq!(cells((Some(40), None)), (4, 2), "只写宽：高照原图比例");
    assert_eq!(cells((None, Some(40))), (4, 2), "只写高：宽照原图比例");
    let (_, fit) = draw(&url, (Some(400), None), cell, 10, 100, &[]).unwrap();
    assert_eq!(fit.cols, 10, "宽过正文的等比缩");
    std::fs::remove_file(&path).unwrap_or_default();
}

#[test]
fn a_local_svg_file_is_drawn_like_an_inline_svg() {
    // 2026-09-29 项目主人问 svg 不能渲染吗：原来 `.svg` 文件当成读不懂，只有那一行字。
    let path = std::env::temp_dir().join(format!("miyu-img-{}.svg", std::process::id()));
    std::fs::write(
        &path,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="40"><rect width="20" height="40" fill="#e5a07a"/></svg>"##,
    )
    .unwrap();
    let url = path.display().to_string();
    let cell = Cell {
        width: 10,
        height: 20,
    };
    let (image, fit) = draw(&url, (None, None), cell, 100, 100, &[]).unwrap();
    assert_eq!((fit.cols, fit.rows), (2, 2), "照它自己的大小");
    assert!(image.pixels().any(|p| p.0[3] > 0), "画出了东西");
    let (_, fit) = draw(&url, (Some(40), None), cell, 100, 100, &[]).unwrap();
    assert_eq!(
        (fit.cols, fit.rows),
        (4, 4),
        "`<img>` 写的宽照第 8 条，高照比例"
    );
    std::fs::remove_file(&path).unwrap_or_default();
}

#[test]
fn ppm_and_bmp_files_are_read() {
    // 2026-09-30 项目主人：`grim` 截的 `.ppm` 画不出来。
    for ext in ["ppm", "bmp"] {
        let path = std::env::temp_dir().join(format!("miyu-img-{}.{ext}", std::process::id()));
        image::RgbImage::from_pixel(10, 10, image::Rgb([200, 100, 50]))
            .save(&path)
            .unwrap();
        let cell = Cell {
            width: 10,
            height: 20,
        };
        let drawn = draw(
            &path.display().to_string(),
            (None, None),
            cell,
            100,
            100,
            &[],
        );
        assert!(drawn.is_ok(), "{ext}：{:?}", drawn.err());
        std::fs::remove_file(&path).unwrap_or_default();
    }
}
