//! 找 `old_string` 在原文件里的位置（`10-自带软件.md` 第六节「`edit` 的细则」，施工 4-6 中）。
//!
//! 两层看法：精确的只把 CRLF 当 LF 看；宽松的另外不管每一行末尾的空白，弯引号、破折号、各种空格当成普通的，逐字做
//! NFKC（参考 pi）。先给原文造一份看法，记下看法里每个字节来自原文的哪一段；`old_string` 照同样的办法换过，在看法里
//! 找，找到的照记下的换回原文里的位置。换回来的就是原文里要换掉的那一段，别的地方一个字节不碰。
//!
//! 哪一层对得上几个就是几个：精确的对上两个，就是不唯一，不往宽松那一层找。一个都对不上的，找最接近的那几行。

use std::collections::BTreeMap;
use std::ops::Range;

use unicode_normalization::UnicodeNormalization;

use crate::read::lines::cut;

/// 不唯一时最多列几个行号；最接近的最多带几行。
pub(crate) const LINES: usize = 10;

/// 最像的那一行，两个字一组比，相同的组过了这个比例，才算最接近。
const NEAR: f64 = 0.5;

/// 找的结果。
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Found {
    /// 原文里对上的几段，照先后：不要全部的只有一段。
    At(Vec<Range<usize>>),
    /// 对得上好几个地方：一共几个，前 [`LINES`] 个在第几行（从 1 数起）。
    Many { count: usize, lines: Vec<usize> },
    /// 一个都对不上：最接近的那几行，没有像的是空的。
    Missing(Option<Closest>),
}

/// 最接近的那几行。
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Closest {
    /// 从第几行起，从 1 数起。
    pub(crate) from: usize,
    /// 到第几行，含这一行。
    pub(crate) to: usize,
    /// 这几行，照 `read` 的样子带上行号：行号、一个制表符、原文。
    pub(crate) text: String,
}

/// 看法的两层。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Level {
    Exact,
    Loose,
}

/// 在原文 `text` 里找 `old`。`all` 的：对上的每个地方都要（不重叠的），用不着唯一。
pub(crate) fn find(text: &str, old: &str, all: bool) -> Found {
    for level in [Level::Exact, Level::Loose] {
        let view = View::of(text, level);
        let needle = View::of(old, level).text;
        // 宽松的规矩只对有正经字的有意义：全是空白、换行的，去掉行尾空白以后每个换行都对得上。精确的照找：换缩进
        // 这类是正当的用法。
        if needle.is_empty() || (level == Level::Loose && needle.trim().is_empty()) {
            continue;
        }
        let starts = if all {
            view.text
                .match_indices(needle.as_str())
                .map(|(at, _)| at)
                .collect()
        } else {
            view.starts(&needle)
        };
        match starts.len() {
            0 => continue,
            1 => return Found::At(vec![view.original(starts[0]..starts[0] + needle.len())]),
            _ if all => {
                let ranges = starts
                    .iter()
                    .map(|at| view.original(*at..*at + needle.len()))
                    .collect();
                return Found::At(ranges);
            }
            count => {
                let lines = starts
                    .iter()
                    .take(LINES)
                    .map(|at| line_of(text, view.spans[*at].0))
                    .collect();
                return Found::Many { count, lines };
            }
        }
    }
    Found::Missing(closest(text, old))
}

/// 一份看法：换过的字，和它每个字节来自原文的哪一段（起、止，都是原文里的字节位置）。
struct View {
    text: String,
    spans: Vec<(usize, usize)>,
}

impl View {
    /// 照 `level` 给 `text` 造一份看法。
    fn of(text: &str, level: Level) -> View {
        let mut view = View {
            text: String::with_capacity(text.len()),
            spans: Vec::with_capacity(text.len()),
        };
        // 宽松的：空白先攒着，后面来了别的字才照写；到了行尾、文末就不要了。
        let mut blanks: Vec<(char, (usize, usize))> = Vec::new();
        let mut chars = text.char_indices().peekable();
        while let Some((at, c)) = chars.next() {
            let mut end = at + c.len_utf8();
            if c == '\r' && chars.peek().is_some_and(|(_, next)| *next == '\n') {
                // CRLF 当 LF 看：这一个换行来自原文的两个字节。
                if let Some((next, _)) = chars.next() {
                    end = next + 1;
                }
                blanks.clear();
                view.push('\n', (at, end));
                continue;
            }
            if c == '\n' {
                blanks.clear();
                view.push('\n', (at, end));
                continue;
            }
            match level {
                Level::Exact => view.push(c, (at, end)),
                Level::Loose => {
                    for folded in fold(c) {
                        if folded == ' ' || folded == '\t' {
                            blanks.push((folded, (at, end)));
                            continue;
                        }
                        for (blank, span) in blanks.drain(..) {
                            view.push(blank, span);
                        }
                        view.push(folded, (at, end));
                    }
                }
            }
        }
        view
    }

    /// 写一个字：它的每个字节都来自原文的 `span`。
    fn push(&mut self, c: char, span: (usize, usize)) {
        self.text.push(c);
        for _ in 0..c.len_utf8() {
            self.spans.push(span);
        }
    }

    /// `needle` 在看法里的每一个起点，重叠的也算：唯一不唯一照它数。
    fn starts(&self, needle: &str) -> Vec<usize> {
        let mut starts = Vec::new();
        let mut from = 0;
        while let Some(found) = self.text[from..].find(needle) {
            let at = from + found;
            starts.push(at);
            // 往后挪一个字接着找：停在字的边界上。
            let step = self.text[at..].chars().next().map_or(1, char::len_utf8);
            from = at + step;
        }
        starts
    }

    /// 看法里的一段，换回原文里的一段：从第一个字节的来处起，到最后一个字节的来处止。
    fn original(&self, range: Range<usize>) -> Range<usize> {
        self.spans[range.start].0..self.spans[range.end - 1].1
    }
}

/// 宽松的一个字：先做 NFKC，再把弯引号、各种破折号、各种空格换成普通的。
fn fold(c: char) -> impl Iterator<Item = char> {
    c.nfkc().map(|c| match c {
        '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}' | '\u{2032}' => '\'',
        '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{201F}' | '\u{2033}' => '"',
        '\u{2010}'..='\u{2015}' | '\u{2212}' => '-',
        '\u{00A0}' | '\u{2000}'..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}' => ' ',
        other => other,
    })
}

/// 原文里第 `at` 个字节在第几行，从 1 数起。
fn line_of(text: &str, at: usize) -> usize {
    text[..at].matches('\n').count() + 1
}

/// 一个都对不上：拿 `old` 第一行不空的那一行，跟原文每一行比像不像，最像的过了一半，就交回从那一行起、和 `old`
/// 一样多的几行（最多 [`LINES`] 行）。
fn closest(text: &str, old: &str) -> Option<Closest> {
    let old_lines: Vec<&str> = old.lines().collect();
    let (skipped, first) = old_lines
        .iter()
        .enumerate()
        .find(|(_, line)| !line.trim().is_empty())?;
    let key = loose_line(first);
    let lines: Vec<&str> = text.lines().collect();
    let mut best: Option<(f64, usize)> = None;
    for (at, line) in lines.iter().enumerate() {
        let score = alike(&key, &loose_line(line));
        if best.is_none_or(|(top, _)| score > top) {
            best = Some((score, at));
        }
    }
    let (score, at) = best?;
    if score < NEAR {
        return None;
    }
    let from = at.saturating_sub(skipped);
    let to = (from + old_lines.len().clamp(1, LINES)).min(lines.len()) - 1;
    let text = lines[from..=to]
        .iter()
        .zip(from + 1..)
        .map(|(line, number)| format!("{number}\t{}\n", cut(line)))
        .collect();
    Some(Closest {
        from: from + 1,
        to: to + 1,
        text,
    })
}

/// 一行宽松的看法，去掉两头的空白：比像不像用。
fn loose_line(line: &str) -> String {
    View::of(line, Level::Loose).text.trim().to_string()
}

/// 两段字有多像：两个字一组，相同的组占两边组数的比例（Dice 系数）。一模一样的是 1。
fn alike(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }
    let (left, right) = (pairs(a), pairs(b));
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let mut counts: BTreeMap<(char, char), usize> = BTreeMap::new();
    for pair in &right {
        *counts.entry(*pair).or_default() += 1;
    }
    let mut common = 0usize;
    for pair in &left {
        if let Some(count) = counts.get_mut(pair)
            && *count > 0
        {
            *count -= 1;
            common += 1;
        }
    }
    let total = left.len() + right.len();
    (2 * common) as f64 / total as f64
}

/// 相邻两个字一组。
fn pairs(text: &str) -> Vec<(char, char)> {
    let chars: Vec<char> = text.chars().collect();
    chars.windows(2).map(|pair| (pair[0], pair[1])).collect()
}

#[cfg(test)]
mod tests;
