//! 复制：走 OSC 52，让终端替我们写系统剪贴板。读剪贴板（`Ctrl+V`）在 `read.rs`。
//!
//! 全屏时鼠标被程序接管，终端自己的选区用不了，只能程序自己复制。OSC 52 不依赖
//! 平台的剪贴板库，经 SSH 也能用（`13-终端界面.md` 第九节）。终端不认它时静悄悄地没效果。

use std::io::{self, Write};

mod image;
mod read;
mod staging;

pub use image::read_image;
pub use read::read;
pub use staging::Staging;

/// 把 `text` 放进系统剪贴板。
///
/// # Errors
///
/// 写终端失败时返回错误。
pub fn copy(text: &str) -> io::Result<()> {
    let mut out = io::stdout().lock();
    write!(out, "\x1b]52;c;{}\x07", base64(text.as_bytes()))?;
    out.flush()
}

/// 标准的 base64，带补齐。只有这一处用，不值得为它加一个依赖。
fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, &b)| n | u32::from(b) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(TABLE[(n >> (18 - 6 * i) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::base64;

    #[test]
    fn base64_matches_the_standard() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64("中".as_bytes()), "5Lit");
    }
}
