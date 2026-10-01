//! 安全地打开以后读一段（`fs.md` 第四节；`web-module.md`「七、分块读」，施工 W-6）：`blob.get`、`fs.read`
//! 都这样读：从 `offset` 起读最多 `length` 个字节，读到结尾就停；`offset` 过了结尾的交回空的；`length` 写 0
//! 只问大小。协议上一块最多 [`MAX_LENGTH`]；这里不管这条上限，调用方（`miyu-endpoint`）先查，报 `bad_params`。

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::open::{OpenError, open_file};

/// 一块最多多少字节（`web-module.md`「怎么走」第六条第 5 款、第七条第 3 款，读和写同一个数）。
pub const MAX_LENGTH: u64 = 512 * 1024;

/// 读出来的一段。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    /// 这一段的字节：`offset` 过了结尾、或者 `length` 是 0 的，是空的。
    pub data: Vec<u8>,
    /// 打开那一刻文件一共多大：不是存的时候记的那个数（`web-module.md`「怎么走」第七条第 3 款）。
    pub size: u64,
}

/// 安全地打开 `real`（要是已经换过真实位置、查过边界的），从 `offset` 起读最多 `length` 个字节。
///
/// # Errors
///
/// 打不开：没有这个文件、不是普通文件、没有权限（见 [`OpenError`]）。
pub fn read_range(real: &Path, offset: u64, length: u64) -> Result<Segment, OpenError> {
    let mut file = open_file(real)?;
    let size = file.metadata().map_err(OpenError::Io)?.len();
    if length == 0 || offset >= size {
        return Ok(Segment {
            data: Vec::new(),
            size,
        });
    }
    file.seek(SeekFrom::Start(offset)).map_err(OpenError::Io)?;
    let mut data = Vec::new();
    file.take(length)
        .read_to_end(&mut data)
        .map_err(OpenError::Io)?;
    Ok(Segment { data, size })
}

#[cfg(test)]
mod tests;
