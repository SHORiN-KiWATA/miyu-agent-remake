//! 照显示宽度折行（引导里几行的格子、大编辑浮窗，「第一次打开的引导」第 21、21a 条）：画的时候折成一行行，按上下键
//! 时照折好的行挪光标。纯字，不管画。

use unicode_width::UnicodeWidthChar;

/// 照显示宽度折行：交回一行行和光标（字节下标 `cursor`）落在第几行第几列。换行符另起一行。
pub fn wrap(text: &str, cursor: usize, width: usize) -> (Vec<String>, (usize, usize)) {
    let mut rows = vec![String::new()];
    let mut caret = (0, 0);
    let mut col = 0;
    for (i, c) in text.char_indices() {
        if i == cursor {
            caret = (rows.len() - 1, col);
        }
        if c == '\n' {
            rows.push(String::new());
            col = 0;
            continue;
        }
        let w = c.width().unwrap_or(0);
        if col + w > width && col > 0 {
            rows.push(String::new());
            col = 0;
        }
        if let Some(last) = rows.last_mut() {
            last.push(c);
        }
        col += w;
    }
    if cursor >= text.len() {
        caret = (rows.len() - 1, col);
    }
    (rows, caret)
}

/// 折成 `width` 宽以后第 `row` 行、第 `goal` 列在原文里是第几个字节：那一行没那么长的落在行尾，没有这一行的是 `None`。
pub fn offset_at(text: &str, width: usize, row: usize, goal: usize) -> Option<usize> {
    let mut at_row = 0;
    let mut col = 0;
    let mut found = None;
    for (i, c) in text.char_indices() {
        let w = c.width().unwrap_or(0);
        let breaks = c != '\n' && col + w > width && col > 0;
        if breaks {
            if at_row == row {
                return Some(i);
            }
            at_row += 1;
            col = 0;
        }
        if at_row == row {
            if col >= goal || c == '\n' {
                return Some(i);
            }
            found = Some(i + c.len_utf8());
        }
        if c == '\n' {
            if at_row > row {
                break;
            }
            at_row += 1;
            col = 0;
            continue;
        }
        col += w;
    }
    match (at_row.cmp(&row), found) {
        (std::cmp::Ordering::Less, _) => None,
        (_, Some(end)) => Some(end),
        // 这一行是空的：换行符后面、或者整段的末尾。
        _ => Some(text.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::{offset_at, wrap};

    #[test]
    fn wrapping_keeps_the_caret_on_its_character() {
        let (rows, caret) = wrap("ab\ncdef", 4, 3);
        assert_eq!(rows, ["ab", "cde", "f"]);
        assert_eq!(caret, (1, 1), "c 后面");
        let (_, end) = wrap("一二三", "一二三".len(), 4);
        assert_eq!(end, (1, 2), "宽字两列，第三个字折到下一行");
        assert_eq!(wrap("", 0, 5), (vec![String::new()], (0, 0)));
    }

    #[test]
    fn a_wrapped_row_and_column_map_back_to_the_text() {
        // "ab" / "cde" / "f"
        let text = "ab\ncdef";
        assert_eq!(offset_at(text, 3, 0, 1), Some(1));
        assert_eq!(
            offset_at(text, 3, 0, 9),
            Some(2),
            "行短：落在行尾（换行符前面）"
        );
        assert_eq!(offset_at(text, 3, 1, 2), Some(5));
        assert_eq!(offset_at(text, 3, 2, 0), Some(6), "软折行的下一行");
        assert_eq!(offset_at(text, 3, 2, 5), Some(7), "最后一行的末尾");
        assert_eq!(offset_at(text, 3, 3, 0), None, "没有这一行");
        assert_eq!(offset_at("a\n", 3, 1, 0), Some(2), "末尾的空行");
        for (row, goal) in [(0, 0), (0, 2), (1, 0), (1, 3), (2, 1)] {
            let at = offset_at(text, 3, row, goal).unwrap();
            assert_eq!(
                wrap(text, at, 3).1.0,
                row,
                "第 {row} 行第 {goal} 列落回第 {row} 行"
            );
        }
    }
}
