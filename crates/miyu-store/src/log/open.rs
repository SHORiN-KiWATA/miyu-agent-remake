//! 打开会话日志时的自检（`docs/designs/07-存储.md` 第四节「打开日志时自检三件事」）：最后一行完整、
//! 每一行都能解析、序号连续。只截最后一段末尾那半行：它一定从未被确认过。别的不对一律报错，写明
//! 是哪一段第几行，不自动修：日志是真相，修错了就是丢了。

use std::fmt;
use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use miyu_kernel::event::Event;
use miyu_kernel::id::Seq;

use super::SessionLog;

/// 打开不了会话日志。
#[derive(Debug)]
pub enum OpenError {
    /// 没有这个会话：目录不存在，或者里面一段日志都没有。
    Missing(PathBuf),
    /// 日志坏了：哪一段、第几行（从 1 数）、怎么坏的。
    Broken {
        /// 哪一段。
        segment: PathBuf,
        /// 第几行。
        line: usize,
        /// 怎么坏的。
        why: String,
    },
    /// 读写出错。
    Io(io::Error),
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpenError::Missing(dir) => write!(f, "{} 里没有会话日志", dir.display()),
            OpenError::Broken { segment, line, why } => {
                write!(f, "{} 第 {line} 行：{why}", segment.display())
            }
            OpenError::Io(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for OpenError {}

impl From<io::Error> for OpenError {
    fn from(error: io::Error) -> OpenError {
        OpenError::Io(error)
    }
}

impl SessionLog {
    /// 打开一个会话的日志：照段的先后一行行读，自检，截掉最后一段末尾那半行。返回开着的日志，
    /// 和读出来的事件（交给内核载入，`02-内核.md` 第六节「载入、崩溃、重启」）。空的段当没有；
    /// 最后一段是空的，接着往里写。
    ///
    /// # Errors
    ///
    /// 没有这个会话；日志坏了；读写出错。
    pub fn open(dir: &Path, limit: u64) -> Result<(SessionLog, Vec<Event>), OpenError> {
        let segments = segments(dir)?;
        let Some((_, last)) = segments.last() else {
            return Err(OpenError::Missing(dir.to_path_buf()));
        };
        let mut events = Vec::new();
        let mut next = Seq::FIRST;
        for (k, (first, path)) in segments.iter().enumerate() {
            let is_last = k + 1 == segments.len();
            let read = read_segment(path, is_last, &mut next)?;
            match read.first() {
                Some(event) if event.seq.get() != *first => {
                    return Err(broken(
                        path,
                        1,
                        format!("这一段叫 {first}，第一条却是 {}", event.seq),
                    ));
                }
                None if is_last && *first != next.get() => {
                    return Err(broken(
                        path,
                        1,
                        format!("空的最后一段叫 {first}，下一条应该是 {next}"),
                    ));
                }
                _ => events.extend(read),
            }
        }
        let size = fs::metadata(last)?.len();
        let file = OpenOptions::new().append(true).open(last)?;
        let log = SessionLog {
            dir: dir.to_path_buf(),
            file,
            size,
            next,
            limit,
        };
        Ok((log, events))
    }
}

/// 读一段：每一行读成事件，序号要接着 `next`。最后一段末尾没写完的半行截掉；别的段末尾有半行，
/// 报错。
fn read_segment(path: &Path, is_last: bool, next: &mut Seq) -> Result<Vec<Event>, OpenError> {
    let bytes = fs::read(path)?;
    let complete = bytes
        .iter()
        .rposition(|&byte| byte == b'\n')
        .map_or(0, |at| at + 1);
    let lines: Vec<&[u8]> = bytes[..complete]
        .split_inclusive(|&byte| byte == b'\n')
        .map(|line| &line[..line.len() - 1])
        .collect();
    if complete < bytes.len() {
        if !is_last {
            return Err(broken(
                path,
                lines.len() + 1,
                "末尾有半行，可它后面还有段".to_string(),
            ));
        }
        truncate(path, complete as u64)?;
    }
    let mut events = Vec::with_capacity(lines.len());
    for (k, line) in lines.iter().enumerate() {
        let number = k + 1;
        let text = std::str::from_utf8(line)
            .map_err(|_| broken(path, number, "不是 UTF-8".to_string()))?;
        let event = Event::from_line(text)
            .map_err(|error| broken(path, number, format!("读不出来：{error}")))?;
        if event.seq != *next {
            return Err(broken(
                path,
                number,
                format!("序号应该是 {next}，写的是 {}", event.seq),
            ));
        }
        *next = next.next();
        events.push(event);
    }
    Ok(events)
}

/// 截到 `len` 字节，再同步。
fn truncate(path: &Path, len: u64) -> io::Result<()> {
    let file = OpenOptions::new().write(true).open(path)?;
    file.set_len(len)?;
    file.sync_all()
}

/// 目录里的段：名字是 12 位数字加 `.jsonl` 的，照数字排。别的文件不看。
fn segments(dir: &Path) -> Result<Vec<(u64, PathBuf)>, OpenError> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(OpenError::Missing(dir.to_path_buf()));
        }
        Err(error) => return Err(error.into()),
    };
    let mut segments = Vec::new();
    for entry in entries {
        let path = entry?.path();
        let first = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(".jsonl"))
            .filter(|stem| stem.len() == 12 && stem.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|stem| stem.parse::<u64>().ok());
        if let Some(first) = first {
            segments.push((first, path));
        }
    }
    segments.sort();
    Ok(segments)
}

fn broken(segment: &Path, line: usize, why: String) -> OpenError {
    OpenError::Broken {
        segment: segment.to_path_buf(),
        line,
        why,
    }
}
