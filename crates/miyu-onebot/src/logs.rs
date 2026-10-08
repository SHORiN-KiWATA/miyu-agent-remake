//! `miyu onebot logs [-f]`（`onebot.md` 第一条「对外的样子」、「施工时定的」第 26 条，施工 O-18）：不连核心，照数据根读两份：
//!
//! - 标准错误那一份 `state/logs/onebot.stderr`（核心拉起桥时接的，崩的时候说的在这里）有内容的，先印它，带一行标题；接着
//!   印一行运行日志的标题。只有运行日志的不带标题。
//! - 运行日志 `state/logs/onebot.log` 原样的字节印在标准输出上；还没有的在标准错误上说一句。
//! - `-f`：印完接着跟运行日志，隔一阵（`bridge.json` 的 `follow_millis`）看一次长了没有，长了的印出来；变短了（满了换了一份）
//!   从头读。每次照路径重开，不一直开着它：换文件、Windows 上改名都不受拦。Ctrl+C 停；标准输出关了也停。

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::Duration;

use miyu_store::root::DataRoot;

use crate::texts::Texts;

/// 标题、提醒（「给人看的字」`logs/`）：怎么说照 `texts`。字段是文件的路径。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Heading {
    /// 标准错误那一段的标题。
    Stderr(String),
    /// 运行日志那一段的标题（有标准错误那一段时才印）。
    Log(String),
    /// 还没有运行日志。
    None(String),
}

/// 印数据根 `root` 里桥的两份日志；`follow` 有的印完照它隔一阵接着跟运行日志，一直跟下去。交回退出码：总是 0（印不出来，
/// 标准输出关了，就停）。
pub fn logs(
    root: &DataRoot,
    follow: Option<Duration>,
    texts: &Texts,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let dir = root.state().join("logs");
    let (log, stderr) = (
        dir.join(format!("{}.log", crate::PACKAGE)),
        dir.join(format!("{}.stderr", crate::PACKAGE)),
    );
    let crashed = std::fs::read(&stderr).unwrap_or_default();
    let mut printed = Ok(());
    if !String::from_utf8_lossy(&crashed).trim().is_empty() {
        printed = section(out, texts, &stderr, &crashed)
            .and_then(|()| writeln!(out, "{}", texts.heading(&Heading::Log(shown(&log)))));
    }
    if printed.is_err() {
        return 0;
    }
    let Ok(mut offset) = copy_from(&log, 0, out) else {
        return 0;
    };
    if offset.is_none() && writeln!(err, "{}", texts.heading(&Heading::None(shown(&log)))).is_err()
    {
        return 0;
    }
    let Some(every) = follow else {
        return 0;
    };
    loop {
        std::thread::sleep(every);
        match copy_from(&log, offset.unwrap_or_default(), out) {
            Ok(Some(now)) => offset = Some(now),
            Ok(None) => {}
            Err(_) => return 0,
        }
    }
}

/// 标准错误那一段：标题、原样的内容，没以换行结尾的补一个。
fn section(out: &mut dyn Write, texts: &Texts, path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    writeln!(out, "{}", texts.heading(&Heading::Stderr(shown(path))))?;
    out.write_all(bytes)?;
    if !bytes.ends_with(b"\n") {
        writeln!(out)?;
    }
    out.flush()
}

/// 把 `path` 从第 `from` 个字节起印到 `out`，交回印到了哪（文件比 `from` 短的是换了一份：从头印）。文件不在、读不了的交回
/// 空的；印不出去（标准输出关了）的是错。
fn copy_from(path: &Path, from: u64, out: &mut dyn Write) -> std::io::Result<Option<u64>> {
    let Ok(mut file) = std::fs::File::open(path) else {
        return Ok(None);
    };
    let Ok(length) = file.metadata().map(|metadata| metadata.len()) else {
        return Ok(None);
    };
    let start = if length < from { 0 } else { from };
    if file.seek(SeekFrom::Start(start)).is_err() {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    if file.read_to_end(&mut bytes).is_err() {
        return Ok(None);
    }
    out.write_all(&bytes)?;
    out.flush()?;
    Ok(Some(start + bytes.len() as u64))
}

/// 印在标题里的路径。
fn shown(path: &Path) -> String {
    path.display().to_string()
}
