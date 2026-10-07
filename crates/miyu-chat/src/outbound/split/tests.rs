//! 按段拆开（`chat.md` 第五条「守着它的」）：正好 `max_chars`、空的和空段、`max_chars` 是 0、按空行装、超长的段按行装、
//! 超长的行硬切、中文按字符数。

use super::split;

#[test]
fn exactly_max_chars_is_one_piece() {
    assert_eq!(split("abcde", 5), ["abcde"]);
    assert_eq!(split("  abcde \n", 5), ["abcde"]);
}

#[test]
fn one_over_max_chars_is_cut() {
    assert_eq!(split("abcdef", 5), ["abcde", "f"]);
}

#[test]
fn blank_gives_nothing() {
    assert!(split("", 5).is_empty());
    assert!(split(" \n\n\t ", 5).is_empty());
    assert!(split("", 0).is_empty());
}

#[test]
fn zero_max_chars_keeps_one_piece() {
    assert_eq!(
        split(" 很长很长的一段\n\n第二段 ", 0),
        ["很长很长的一段\n\n第二段"]
    );
}

#[test]
fn paragraphs_are_packed() {
    assert_eq!(split("aaa\n\nbbb\n\nccc", 8), ["aaa\n\nbbb", "ccc"]);
    assert_eq!(split("aaa\n\nbbb\n\nccc", 7), ["aaa", "bbb", "ccc"]);
}

#[test]
fn blank_paragraphs_are_not_sent() {
    assert_eq!(split("aaaa\n\n   \n\nbbbb", 5), ["aaaa", "bbbb"]);
}

#[test]
fn long_paragraph_is_packed_by_lines() {
    assert_eq!(split("l1aa\nl2bb\nl3cc", 9), ["l1aa\nl2bb", "l3cc"]);
    // 前一段已经装进去的先发，超长的段从新的一块开始。
    assert_eq!(
        split("x\n\nl1aa\nl2bb\nl3cc\n\ny", 9),
        ["x", "l1aa\nl2bb", "l3cc", "y"]
    );
}

#[test]
fn long_line_is_cut_by_chars() {
    assert_eq!(
        split("ab\nabcdefghijk\ncd", 4),
        ["ab", "abcd", "efgh", "ijk", "cd"]
    );
}

#[test]
fn cut_pieces_are_trimmed() {
    assert_eq!(split("abcd efgh", 5), ["abcd", "efgh"]);
    assert_eq!(split("abcd      efgh", 5), ["abcd", "efgh"]);
}

#[test]
fn chinese_is_counted_by_chars() {
    assert_eq!(split("你好", 2), ["你好"]);
    assert_eq!(split("你好世界你好", 4), ["你好世界", "你好"]);
    assert_eq!(split("你好\n\n世界", 6), ["你好\n\n世界"]);
}
