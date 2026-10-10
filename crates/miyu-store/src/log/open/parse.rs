//! 一段日志的每一行读成事件（施工 V-2 下补）：行多的分几块在几个线程里一起读，读完照先后交回。一万条事件的会话读、解日志
//! 约 23 ms，是重启以后打开大会话的大头；读成事件和别的行不相干，序号接没接上留给调的一方照先后查。
//!
//! 交回的和一行行读的一样：照先后每一行一项，读不成的那一行是第几行（从 1 数）、为什么；第一处读不成的后面的不交（一块里读不
//! 成就不往下读了）。

use miyu_kernel::event::Event;

/// 行数到了这么多才分块：少的一个线程读完比起线程还快。
const SPLIT_AT: usize = 2048;

/// 最多分几块。
const MOST: usize = 8;

/// 读成的一行：第几行和事件；读不成的是第几行和为什么。
pub(super) type Parsed = Result<(usize, Event), (usize, String)>;

/// 每一行读成事件，照先后交回。
pub(super) fn lines(lines: &[&[u8]]) -> Vec<Parsed> {
    let parts = parts(lines.len());
    if parts <= 1 {
        return chunk(lines, 0);
    }
    let size = lines.len().div_ceil(parts);
    std::thread::scope(|scope| {
        let handles: Vec<_> = lines
            .chunks(size)
            .enumerate()
            .map(|(k, part)| scope.spawn(move || chunk(part, k * size)))
            .collect();
        let mut all = Vec::with_capacity(lines.len());
        for handle in handles {
            // 读一行不会 panic；真的 panic 了，照原样抛出去，和一个线程读的时候一样。
            let part = handle
                .join()
                .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
            let stop = part.last().is_some_and(Result::is_err);
            all.extend(part);
            if stop {
                break;
            }
        }
        all
    })
}

/// 分几块：行少的不分；多的照机器的核数，最多 [`MOST`]。
fn parts(count: usize) -> usize {
    if count < SPLIT_AT {
        return 1;
    }
    let cores = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    cores.min(MOST).min(count / (SPLIT_AT / 2)).max(1)
}

/// 一块：第一行是整段的第 `before + 1` 行。读不成的那一行交了就停。
fn chunk(part: &[&[u8]], before: usize) -> Vec<Parsed> {
    let mut parsed = Vec::with_capacity(part.len());
    for (k, line) in part.iter().enumerate() {
        let number = before + k + 1;
        let one = std::str::from_utf8(line)
            .map_err(|_| (number, "not UTF-8".to_string()))
            .and_then(|text| {
                Event::from_line(text).map_err(|error| (number, format!("not readable: {error}")))
            })
            .map(|event| (number, event));
        let stop = one.is_err();
        parsed.push(one);
        if stop {
            break;
        }
    }
    parsed
}

#[cfg(test)]
mod tests;
