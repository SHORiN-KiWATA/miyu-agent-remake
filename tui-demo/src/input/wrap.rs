//! 折行：把一段文字按显示宽度切成屏幕上的一行一行，以及屏幕坐标和字节下标的互换。
//!
//! 输入框按字素簇折（[`wrap`]），编辑时光标好算；正文照蓝图的折行规则折（[`wrap_words`]、[`pieces`]：英文照空格断、
//! 避头尾，`linebreak.rs`）。

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// 屏幕上的一行，对应原文的字节范围（不含结尾的换行符）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisualLine {
    /// 这一行第一个字的字节下标。
    pub start: usize,
    /// 这一行最后一个字之后的字节下标。
    pub end: usize,
}

/// 按 `width` 列折行。空文字也有一行，光标要有地方待。
pub fn wrap(text: &str, width: u16) -> Vec<VisualLine> {
    let width = usize::from(width.max(1));
    let mut lines = Vec::new();
    let mut offset = 0;
    for paragraph in text.split('\n') {
        let mut start = offset;
        let mut used = 0;
        for (i, g) in paragraph.grapheme_indices(true) {
            let w = g.width();
            if used + w > width && used > 0 {
                lines.push(VisualLine {
                    start,
                    end: offset + i,
                });
                start = offset + i;
                used = 0;
            }
            used += w;
        }
        lines.push(VisualLine {
            start,
            end: offset + paragraph.len(),
        });
        offset += paragraph.len() + 1;
    }
    lines
}

/// 正文里照蓝图的折行规则折（英文照空格断、避头尾，`linebreak.rs`）：每一行的字节范围，和 [`wrap`] 一样的写法。
pub fn wrap_words(text: &str, width: u16) -> Vec<VisualLine> {
    let mut lines = Vec::new();
    let mut offset = 0;
    for paragraph in text.split('\n') {
        let cells: Vec<(usize, &str)> = paragraph.grapheme_indices(true).collect();
        let marks: Vec<&str> = cells.iter().map(|(_, g)| *g).collect();
        let at = |i: usize| cells.get(i).map_or(paragraph.len(), |(b, _)| *b);
        for range in crate::linebreak::lines(&marks, usize::from(width.max(1))) {
            lines.push(VisualLine {
                start: offset + at(range.start),
                end: offset + at(range.end),
            });
        }
        offset += paragraph.len() + 1;
    }
    lines
}

/// 正文里按 `width` 列折好的一行行字，带着「这一行是不是上一行折下来的」：复制时折下来的接回去，不加换行。
pub fn pieces(text: &str, width: u16) -> Vec<(String, bool)> {
    let lines = wrap_words(text, width);
    let mut prev_end = None;
    lines
        .into_iter()
        .map(|l| {
            let joined = prev_end == Some(l.start);
            prev_end = Some(l.end);
            (text[l.start..l.end].to_string(), joined)
        })
        .collect()
}

/// 只要折好的最后 `rows` 行：从末尾往前照原文的换行一段段折，够了就停。和整段 `pieces` 以后取最后几行一样，
/// 但不管前面有多长都只折最后那几段（思考的预览每一帧都要，整段重折的时间跟着思考长度涨）。
pub fn tail_pieces(text: &str, width: u16, rows: usize) -> Vec<(String, bool)> {
    let mut groups = Vec::new();
    let mut have = 0;
    for paragraph in text.rsplit('\n') {
        if have >= rows {
            break;
        }
        let lines = pieces(paragraph, width);
        have += lines.len();
        groups.push(lines);
    }
    let all: Vec<(String, bool)> = groups.into_iter().rev().flatten().collect();
    let skip = all.len().saturating_sub(rows);
    all.into_iter().skip(skip).collect()
}

/// 光标在第几行、第几列。
///
/// 软折行处，上一行的结尾和下一行的开头是同一个下标，这时光标算在下一行开头：
/// 往后打字的字就出现在那里。
pub fn locate(text: &str, lines: &[VisualLine], cursor: usize) -> (usize, usize) {
    let row = lines.iter().rposition(|l| l.start <= cursor).unwrap_or(0);
    let line = lines[row];
    let col = text[line.start..cursor.min(line.end)].width();
    (row, col)
}

/// 第 `row` 行第 `col` 列对应的字节下标，给鼠标点击和上下移动用。
///
/// 点在一个宽字的左半边，落在它前面；右半边，落在它后面。点过行尾，落在行尾。
pub fn offset_at(text: &str, lines: &[VisualLine], row: usize, col: usize) -> usize {
    let Some(line) = lines.get(row.min(lines.len().saturating_sub(1))) else {
        return 0;
    };
    let mut used = 0;
    for (i, g) in text[line.start..line.end].grapheme_indices(true) {
        let w = g.width();
        if col < used + w {
            let before = col - used < w.div_ceil(2);
            return line.start + i + if before { 0 } else { g.len() };
        }
        used += w;
    }
    line.end
}
