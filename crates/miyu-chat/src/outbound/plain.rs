//! Markdown 转纯文本（`docs/blueprint/chat.md` 第五条「怎么走」第 5 条）：QQ 不画 Markdown，`**`、反引号原样发出去很难看。
//! 故意保守，照旧版：代码块里原样留着，单个 `*` 留着（可能是乘号），列表和换行留着；只去那几样不会认错的标记。

/// 代码块的开关：一行（去掉开头的空白以后）以它们之一开头。
const FENCES: [&str; 2] = ["```", "~~~"];

/// 标题的记号。
const HEADING: char = '#';

/// 引用的记号，连着后面的空格。
const QUOTE: &str = "> ";

/// Markdown 转纯文本：一行一行看，代码块里的原样留着、开关代码块的那一行不要；别的行去掉标题的 `#`、引用的 `> `，再去掉
/// 行内的 `**`、`__`、反引号，`[字](地址)` 写成 `字 (地址)`（地址空的或和字一样的只留字）。最后去掉末尾的空白。
///
/// 没关上的代码块一直到末尾。
pub fn plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut fenced = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if FENCES.iter().any(|fence| trimmed.starts_with(fence)) {
            fenced = !fenced;
            continue;
        }
        if fenced {
            out.push_str(line);
        } else {
            let body = if trimmed.starts_with(HEADING) {
                trimmed.trim_start_matches(HEADING).trim_start()
            } else {
                trimmed.strip_prefix(QUOTE).unwrap_or(line)
            };
            inline(body, &mut out);
        }
        out.push('\n');
    }
    out.truncate(out.trim_end().len());
    out
}

/// 去掉一行里的 `**`、`__`、反引号，`[字](地址)` 写成 `字 (地址)`，写进 `out`。不成对的 `[`、`]`、`(` 原样留着。
fn inline(line: &str, out: &mut String) {
    let chars: Vec<char> = line.chars().collect();
    // 每个位置往后最近的 `]`、`)` 在哪，先倒着扫一遍记下。逐个 `[` 往后找是平方的：旧版实测一行 16000 个不成对的 `[`
    // 要 358ms，而每条出站的消息都走这里。
    let close = next_of(&chars, ']');
    let paren = next_of(&chars, ')');
    let mut i = 0;
    while let Some(&c) = chars.get(i) {
        let next = chars.get(i + 1).copied();
        match c {
            '*' | '_' if next == Some(c) => i += 2,
            '`' => i += 1,
            '[' => match link(&chars, &close, &paren, i) {
                Some((label, address, end)) => {
                    out.push_str(&label);
                    if !address.is_empty() && address != label {
                        out.push_str(" (");
                        out.push_str(&address);
                        out.push(')');
                    }
                    i = end;
                }
                None => {
                    out.push(c);
                    i += 1;
                }
            },
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
}

/// 从第 `at` 个字（`[`）起的链接 `[字](地址)`：字、地址、链接后面那个字的下标。`]` 是往后最近的一个，紧跟着 `(`，`)`
/// 是再往后最近的一个；不是这样的是 `None`。
fn link(
    chars: &[char],
    close: &[Option<usize>],
    paren: &[Option<usize>],
    at: usize,
) -> Option<(String, String, usize)> {
    let label_end = close.get(at + 1).copied().flatten()?;
    if chars.get(label_end + 1) != Some(&'(') {
        return None;
    }
    let address_end = paren.get(label_end + 2).copied().flatten()?;
    let label = chars.get(at + 1..label_end)?.iter().collect();
    let address = chars.get(label_end + 2..address_end)?.iter().collect();
    Some((label, address, address_end + 1))
}

/// 每个下标往后（含它自己）最近的 `target` 在哪；多出一格放在末尾，是 `None`，下标越过末尾一个也能查。
fn next_of(chars: &[char], target: char) -> Vec<Option<usize>> {
    let mut nearest = None;
    let mut next: Vec<Option<usize>> = chars
        .iter()
        .enumerate()
        .rev()
        .map(|(i, &c)| {
            if c == target {
                nearest = Some(i);
            }
            nearest
        })
        .collect();
    next.reverse();
    next.push(None);
    next
}

#[cfg(test)]
mod tests;
