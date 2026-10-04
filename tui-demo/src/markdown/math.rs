//! 公式转成 Unicode（蓝图 `tui.md`「图片、公式和 mermaid 图」第 5 条，照旧版 `render/math/unicode.rs`）：
//! `x^2` → `x²`，`\alpha` → `α`，`\frac{a}{b}` → `a/b`。尽力而为，转不动的命令原样留着，永不失败。
//!
//! 对照表（命令到符号、上下标、附标）写在 `resources/math.json`。

use std::collections::HashMap;

use serde::Deserialize;

/// 嵌套的上限：公式是模型写的，递归没有上限就是一个爆栈的口子。超了的原样交回。
const MAX_NESTING: usize = 64;

/// 公式的对照表（`resources/math.json`）。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Math {
    /// 命令名（不带 `\`）到写出来的字，例如 `alpha` → `α`，`sin` → ` sin `。
    pub symbols: HashMap<String, String>,
    /// 上标字符：`2` → `²`。有一个字没有上标写法的，整段写成 `^(…)`。
    pub superscripts: HashMap<char, char>,
    /// 下标字符：`n` → `ₙ`，规则同上标。
    pub subscripts: HashMap<char, char>,
    /// 附标命令到组合字符：`hat` → U+0302，接在参数的字后面。
    pub accents: HashMap<String, String>,
    /// 只留参数的命令：`\text{…}`、`\mathbf{…}` 这类。
    pub text: Vec<String>,
    /// 整个丢掉的命令：`\left`、`\displaystyle` 这类只管排版的。
    pub ignored: Vec<String>,
    /// 写成一个空格的命令：`\quad`、`\qquad`。
    pub spaces: Vec<String>,
}

/// 把一段 LaTeX 转成一行 Unicode；多个空白并成一个。
pub fn unicode(tex: &str, math: &Math) -> String {
    let chars: Vec<char> = tex.chars().collect();
    if depth(&chars) > MAX_NESTING {
        return tex.to_string();
    }
    let mut at = 0;
    let out = Converter {
        chars: &chars,
        math,
    }
    .sequence(&mut at, None);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 没转义的 `{` 最深套了几层。
fn depth(chars: &[char]) -> usize {
    let (mut depth, mut max, mut escaped) = (0usize, 0usize, false);
    for &c in chars {
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' => escaped = true,
            '{' => {
                depth += 1;
                max = max.max(depth);
            }
            '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    max
}

struct Converter<'a> {
    chars: &'a [char],
    math: &'a Math,
}

impl Converter<'_> {
    /// 一串，遇到 `stop`（组的 `}`）停。
    fn sequence(&self, at: &mut usize, stop: Option<char>) -> String {
        let mut out = String::new();
        while let Some(&c) = self.chars.get(*at) {
            *at += 1;
            if Some(c) == stop {
                return out;
            }
            match c {
                '\\' => out.push_str(&self.command(at)),
                '^' => {
                    let script = self.group(at);
                    out.push_str(&self.script(&script, &self.math.superscripts, '^'));
                }
                '_' => {
                    let script = self.group(at);
                    out.push_str(&self.script(&script, &self.math.subscripts, '_'));
                }
                '{' => out.push_str(&self.sequence(at, Some('}'))),
                '\'' => out.push('′'),
                '~' => out.push(' '),
                c => out.push(c),
            }
        }
        out
    }

    /// 一个参数：`{…}`，或者一个命令、一个字。
    fn group(&self, at: &mut usize) -> String {
        match self.chars.get(*at) {
            Some('{') => {
                *at += 1;
                self.sequence(at, Some('}'))
            }
            Some('\\') => {
                *at += 1;
                self.command(at)
            }
            Some(c) => {
                *at += 1;
                c.to_string()
            }
            None => String::new(),
        }
    }

    /// `\` 后面的命令。
    fn command(&self, at: &mut usize) -> String {
        // 单个字的转义：`\{`、`\,` 这些。
        if let Some(&c) = self.chars.get(*at)
            && !c.is_ascii_alphabetic()
        {
            *at += 1;
            return if matches!(c, ',' | ';' | ':' | ' ' | '!') {
                " ".to_string()
            } else {
                c.to_string()
            };
        }
        let start = *at;
        while self.chars.get(*at).is_some_and(char::is_ascii_alphabetic) {
            *at += 1;
        }
        let name: String = self.chars[start..*at].iter().collect();
        // 命令后面的一个空格是分隔，不算字（TeX 的规矩）。
        if self.chars.get(*at) == Some(&' ') {
            *at += 1;
        }
        let math = self.math;
        match name.as_str() {
            "frac" | "dfrac" | "tfrac" => {
                let top = self.group(at);
                let bottom = self.group(at);
                format!("{}/{}", parenthesize(&top), parenthesize(&bottom))
            }
            "sqrt" => format!("√{}", parenthesize(&self.group(at))),
            "binom" | "dbinom" | "tbinom" => {
                let upper = self.group(at);
                let lower = self.group(at);
                format!("C({},{})", upper.trim(), lower.trim())
            }
            name if math.text.iter().any(|t| t == name) => self.group(at),
            name if math.ignored.iter().any(|t| t == name) => String::new(),
            name if math.spaces.iter().any(|t| t == name) => " ".to_string(),
            name if math.accents.contains_key(name) => {
                let argument = self.group(at);
                format!("{argument}{}", math.accents[name])
            }
            name => math
                .symbols
                .get(name)
                .cloned()
                .unwrap_or_else(|| format!("\\{name}")),
        }
    }

    /// 上下标：每个字都有对应的才转，不然写成 `^(…)`。
    fn script(&self, content: &str, table: &HashMap<char, char>, marker: char) -> String {
        let trimmed = content.trim();
        let converted: Option<String> = trimmed.chars().map(|c| table.get(&c).copied()).collect();
        match converted {
            Some(text) => text,
            None => format!("{marker}{}", parenthesize(trimmed)),
        }
    }
}

/// 不是一个字、也没整个括起来的，加括号：`\frac{a+1}{b}` 写成 `(a+1)/b`，不然优先级错了。
fn parenthesize(text: &str) -> String {
    let trimmed = text.trim();
    let simple = trimmed.chars().count() <= 1
        || trimmed
            .chars()
            .all(|c| c.is_alphanumeric() || c == '.' || c == '′')
        || wrapped(trimmed);
    if simple {
        trimmed.to_string()
    } else {
        format!("({trimmed})")
    }
}

/// 整个被同一对括号包着：`(a)+(b)` 两头虽是括号，头一个在中间就合上了，不算。
fn wrapped(text: &str) -> bool {
    if !text.starts_with('(') || !text.ends_with(')') {
        return false;
    }
    let mut depth = 0usize;
    for (i, c) in text.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return i + c.len_utf8() == text.len();
                }
            }
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{Math, unicode};

    fn math() -> Math {
        serde_json::from_str(include_str!("../../resources/math.json")).unwrap()
    }

    #[test]
    fn scripts_greek_and_fractions() {
        let m = math();
        assert_eq!(unicode("x^2 + y_n", &m), "x² + yₙ");
        // 命令后面的一个空格是分隔，吃掉。
        assert_eq!(unicode(r"\alpha \in (0,1)", &m), "α∈(0,1)");
        assert_eq!(unicode(r"\frac{a+1}{b}", &m), "(a+1)/b");
        assert_eq!(unicode(r"\frac{(a)+(b)}{c}", &m), "((a)+(b))/c");
        assert_eq!(unicode(r"\sqrt{\pi}", &m), "√π");
        assert_eq!(unicode(r"\hat{x}", &m), "x\u{302}");
        // 没有上标写法的字，整段写成 ^(…)。
        assert_eq!(unicode("e^{-x^2}", &m), "e^(-x²)");
        assert_eq!(unicode("2^{f+g}", &m), "2^(f+g)");
    }

    #[test]
    fn unknown_commands_and_deep_nesting_stay_as_written() {
        let m = math();
        assert_eq!(unicode(r"\foo{x}", &m), r"\foox");
        let deep = format!("{}x{}", "{".repeat(100), "}".repeat(100));
        assert_eq!(unicode(&deep, &m), deep);
    }
}
