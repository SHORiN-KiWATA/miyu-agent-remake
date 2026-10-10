//! 打开会话日志时的自检（`docs/designs/07-存储.md` 第四节「打开日志时自检三件事」）：最后一行完整、
//! 每一行都能解析、序号连续。只截最后一段末尾那半行：它一定从未被确认过。别的不对一律报错，写明
//! 是哪一段第几行，不自动修：日志是真相，修错了就是丢了。

use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use miyu_kernel::event::Event;
use miyu_kernel::id::Seq;

use super::{Mark, SessionLog};

mod parse;

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
            OpenError::Missing(dir) => write!(f, "no session log in {}", dir.display()),
            OpenError::Broken { segment, line, why } => {
                write!(f, "{} line {line}: {why}", segment.display())
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
        let (events, end) = read_all(dir, HalfLine::Cut)?;
        let last = dir.join(super::segment_name_of(end.segment));
        let size = fs::metadata(&last)?.len();
        let file = OpenOptions::new().append(true).open(&last)?;
        let log = SessionLog {
            dir: dir.to_path_buf(),
            file,
            segment: end.segment,
            size,
            next: end.next,
            limit,
        };
        Ok((log, events))
    }
}

/// 只读地读整份会话日志（施工 3-9 下）：和 [`SessionLog::open`] 一样自检，只是最后一段末尾没写完的半行
/// 跳过、不截，一个字节都不写：会话可能正在往里写。测试盯着一个在跑的会话时用它；[`SessionLog::open`]
/// 会截掉正在写的那半行，把活的日志写坏。
///
/// # Errors
///
/// 没有这个会话（目录没有，或者一段都还没有）；日志坏了；读写出错。
pub fn read_events(dir: &Path) -> Result<Vec<Event>, OpenError> {
    read_all(dir, HalfLine::Skip).map(|(events, _)| events)
}

/// 同 [`read_events`]，只是读一段交一段给 `each`，它交回 `false` 就不读下去（施工 6-4：`history` 翻长会话，叫停了
/// 不用读完整份）。自检照旧，一个字节都不写。
///
/// # Errors
///
/// 同 [`read_events`]。
pub fn read_segments(dir: &Path, each: impl FnMut(Vec<Event>) -> bool) -> Result<(), OpenError> {
    walk(dir, HalfLine::Skip, None, each).map(|_| ())
}

/// 同 [`read_segments`]，只是从 `from` 读起（施工 3-8 七补：列会话照索引记的位置，只读多出来的那一截），交回读到了哪里：
/// 最后一段、照到的整行末尾、下一条该是几号。`from` 是空的从头读。
///
/// `from` 和日志对不上的交回空的，一条都不交给 `each`：记的那一段没有了、比记的短了、记的位置前面一个字节不是 `\n`（不在
/// 一行的开头）。对得上的照旧自检，只是从 `from` 读起的那一段，报坏了时的行号从 `from` 数起。
///
/// # Errors
///
/// 同 [`read_events`]。
pub fn read_marked(
    dir: &Path,
    from: Option<&Mark>,
    each: impl FnMut(Vec<Event>) -> bool,
) -> Result<Option<Mark>, OpenError> {
    walk(dir, HalfLine::Skip, from, each)
}

/// 最后一段末尾没写完的半行怎么办。
#[derive(Clone, Copy, PartialEq, Eq)]
enum HalfLine {
    /// 截掉：载入以后接着往里写。
    Cut,
    /// 跳过、不动：只读。
    Skip,
}

/// 照段的先后一行行读、自检：交回事件、读到了哪里。
fn read_all(dir: &Path, half: HalfLine) -> Result<(Vec<Event>, Mark), OpenError> {
    let mut events = Vec::new();
    let end = walk(dir, half, None, |read| {
        events.extend(read);
        true
    })?
    .ok_or_else(|| OpenError::Missing(dir.to_path_buf()))?;
    Ok((events, end))
}

/// 照段的先后一段段读、自检，每读完一段交给 `each`，它交回 `false` 就停。`from` 是空的从第一段开头读，不是空的从它记的
/// 那一段、那个字节读起（[`read_marked`]）。交回读到了哪里（停下的，是停在哪一段后面）；`from` 和日志对不上的交回空的。
fn walk(
    dir: &Path,
    half: HalfLine,
    from: Option<&Mark>,
    mut each: impl FnMut(Vec<Event>) -> bool,
) -> Result<Option<Mark>, OpenError> {
    let segments = segments(dir)?;
    if segments.is_empty() {
        return Err(OpenError::Missing(dir.to_path_buf()));
    }
    let (start, mut offset, mut next) = match from {
        None => (0, 0, Seq::FIRST),
        Some(mark) => match segments
            .iter()
            .position(|(first, _)| *first == mark.segment)
        {
            Some(k) => (k, mark.bytes, mark.next),
            None => return Ok(None),
        },
    };
    let mut end = Mark {
        segment: segments[start].0,
        bytes: offset,
        next,
    };
    for (k, (first, path)) in segments.iter().enumerate().skip(start) {
        let is_last = k + 1 == segments.len();
        let Some((read, bytes)) = read_segment(path, offset, is_last, &mut next, half)? else {
            return Ok(None);
        };
        // 从一段中间读起的，读到的第一条不是这一段的第一条，这一段也不是空的：名字不在这里查。
        let whole = offset == 0;
        offset = 0;
        match read.first() {
            Some(event) if whole && event.seq.get() != *first => {
                return Err(broken(
                    path,
                    1,
                    format!("the segment is named {first} but starts with {}", event.seq),
                ));
            }
            None if whole && is_last && *first != next.get() => {
                return Err(broken(
                    path,
                    1,
                    format!("the empty last segment is named {first} but the next event is {next}"),
                ));
            }
            _ => {
                end = Mark {
                    segment: *first,
                    bytes,
                    next,
                };
                if !each(read) {
                    break;
                }
            }
        }
    }
    Ok(Some(end))
}

/// 读一段，从第 `from` 个字节读起：每一行读成事件，序号要接着 `next`。最后一段末尾没写完的半行照 `half` 截掉或者跳过；
/// 别的段末尾有半行，报错。交回读出来的事件、照到的整行末尾；`from` 前面一个字节不是 `\n` 的（这一段比 `from` 短的也是），
/// 交回空的（对不上）。
fn read_segment(
    path: &Path,
    from: u64,
    is_last: bool,
    next: &mut Seq,
    half: HalfLine,
) -> Result<Option<(Vec<Event>, u64)>, OpenError> {
    let mut file = fs::File::open(path)?;
    // 从一行中间读起的不算数：多读前面那一个字节，看它是不是 `\n`。这一段比记的短了的，那个字节读不到，也算对不上。
    let skip = u64::from(from > 0);
    file.seek(SeekFrom::Start(from - skip))?;
    let mut read = Vec::new();
    file.read_to_end(&mut read)?;
    if skip == 1 && read.first() != Some(&b'\n') {
        return Ok(None);
    }
    let bytes = &read[usize::from(skip == 1)..];
    let complete = bytes
        .iter()
        .rposition(|&byte| byte == b'\n')
        .map_or(0, |at| at + 1);
    let lines: Vec<&[u8]> = bytes[..complete]
        .split_inclusive(|&byte| byte == b'\n')
        .map(|line| &line[..line.len() - 1])
        .collect();
    let end = from + complete as u64;
    if complete < bytes.len() {
        if !is_last {
            return Err(broken(
                path,
                lines.len() + 1,
                "ends in a partial line, yet more segments follow".to_string(),
            ));
        }
        if half == HalfLine::Cut {
            truncate(path, end)?;
        }
    }
    let mut events = Vec::with_capacity(lines.len());
    for parsed in parse::lines(&lines) {
        let (number, event) = parsed.map_err(|(number, why)| broken(path, number, why))?;
        if event.seq != *next {
            return Err(broken(
                path,
                number,
                format!("seq should be {next}, got {}", event.seq),
            ));
        }
        *next = next.next();
        events.push(event);
    }
    Ok(Some((events, end)))
}

/// 截到 `len` 字节，再同步。
fn truncate(path: &Path, len: u64) -> io::Result<()> {
    let file = OpenOptions::new().write(true).open(path)?;
    file.set_len(len)?;
    file.sync_all()
}

/// 只读地拿会话日志的第一条（`session.created`）：列出会话时用（施工 3-9 下）。只读第一段开头那一行，
/// 不截、不写：会话可能正在往最后一段里写。第一条落了盘，会话才算造好，所以它总是完整的一行；
/// 还没写完的当没有这个会话。
///
/// # Errors
///
/// 没有这个会话；第一行读不懂；读写出错。
pub fn first_event(dir: &Path) -> Result<Event, OpenError> {
    let segments = segments(dir)?;
    let Some((_, first)) = segments.first() else {
        return Err(OpenError::Missing(dir.to_path_buf()));
    };
    let mut line = String::new();
    BufReader::new(fs::File::open(first)?).read_line(&mut line)?;
    let Some(line) = line.strip_suffix('\n') else {
        return Err(OpenError::Missing(dir.to_path_buf()));
    };
    Event::from_line(line).map_err(|error| broken(first, 1, error.to_string()))
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
