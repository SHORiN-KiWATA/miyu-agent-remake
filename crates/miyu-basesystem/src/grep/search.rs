//! 照正则搜（施工 4-4 下）：和 ripgrep 同一套库。正则照 ripgrep 的写法、不跨行；二进制文件跳过；带 BOM 的
//! UTF-16 照认；读不了的文件跳过，ripgrep 也是这样。

use std::io;
use std::path::PathBuf;

use grep_regex::{RegexMatcher, RegexMatcherBuilder};
use grep_searcher::{BinaryDetection, Searcher, SearcherBuilder, Sink, SinkContext, SinkMatch};

use miyu_tool::Stop;

use super::{LINE_CHARS, Page};

/// 认好的正则。`ignore_case` 是不分大小写。
///
/// # Errors
///
/// 写法不对：交回说哪里不对的那一句。
pub(super) fn matcher(pattern: &str, ignore_case: bool) -> Result<RegexMatcher, String> {
    RegexMatcherBuilder::new()
        .case_insensitive(ignore_case)
        .line_terminator(Some(b'\n'))
        .build(pattern)
        .map_err(|error| last_line(&error.to_string()))
}

/// 正则的报错有好几行（原文、指着哪里的 `^`、`error: …`），留最后说哪里不对的那一句。
fn last_line(error: &str) -> String {
    let last = error
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or(error);
    last.strip_prefix("error: ").unwrap_or(last).to_string()
}

/// 前后各带几行的搜法；二进制的碰到 NUL 就不往下搜了。
fn searcher(before: usize, after: usize) -> Searcher {
    SearcherBuilder::new()
        .binary_detection(BinaryDetection::quit(b'\x00'))
        .line_number(true)
        .before_context(before)
        .after_context(after)
        .build()
}

/// `files` 里有匹配的那几个，照原来的先后。
pub(super) fn with_matches(matcher: &RegexMatcher, files: &[PathBuf], stop: &Stop) -> Vec<PathBuf> {
    let mut searcher = searcher(0, 0);
    files
        .iter()
        .take_while(|_| !stop.stopped())
        .filter(|file| {
            let mut sink = First::default();
            searched(searcher.search_path(matcher, file, &mut sink)) && sink.found && !sink.binary
        })
        .cloned()
        .collect()
}

/// `files` 里有匹配的那几个，各有几行匹配，照原来的先后。
pub(super) fn counts(
    matcher: &RegexMatcher,
    files: &[PathBuf],
    stop: &Stop,
) -> Vec<(PathBuf, u64)> {
    let mut searcher = searcher(0, 0);
    files
        .iter()
        .take_while(|_| !stop.stopped())
        .filter_map(|file| {
            let mut sink = Count::default();
            let ok = searched(searcher.search_path(matcher, file, &mut sink));
            (ok && sink.lines > 0 && !sink.binary).then(|| (file.clone(), sink.lines))
        })
        .collect()
}

/// 一个文件里搜到的：匹配的行和前后带出来的行，照行号先后。
#[derive(Debug)]
pub(super) struct Found {
    pub(super) path: PathBuf,
    pub(super) lines: Vec<Line>,
}

/// 搜到的一行。
#[derive(Debug)]
pub(super) struct Line {
    /// 行号，从 1 数起。
    pub(super) number: u64,
    /// 这一行的字，去掉了行尾；最长 [`LINE_CHARS`] 个字，多的截掉、补一个 `…`。收下来时就截，一行几 MB 的
    /// 压缩过的代码不占着内存。
    pub(super) text: String,
    /// 是匹配的那一行；不是就是前后带出来的。
    pub(super) matched: bool,
}

/// 照先后搜 `files`，一共搜够 `need` 行匹配就停；前后各带 `page` 说的那么多行。
pub(super) fn lines(
    matcher: &RegexMatcher,
    files: &[PathBuf],
    page: Page,
    need: usize,
    stop: &Stop,
) -> Vec<Found> {
    let mut searcher = searcher(page.before, page.after);
    let mut found = Vec::new();
    let mut matches = 0;
    for file in files {
        if matches >= need || stop.stopped() {
            break;
        }
        let mut sink = Collect {
            budget: need - matches,
            ..Collect::default()
        };
        if !searched(searcher.search_path(matcher, file, &mut sink)) || sink.binary {
            continue;
        }
        if sink.matches > 0 {
            matches += sink.matches;
            found.push(Found {
                path: file.clone(),
                lines: sink.lines,
            });
        }
    }
    found
}

/// 搜这一个文件成没成：读不了的（没有权限这类）当没搜到。
fn searched(result: io::Result<()>) -> bool {
    result.is_ok()
}

/// 只要知道有没有匹配：碰到第一处就停。
#[derive(Default)]
struct First {
    found: bool,
    binary: bool,
}

impl Sink for First {
    type Error = io::Error;

    fn matched(&mut self, _: &Searcher, _: &SinkMatch<'_>) -> Result<bool, io::Error> {
        self.found = true;
        Ok(false)
    }

    fn binary_data(&mut self, _: &Searcher, _: u64) -> Result<bool, io::Error> {
        self.binary = true;
        Ok(false)
    }
}

/// 数有几行匹配。
#[derive(Default)]
struct Count {
    lines: u64,
    binary: bool,
}

impl Sink for Count {
    type Error = io::Error;

    fn matched(&mut self, _: &Searcher, found: &SinkMatch<'_>) -> Result<bool, io::Error> {
        self.lines += found.lines().count() as u64;
        Ok(true)
    }

    fn binary_data(&mut self, _: &Searcher, _: u64) -> Result<bool, io::Error> {
        self.binary = true;
        Ok(false)
    }
}

/// 把匹配的行和前后带出来的行收下来，收够 `budget` 行匹配，再碰到下一行匹配就停。
#[derive(Default)]
struct Collect {
    budget: usize,
    matches: usize,
    lines: Vec<Line>,
    binary: bool,
}

impl Sink for Collect {
    type Error = io::Error;

    fn matched(&mut self, _: &Searcher, found: &SinkMatch<'_>) -> Result<bool, io::Error> {
        let first = found.line_number().unwrap_or(0);
        for (at, bytes) in (first..).zip(found.lines()) {
            if self.matches >= self.budget {
                return Ok(false);
            }
            self.lines.push(line(at, bytes, true));
            self.matches += 1;
        }
        Ok(true)
    }

    fn context(&mut self, _: &Searcher, context: &SinkContext<'_>) -> Result<bool, io::Error> {
        let number = context.line_number().unwrap_or(0);
        self.lines.push(line(number, context.bytes(), false));
        Ok(true)
    }

    fn binary_data(&mut self, _: &Searcher, _: u64) -> Result<bool, io::Error> {
        self.binary = true;
        Ok(false)
    }
}

/// 一行的字：去掉行尾的换行和回车，不是 UTF-8 的字节换成替换符，截到 [`LINE_CHARS`] 个字。
fn line(number: u64, bytes: &[u8], matched: bool) -> Line {
    let bytes = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    let bytes = bytes.strip_suffix(b"\r").unwrap_or(bytes);
    let text = String::from_utf8_lossy(bytes);
    let text = match text.char_indices().nth(LINE_CHARS) {
        Some((at, _)) => format!("{}…", &text[..at]),
        None => text.into_owned(),
    };
    Line {
        number,
        text,
        matched,
    }
}
