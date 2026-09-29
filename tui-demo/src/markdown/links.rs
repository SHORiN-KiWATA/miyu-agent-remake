//! 认链接（蓝图 `tui.md`「她的回答：Markdown」第 9 条，规则照旧版 `render/link.rs`）：
//! 裸地址，和独占一行的「标题 (地址)」。

use super::inline::Piece;
use crate::theme;

/// 认得的协议头。
const SCHEMES: [&str; 3] = ["https://", "http://", "file://"];

/// 地址末尾不算地址的标点。
const TAIL: &str = ".,;:!?'\"。，；：！？、";

/// 能出现在地址里的字：可见的 ASCII，去掉会把地址和正文粘在一起的几个。
fn url_char(c: char) -> bool {
    c.is_ascii_graphic() && !matches!(c, '<' | '>' | '"' | '`' | '{' | '}' | '|' | '\\' | '^')
}

/// 找出一段字里的裸地址：交回（起、止、地址）的字节范围。前一个字是字母数字的不算（`xhttps://`）。
pub fn bare_urls(text: &str) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some((start, scheme)) = next_scheme(text, from) {
        let glued = text[..start]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_ascii_alphanumeric());
        let body = text[start..]
            .find(|c: char| !url_char(c))
            .map_or(text.len(), |i| start + i);
        let end = start + trim_tail(&text[start..body]).len();
        if !glued && end > start + scheme.len() {
            found.push((start, end));
        }
        from = body.max(start + scheme.len());
    }
    found
}

fn next_scheme(text: &str, from: usize) -> Option<(usize, &'static str)> {
    SCHEMES
        .iter()
        .filter_map(|s| text[from..].find(s).map(|i| (from + i, *s)))
        .min_by_key(|(i, _)| *i)
}

/// 去掉地址末尾的标点；成对的括号留着（维基、GitHub 的地址自己就带括号）。
pub fn trim_tail(raw: &str) -> &str {
    let mut end = raw.len();
    while let Some(last) = raw[..end].chars().next_back() {
        let opener = match last {
            ')' => Some('('),
            ']' => Some('['),
            _ => None,
        };
        if let Some(open) = opener {
            let body = &raw[..end];
            if body.matches(last).count() <= body.matches(open).count() {
                break;
            }
        } else if !TAIL.contains(last) {
            break;
        }
        end -= last.len_utf8();
    }
    &raw[..end]
}

/// 地址最后一截的文件名：去掉 `?`、`#` 后面的，照最后一个 `/` 切；没写说明的图片写它（蓝图第 12 条）。
pub fn file_name(url: &str) -> &str {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(path)
}

/// 链接标题后面那一截：` <`、地址、`>`，都属于这个链接，悬停时下划线连成一条。
pub fn address(url: &str) -> [Piece; 3] {
    [
        Piece::linked(" <", theme::dim(), url),
        Piece::linked(url, theme::md_url(), url),
        Piece::linked(">", theme::dim(), url),
    ]
}

/// 独占一行的「标题 (地址)」整个成一个链接，字原样不动：标题链接色加粗，括号和空格暗，地址照旧是地址色。
/// 不是这种行的原样交回。
pub fn relink_title(pieces: Vec<Piece>) -> Vec<Piece> {
    let text: String = pieces.iter().map(|p| p.text.as_str()).collect();
    if text.contains('\n') {
        return pieces;
    }
    let Some((label_end, url)) = title_url(&text) else {
        return pieces;
    };
    let mut out = Vec::new();
    let mut at = 0;
    for mut piece in pieces {
        // 标题结束在这一段中间的，切成两段：前面是标题，后面是括号。
        if at < label_end && label_end < at + piece.text.len() {
            let rest = piece.text.split_off(label_end - at);
            let label = Piece::new(piece.text, piece.style.patch(theme::md_link()));
            out.push(label);
            piece.text = rest;
            at = label_end;
        }
        piece.style = if at < label_end {
            piece.style.patch(theme::md_link())
        } else if piece.link.as_deref() == Some(url) {
            piece.style
        } else {
            theme::dim()
        };
        at += piece.text.len();
        out.push(piece);
    }
    for piece in &mut out {
        piece.link = Some(url.to_string());
    }
    out
}

/// 独占一行的「标题 (地址)」「标题（地址）」：交回标题在哪个字节结束，和地址。
/// 标题是一整段话的（有句号、问号、叹号、分号，或者超过 40 个字）不算，只让地址那一截成链（照旧版）。
pub fn title_url(line: &str) -> Option<(usize, &str)> {
    let body = line.trim();
    let close = body.chars().next_back()?;
    let open = match close {
        ')' => '(',
        '）' => '（',
        _ => return None,
    };
    let at = body.rfind(open)?;
    let url = body[at + open.len_utf8()..body.len() - close.len_utf8()].trim();
    let valid = SCHEMES.iter().any(|s| url.starts_with(s)) && url.chars().all(url_char);
    let label = body[..at].trim_end();
    let sentence =
        label.contains(['。', '？', '！', '；', '.', '?', '!', ';']) || label.chars().count() > 40;
    let lead = line.len() - line.trim_start().len();
    (valid && !label.is_empty() && !label.contains("://") && !label.ends_with(']') && !sentence)
        .then_some((lead + label.len(), url))
}

#[cfg(test)]
mod tests {
    use super::{bare_urls, title_url, trim_tail};

    fn urls(text: &str) -> Vec<&str> {
        bare_urls(text)
            .into_iter()
            .map(|(s, e)| &text[s..e])
            .collect()
    }

    #[test]
    fn bare_urls_drop_sentence_punctuation_and_keep_paired_brackets() {
        assert_eq!(urls("见 https://a.com。"), vec!["https://a.com"]);
        assert_eq!(
            urls("see https://en.wikipedia.org/wiki/Rust_(language), ok"),
            vec!["https://en.wikipedia.org/wiki/Rust_(language)"]
        );
        assert_eq!(urls("(https://a.com/x)"), vec!["https://a.com/x"]);
        assert!(urls("xhttps://a.com").is_empty());
        assert!(urls("https:// 空的").is_empty());
    }

    #[test]
    fn trim_keeps_balanced_closers() {
        assert_eq!(trim_tail("https://a.b/(c)."), "https://a.b/(c)");
        assert_eq!(trim_tail("https://a.b/c)"), "https://a.b/c");
    }

    #[test]
    fn title_url_lines_but_not_sentences() {
        assert_eq!(
            title_url("Miyu 仓库 (https://example.com)"),
            Some((11, "https://example.com"))
        );
        assert_eq!(
            title_url(" 官方文档（ https://docs.rs/x ）"),
            Some((13, "https://docs.rs/x"))
        );
        assert_eq!(title_url("这是一整句话。详情见 (https://a.com)"), None);
        assert_eq!(title_url("[x](https://a.com)"), None);
    }
}
