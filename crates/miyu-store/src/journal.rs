//! 系统日志、账号日志 `journal.jsonl`（`docs/blueprint/config.md`「系统日志、账号日志」、「怎么走」第六条，`07-存储.md`
//! 第三节，施工 8-3）：配置、密钥的改动，项目配置的信任，一行一条。
//!
//! 外壳照事件的写法（`seq`、`at`、`kind`、`by`、`cause`、`body`，没有 `turn`），种类不进内核的种类表，内核读到照不认识的
//! 种类原样留着。`seq` 一份文件里从 1 数起。一份文件只有一个写者（M8 是配置服务），所以不加锁。
//!
//! 每追加一条都重新打开：照会话日志的规矩截掉最后那半行（`store.md` 第 6 条），读最后一行拿 `seq`，追加一行、
//! `sync_data`。改配置是很少的事，一次读整份换来不用在内存里留一个开着的文件、手改过的也认得。

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::{CommandId, EventKind, Seq};
use miyu_kernel::origin::By;
use miyu_kernel::raw::RawJson;
use miyu_kernel::time::Timestamp;

use crate::durable::{create_dir, sync_dir};

/// 文件名：系统的在 `system/` 里，账号的在 `home/<账号>/` 里。
pub const FILE: &str = "journal.jsonl";

/// 要记的一条：什么时候、谁、哪个命令引起的、什么种类、内容。
#[derive(Debug, Clone)]
pub struct Entry {
    /// 什么时候。
    pub at: Timestamp,
    /// 谁：经命令改的是那个人，手改被看到的是内核。
    pub by: By,
    /// 引起它的命令；手改被看到的没有。
    pub cause: Option<CommandId>,
    /// 种类，例如 `config.changed`。
    pub kind: EventKind,
    /// 内容，原样写进去：格的先后由写它的一方定。
    pub body: RawJson,
}

/// 在 `path` 这一份日志末尾追加 `entry`，交回它的序号。文件、目录没有的建上。
///
/// # Errors
///
/// 建不了、读不了、写不进、同步不了；最后一行完整、却读不懂（手改坏了），不往后写，报是哪一行。
pub fn append(path: &Path, entry: Entry) -> io::Result<Seq> {
    let dir = path.parent().unwrap_or(Path::new("."));
    create_dir(dir)?;
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error),
    };
    let complete = bytes
        .iter()
        .rposition(|&byte| byte == b'\n')
        .map_or(0, |at| at + 1);
    if complete < bytes.len() {
        // 截另开一个能写的：Windows 上只能追加的打开方式改不了长短。
        let file = OpenOptions::new().write(true).open(path)?;
        file.set_len(complete as u64)?;
        file.sync_all()?;
    }
    let seq = match last_line(&bytes[..complete]) {
        None => Seq::FIRST,
        Some((number, line)) => {
            let unreadable = |why: String| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{} line {number}: {why}", path.display()),
                )
            };
            let text = std::str::from_utf8(line).map_err(|_| unreadable("not UTF-8".into()))?;
            Event::from_line(text)
                .map_err(|error| unreadable(error.to_string()))?
                .seq
                .next()
        }
    };
    let existed = path.exists();
    let mut file = OpenOptions::new().append(true).create(true).open(path)?;
    if !existed {
        sync_dir(dir)?;
    }
    let event = Event {
        seq,
        at: entry.at,
        turn: None,
        by: entry.by,
        cause: entry.cause,
        body: Body::Unknown {
            kind: entry.kind,
            body: entry.body,
        },
    };
    file.write_all(format!("{}\n", event.to_line()).as_bytes())?;
    file.sync_data()?;
    Ok(seq)
}

/// 完整的那几行里最后一行（去掉换行）和它是第几行；一行都没有的是空的。
fn last_line(complete: &[u8]) -> Option<(usize, &[u8])> {
    let body = complete.strip_suffix(b"\n")?;
    let number = body.iter().filter(|&&byte| byte == b'\n').count() + 1;
    let start = body
        .iter()
        .rposition(|&byte| byte == b'\n')
        .map_or(0, |at| at + 1);
    Some((number, &body[start..]))
}

#[cfg(test)]
mod tests;
