//! 编辑、写入加减的行数，照参数估（结果来之前，`docs/blueprint/view.md`「种类」`tool` 的 `diff`）：写入的全部算加上的，
//! 编辑的每一处按行比。和终端原来照参数算的一样。

use serde::Deserialize;
use serde_json::Value;
use similar::{ChangeTag, TextDiff};

use crate::entry::Diff;

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

/// 照参数估：既不是写入、也不是编辑的样子的，是 `None`。
pub(crate) fn from_args(args: &Value) -> Option<Diff> {
    if let Ok(write) = Write::deserialize(args) {
        return Some(estimated(count("", &write.content)));
    }
    let edit = Edit::deserialize(args).ok()?;
    let (added, removed) = edit
        .edits
        .iter()
        .map(|piece| count(&piece.old_string, &piece.new_string))
        .fold((0, 0), |(a, r), (x, y)| (a + x, r + y));
    Some(estimated((added, removed)))
}

fn estimated((added, removed): (u64, u64)) -> Diff {
    Diff {
        added,
        removed,
        estimated: true,
    }
}

/// 两份按行比：加了几行、删了几行。
fn count(before: &str, after: &str) -> (u64, u64) {
    TextDiff::from_lines(before, after)
        .iter_all_changes()
        .fold((0, 0), |(a, r), change| match change.tag() {
            ChangeTag::Insert => (a + 1, r),
            ChangeTag::Delete => (a, r + 1),
            ChangeTag::Equal => (a, r),
        })
}
