//! 一行代码怎么认（蓝图 `tui.md`「代码着色」第 1 条）：先接上一行的块注释、三引号字符串，再从左往右一样一样认，
//! 认不出的照原样。只看这一行的字，不做语法分析。

use ratatui::style::Style;

use super::{Highlighter, State};
use crate::markdown::inline::Piece;
use crate::theme;

/// 攒片段：挨着的同样子的接成一段。
#[derive(Default)]
struct Out(Vec<Piece>);

impl Out {
    fn push(&mut self, text: &str, style: Style) {
        if text.is_empty() {
            return;
        }
        match self.0.last_mut() {
            Some(last) if last.style == style => last.text.push_str(text),
            _ => self.0.push(Piece::new(text, style)),
        }
    }
}

/// 一行的字，按字走。
struct Line<'l> {
    text: &'l str,
    chars: Vec<(usize, char)>,
}

impl Line<'_> {
    /// 第 `i` 个字的字节下标；过了末尾是长度。
    fn byte(&self, i: usize) -> usize {
        self.chars.get(i).map_or(self.text.len(), |(b, _)| *b)
    }

    /// 字节下标 `b` 是第几个字。
    fn index(&self, b: usize) -> usize {
        self.chars.partition_point(|(at, _)| *at < b)
    }

    fn char(&self, i: usize) -> Option<char> {
        self.chars.get(i).map(|(_, c)| *c)
    }

    fn slice(&self, from: usize, to: usize) -> &str {
        &self.text[self.byte(from)..self.byte(to)]
    }

    /// 从第 `i` 个字起连着的名字字符到哪。
    fn word_end(&self, i: usize) -> usize {
        (i..self.chars.len())
            .find(|&j| !is_word(self.chars[j].1))
            .unwrap_or(self.chars.len())
    }

    /// 第 `i` 个字往后（含）第一个不是空白的字。
    fn next_solid(&self, i: usize) -> Option<char> {
        self.chars[i.min(self.chars.len())..]
            .iter()
            .map(|(_, c)| *c)
            .find(|c| !c.is_whitespace())
    }

    /// 第 `i` 个字往前（不含）第一个不是空白的字。
    fn prev_solid(&self, i: usize) -> Option<char> {
        self.chars[..i]
            .iter()
            .rev()
            .map(|(_, c)| *c)
            .find(|c| !c.is_whitespace())
    }
}

impl Highlighter<'_> {
    /// 一行着色（不是 diff 的）。
    pub(super) fn scan(&mut self, text: &str) -> Vec<Piece> {
        let line = Line {
            text,
            chars: text.char_indices().collect(),
        };
        let mut out = Out::default();
        let Some(mut i) = self.resume(&line, &mut out) else {
            return out.0;
        };
        let lang = self.language;
        // 一行只有 `[段]` 的：段名（TOML、INI）。
        if i == 0 && lang.is_some_and(|l| l.sections) {
            let body = text.trim();
            if body.starts_with('[') && body.ends_with(']') {
                let lead = text.len() - text.trim_start().len();
                out.push(&text[..lead], Style::new());
                out.push(body, theme::code_type());
                out.push(&text[lead + body.len()..], Style::new());
                return out.0;
            }
        }
        let mut in_tag = false;
        let variables = lang.is_some_and(|l| l.variables);
        while i < line.chars.len() {
            // 双引号里的变量会展开（shell、PHP）：字符串里的 `$名字` 单独着色。
            if variables && line.char(i) == Some('"') {
                i = interpolated(&line, i, &mut out);
                continue;
            }
            match self.token(&line, i, &mut in_tag) {
                Some((end, style)) => {
                    out.push(line.slice(i, end), style);
                    i = end;
                }
                // 到行尾都是它（注释、没收尾的块注释和三引号字符串）。
                None => {
                    let (rest, style) = self.until_end(&line, i);
                    out.push(rest, style);
                    break;
                }
            }
        }
        out.0
    }

    /// 接上一行没收尾的块注释、三引号字符串。交回接完以后从第几个字接着认；整行都是的交回 `None`。
    fn resume(&mut self, line: &Line, out: &mut Out) -> Option<usize> {
        let (close, style) = match self.state {
            State::Code => return Some(0),
            State::Comment => (
                self.language
                    .and_then(|l| l.block_comment.as_ref())
                    .map_or(String::new(), |[_, close]| close.clone()),
                theme::code_comment(),
            ),
            State::Triple(q) => (q.to_string().repeat(3), theme::code_string()),
        };
        match line.text.find(close.as_str()).filter(|_| !close.is_empty()) {
            Some(at) => {
                let stop = at + close.len();
                out.push(&line.text[..stop], style);
                self.state = State::Code;
                Some(line.index(stop))
            }
            None => {
                out.push(line.text, style);
                None
            }
        }
    }

    /// 第 `i` 个字起的一样东西：交回它到第几个字（不含）和样子；到行尾都是它的交回 `None`（顺手记下跨行）。
    fn token(&mut self, line: &Line, i: usize, in_tag: &mut bool) -> Option<(usize, Style)> {
        let lang = self.language;
        let rest = &line.text[line.byte(i)..];
        let c = line.char(i).unwrap_or(' ');
        let prev = i.checked_sub(1).and_then(|p| line.char(p));
        if let Some([open, close]) = lang.and_then(|l| l.block_comment.as_ref())
            && rest.starts_with(open.as_str())
        {
            let Some(at) = rest[open.len()..].find(close.as_str()) else {
                self.state = State::Comment;
                return None;
            };
            let stop = line.byte(i) + open.len() + at + close.len();
            return Some((line.index(stop), theme::code_comment()));
        }
        if let Some(l) = lang {
            let attribute = l.attributes && (rest.starts_with("#[") || rest.starts_with("#!["));
            // `#` 当注释的，要在一行开头或空白后面（`a#b` 不是注释）。
            let spaced = l.comment != "#" || prev.is_none_or(char::is_whitespace);
            if !l.comment.is_empty() && rest.starts_with(&l.comment) && spaced && !attribute {
                return None;
            }
            if attribute {
                let end = bracket_end(rest);
                return Some((line.index(line.byte(i) + end), theme::code_macro()));
            }
        }
        // 预处理指令：一行打头（前面只有空白）的 `#名字`。
        if c == '#'
            && lang.is_some_and(|l| l.preprocessor)
            && line.prev_solid(i).is_none()
            && line.char(i + 1).is_some_and(is_word)
        {
            return Some((line.word_end(i + 1), theme::code_macro()));
        }
        if c == '@' && lang.is_some_and(|l| l.decorators) && line.char(i + 1).is_some_and(is_word) {
            return Some((line.word_end(i + 1), theme::code_macro()));
        }
        if c == '$' && lang.is_some_and(|l| l.variables) {
            return Some((variable_end(line, i), theme::code_macro()));
        }
        if lang.is_some_and(|l| l.tags) {
            if c == '<' && line.char(i + 1).is_some_and(|n| is_word(n) || n == '/') {
                *in_tag = true;
                let skip = if line.char(i + 1) == Some('/') { 2 } else { 1 };
                return Some((i + skip, theme::code_operator()));
            }
            if *in_tag && c == '>' {
                *in_tag = false;
                return Some((i + 1, theme::code_operator()));
            }
        }
        self.literal(line, i, c, prev, *in_tag)
    }

    /// 字符串、数字、参数、名字、运算符；都不是的一个字原样。
    fn literal(
        &mut self,
        line: &Line,
        i: usize,
        c: char,
        prev: Option<char>,
        in_tag: bool,
    ) -> Option<(usize, Style)> {
        let lang = self.language;
        let rest = &line.text[line.byte(i)..];
        if lang.is_some_and(|l| l.triple_quote_strings)
            && (rest.starts_with("\"\"\"") || rest.starts_with("'''"))
        {
            let close = c.to_string().repeat(3);
            let Some(at) = rest[3..].find(close.as_str()) else {
                self.state = State::Triple(c);
                return None;
            };
            return Some((line.index(line.byte(i) + 3 + at + 3), theme::code_string()));
        }
        let quote = c == '"'
            || (c == '\'' && lang.is_some_and(|l| l.single_quote_strings))
            || (c == '`' && lang.is_some_and(|l| l.backtick_strings));
        if quote {
            let end = string_end(line, i, c);
            let style = if self.key_here(line, i, end) {
                theme::code_property()
            } else {
                theme::code_string()
            };
            return Some((end, style));
        }
        if c.is_ascii_digit() && !prev.is_some_and(is_word) {
            let end = (i..line.chars.len())
                .find(|&j| !matches!(line.chars[j].1, c if c.is_ascii_alphanumeric() || c == '.' || c == '_'))
                .unwrap_or(line.chars.len());
            return Some((end, theme::code_number()));
        }
        if c == '-'
            && lang.is_some_and(|l| l.flags)
            && prev.is_none_or(char::is_whitespace)
            && line
                .char(i + 1)
                .is_some_and(|n| n == '-' || n.is_alphabetic())
        {
            let end = (i..line.chars.len())
                .find(|&j| line.chars[j].1.is_whitespace() || line.chars[j].1 == '=')
                .unwrap_or(line.chars.len());
            return Some((end, theme::code_parameter()));
        }
        if is_word(c) {
            let end = line.word_end(i);
            return Some(self.word(line, i, end, in_tag));
        }
        if is_operator(c) {
            let end = (i..line.chars.len())
                .find(|&j| !is_operator(line.chars[j].1))
                .unwrap_or(line.chars.len());
            return Some((end, theme::code_operator()));
        }
        Some((i + 1, Style::new()))
    }

    /// 一个名字：宏、键、关键字、类型、常量、函数，照这个先后认。交回到第几个字（宏带上 `!`）和样子。
    fn word(&self, line: &Line, i: usize, end: usize, in_tag: bool) -> (usize, Style) {
        let word = line.slice(i, end);
        let lang = self.language;
        let bang = line.char(end) == Some('!') && line.char(end + 1) != Some('=');
        if bang && lang.is_some_and(|l| l.macros) {
            return (end + 1, theme::code_macro());
        }
        let next = line.next_solid(end);
        if (in_tag && next == Some('=')) || self.key_here(line, i, end) {
            return (end, theme::code_property());
        }
        // 标签名：紧跟在 `<`、`</` 后面的。
        if in_tag && matches!(line.prev_solid(i), Some('<' | '/')) {
            return (end, theme::code_keyword());
        }
        // 常量先于关键字：有的语言的关键字表里也写着 `true`、`None`。
        let style = if self.constants.contains(word) {
            theme::code_constant()
        } else if self.is_keyword(word) {
            theme::code_keyword()
        } else if self.types.contains(word) {
            theme::code_type()
        } else if shouting(word) {
            theme::code_constant()
        } else if next == Some('(') {
            theme::code_function()
        } else if lang.is_some_and(|l| l.capital_types) && capitalized(word) {
            theme::code_type()
        } else {
            Style::new()
        };
        (end, style)
    }

    /// `[i, end)` 这个名字、字符串是不是键：语言认键，它在一行开头（或 `{`、`,`、`-` 后面），后面紧跟键的记号。
    fn key_here(&self, line: &Line, i: usize, end: usize) -> bool {
        let Some(seps) = self
            .language
            .map(|l| l.keys.as_str())
            .filter(|k| !k.is_empty())
        else {
            return false;
        };
        let opens = line
            .prev_solid(i)
            .is_none_or(|p| matches!(p, '{' | ',' | '-'));
        let next = line.next_solid(end);
        opens && next.is_some_and(|n| seps.contains(n)) && line.char(end + 1) != Some(':')
    }

    /// 从第 `i` 个字到行尾，和它的样子（行注释、没收尾的块注释和三引号字符串）。
    fn until_end<'l>(&self, line: &Line<'l>, i: usize) -> (&'l str, Style) {
        let style = match self.state {
            State::Triple(_) => theme::code_string(),
            _ => theme::code_comment(),
        };
        (&line.text[line.byte(i)..], style)
    }
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn is_operator(c: char) -> bool {
    matches!(
        c,
        '=' | '+' | '-' | '*' | '/' | '%' | '<' | '>' | '!' | '&' | '|' | '^' | '~' | '?' | ':'
    )
}

/// 全大写的名字（至少两个字、有字母、没有小写）：当常量。
fn shouting(word: &str) -> bool {
    word.chars().count() >= 2
        && word.chars().any(char::is_alphabetic)
        && !word.chars().any(char::is_lowercase)
}

/// 大写开头、带小写的名字：当类型。
fn capitalized(word: &str) -> bool {
    word.chars().next().is_some_and(char::is_uppercase) && word.chars().any(char::is_lowercase)
}

/// 字符串到哪结束：下一个没被反斜杠转义的同样的引号之后；没有的到行尾。
fn string_end(line: &Line, start: usize, quote: char) -> usize {
    let mut i = start + 1;
    while i < line.chars.len() {
        match line.chars[i].1 {
            '\\' => i += 2,
            c if c == quote => return i + 1,
            _ => i += 1,
        }
    }
    line.chars.len()
}

/// 从第 `i` 个字起的双引号字符串，里面的变量单独着色，攒进 `out`；交回字符串之后是第几个字。
fn interpolated(line: &Line, i: usize, out: &mut Out) -> usize {
    let end = string_end(line, i, '"');
    let mut from = i;
    let mut j = i + 1;
    while j < end {
        match line.char(j) {
            Some('\\') => j += 2,
            Some('$') if j + 1 < end => {
                out.push(line.slice(from, j), theme::code_string());
                let stop = variable_end(line, j).min(end);
                out.push(line.slice(j, stop), theme::code_macro());
                from = stop;
                j = stop.max(j + 1);
            }
            _ => j += 1,
        }
    }
    out.push(line.slice(from, end), theme::code_string());
    end
}

/// `#[…]`、`#![…]` 到哪结束（字节数）：方括号配平；没配平的到行尾。
fn bracket_end(rest: &str) -> usize {
    let mut depth = 0;
    for (at, c) in rest.char_indices() {
        match c {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return at + 1;
                }
            }
            _ => {}
        }
    }
    rest.len()
}

/// `$名字`、`${…}`、`$(…)`、`$1`、`$?` 到哪结束。
fn variable_end(line: &Line, i: usize) -> usize {
    match line.char(i + 1) {
        Some(open @ ('{' | '(')) => {
            let close = if open == '{' { '}' } else { ')' };
            (i + 2..line.chars.len())
                .find(|&j| line.chars[j].1 == close)
                .map_or(line.chars.len(), |j| j + 1)
        }
        Some(c) if is_word(c) => line.word_end(i + 1),
        Some('@' | '#' | '?' | '*' | '!' | '$' | '-') => i + 2,
        _ => i + 1,
    }
}
