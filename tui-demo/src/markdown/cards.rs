//! 独占一行的链接（蓝图 `tui.md`「链接卡片」第 1 条，2026-10-02 项目主人定）：一段字照换行切成一行行，一行里不是空白的
//! 字都属于同一个链接的，这一行能换成一张卡片。别的行原样。

use super::inline::Piece;

/// 把一段的字切开：连着的普通行一块，能换成卡片的那一行单独一块，带着它的地址。
pub fn split(pieces: Vec<Piece>) -> Vec<(Vec<Piece>, Option<String>)> {
    let mut lines: Vec<Vec<Piece>> = vec![Vec::new()];
    for piece in pieces {
        let mut parts = piece.text.split('\n').peekable();
        while let Some(part) = parts.next() {
            if !part.is_empty() {
                lines.last_mut().expect("至少一行").push(Piece {
                    text: part.to_string(),
                    ..piece.clone()
                });
            }
            if parts.peek().is_some() {
                lines.push(Vec::new());
            }
        }
    }
    let mut out: Vec<(Vec<Piece>, Option<String>)> = Vec::new();
    for line in lines {
        let card = alone(&line);
        match out.last_mut() {
            Some((pieces, None)) if card.is_none() => {
                pieces.push(Piece::new("\n", ratatui::style::Style::new()));
                pieces.extend(line);
            }
            _ => out.push((line, card)),
        }
    }
    out
}

/// 你说的话里独占一行的裸地址（不走 Markdown，蓝图「链接卡片」第 1 条）：这一行去掉前后空白正好是一个地址，交回它。
pub fn lone_url(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    match super::links::bare_urls(trimmed).as_slice() {
        [(0, end)] if *end == trimmed.len() => Some(trimmed),
        _ => None,
    }
}

/// 这一行不是空白的字都属于同一个链接：交回地址。
fn alone(line: &[Piece]) -> Option<String> {
    let mut url: Option<&str> = None;
    let mut any = false;
    for piece in line.iter().filter(|p| !p.text.trim().is_empty()) {
        any = true;
        let link = piece.link.as_deref()?;
        if url.is_some_and(|u| u != link) {
            return None;
        }
        url = Some(link);
    }
    any.then(|| url.map(str::to_string)).flatten()
}

#[cfg(test)]
mod tests {
    use ratatui::style::Style;

    use super::split;
    use crate::markdown::inline::Piece;

    fn text(t: &str) -> Piece {
        Piece::new(t, Style::new())
    }

    fn link(t: &str, url: &str) -> Piece {
        Piece {
            link: Some(url.into()),
            ..text(t)
        }
    }

    #[test]
    fn a_line_holding_only_a_link_is_cut_out_with_its_address() {
        let pieces = vec![
            text("参考：\n"),
            link("https://a.dev/x", "https://a.dev/x"),
            text("\n还有 "),
            link("这个", "https://b.dev"),
            text(" 也行"),
        ];
        let got = split(pieces);
        let shape: Vec<(String, Option<&str>)> = got
            .iter()
            .map(|(p, c)| (p.iter().map(|x| x.text.as_str()).collect(), c.as_deref()))
            .collect();
        assert_eq!(
            shape,
            [
                ("参考：".to_string(), None),
                ("https://a.dev/x".to_string(), Some("https://a.dev/x")),
                ("还有 这个 也行".to_string(), None),
            ],
            "夹在字里的链接不算"
        );
    }

    #[test]
    fn a_bare_address_alone_on_a_line_is_found() {
        use super::lone_url;
        assert_eq!(lone_url("  https://a.dev/x  "), Some("https://a.dev/x"));
        assert_eq!(lone_url("看 https://a.dev/x"), None);
        assert_eq!(
            lone_url("https://a.dev/x。"),
            None,
            "末尾的标点不算地址，这一行就不只一个地址"
        );
    }

    #[test]
    fn a_whole_paragraph_of_plain_text_stays_one_block() {
        let got = split(vec![text("一行\n两行")]);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].1, None);
        let joined: String = got[0].0.iter().map(|p| p.text.as_str()).collect();
        assert_eq!(joined, "一行\n两行");
    }
}
