//! 会话日志（`docs/designs/07-存储.md` 第三节「事件日志」、第四节「写入与崩溃」）：一个会话一个
//! 目录，按段存成 JSONL；一批事件一次写入、一次同步，同步完了才算落盘；打开时自检（`log/open.rs`）。
//!
//! 一个会话只有一个写者，就是它的 actor，所以不加锁。内核的「追加事件」动作落到这里，写完了
//! 执行器送一条「落盘了」回去（`02-内核.md` 第四节「执行器怎么回动作」）。

mod open;

pub use open::OpenError;

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use miyu_kernel::event::Event;
use miyu_kernel::id::Seq;

use crate::durable::{create_dir, sync_dir};

/// 一段的上限，初值，实测再定（07 第三节）：写一批之前这一段已经到了它，就开下一段。
pub const SEGMENT_LIMIT: u64 = 64 * 1024 * 1024;

/// 一个会话的日志，开着的，只往后追加。
#[derive(Debug)]
pub struct SessionLog {
    /// 会话的目录。
    dir: PathBuf,
    /// 正在写的那一段。
    file: File,
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
            size: 0,
            next: Seq::FIRST,
            limit,
        })
    }

    /// 下一条该是几号。
    pub fn next_seq(&self) -> Seq {
        self.next
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
                    format!("日志的下一条应该是 {expected}，来的是 {}", event.seq),
                ));
            }
            bytes.extend_from_slice(event.to_line().as_bytes());
            bytes.push(b'\n');
            expected = expected.next();
        }
        if self.size > 0 && self.size >= self.limit {
            self.file = new_segment(&self.dir, first.seq)?;
            self.size = 0;
        }
        self.file.write_all(&bytes)?;
        self.file.sync_data()?;
        self.size += bytes.len() as u64;
        self.next = expected;
        Ok(())
    }
}

/// 段文件的名字：这一段第一条的序号，补零到 12 位（07 第三节「段怎么存」）。
fn segment_name(first: Seq) -> String {
    format!("{:012}.jsonl", first.get())
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
