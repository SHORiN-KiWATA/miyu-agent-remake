//! 读图片（施工 4-13，`docs/blueprint/tools/read.md`「读图片」）：看开头的字节认格式，量宽高，交回图片；太大的读的时候
//! 就拦下，告诉她先缩小——图跟着对话每次都发出去，一张被供应商拒掉的图，会让这个会话以后的请求都失败。什么算一张图、
//! 上限多少，和人附的附件共用一套（`miyu_tool::picture`，施工 3-9 三补挪过去）。

use std::fs::File;
use std::io::{self, Read};
use std::path::PathBuf;

use miyu_kernel::id::{ContentHash, Hasher, MediaType};
use miyu_tool::picture::{self, MAX_BYTES};
use miyu_tool::{Done, Effect, Picture};

use super::Texts;
use crate::common::said;
use crate::load::say;

/// 读一张图：`head` 是认格式时读出来的开头，`file` 接着往下读。都报 `file.read`：整份的哈希，没有行的范围。
pub(crate) fn read(
    texts: &Texts,
    path: &str,
    real: PathBuf,
    mut file: File,
    head: Vec<u8>,
    media_type: &'static str,
) -> Done {
    let mut bytes = head;
    // 最多多读一个字节：读出来多过上限的，就是太大了，文件在打开以后变大的也拦得住。
    let room = MAX_BYTES + 1 - bytes.len() as u64;
    if let Err(error) = (&mut file).take(room).read_to_end(&mut bytes) {
        return texts.common.failed(path, &error);
    }
    if bytes.len() as u64 > MAX_BYTES {
        // 不给内容，照样过一遍整份算哈希，和二进制的一样：她读过它，`write` 才盖得了。
        let mut hasher = Hasher::default();
        hasher.update(&bytes);
        let total = match rest(&mut file, &mut hasher) {
            Ok(rest) => bytes.len() as u64 + rest,
            Err(error) => return texts.common.failed(path, &error),
        };
        let size = mib(total);
        return Done::error(say(
            &texts.image_too_big,
            &[("path", path), ("size", &size)],
        ))
        .said(
            said("read/image-too-big")
                .with("path", path)
                .with("size", size),
        )
        .effect(read_effect(real, hasher.finish()));
    }
    let hash = ContentHash::of(&bytes);
    let Some((width, height)) = picture::measure(&bytes) else {
        // 量不出宽高的不当图片，当二进制（`03-事件模型.md` 第四节）。
        return Done::error(say(&texts.binary, &[("path", path)]))
            .said(said("read/binary").with("path", path))
            .effect(read_effect(real, hash));
    };
    // 字节数上面拦过了，这里只剩宽高。
    if !picture::fits(bytes.len() as u64, width, height) {
        let (w, h) = (width.to_string(), height.to_string());
        return Done::error(say(
            &texts.image_too_wide,
            &[("path", path), ("width", &w), ("height", &h)],
        ))
        .said(
            said("read/image-too-wide")
                .with("path", path)
                .with("width", w)
                .with("height", h),
        )
        .effect(read_effect(real, hash));
    }
    let Ok(media_type) = MediaType::parse(media_type) else {
        return texts
            .common
            .failed(path, &io::Error::other("unknown media type"));
    };
    // 只交图，不另写字：驱动在这条结果里写现成的那一句，把图挪到后面一条 user 消息里。
    Done {
        blocks: Vec::new(),
        ..Done::ok("")
    }
    .image(Picture {
        bytes,
        media_type,
        width,
        height,
    })
    .said(
        said("read/image")
            .with("width", width.to_string())
            .with("height", height.to_string()),
    )
    .effect(read_effect(real, hash))
}

/// 剩下的字节喂给哈希，交回一共几个字节。
fn rest(file: &mut File, hasher: &mut Hasher) -> io::Result<u64> {
    let mut buf = [0u8; 64 * 1024];
    let mut total = 0;
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            return Ok(total);
        }
        hasher.update(&buf[..n]);
        total += n as u64;
    }
}

/// 几个字节写成 MiB，一位小数，例如 `7.3 MiB`。
fn mib(bytes: u64) -> String {
    format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
}

/// 读了一张图：整份的哈希，没有行的范围。
fn read_effect(path: PathBuf, hash: ContentHash) -> Effect {
    Effect::Read {
        path,
        lines: None,
        hash,
    }
}

#[cfg(test)]
mod tests;
