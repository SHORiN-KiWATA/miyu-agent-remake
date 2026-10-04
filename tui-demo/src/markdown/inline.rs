//! 行内：一段段带样子的字（`Piece`），和把它们按显示宽度折成行（蓝图 `tui.md`「她的回答：Markdown」第 2 条）。
//!
//! 折行按字素簇、按显示宽度；英文尽量在空格处断，一个词比一行还长时才从词中间断；中文哪里都能断。
//! 字里的 `\n` 是硬换行。

use ratatui::style::Style;
use ratatui::text::Span;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// 一段带样子的字；是链接的一部分时带着地址。
#[derive(Debug, Clone, PartialEq)]
pub struct Piece {
    /// 字，可以带 `\n`。
    pub text: String,
    /// 样子。
    pub style: Style,
    /// 属于哪个链接。
    pub link: Option<String>,
}

impl Piece {
    /// 一段字。
    pub fn new(text: impl Into<String>, style: Style) -> Self {
        Self {
            text: text.into(),
            style,
            link: None,
        }
    }

    /// 一段属于链接的字。
    pub fn linked(text: impl Into<String>, style: Style, url: &str) -> Self {
        Self {
            text: text.into(),
            style,
            link: Some(url.to_string()),
        }
    }
}

/// 折好的一行：片段、链接落在第几列到第几列（从这一行的开头算）、是不是上一行折下来的。
#[derive(Debug, Clone, Default)]
pub struct Folded {
    /// 这一行的片段。
    pub spans: Vec<Span<'static>>,
    /// 链接：起列、止列（不含）、地址。
    pub links: Vec<(u16, u16, String)>,
    /// 这一行是上一行折下来的（软折行）。
    pub joined: bool,
}

impl Folded {
    fn width(&self) -> usize {
        self.spans.iter().map(Span::width).sum()
    }

    fn push(&mut self, text: &str, piece: &Piece) {
        let start = u16::try_from(self.width()).unwrap_or(u16::MAX);
        let end = start.saturating_add(u16::try_from(text.width()).unwrap_or(0));
        // 和前一段样子一样的接在一起，少几个片段；链接按列另记，不受影响。
        match self.spans.last_mut() {
            Some(last) if last.style == piece.style => last.content.to_mut().push_str(text),
            _ => self.spans.push(Span::styled(text.to_string(), piece.style)),
        }
        if let Some(url) = &piece.link {
            match self.links.last_mut() {
                Some((_, to, last)) if last == url && *to == start => *to = end,
                _ => self.links.push((start, end, url.clone())),
            }
        }
    }
}

/// 把片段按 `width` 列折成行：换行是硬换行，一段里照 `linebreak.rs` 的规则折（英文照空格断、避头尾）。
pub fn fold(pieces: &[Piece], width: u16) -> Vec<Folded> {
    let mut out = Vec::new();
    // 一段里的字素簇：（字、它属于哪个片段）。
    let mut para: Vec<(&str, usize)> = Vec::new();
    for (index, piece) in pieces.iter().enumerate() {
        for g in piece.text.graphemes(true) {
            if g == "\n" {
                lay(&mut out, &para, pieces, width);
                para.clear();
                continue;
            }
            para.push((g, index));
        }
    }
    lay(&mut out, &para, pieces, width);
    out
}

/// 一段折好放进 `out`：第一行接着上一段的硬换行，后面的是折下来的。
fn lay(out: &mut Vec<Folded>, para: &[(&str, usize)], pieces: &[Piece], width: u16) {
    let cells: Vec<&str> = para.iter().map(|(g, _)| *g).collect();
    for (k, range) in crate::linebreak::lines(&cells, usize::from(width.max(1)))
        .into_iter()
        .enumerate()
    {
        let mut line = Folded {
            joined: k > 0,
            ..Folded::default()
        };
        for &(g, index) in &para[range] {
            line.push(g, &pieces[index]);
        }
        out.push(line);
    }
}

/// 一行里的字，拼起来。
pub fn plain(folded: &Folded) -> String {
    folded.spans.iter().map(|s| s.content.as_ref()).collect()
}

#[cfg(test)]
mod tests {
    use ratatui::style::Style;

    use super::{Piece, fold, plain};

    fn lines(text: &str, width: u16) -> Vec<(String, bool)> {
        fold(&[Piece::new(text, Style::new())], width)
            .iter()
            .map(|l| (plain(l), l.joined))
            .collect()
    }

    #[test]
    fn english_breaks_at_spaces_and_chinese_anywhere() {
        assert_eq!(
            lines("hello wonderful world", 12),
            vec![
                ("hello ".into(), false),
                ("wonderful ".into(), true),
                ("world".into(), true)
            ]
        );
        assert_eq!(
            lines("你好世界", 5),
            vec![("你好".into(), false), ("世界".into(), true)]
        );
    }

    #[test]
    fn a_full_stop_never_starts_a_line() {
        // 2026-09-30 项目主人：回答里 `。` 落在行首（蓝图「她的回答：Markdown」第 2 条）。
        let got = lines("会先睡十五秒再读文件。", 20);
        assert!(got.iter().all(|(l, _)| !l.starts_with('。')), "{got:?}");
    }

    #[test]
    fn a_word_longer_than_the_line_is_cut() {
        assert_eq!(
            lines("abcdefgh", 3),
            vec![
                ("abc".into(), false),
                ("def".into(), true),
                ("gh".into(), true)
            ]
        );
    }

    #[test]
    fn newlines_are_hard_breaks() {
        assert_eq!(
            lines("a\nb", 10),
            vec![("a".into(), false), ("b".into(), false)]
        );
    }

    #[test]
    fn links_keep_their_columns_across_pieces() {
        let pieces = [
            Piece::new("see ", Style::new()),
            Piece::linked("docs", Style::new().bold(), "https://a.b"),
        ];
        let folded = fold(&pieces, 20);
        assert_eq!(folded[0].links, vec![(4, 8, "https://a.b".to_string())]);
    }
}
