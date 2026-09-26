//! 模板与转义：给模型看的字怎么拼（`docs/designs/08-上下文投影.md` 第五节「模板与转义怎么写」）。
//!
//! 模板只做字段替换，不含逻辑（`05-内核接口.md` 第五节第 4 条）。换进去的每个字段都先转义，
//! 转出来是一行字，没有引号、没有尖括号：不可信的文字伪造不了标签，也伪造不了一行一条的
//! 记录（08 C7）。

use std::collections::BTreeMap;
use std::fmt;

/// 一个读好的模板：照先后排的原文和字段。
///
/// 模板用 `{名字}` 标出要换进去的字段；要写 `{`、`}` 本身，就写两遍：`{{`、`}}`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    parts: Vec<Part>,
}

/// 模板的一段。
#[derive(Debug, Clone, PartialEq, Eq)]
enum Part {
    /// 原样照抄的字。
    Text(String),
    /// 要换进来的字段的名字。
    Field(String),
}

impl Template {
    /// 读一个模板。
    ///
    /// # Errors
    ///
    /// `{` 没配上 `}`、单独一个 `}`、字段的名字不合写法（小写字母开头，只用小写字母、
    /// 数字、`_`），都返回 [`TemplateError`]，写明哪里坏了。
    pub fn parse(source: &str) -> Result<Template, TemplateError> {
        let mut parts = Vec::new();
        let mut text = String::new();
        let mut chars = source.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '{' if chars.next_if_eq(&'{').is_some() => text.push('{'),
                '}' if chars.next_if_eq(&'}').is_some() => text.push('}'),
                '{' => {
                    let mut name = String::new();
                    let mut closed = false;
                    for c in chars.by_ref() {
                        if c == '}' {
                            closed = true;
                            break;
                        }
                        name.push(c);
                    }
                    if !closed {
                        return Err(TemplateError::new(format!(
                            "{{{name} 没配上 }}：要换的字段写成 {{名字}}，要写 {{ 本身就写两遍 {{{{"
                        )));
                    }
                    if !is_field_name(&name) {
                        return Err(TemplateError::new(format!(
                            "{{{name}}} 不是一个字段：名字小写字母开头，只用小写字母、数字、_"
                        )));
                    }
                    if !text.is_empty() {
                        parts.push(Part::Text(std::mem::take(&mut text)));
                    }
                    parts.push(Part::Field(name));
                }
                '}' => {
                    return Err(TemplateError::new(
                        "有一个单独的 }：要写 } 本身，就写两遍 }}".to_string(),
                    ));
                }
                c => text.push(c),
            }
        }
        if !text.is_empty() {
            parts.push(Part::Text(text));
        }
        Ok(Template { parts })
    }

    /// 照字段换出原文。每个字段都先转义（[`escape`]），可信的、不可信的一样。
    ///
    /// # Errors
    ///
    /// 模板要的字段在 `fields` 里没有，返回 [`TemplateError`]，写明是哪一个。
    pub fn render(&self, fields: &BTreeMap<&str, &str>) -> Result<String, TemplateError> {
        let mut out = String::new();
        for part in &self.parts {
            match part {
                Part::Text(text) => out.push_str(text),
                Part::Field(name) => {
                    let value = fields
                        .get(name.as_str())
                        .ok_or_else(|| TemplateError::new(format!("少了字段 {name}")))?;
                    out.push_str(&escape(value));
                }
            }
        }
        Ok(out)
    }
}

/// 字段的名字：小写字母开头，只用小写字母、数字、`_`。
fn is_field_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// 转义一个要换进模板的字段（08 第五节「模板与转义怎么写」）。
///
/// 照 JSON 字符串的写法：反斜杠写成 `\\`，换行、回车、制表写成 `\n`、`\r`、`\t`，别的控制字符
/// 写成 `\u001b` 这样；再把 `"`、`&`、`<`、`>` 和 Unicode 的行分隔符、段分隔符也写成 `\u`
/// 的样子。转出来是一行字，里面没有引号、没有尖括号，本身又是一段合法的 JSON 字符串。
pub fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // U+2028、U+2029 不算控制字符，可有的地方把它们当换行。
            '"' | '&' | '<' | '>' | '\u{2028}' | '\u{2029}' => push_unicode(&mut out, c),
            c if c.is_control() => push_unicode(&mut out, c),
            c => out.push(c),
        }
    }
    out
}

/// 写成 `\u` 加四位小写十六进制。要这样写的字都在基本平面里，四位就够。
fn push_unicode(out: &mut String, c: char) {
    let code = u32::from(c);
    out.push_str("\\u");
    for shift in [12, 8, 4, 0] {
        out.push(char::from_digit((code >> shift) & 0xf, 16).unwrap_or('0'));
    }
}

/// 模板用不了：写法坏了，或者少了要换的字段。
///
/// 报错是中文，给写模板的人看：模板是出厂的数据或者模块带来的，坏了要改的是模板。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateError {
    /// 哪里坏了。
    pub why: String,
}

impl TemplateError {
    fn new(why: String) -> TemplateError {
        TemplateError { why }
    }
}

impl fmt::Display for TemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "模板用不了：{}", self.why)
    }
}

impl std::error::Error for TemplateError {}

#[cfg(test)]
mod tests;
