//! 会话日志（`docs/designs/07-存储.md` 第三节「事件日志」、第四节「写入与崩溃」）：一个会话一个
//! 目录，按段存成 JSONL；一批事件一次写入、一次同步，同步完了才算落盘；打开时自检（`log/open.rs`）。
//!
//! 一个会话只有一个写者，就是它的 actor，所以不加锁。内核的「追加事件」动作落到这里，写完了
//! 执行器送一条「落盘了」回去（`02-内核.md` 第四节「执行器怎么回动作」）。

mod open;

pub use open::{OpenError, first_event, read_events, read_marked, read_segments};

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use miyu_kernel::event::Event;
use miyu_kernel::id::Seq;

use crate::durable::{create_dir, sync_dir};

/// 一段的上限，初值，实测再定（07 第三节）：写一批之前这一段已经到了它，就开下一段。
pub const SEGMENT_LIMIT: u64 = 64 * 1024 * 1024;

/// 日志里的一个位置（施工 3-8 七补）：哪一段、这一段照到第几个字节、下一条该是几号。会话列表的索引记着每一行照到日志
/// 的哪里（[`crate::index`]），列会话时只读它后面多出来的那一截（[`read_marked`]）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mark {
    /// 哪一段：这一段第一条的序号，也就是段的名字。
    pub segment: u64,
    /// 这一段照到第几个字节：一整行的末尾，不含后面没写完的半行。
    pub bytes: u64,
    /// 下一条该是几号。
    pub next: Seq,
}

/// 一个会话的日志，开着的，只往后追加。
#[derive(Debug)]
pub struct SessionLog {
    /// 会话的目录。
    dir: PathBuf,
    /// 正在写的那一段。
    file: File,
    /// 正在写的那一段叫什么：它第一条的序号（施工 3-8 七补，[`SessionLog::mark`] 要）。
    segment: u64,
    /// 这一段已经写了多少字节。
    size: u64,
    /// 下一条该是几号。
    next: Seq,
    /// 一段的上限。
    limit: u64,
}

impl SessionLog {
    /// 新会话：建好目录，和空的第一段。`limit` 是一段的上限，平时用 [`SEGMENT_LIMIT`]。
    ///
    /// # Errors
    ///
    /// 建不了目录；第一段已经有了（不覆盖）。
    pub fn create(dir: &Path, limit: u64) -> io::Result<SessionLog> {
        create_dir(dir)?;
        let file = new_segment(dir, Seq::FIRST)?;
        Ok(SessionLog {
            dir: dir.to_path_buf(),
            file,
            segment: Seq::FIRST.get(),
            size: 0,
            next: Seq::FIRST,
            limit,
        })
    }

    /// 会话的目录：只读地读回整份日志时用（[`read_events`]，施工 4-7 上）。
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// 下一条该是几号。
    pub fn next_seq(&self) -> Seq {
        self.next
    }

    /// 写到哪了（施工 3-8 七补）：正在写的那一段、它的长度、下一条该是几号。会话列表的索引照它记一行照到哪里。
    pub fn mark(&self) -> Mark {
        Mark {
            segment: self.segment,
            bytes: self.size,
            next: self.next,
        }
    }

    /// 追加一批：拼成一块，一次写入，再同步（`sync_data`：数据和读得出数据要的文件长度）。返回时
    /// 这一批都落了盘。写之前这一段已经到了上限，先开下一段；一批不拆到两段里。
    ///
    /// # Errors
    ///
    /// 序号接不上（调用的一方的 bug，不写进去）；写不进、同步不了。
    pub fn append(&mut self, events: &[Event]) -> io::Result<()> {
        let Some(first) = events.first() else {
            return Ok(());
        };
        let mut expected = self.next;
        let mut bytes = Vec::new();
        for event in events {
            if event.seq != expected {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "the next event in the log should be {expected}, got {}",
                        event.seq
                    ),
                ));
            }
            bytes.extend_from_slice(event.to_line().as_bytes());
            bytes.push(b'\n');
            expected = expected.next();
        }
        if self.size > 0 && self.size >= self.limit {
            self.file = new_segment(&self.dir, first.seq)?;
            self.segment = first.seq.get();
            self.size = 0;
        }
        self.file.write_all(&bytes)?;
        self.file.sync_data()?;
        self.size += bytes.len() as u64;
        self.next = expected;
        Ok(())
    }
}

/// 造会话没成（`session.created` 没落盘）时收拾会话目录：里面只有一段空的第一段，才连目录一起删掉，交回删了
/// 没有；有别的东西的，一个字节不动（施工 4-9 再补四下）。
///
/// # Errors
///
/// 读不了目录、删不掉。
pub fn abandon(dir: &Path) -> io::Result<bool> {
    let entries = fs::read_dir(dir)?.collect::<io::Result<Vec<_>>>()?;
    let first = dir.join(segment_name(Seq::FIRST));
    let [only] = entries.as_slice() else {
        return Ok(false);
    };
    let empty_first =
        only.path() == first && only.file_type()?.is_file() && only.metadata()?.len() == 0;
    if !empty_first {
        return Ok(false);
    }
    fs::remove_file(&first)?;
    fs::remove_dir(dir)?;
    Ok(true)
}

/// 段文件的名字：这一段第一条的序号，补零到 12 位（07 第三节「段怎么存」）。
fn segment_name(first: Seq) -> String {
    segment_name_of(first.get())
}

/// 同 [`segment_name`]，照序号的数字。
fn segment_name_of(first: u64) -> String {
    format!("{first:012}.jsonl")
}

/// 建一段新的，只许新建；建好了同步所在目录，新文件本身才算落盘。
fn new_segment(dir: &Path, first: Seq) -> io::Result<File> {
    let file = OpenOptions::new()
        .append(true)
        .create_new(true)
        .open(dir.join(segment_name(first)))?;
    sync_dir(dir)?;
    Ok(file)
}

#[cfg(test)]
mod tests;
