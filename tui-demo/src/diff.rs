//! 编辑、写入点开以后的差异（蓝图 `tui.md`「编辑、写入点开的差异」）：照工具调用的参数比，按行排。
//!
//! 头不读核心的存储（`01-架构.md` 第五节第 1 条），所以只照参数：写入的全部是加上的，带行号；编辑的每一处
//! 按行比，不知道在文件的第几行，不带行号。完整的差异等核心经协议交出来（蓝图「还没有的」）。

use serde::Deserialize;
use similar::{ChangeTag, TextDiff};

/// 每处改动前后带几行不变的。
const CONTEXT: usize = 3;

/// 写入的参数里要的那一格。
#[derive(Deserialize)]
struct Write {
    content: String,
}

/// 编辑的参数里要的那一格。
#[derive(Deserialize)]
struct Edit {
    edits: Vec<Piece>,
}

/// 编辑的一处。
#[derive(Deserialize)]
struct Piece {
    old_string: String,
    new_string: String,
}

/// 差异里一行是什么。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    /// 没变的。
    Keep,
    /// 删掉的。
    Removed,
    /// 加上的。
    Added,
    /// 两处改动隔得远，中间略过的。
    Gap,
}

/// 差异里的一行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// 是什么。
    pub mark: Mark,
    /// 行号：删掉的是原来的，别的是改后的；略过的那一行没有。
    pub number: Option<usize>,
    /// 这一行的字，不带行尾的换行。
    pub text: String,
}

/// 一个文件的差异。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Diff {
    /// 排好的行。
    pub lines: Vec<Line>,
    /// 加了几行。
    pub added: usize,
    /// 删了几行。
    pub removed: usize,
}

/// 比两份内容。新建的文件，`before` 传空的。
pub fn compare(before: &str, after: &str) -> Diff {
    let diff = TextDiff::from_lines(before, after);
    let mut out = Diff::default();
    for (i, group) in diff.grouped_ops(CONTEXT).iter().enumerate() {
        if i > 0 {
            out.lines.push(Line {
                mark: Mark::Gap,
                number: None,
                text: String::new(),
            });
        }
        for op in group {
            for change in diff.iter_changes(op) {
                let (mark, number) = match change.tag() {
                    ChangeTag::Equal => (Mark::Keep, change.new_index()),
                    ChangeTag::Delete => (Mark::Removed, change.old_index()),
                    ChangeTag::Insert => (Mark::Added, change.new_index()),
                };
                match mark {
                    Mark::Added => out.added += 1,
                    Mark::Removed => out.removed += 1,
                    _ => {}
                }
                let text = change.value().trim_end_matches(['\n', '\r']).to_string();
                out.lines.push(Line {
                    mark,
                    number: number.map(|n| n + 1),
                    text,
                });
            }
        }
    }
    out
}

/// 照工具调用的参数排差异：有 `content` 的是写入，有 `edits` 的是编辑；都不是的是 `None`。
pub fn from_args(args: &serde_json::Value) -> Option<Diff> {
    if let Ok(write) = Write::deserialize(args) {
        return Some(compare("", &write.content));
    }
    let edit = Edit::deserialize(args).ok()?;
    let mut out = Diff::default();
    for (i, piece) in edit.edits.iter().enumerate() {
        if i > 0 {
            out.lines.push(Line {
                mark: Mark::Gap,
                number: None,
                text: String::new(),
            });
        }
        let one = compare(&piece.old_string, &piece.new_string);
        out.added += one.added;
        out.removed += one.removed;
        // 片段里的行号不是文件里的行号，不写。
        out.lines
            .extend(one.lines.into_iter().map(|l| Line { number: None, ..l }));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::{Mark, compare};

    fn marks(before: &str, after: &str) -> Vec<(Mark, Option<usize>, String)> {
        compare(before, after)
            .lines
            .into_iter()
            .map(|l| (l.mark, l.number, l.text))
            .collect()
    }

    #[test]
    fn removed_lines_keep_old_numbers_and_added_ones_new() {
        let before = "a\nb\nc\n";
        let after = "a\nB\nc\nd\n";
        assert_eq!(
            marks(before, after),
            vec![
                (Mark::Keep, Some(1), "a".into()),
                (Mark::Removed, Some(2), "b".into()),
                (Mark::Added, Some(2), "B".into()),
                (Mark::Keep, Some(3), "c".into()),
                (Mark::Added, Some(4), "d".into()),
            ]
        );
    }

    #[test]
    fn far_apart_changes_get_a_gap_and_three_lines_of_context() {
        let before: String = (1..=20).map(|n| format!("line {n}\n")).collect();
        let after = before
            .replace("line 2\n", "line two\n")
            .replace("line 19\n", "line nineteen\n");
        let diff = compare(&before, &after);
        let gaps = diff.lines.iter().filter(|l| l.mark == Mark::Gap).count();
        assert_eq!(gaps, 1);
        // 第一处：第 1 行、改掉的第 2 行、加上的第 2 行、第 3 到 5 行。
        assert_eq!(
            diff.lines[..6]
                .iter()
                .filter(|l| l.mark == Mark::Keep)
                .count(),
            4
        );
        assert_eq!((diff.added, diff.removed), (2, 2));
    }

    #[test]
    fn edits_from_args_have_no_numbers_and_a_gap_between() {
        let args = serde_json::json!({"file_path": "a", "edits": [
            {"old_string": "x\ny", "new_string": "x\nY"},
            {"old_string": "z", "new_string": ""}]});
        let diff = super::from_args(&args).unwrap();
        assert!(diff.lines.iter().all(|l| l.number.is_none()));
        assert_eq!(diff.lines.iter().filter(|l| l.mark == Mark::Gap).count(), 1);
        assert_eq!((diff.added, diff.removed), (1, 2));
        let write =
            super::from_args(&serde_json::json!({"file_path": "a", "content": "1\n2\n"})).unwrap();
        assert_eq!(write.lines.last().map(|l| l.number), Some(Some(2)));
    }

    #[test]
    fn a_new_file_is_all_added() {
        let diff = compare("", "x\ny\n");
        assert!(diff.lines.iter().all(|l| l.mark == Mark::Added));
        assert_eq!((diff.added, diff.removed), (2, 0));
    }
}
