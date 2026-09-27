//! 读到目录（`10-自带软件.md` 第三节）：照名字列出里面的每一项，目录后面带 `/`，最多 1000 项。

use std::path::Path;

use miyu_tool::Done;

use super::Texts;

/// 一次最多列多少项。
pub(crate) const ENTRY_LIMIT: usize = 1000;

/// 列出目录 `real` 里的每一项；她给的路径是 `path`，出错时照它说。
pub(super) fn list(texts: &Texts, path: &str, real: &Path) -> Done {
    let entries = match std::fs::read_dir(real) {
        Ok(entries) => entries,
        Err(error) => {
            return Done::error(texts.say(
                &texts.failed,
                &[("path", path), ("error", &error.to_string())],
            ));
        }
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
        return Done::ok(texts.say(&texts.empty, &[]));
    }
    names.sort();
    let rest = names.len().saturating_sub(ENTRY_LIMIT);
    names.truncate(ENTRY_LIMIT);
    let mut text = names.join("\n");
    text.push('\n');
    if rest > 0 {
        text.push_str(&texts.say(&texts.more_entries, &[("rest", &rest.to_string())]));
    }
    Done::ok(text)
}
