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

/// 把片段按 `width` 列折成行。
pub fn fold(pieces: &[Piece], width: u16) -> Vec<Folded> {
    let width = usize::from(width.max(1));
    // 一个一个字素簇排：（字、它属于哪个片段）。
    let mut out = vec![Folded::default()];
    let mut used = 0;
    // 这一行最后一个能断的地方：空格之后第几个字素簇、那时候用了几列。
    let mut cells: Vec<(String, usize)> = Vec::new();
    let mut last_space: Option<usize> = None;
    for (index, piece) in pieces.iter().enumerate() {
        for g in piece.text.graphemes(true) {
            if g == "\n" {
                flush(&mut out, &mut cells, pieces);
                out.push(Folded::default());
                used = 0;
                last_space = None;
                continue;
            }
            let w = g.width();
            let blank = g.trim().is_empty();
            // 满了的那一格是空白：留在这一行末尾（超出的会被裁掉、看不见，复制时还在），下一行不从空白起。
            if used + w > width && blank && !cells.is_empty() {
                cells.push((g.to_string(), index));
                flush(&mut out, &mut cells, pieces);
                out.push(Folded {
                    joined: true,
                    ..Folded::default()
                });
                used = 0;
                last_space = None;
                continue;
            }
            if used + w > width && !cells.is_empty() {
                // 在词中间满了：退回到这一行最后一个空格之后断，那一截挪到下一行。
                let wide = g.chars().any(|c| c.len_utf8() > 2);
                let cut = last_space.filter(|_| !wide);
                let carry = match cut {
                    Some(at) if at < cells.len() => cells.split_off(at),
                    _ => Vec::new(),
                };
                flush(&mut out, &mut cells, pieces);
                out.push(Folded {
                    joined: true,
                    ..Folded::default()
                });
                used = carry.iter().map(|(t, _)| t.width()).sum();
                cells = carry;
                last_space = None;
            }
            cells.push((g.to_string(), index));
            used += w;
            if blank {
                last_space = Some(cells.len());
            }
        }
    }
    flush(&mut out, &mut cells, pieces);
    out
}

fn flush(out: &mut [Folded], cells: &mut Vec<(String, usize)>, pieces: &[Piece]) {
    let Some(line) = out.last_mut() else {
        return;
    };
    for (text, index) in cells.drain(..) {
        line.push(&text, &pieces[index]);
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
