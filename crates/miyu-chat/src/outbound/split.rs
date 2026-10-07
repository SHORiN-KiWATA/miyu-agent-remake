//! 按段拆开（`docs/blueprint/chat.md` 第五条「怎么走」第 6 条）：太长的回复切成几条发。切点先落在空行上，再落在换行上，
//! 实在不行才按字符硬切：硬切会把一句话腰斩。按字符数算，不按字节，中文不会被切到半个字。

/// 段和段之间：空行。
const PARAGRAPH: &str = "\n\n";

/// 行和行之间。
const LINE: &str = "\n";

/// 按段拆开：去掉首尾空白；空的不出；不超过 `max_chars` 个字符或 `max_chars` 是 0 的原样一段。别的先按空行分段往一块里
/// 装，装不下就换一块；一段本身就超了，按行装；一行本身就超了，按字符硬切。每一块去掉首尾空白，空的不出。
pub fn split(text: &str, max_chars: usize) -> Vec<String> {
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }
    if max_chars == 0 || chars(text) <= max_chars {
        return vec![text.to_string()];
    }
    let mut pieces = Pieces {
        max: max_chars,
        done: Vec::new(),
        current: String::new(),
        count: 0,
    };
    for paragraph in text.split(PARAGRAPH) {
        if chars(paragraph) <= max_chars {
            pieces.add(paragraph, PARAGRAPH);
            continue;
        }
        // 超长的段从新的一块开始，装完也自成一块，不和后面的段拼。
        pieces.flush();
        for line in paragraph.lines() {
            if chars(line) <= max_chars {
                pieces.add(line, LINE);
                continue;
            }
            pieces.flush();
            let line: Vec<char> = line.chars().collect();
            for cut in line.chunks(max_chars) {
                pieces.emit(&cut.iter().collect::<String>());
            }
        }
        pieces.flush();
    }
    pieces.flush();
    pieces.done
}

/// 字符数。
fn chars(text: &str) -> usize {
    text.chars().count()
}

/// 拆出来的几块，和正在装的一块。
struct Pieces {
    /// 一块最多多少个字符。
    max: usize,
    /// 装好的几块。
    done: Vec<String>,
    /// 正在装的一块。
    current: String,
    /// 正在装的一块有多少个字符：不每次重数。
    count: usize,
}

impl Pieces {
    /// 往正在装的一块里加一份，和前面的用 `separator` 隔开；加上就超了的，先把正在装的一块装好。`unit` 自己不超过 `max`。
    fn add(&mut self, unit: &str, separator: &str) {
        let units = chars(unit);
        let separators = chars(separator);
        if !self.current.is_empty() && self.count + separators + units > self.max {
            self.flush();
        }
        if !self.current.is_empty() {
            self.current.push_str(separator);
            self.count += separators;
        }
        self.current.push_str(unit);
        self.count += units;
    }

    /// 正在装的一块装好，从空的重新装。
    fn flush(&mut self) {
        let current = std::mem::take(&mut self.current);
        self.count = 0;
        self.emit(&current);
    }

    /// 装好一块：去掉首尾空白，空的不出。
    fn emit(&mut self, piece: &str) {
        let piece = piece.trim();
        if !piece.is_empty() {
            self.done.push(piece.to_string());
        }
    }
}

#[cfg(test)]
mod tests;
