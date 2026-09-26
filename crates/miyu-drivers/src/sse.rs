//! SSE 分帧（WHATWG HTML 标准「Server-sent events」里的解析规矩）：字节一片一片进，一条一条的
//! 事件出。分片从哪里切开都一样。
//!
//! - 换行：`\n`、`\r\n`、`\r` 都算；`\r\n` 被切在两片之间，不多出一个空行。
//! - 一行收全了才按 UTF-8 解：一个汉字被切在两片之间也不坏。坏的字节换成 U+FFFD，规矩就是这样。
//! - `data:` 后面的一个空格去掉，几行 `data` 用换行接起来；`event:` 记下名字；`:` 开头的是注释；
//!   `id:`、`retry:` 和不认识的字段不理。空行结束一条，有 `data` 的才交出来。
//! - 流断在半条上：[`Sse::finish`] 把最后一条照样交出来。

use std::mem;

/// 一条事件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// `event:` 写的名字；没写的是 `None`（规矩里的默认名字是 `message`）。
    pub event: Option<String>,
    /// `data`，几行用换行接起来。
    pub data: String,
}

/// 分帧的状态：还没收全的一行，和这一条攒到的字段。
#[derive(Debug, Clone, Default)]
pub struct Sse {
    /// 还没收全的一行。
    line: Vec<u8>,
    /// 上一个字节是 `\r`：紧跟着的 `\n` 是同一个换行。
    after_cr: bool,
    /// 这一条的 `data`，每一行后面都跟一个换行，交出来时去掉最后那个。
    data: String,
    /// 这一条的名字。
    event: Option<String>,
}

impl Sse {
    /// 空的。
    pub fn new() -> Sse {
        Sse::default()
    }

    /// 喂一片字节，交回这一片里收全了的事件。
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<Event> {
        let mut out = Vec::new();
        for &byte in bytes {
            let after_cr = mem::replace(&mut self.after_cr, byte == b'\r');
            match byte {
                b'\n' if after_cr => {}
                b'\n' | b'\r' => self.end_line(&mut out),
                _ => self.line.push(byte),
            }
        }
        out
    }

    /// 流完了：没收全的最后一行、最后一条，照样交出来。
    pub fn finish(mut self) -> Vec<Event> {
        let mut out = Vec::new();
        if !self.line.is_empty() {
            self.end_line(&mut out);
        }
        self.dispatch(&mut out);
        out
    }

    /// 收全了一行。
    fn end_line(&mut self, out: &mut Vec<Event>) {
        let line = String::from_utf8_lossy(&mem::take(&mut self.line)).into_owned();
        if line.is_empty() {
            self.dispatch(out);
            return;
        }
        if line.starts_with(':') {
            return;
        }
        let (field, value) = match line.split_once(':') {
            Some((field, value)) => (field, value.strip_prefix(' ').unwrap_or(value)),
            None => (line.as_str(), ""),
        };
        match field {
            "data" => {
                self.data.push_str(value);
                self.data.push('\n');
            }
            "event" => self.event = Some(value.to_string()),
            _ => {}
        }
    }

    /// 空行：有 `data` 的交出一条，名字和 `data` 都清掉。
    fn dispatch(&mut self, out: &mut Vec<Event>) {
        let event = self.event.take();
        if self.data.is_empty() {
            return;
        }
        let mut data = mem::take(&mut self.data);
        data.pop();
        out.push(Event { event, data });
    }
}

#[cfg(test)]
mod tests;
