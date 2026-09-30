//! `job.output`（施工 7-4 补，`docs/blueprint/protocol.md` 的 `job.output`）：头读一条后台命令到这时为止的输出，只要最后
//! 几行。读哪一份由执行器定，和她用 `jobs` 读的是同一份（`Handle::job_output`，`session/tools.md` 第 6 条第 3 款）；这里
//! 只取尾巴、量上限。
//!
//! 输出可能很长（一直开着的服务），头又照一秒一次来读：整份边读边数，不整份读进内存；一行再长，留在内存里的也只有它的
//! 末尾。

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, ErrorKind, Read};
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::id::{JobId, SessionId};
use miyu_session::Unreadable;

use crate::Core;
use crate::refusal::Refusal;

const TARGET: &str = "miyu::endpoint";

/// 不写 `tail` 时交最后几行。
const TAIL: u64 = 200;

/// `tail` 最多几行。
const MOST_LINES: u64 = 2000;

/// 一次回应里 `output` 最多多少字节（UTF-8）。一行最长 1 MiB（`wire::LINE_LIMIT`）；字写进 JSON，最坏一个字节变六个
/// （控制字符写成 `\u001b` 这样），回应的其余部分不到 1 KiB：6 × 128 KiB + 1 KiB 放得进一行（`protocol.md` 的
/// `job.output` 第 3 条）。
const CAP: usize = 128 * 1024;

/// `job.output` 的参数。
#[derive(Debug, Deserialize)]
pub(crate) struct Params {
    session: String,
    job: String,
    /// 要最后几行，不写是 [`TAIL`]。写 `null` 的、不是非负整数的读不成，是参数不对。
    #[serde(default = "default_tail")]
    tail: u64,
}

fn default_tail() -> u64 {
    TAIL
}

/// 读 `params` 说的那一条后台命令的输出，交回回应的 `result`。`tail` 先查，不对的不找会话；再照 `job.stop` 查编号、找会话。
pub(crate) async fn read(core: &Arc<Core>, params: Params) -> Result<Value, Refusal> {
    if !(1..=MOST_LINES).contains(&params.tail) {
        return Err(Refusal::BAD_PARAMS);
    }
    let want = usize::try_from(params.tail).map_err(|_| Refusal::BAD_PARAMS)?;
    let session = SessionId::parse(&params.session).map_err(|_| Refusal::BAD_PARAMS)?;
    let job = JobId::parse(&params.job).map_err(|_| Refusal::BAD_PARAMS)?;
    let found = core.sessions.get(core, &session, None, None).await?;
    let output = match found.handle.job_output(job).await {
        Ok(Ok(output)) => output,
        Ok(Err(Unreadable::Unknown)) => return Err(Refusal::UNKNOWN_JOB),
        Ok(Err(Unreadable::Agent)) => return Err(Refusal::NOT_A_COMMAND),
        Err(_) => {
            core.sessions.forget(&session).await;
            return Err(Refusal::STOPPED);
        }
    };
    let tail = match output.text {
        // 开不了的（没存下来）和空的一样，和 `jobs` 一样。
        None => Tail::default(),
        Some(text) => tokio::task::spawn_blocking(move || Tail::read(text, want))
            .await
            .map_err(|error| {
                tracing::error!(target: TARGET, error = %error, "job output panicked");
                Refusal::INTERNAL
            })?,
    };
    Ok(reply(tail, output.running))
}

/// 回应的 `result`：格照名字的字母先后排（`serde_json` 的对象就是这样排的）。
fn reply(tail: Tail, running: bool) -> Value {
    json!({
        "lines": tail.lines,
        "output": tail.output,
        "running": running,
        "truncated": tail.truncated,
    })
}

/// 读到的尾巴。
#[derive(Debug, Default, PartialEq, Eq)]
struct Tail {
    /// 交的字：最后几行照原样接起来，每一行带着它的换行（最后一段没有换行的照样没有）。
    output: String,
    /// 一共几行。
    lines: u64,
    /// 前面还有没交的：去掉了前面的行，或者只交了一行的末尾。
    truncated: bool,
}

impl Tail {
    /// 从 `source` 读出最后 `want` 行（至少 1），一共不过 [`CAP`] 字节。一行照换行切，最后一段没有换行的也算一行，和 `jobs`
    /// 的数法一样（`tools/jobs.md` 第 3 条）；解不开的字节换成 `�`，也和它一样。读到一半读不下去的，读到多少算多少。
    fn read(source: impl Read, want: usize) -> Tail {
        let mut reader = BufReader::new(source);
        let mut kept = Kept::new(want);
        let mut line = Vec::new();
        let mut cut = false;
        loop {
            let chunk = match reader.fill_buf() {
                Ok([]) => break,
                Ok(chunk) => chunk,
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(_) => break,
            };
            let (take, ends) = match chunk.iter().position(|&byte| byte == b'\n') {
                Some(at) => (at + 1, true),
                None => (chunk.len(), false),
            };
            line.extend_from_slice(&chunk[..take]);
            reader.consume(take);
            // 一行太长：只留末尾，内存不跟着它涨。攒到两倍才挪，挪一次去掉一半。
            if line.len() > 2 * CAP {
                line.drain(..line.len() - CAP);
                cut = true;
            }
            if ends {
                kept.push(std::mem::take(&mut line), cut);
                cut = false;
            }
        }
        if !line.is_empty() {
            kept.push(line, cut);
        }
        kept.finish()
    }
}

/// 边读边留着的最后几行。
struct Kept {
    /// 最多留几行。
    want: usize,
    lines: VecDeque<String>,
    /// 留着的一共多少字节。
    bytes: usize,
    /// 一共读到几行。
    total: u64,
    /// 留着的只有一行，而且只留了它的末尾。
    cut: bool,
}

impl Kept {
    fn new(want: usize) -> Kept {
        Kept {
            want,
            lines: VecDeque::new(),
            bytes: 0,
            total: 0,
            cut: false,
        }
    }

    /// 读到一行（带着它的换行）；`cut` 是读的时候已经去掉了它的前面。
    fn push(&mut self, raw: Vec<u8>, cut: bool) {
        self.total += 1;
        let (text, cut) = fit(raw, cut);
        // 只留了末尾的一行，只在它是最后一行时交，而且只交它：前面接着别的行，看着就像它从那里开头（`protocol.md` 的
        // `job.output` 第 3 条）。它后面又来了一行，它整行去掉。
        if cut || self.cut {
            self.lines.clear();
            self.bytes = 0;
        }
        self.cut = cut;
        self.bytes += text.len();
        self.lines.push_back(text);
        // 刚来的这一行放得下（`fit`），`want` 至少是 1：去掉的只会是前面的。
        while self.lines.len() > self.want || self.bytes > CAP {
            match self.lines.pop_front() {
                Some(front) => self.bytes -= front.len(),
                None => break,
            }
        }
    }

    fn finish(self) -> Tail {
        let kept = u64::try_from(self.lines.len()).unwrap_or(u64::MAX);
        Tail {
            truncated: self.cut || kept < self.total,
            lines: self.total,
            output: self.lines.into_iter().collect(),
        }
    }
}

/// 一行写成字，最多 [`CAP`] 字节：超了的只留末尾，从一个字的开头起。交回的第二样是它的前面去掉过没有。
fn fit(mut raw: Vec<u8>, cut: bool) -> (String, bool) {
    // 读的时候去掉过前面的，开头可能是半个字：跳过接续字节，不让开头是 `�`。
    if cut {
        let partial = raw.iter().take_while(|&&byte| byte & 0xC0 == 0x80).count();
        raw.drain(..partial);
    }
    let mut text = String::from_utf8(raw)
        .unwrap_or_else(|error| String::from_utf8_lossy(error.as_bytes()).into_owned());
    if text.len() <= CAP {
        return (text, cut);
    }
    let mut at = text.len() - CAP;
    while !text.is_char_boundary(at) {
        at += 1;
    }
    text.drain(..at);
    (text, true)
}

#[cfg(test)]
mod tests;
