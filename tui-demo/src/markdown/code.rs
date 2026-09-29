//! 代码着色（蓝图 `tui.md`「代码着色」，照旧版 `render/code.rs`）：词法上的近似，一行一行认关键字、函数名、
//! 字符串、数字、注释，不做语法分析。各语言的关键字和注释记号在 `resources/code.json`。

use std::collections::HashMap;

use ratatui::style::Style;
use serde::Deserialize;

use super::inline::Piece;
use crate::theme;

/// `code.json` 的样子。
#[derive(Debug, Clone, Deserialize)]
pub struct Languages {
    /// 一种语言一条。
    pub languages: Vec<Language>,
}

/// 一种语言。
#[derive(Debug, Clone, Deserialize)]
pub struct Language {
    /// 名字和别名，照代码块开头写的那个认，不分大小写。
    pub names: Vec<String>,
    /// 关键字。
    pub keywords: Vec<String>,
    /// 行注释的记号。
    pub comment: String,
    /// 单引号括起来的也是字符串。
    pub single_quote_strings: bool,
    /// 反引号括起来的也是字符串。
    pub backtick_strings: bool,
}

impl Languages {
    /// 照代码块开头写的语言找；没登记的是 `None`。
    pub fn find(&self, name: &str) -> Option<&Language> {
        let name = name.trim().to_lowercase();
        self.languages
            .iter()
            .find(|l| l.names.iter().any(|n| n.to_lowercase() == name))
    }
}

/// 一行代码着好色的片段。没登记的语言只认字符串、数字。
pub fn highlight(line: &str, language: Option<&Language>) -> Vec<Piece> {
    let keywords: HashMap<&str, ()> = language
        .map(|l| l.keywords.iter().map(|k| (k.as_str(), ())).collect())
        .unwrap_or_default();
    let chars: Vec<(usize, char)> = line.char_indices().collect();
    let mut out: Vec<Piece> = Vec::new();
    let mut push = |text: &str, style: Style| match out.last_mut() {
        Some(last) if last.style == style => last.text.push_str(text),
        _ => out.push(Piece::new(text, style)),
    };
    let mut i = 0;
    while i < chars.len() {
        let (at, c) = chars[i];
        let rest = &line[at..];
        if let Some(lang) = language
            && !lang.comment.is_empty()
            && rest.starts_with(&lang.comment)
        {
            push(rest, theme::code_comment());
            break;
        }
        let quote = c == '"'
            || (c == '\'' && language.is_some_and(|l| l.single_quote_strings))
            || (c == '`' && language.is_some_and(|l| l.backtick_strings));
        if quote {
            let end = string_end(&chars, i, c);
            push(&slice(line, &chars, i, end), theme::code_string());
            i = end;
            continue;
        }
        let after_word = i > 0 && is_word(chars[i - 1].1);
        if c.is_ascii_digit() && !after_word {
            let mut end = i;
            while end < chars.len()
                && (chars[end].1.is_ascii_alphanumeric() || matches!(chars[end].1, '.' | '_'))
            {
                end += 1;
            }
            push(&slice(line, &chars, i, end), theme::code_number());
            i = end;
            continue;
        }
        if is_word(c) {
            let mut end = i;
            while end < chars.len() && is_word(chars[end].1) {
                end += 1;
            }
            let word = slice(line, &chars, i, end);
            let next = chars[end..]
                .iter()
                .map(|(_, c)| *c)
                .find(|c| !c.is_whitespace());
            let style = if keywords.contains_key(word.as_str()) {
                theme::code_keyword()
            } else if next == Some('(') {
                theme::code_function()
            } else {
                Style::new()
            };
            push(&word, style);
            i = end;
            continue;
        }
        push(&c.to_string(), Style::new());
        i += 1;
    }
    out
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// 字符串到哪结束：下一个没被反斜杠转义的同样的引号之后；没有的到行尾。
fn string_end(chars: &[(usize, char)], start: usize, quote: char) -> usize {
    let mut i = start + 1;
    while i < chars.len() {
        match chars[i].1 {
            '\\' => i += 2,
            c if c == quote => return i + 1,
            _ => i += 1,
        }
    }
    chars.len()
}

fn slice(line: &str, chars: &[(usize, char)], from: usize, to: usize) -> String {
    let start = chars.get(from).map_or(line.len(), |(i, _)| *i);
    let end = chars.get(to).map_or(line.len(), |(i, _)| *i);
    line[start..end].to_string()
}

#[cfg(test)]
mod tests {
    use super::{Languages, highlight};
    use crate::theme;

    fn languages() -> Languages {
        serde_json::from_str(include_str!("../../resources/code.json")).unwrap()
    }

    #[test]
    fn keywords_functions_strings_numbers_comments() {
        let langs = languages();
        let python = langs.find("Py");
        let pieces = highlight(r#"def norm(x): return "a\"b" + 0.5  # done"#, python);
        let styled = |text: &str| pieces.iter().find(|p| p.text == text).map(|p| p.style);
        assert_eq!(styled("def"), Some(theme::code_keyword()));
        assert_eq!(styled("norm"), Some(theme::code_function()));
        assert_eq!(styled(r#""a\"b""#), Some(theme::code_string()));
        assert_eq!(styled("0.5"), Some(theme::code_number()));
        assert_eq!(styled("# done"), Some(theme::code_comment()));
        let joined: String = pieces.iter().map(|p| p.text.as_str()).collect();
        assert_eq!(
            joined, r#"def norm(x): return "a\"b" + 0.5  # done"#,
            "一个字都不丢"
        );
    }

    #[test]
    fn unknown_languages_still_get_strings_and_numbers() {
        let pieces = highlight(r#"x = "s" 42"#, None);
        assert!(
            pieces
                .iter()
                .any(|p| p.text == r#""s""# && p.style == theme::code_string())
        );
        assert!(
            pieces
                .iter()
                .any(|p| p.text == "42" && p.style == theme::code_number())
        );
    }
}
