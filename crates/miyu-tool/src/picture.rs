//! 什么算一张图（施工 4-13 定在 `read` 里，施工 3-9 三补挪到这里共用）：工具读到的、人附上的，都照这一套认
//! （`docs/blueprint/tools/read.md`「读图片」，`docs/blueprint/protocol.md` 的 `blob.put`）。
//!
//! 看开头的字节认格式，不看扩展名；只认 DeepSeek 收的四种。量宽高只看文件头，不解码。上限取几家接口里最严的：图跟着
//! 对话每次都发出去，一张被供应商拒掉的图，会让这个会话以后的请求都失败，所以超了的进来时就拦下。

/// 认格式要看开头几个字节。
pub const HEAD: usize = 12;
/// 一张图最多几个字节：5 MiB。几家接口里最严的（Anthropic 5 MB）。
pub const MAX_BYTES: u64 = 5 * 1024 * 1024;
/// 每边最多几个像素。几家接口里最严的（Anthropic 8000，DeepSeek 8192）。
pub const MAX_SIDE: u32 = 8000;

/// 开头的字节是哪种图：交回媒体类型。不是 PNG、JPEG、GIF、WebP 的是空的（DeepSeek 只收这四种；BMP、TIFF、HEIC
/// 这些照旧不当图）。
pub fn kind(head: &[u8]) -> Option<&'static str> {
    if head.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if head.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if head.starts_with(b"GIF87a") || head.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if head.len() >= HEAD && head.starts_with(b"RIFF") && head[8..12] == *b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

/// 量宽高，像素：只看文件头，不解码。量不出的（头坏了、截断了、放不进 `u32` 的）是空的：量不出尺寸的不当图片
/// （`03-事件模型.md` 第四节）。
pub fn measure(bytes: &[u8]) -> Option<(u32, u32)> {
    let size = imagesize::blob_size(bytes).ok()?;
    Some((
        u32::try_from(size.width).ok()?,
        u32::try_from(size.height).ok()?,
    ))
}

/// 这么大的图收不收：不超过 [`MAX_BYTES`] 个字节、每边不超过 [`MAX_SIDE`] 个像素的才收，正好在线上的也收。
pub fn fits(bytes: u64, width: u32, height: u32) -> bool {
    bytes <= MAX_BYTES && width <= MAX_SIDE && height <= MAX_SIDE
}

#[cfg(test)]
mod tests;
