//! 按行读一份文本文件（`10-自带软件.md` 第三节）：认 UTF-8 和带 BOM 的 UTF-16，前 8 KiB 里有 NUL 字节的当
//! 二进制；每行是行号、一个制表符、原文，行号前不补空格（Claude Code 现在的写法，施工 4-4 下）；一行最长 2000
//! 个字，一次最多 64 KiB。
//!
//! 读的同一遍里算出整份文件的内容哈希（施工 4-6 上）：她改之前照它核对。不另读一遍，也不把整份读进内存。

use std::fs::File;
use std::io::{self, BufRead, BufReader, Cursor, Read};

use miyu_kernel::id::{ContentHash, Hasher};

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

/// 读下来的：这一页，和整份文件的内容哈希。二进制文件不读，没有哈希。
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Paged {
    /// 这一页。
    pub(crate) page: Page,
    /// 整份文件的内容哈希，读了一段的也是整份的。
    pub(crate) hash: Option<ContentHash>,
}

/// 从第 `offset` 行起（从 1 数起），最多读 `limit` 行。
pub(crate) fn read(mut file: File, offset: u64, limit: u64) -> io::Result<Paged> {
    let mut head = Vec::with_capacity(SNIFF);
    (&mut file).take(SNIFF as u64).read_to_end(&mut head)?;
    match head.as_slice() {
        [0xFF, 0xFE, ..] => return utf16(head, file, u16::from_le_bytes, offset, limit),
        [0xFE, 0xFF, ..] => return utf16(head, file, u16::from_be_bytes, offset, limit),
        _ => {}
    }
    if head.contains(&0) {
        return Ok(Paged {
            page: Page::Binary,
            hash: None,
        });
    }
    let bom = if head.starts_with(&[0xEF, 0xBB, 0xBF]) {
        3
    } else {
        0
    };
    // 开头认编码读过的那一段，连同 BOM 先喂给哈希；往后从文件里读出来的，读一段喂一段。
    let mut hasher = Hasher::default();
    hasher.update(&head);
    let mut start = Cursor::new(head);
    start.set_position(bom);
    let mut reader = BufReader::new(start.chain(Hashing {
        inner: file,
        hasher,
    }));
    let lines = (&mut reader).split(b'\n').map(|line| {
        line.map(|mut bytes| {
            if bytes.last() == Some(&b'\r') {
                bytes.pop();
            }
            String::from_utf8_lossy(&bytes).into_owned()
        })
    });
    // 数一共几行要读到结尾，所以这一页数完了，整份都过了一遍哈希。
    let page = page(lines, offset, limit)?;
    let (_, rest) = reader.into_inner().into_inner();
    Ok(Paged {
        page,
        hash: Some(rest.hasher.finish()),
    })
}

/// 读过去的字节都喂给哈希。
struct Hashing {
    inner: File,
    hasher: Hasher,
}

impl Read for Hashing {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.hasher.update(&buf[..n]);
        Ok(n)
    }
}

/// 带 BOM 的 UTF-16：整份读进来再解。开头认编码读过的 `head` 接上剩下的，就是整份。
fn utf16(
    head: Vec<u8>,
    mut file: File,
    unit: fn([u8; 2]) -> u16,
    offset: u64,
    limit: u64,
) -> io::Result<Paged> {
    let mut bytes = head;
    file.read_to_end(&mut bytes)?;
    let hash = ContentHash::of(&bytes);
    let units: Vec<u16> = bytes[2..]
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
    Ok(Paged {
        page: page(lines.take(count), offset, limit)?,
        hash: Some(hash),
    })
}

/// 照先后给出的每一行，挑出这一页：带行号，一行最长 [`LINE_CHARS`] 个字，一页最多 [`OUTPUT_BYTES`]。每一行都要
/// 读到：一共几行靠它数，整份文件的哈希也靠它读到结尾。
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
