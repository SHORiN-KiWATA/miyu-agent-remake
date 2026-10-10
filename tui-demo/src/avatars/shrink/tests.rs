use std::path::PathBuf;

use image::{ImageFormat, Rgba, RgbaImage};

use super::{Look, ready_with};

fn dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("miyu-shrink-{tag}-{}", std::process::id()));
    drop(std::fs::remove_dir_all(&dir));
    drop(std::fs::create_dir_all(&dir));
    dir
}

fn look(out: &std::path::Path) -> Look {
    Look {
        max_side: 1024,
        max_bytes: 1024 * 1024,
        jpeg_quality: 85,
        out: Some(out.to_path_buf()),
    }
}

/// 一张有杂点的图（压不小，PNG 存出来够大）。
fn noisy(w: u32, h: u32) -> RgbaImage {
    RgbaImage::from_fn(w, h, |x, y| {
        let v = x
            .wrapping_mul(2_654_435_761)
            .wrapping_add(y.wrapping_mul(40_503));
        Rgba([(v >> 3) as u8, (v >> 11) as u8, (v >> 19) as u8, 255])
    })
}

#[test]
fn a_big_photo_is_shrunk_under_the_cores_limits_before_upload() {
    // 2026-10-11 项目主人：头像超过 1 MiB 或 1024 像素就拒「不太合理」，照推荐由终端先缩小再传。
    let dir = dir("big");
    let big = dir.join("photo.png");
    noisy(3000, 2000)
        .save_with_format(&big, ImageFormat::Png)
        .expect("写图");
    let got = ready_with(&big, &look(&dir.join("out")));
    assert_ne!(got, big, "换成缩小的那份");
    let bytes = std::fs::read(&got).expect("读缩小的");
    assert!(bytes.len() <= 1024 * 1024, "不超过 1 MiB：{}", bytes.len());
    let small = image::load_from_memory(&bytes).expect("是图");
    assert_eq!(
        (small.width(), small.height()),
        (1024, 683),
        "照比例缩到最长边 1024"
    );
    drop(std::fs::remove_dir_all(&dir));
}

#[test]
fn a_small_picture_goes_as_it_is_and_others_are_converted() {
    let dir = dir("small");
    let small = dir.join("small.png");
    noisy(64, 64)
        .save_with_format(&small, ImageFormat::Png)
        .expect("写图");
    assert_eq!(
        ready_with(&small, &look(&dir.join("out"))),
        small,
        "没超的原样传"
    );
    // 核心不认的格式（GIF）：转成 PNG。
    let gif = dir.join("cat.gif");
    noisy(64, 64)
        .save_with_format(&gif, ImageFormat::Gif)
        .expect("写图");
    let got = ready_with(&gif, &look(&dir.join("out")));
    assert_eq!(got.extension().and_then(|e| e.to_str()), Some("png"));
    // 不是图的：原样交给核心，照核心的原话拒。
    let text = dir.join("note.txt");
    std::fs::write(&text, "hi").expect("写字");
    assert_eq!(ready_with(&text, &look(&dir.join("out"))), text);
    drop(std::fs::remove_dir_all(&dir));
}

#[test]
fn the_shipped_numbers_parse() {
    assert!(super::shipped().is_some(), "resources/avatar.json 读得出来");
}
