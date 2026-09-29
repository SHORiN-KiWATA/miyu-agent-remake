//! 读输出的一页（`docs/blueprint/tools/jobs.md`「output」）：照 `read` 的分页，从第几行起，一页最多 [`LIMIT`] 个字，停在
//! 整行上；一行就超过的截在上限、补一个 `…`。边读边数，不把整份读进内存：一直开着的服务的输出可能很长。

use std::io::{BufRead, BufReader, Read};

/// 一页最多几个字（策略数据 `jobs.output_chars` 的出厂值，`agents.md`「对外的样子」）：和 `shell` 一样多。
pub(super) const LIMIT: usize = 30_000;

/// 读出来的一页。
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Page {
    /// 这一页的字：每一行以换行结尾。
    pub(super) text: String,
    /// 这一页是第几行到第几行（从 1 数起，含两头）；从的那一行过了结尾的是空的。
    pub(super) lines: Option<(u64, u64)>,
    /// 一共几行：最后一段没有换行的也算一行。
    pub(super) total: u64,
}

impl Page {
    /// 从 `source` 的第 `offset` 行（从 1 数起）读一页。读到一半读不下去的（文件被删了之类），读到多少算多少。解不开的
    /// 字节换成 `�`：写进去的都是解好的字，只有一行被截在半个字上时会碰到。
    pub(super) fn read(source: Box<dyn Read + Send>, offset: u64) -> Page {
        let mut page = Page::default();
        let mut chars = 0;
        let mut full = false;
        let mut reader = BufReader::new(source);
        let mut line = Vec::new();
        loop {
            line.clear();
            match reader.read_until(b'\n', &mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            page.total += 1;
            if page.total < offset || full {
                continue;
            }
            let text = String::from_utf8_lossy(&line);
            let text = text.strip_suffix('\n').unwrap_or(&text);
            let count = text.chars().count();
            let first = page.lines.is_none();
            if !first && chars + count > LIMIT {
                full = true;
                continue;
            }
            match count > LIMIT {
                true => {
                    page.text.extend(text.chars().take(LIMIT));
                    page.text.push('…');
                }
                false => page.text.push_str(text),
            }
            page.text.push('\n');
            chars += count.min(LIMIT);
            let from = page.lines.map_or(page.total, |(from, _)| from);
            page.lines = Some((from, page.total));
        }
        page
    }
}

#[cfg(test)]
mod tests;
