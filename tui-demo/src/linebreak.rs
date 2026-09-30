//! 折在哪（蓝图 `tui.md`「她的回答：Markdown」第 2 条）：正文里的字都照这一套折，输入框照字素簇折（`input/wrap.rs`）。
//! 英文照空格断，一个词不劈开；中文字与字之间、中英文交界处都能断；避头尾：句读、收尾的括号引号不放在行首，
//! 开头的括号引号不放在行尾，`……`、`——` 不拆开；一个词比一行还长的硬切。

use std::ops::Range;

use unicode_width::UnicodeWidthStr;

/// 不放在行首的：碰上了带着前一个字一起折下去。
const NO_START: &str = "。，、．：；！？）］｝」』】〕〉》〗〙〛”’…—～ー々ゝゞ,.;:!?)]}%";
/// 不放在行尾的：跟着后面的字折下去。
const NO_END: &str = "（［｛「『【〔〈《〖〘〚“‘([{";

/// 一段（没有换行）的字素簇按 `width` 列折成几行，交回每一行是第几个到第几个字素簇。空的一段也有一行。
/// 满了的那一格是空白：留在这一行末尾（超出的看不见，复制时还在），下一行不从空白起。
pub fn lines(cells: &[&str], width: usize) -> Vec<Range<usize>> {
    let width = width.max(1);
    let mut out = Vec::new();
    let mut start = 0;
    let mut used = 0;
    let mut i = 0;
    while i < cells.len() {
        let w = cells[i].width();
        if used + w > width && i > start {
            if blank(cells[i]) {
                out.push(start..i + 1);
                start = i + 1;
                used = 0;
                i += 1;
                continue;
            }
            let cut = (start + 1..=i)
                .rev()
                .find(|&j| breakable(cells, j))
                .unwrap_or_else(|| hard(cells, start, i));
            out.push(start..cut);
            start = cut;
            used = cells[start..i].iter().map(|g| g.width()).sum();
            continue;
        }
        used += w;
        i += 1;
    }
    out.push(start..cells.len());
    out
}

/// 第 `j` 个字前面能不能断：空白后面、宽字（中文）前后能断；避头尾的不能。
fn breakable(cells: &[&str], j: usize) -> bool {
    let (before, after) = (cells[j - 1], cells[j]);
    if marked(NO_START, after) || marked(NO_END, before) {
        return false;
    }
    blank(before) || before.width() > 1 || after.width() > 1
}

/// 一行里找不到能断的地方（一个词比一行还长）：在满了的地方硬切；切下来打头的是不放在行首的，往前挪一个字。
fn hard(cells: &[&str], start: usize, at: usize) -> usize {
    if at > start + 1 && marked(NO_START, cells[at]) {
        at - 1
    } else {
        at.max(start + 1)
    }
}

fn blank(g: &str) -> bool {
    g.trim().is_empty()
}

fn marked(set: &str, g: &str) -> bool {
    g.chars().next().is_some_and(|c| set.contains(c))
}

#[cfg(test)]
mod tests {
    use unicode_segmentation::UnicodeSegmentation;

    use super::lines;

    /// 折好的一行行字。
    fn fold(text: &str, width: usize) -> Vec<String> {
        let cells: Vec<&str> = text.graphemes(true).collect();
        lines(&cells, width)
            .into_iter()
            .map(|r| cells[r].concat())
            .collect()
    }

    #[test]
    fn english_breaks_at_spaces_and_chinese_anywhere() {
        assert_eq!(
            fold("hello wonderful world", 12),
            ["hello ", "wonderful ", "world"]
        );
        assert_eq!(fold("你好世界", 5), ["你好", "世界"]);
    }

    #[test]
    fn an_english_word_after_chinese_is_not_split() {
        // 2026-09-30 项目主人：你说的话里 `p` / `rompt` 从中间劈开。
        assert_eq!(
            fold("派子代理，prompt 写：先", 14),
            ["派子代理，", "prompt 写：先"]
        );
    }

    #[test]
    fn closing_punctuation_never_starts_a_line() {
        // 同一天：回答里 `。` 落在行首。满了的地方是 `。`，带着前一个字一起折下去。
        assert_eq!(
            fold("会先睡十五秒再读文件。）", 20),
            ["会先睡十五秒再读文", "件。）"]
        );
        assert_eq!(
            fold("say hi, world", 6),
            ["say ", "hi, ", "world"],
            "英文的逗号也不放行首"
        );
    }

    #[test]
    fn opening_brackets_never_end_a_line_and_ellipses_stay_whole() {
        assert_eq!(fold("看这里（注释）", 8), ["看这里", "（注释）"]);
        // `…` 算一列：宽 7 时满在第二个 `…` 上，两个 `…` 连同前一个字一起折下去。
        assert_eq!(fold("等一下……好", 7), ["等一", "下……好"]);
    }

    #[test]
    fn a_word_longer_than_the_line_is_cut_and_a_full_space_stays_behind() {
        assert_eq!(fold("abcdefgh", 3), ["abc", "def", "gh"]);
        assert_eq!(fold("ab cd", 2), ["ab ", "cd"]);
        assert_eq!(fold("", 5), [""]);
    }
}
