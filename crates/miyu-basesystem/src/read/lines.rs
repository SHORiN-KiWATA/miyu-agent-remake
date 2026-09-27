//! 按行读一份文本文件（`10-自带软件.md` 第三节）：认 UTF-8 和带 BOM 的 UTF-16，前 8 KiB 里有 NUL 字节的当
//! 二进制；每行是行号、一个制表符、原文，行号前不补空格（Claude Code 现在的写法，施工 4-4 下）；一行最长 2000
//! 个字，一次最多 64 KiB。

use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};

use crate::common::OUTPUT_BYTES;

/// 看前多少个字节认编码、认二进制。
const SNIFF: usize = 8 * 1024;
/// 一行最长多少个字，多的截掉、补一个 `…`。
pub(crate) const LINE_CHARS: usize = 2000;

/// 读下来的一页。
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Page {
    /// 二进制文件，不读。
    Binary,
    /// 空文件。
    Empty,
    /// `offset` 过了结尾：一共几行。
    PastEnd { total: u64 },
    /// 读到的几行：带行号的字，第几行到第几行（从 1 数起，含两头），一共几行。
    Lines {
        text: String,
        from: u64,
        to: u64,
        total: u64,
    },
}

/// 从第 `offset` 行起（从 1 数起），最多读 `limit` 行。
pub(crate) fn read(mut file: File, offset: u64, limit: u64) -> io::Result<Page> {
    let mut head = Vec::with_capacity(SNIFF);
    (&mut file).take(SNIFF as u64).read_to_end(&mut head)?;
    match head.as_slice() {
        [0xFF, 0xFE, ..] => return utf16(file, u16::from_le_bytes, offset, limit),
        [0xFE, 0xFF, ..] => return utf16(file, u16::from_be_bytes, offset, limit),
        _ => {}
    }
    if head.contains(&0) {
        return Ok(Page::Binary);
    }
    let bom = head.starts_with(&[0xEF, 0xBB, 0xBF]);
    file.seek(SeekFrom::Start(if bom { 3 } else { 0 }))?;
    let lines = BufReader::new(file).split(b'\n').map(|line| {
        line.map(|mut bytes| {
            if bytes.last() == Some(&b'\r') {
                bytes.pop();
            }
            String::from_utf8_lossy(&bytes).into_owned()
        })
    });
    page(lines, offset, limit)
}

/// 带 BOM 的 UTF-16：整份读进来再解。
fn utf16(mut file: File, unit: fn([u8; 2]) -> u16, offset: u64, limit: u64) -> io::Result<Page> {
    file.seek(SeekFrom::Start(2))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| unit([pair[0], pair[1]]))
        .collect();
    let text = String::from_utf16_lossy(&units);
    let lines = text
        .split('\n')
        .map(|line| Ok(line.strip_suffix('\r').unwrap_or(line).to_string()));
    // 以换行结尾的，最后切出来的那一段是空的，不算一行；什么都没有的，一行都没有。
    let count = if text.is_empty() {
        0
    } else {
        text.split('\n').count() - usize::from(text.ends_with('\n'))
    };
    page(lines.take(count), offset, limit)
}

/// 照先后给出的每一行，挑出这一页：带行号，一行最长 [`LINE_CHARS`] 个字，一页最多 [`OUTPUT_BYTES`]。
fn page(
    lines: impl Iterator<Item = io::Result<String>>,
    offset: u64,
    limit: u64,
) -> io::Result<Page> {
    let mut text = String::new();
    let mut total = 0;
    let mut to = offset.saturating_sub(1);
    let mut full = false;
    for line in lines {
        let line = line?;
        total += 1;
        if total < offset || full || total >= offset + limit {
            continue;
        }
        let numbered = format!("{total}\t{}\n", cut(&line));
        if text.len() + numbered.len() > OUTPUT_BYTES && total > offset {
            full = true;
            continue;
        }
        text.push_str(&numbered);
        to = total;
    }
    Ok(if total == 0 {
        Page::Empty
    } else if offset > total {
        Page::PastEnd { total }
    } else {
        Page::Lines {
            text,
            from: offset,
            to,
            total,
        }
    })
}

/// 一行最长 [`LINE_CHARS`] 个字，多的截掉、补一个 `…`。
fn cut(line: &str) -> String {
    match line.char_indices().nth(LINE_CHARS) {
        Some((at, _)) => format!("{}…", &line[..at]),
        None => line.to_string(),
    }
}

#[cfg(test)]
mod tests;
