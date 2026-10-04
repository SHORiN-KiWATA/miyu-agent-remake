//! 光标前面那个 `@` 打头的词（蓝图 `tui.md`「`@` 文件列表」第 1 条）：`@` 在最前面或者前面是空白，到光标为止没有
//! 空白；`\ ` 算词里的空格（Tab 补全名字带空格的目录时照 shell 的样子写）。

/// 光标前面的 `@` 词：`@` 在第几个字节，`@` 后面打的字（`\ ` 换回空格）。不是的交回 `None`。
pub fn token(text: &str, cursor: usize) -> Option<(usize, String)> {
    let before = text.get(..cursor)?;
    let at = before.rfind('@')?;
    let lead = before[..at].chars().next_back();
    if lead.is_some_and(|c| !c.is_whitespace()) {
        return None;
    }
    let mut query = String::new();
    let mut chars = before[at + 1..].chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => query.push(chars.next().unwrap_or('\\')),
            c if c.is_whitespace() => return None,
            c => query.push(c),
        }
    }
    Some((at, query))
}

/// 写回词里的样子：空格、反斜杠前面加反斜杠。
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if c == ' ' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{escape, token};

    #[test]
    fn an_at_word_starts_the_line_or_follows_a_space() {
        let at = |text: &str| token(text, text.len());
        assert_eq!(at("@ma"), Some((0, "ma".into())));
        assert_eq!(at("看看 @src/ma"), Some((7, "src/ma".into())));
        assert_eq!(at("@"), Some((0, String::new())));
        assert_eq!(at("a@b.com"), None, "前面不是空白的不算");
        assert_eq!(at("@ma 接着"), None, "到光标为止有空白就不是了");
        assert_eq!(
            at("@My\\ Documents/re"),
            Some((0, "My Documents/re".into()))
        );
        assert_eq!(token("@main 后面", 5), Some((0, "main".into())), "照光标算");
    }

    #[test]
    fn escaping_round_trips() {
        let text = format!("@{}", escape("My Documents/a\\b"));
        assert_eq!(
            token(&text, text.len()),
            Some((0, "My Documents/a\\b".into()))
        );
    }
}
