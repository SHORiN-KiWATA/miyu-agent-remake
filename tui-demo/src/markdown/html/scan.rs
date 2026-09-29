//! 把一段 HTML 拆成标签和字（蓝图 `tui.md`「她的回答：Markdown」第 12 条）：只认标签的样子，不做 HTML 解析。

/// 一段 HTML 拆出来的一截：标签，或者标签之间的字。
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Token<'a> {
    /// 标签：名字（小写）、属性（名字小写）、是不是收尾的 `</…>`、原文。
    Tag {
        name: String,
        attrs: Vec<(String, String)>,
        closing: bool,
        raw: &'a str,
    },
    /// 字。
    Text(&'a str),
}

/// `needle`（小写的 ASCII）在 `hay` 里第一次出现的地方，不分大小写。
pub(super) fn find(hay: &str, needle: &str) -> Option<usize> {
    hay.to_ascii_lowercase().find(needle)
}

/// 把一段 HTML 拆成标签和字。注释 `<!-- -->` 丢掉；不像标签的 `<` 当字。
pub(super) fn tokens(html: &str) -> Vec<Token<'_>> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < html.len() {
        let rest = &html[at..];
        if let Some(body) = rest.strip_prefix("<!--") {
            let end = body.find("-->").map_or(html.len(), |i| at + 4 + i + 3);
            at = end;
            continue;
        }
        let tag = rest.starts_with('<').then(|| tag(rest)).flatten();
        match tag {
            Some((token, len)) => {
                out.push(token);
                at += len;
            }
            None => {
                // 到下一个 `<` 为止都是字（开头这个 `<` 不像标签的，也算字）。
                let first = rest.chars().next().map_or(1, char::len_utf8);
                let next = rest[first..].find('<').map_or(rest.len(), |i| i + first);
                out.push(Token::Text(&rest[..next]));
                at += next;
            }
        }
    }
    out
}

/// `rest` 开头的一个标签和它的长度；不像标签的交回 `None`。
fn tag(rest: &str) -> Option<(Token<'_>, usize)> {
    let end = rest.find('>')?;
    let raw = &rest[..=end];
    let inner = raw[1..end].trim_end_matches('/');
    let (closing, inner) = match inner.strip_prefix('/') {
        Some(inner) => (true, inner),
        None => (false, inner),
    };
    let name_len = inner
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
        .unwrap_or(inner.len());
    if name_len == 0 || !inner.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return None;
    }
    let name = inner[..name_len].to_ascii_lowercase();
    Some((
        Token::Tag {
            name,
            attrs: attrs(&inner[name_len..]),
            closing,
            raw,
        },
        end + 1,
    ))
}

/// 属性：`名="值"`、`名='值'`、`名=值`、只有名字的。名字小写。
fn attrs(mut s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    loop {
        s = s.trim_start();
        let name_len = s
            .find(|c: char| c.is_whitespace() || c == '=')
            .unwrap_or(s.len());
        if name_len == 0 {
            return out;
        }
        let name = s[..name_len].to_ascii_lowercase();
        s = s[name_len..].trim_start();
        let value = match s.strip_prefix('=') {
            Some(v) => {
                let v = v.trim_start();
                let (value, rest) = match v.chars().next() {
                    Some(q @ ('"' | '\'')) => {
                        let close = v[1..].find(q).map_or(v.len(), |i| i + 1);
                        (&v[1..close], v.get(close + 1..).unwrap_or(""))
                    }
                    _ => {
                        let stop = v.find(char::is_whitespace).unwrap_or(v.len());
                        (&v[..stop], &v[stop..])
                    }
                };
                s = rest;
                value.to_string()
            }
            None => String::new(),
        };
        out.push((name, value));
    }
}
