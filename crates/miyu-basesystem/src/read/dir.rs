//! 读到目录（`10-自带软件.md` 第三节）：照名字列出里面的每一项，目录后面带 `/`；和文件一样照 `offset`、`limit`
//! 分页（opencode 的做法，施工 4-4 下），一次的输出最多 64 KiB。

use std::path::Path;

use miyu_tool::Done;

use super::Texts;
use crate::common::OUTPUT_BYTES;
use crate::load::say;

/// 列出目录 `real` 里从第 `offset` 项起（从 1 数起）的最多 `limit` 项；她给的路径是 `path`，出错时照它说。
pub(super) fn list(texts: &Texts, path: &str, real: &Path, offset: u64, limit: u64) -> Done {
    let entries = match std::fs::read_dir(real) {
        Ok(entries) => entries,
        Err(error) => return texts.common.failed(path, &error),
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let dir = entry.file_type().is_ok_and(|kind| kind.is_dir());
            if dir { format!("{name}/") } else { name }
        })
        .collect();
    if names.is_empty() {
        return Done::ok(say(&texts.empty, &[]));
    }
    names.sort();
    let total = names.len() as u64;
    if offset > total {
        return Done::ok(say(
            &texts.past_end_entries,
            &[
                ("total", &total.to_string()),
                ("offset", &offset.to_string()),
            ],
        ));
    }
    let mut text = String::new();
    let mut to = offset - 1;
    for name in names
        .iter()
        .skip((offset - 1) as usize)
        .take(limit as usize)
    {
        if text.len() + name.len() + 1 > OUTPUT_BYTES && to >= offset {
            break;
        }
        text.push_str(name);
        text.push('\n');
        to += 1;
    }
    if to < total {
        text.push_str(&say(
            &texts.more_entries,
            &[
                ("from", &offset.to_string()),
                ("to", &to.to_string()),
                ("total", &total.to_string()),
                ("next", &(to + 1).to_string()),
            ],
        ));
    }
    Done::ok(text)
}
