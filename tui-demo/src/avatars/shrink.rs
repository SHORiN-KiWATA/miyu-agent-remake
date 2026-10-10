//! 传头像以前先缩小（蓝图 `tui.md`「第一次打开的引导」第 21 条、「配置页」第 36 条，2026-10-11 项目主人定）：核心只收
//! 1 MiB、1024 像素以内的 PNG、JPEG、WebP（核心 P-5），超了的、别的格式（GIF、BMP 这些）终端先照比例缩到最长边 1024、
//! 存成 PNG，还超 1 MiB 的存成 JPEG（透明的地方铺白），写进临时目录的一份文件，把它交给 `blob.put`；传完 [`done`] 删掉。
//! 没超的原样交；读不出来、缩了还超的也原样交，由核心照原话拒。数值在 `resources/avatar.json`。

use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat, ImageReader, Rgb, RgbImage};
use serde::Deserialize;

/// 出厂的数值。
const SHIPPED: &str = include_str!("../../resources/avatar.json");

/// 缩到多大。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Look {
    /// 最长边最多几个像素。
    pub max_side: u32,
    /// 最多几个字节。
    pub max_bytes: usize,
    /// 存成 JPEG 时的质量。
    pub jpeg_quality: u8,
    /// 缩小的那份写在哪个目录；`None` 是系统的临时目录（测试里换成自己的）。
    #[serde(skip)]
    pub out: Option<PathBuf>,
}

/// 读出厂的数值；读不出的是 `None`（那就不缩，原样交）。
pub fn shipped() -> Option<Look> {
    serde_json::from_str(SHIPPED).ok()
}

/// 要交给 `blob.put` 的那份：照出厂的数值缩，用不着缩、缩不成的是原来那份。
pub fn ready(path: &Path) -> PathBuf {
    match shipped() {
        Some(look) => ready_with(path, &look),
        None => path.to_path_buf(),
    }
}

/// 同 [`ready`]，照给的数值。
pub fn ready_with(path: &Path, look: &Look) -> PathBuf {
    shrink(path, look).unwrap_or_else(|| path.to_path_buf())
}

/// 传完了：删掉这个进程写的缩小的那份（有的话）。
pub fn done() {
    for ext in ["png", "jpg"] {
        drop(std::fs::remove_file(out_path(None, ext)));
    }
}

/// 缩小的那份写在哪：`<目录>/miyu-avatar-<进程号>.<扩展名>`，一次只传一张，同一个名字覆盖。
fn out_path(dir: Option<&Path>, ext: &str) -> PathBuf {
    let dir = dir.map_or_else(std::env::temp_dir, Path::to_path_buf);
    dir.join(format!("miyu-avatar-{}.{ext}", std::process::id()))
}

/// 用得着缩的缩好、写成文件，交回它；用不着、缩不成的是 `None`。
fn shrink(path: &Path, look: &Look) -> Option<PathBuf> {
    let bytes = std::fs::read(path).ok()?;
    let reader = ImageReader::new(Cursor::new(&bytes))
        .with_guessed_format()
        .ok()?;
    let format = reader.format()?;
    let (width, height) = reader.into_dimensions().ok()?;
    let taken = matches!(
        format,
        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP
    );
    if taken && bytes.len() <= look.max_bytes && width.max(height) <= look.max_side {
        return None;
    }
    let mut image = image::load_from_memory(&bytes).ok()?;
    if width.max(height) > look.max_side {
        image = image.resize(look.max_side, look.max_side, FilterType::Triangle);
    }
    let png = encode_png(&image)?;
    let (data, ext) = if png.len() <= look.max_bytes {
        (png, "png")
    } else {
        (encode_jpeg(&image, look.jpeg_quality)?, "jpg")
    };
    if data.len() > look.max_bytes {
        return None;
    }
    let out = out_path(look.out.as_deref(), ext);
    std::fs::create_dir_all(out.parent()?).ok()?;
    std::fs::write(&out, data).ok()?;
    Some(out)
}

/// 存成 PNG。
fn encode_png(image: &DynamicImage) -> Option<Vec<u8>> {
    let mut out = Cursor::new(Vec::new());
    image.write_to(&mut out, ImageFormat::Png).ok()?;
    Some(out.into_inner())
}

/// 存成 JPEG：透明的地方铺白（JPEG 没有透明）。
fn encode_jpeg(image: &DynamicImage, quality: u8) -> Option<Vec<u8>> {
    let rgba = image.to_rgba8();
    let flat = RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let [r, g, b, a] = rgba.get_pixel(x, y).0;
        let on_white = |c: u8| {
            let (c, a) = (u16::from(c), u16::from(a));
            u8::try_from((c * a + 255 * (255 - a)) / 255).unwrap_or(u8::MAX)
        };
        Rgb([on_white(r), on_white(g), on_white(b)])
    });
    let mut out = Vec::new();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
    flat.write_with_encoder(encoder).ok()?;
    Some(out)
}

#[cfg(test)]
mod tests;
