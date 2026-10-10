//! `persona.toml` 里 `[persona]` 以外的两张表（施工 P-6 从 `persona.rs` 挪出来守 500 行上限）：`[memory]`（施工 R-3 下）、
//! `[appearance]`（施工 P-6）。

use toml_edit::Item;

use super::{Code, Problem, problem};
use crate::memory::MemoryScope;

/// `[memory]`：只有 `scope`，`persona` 或 `session`。`off` 不在这里：不开记忆是开会话时、预设的事。
pub(super) fn read_memory(
    item: &Item,
    at: &dyn Fn(&Item) -> Option<usize>,
) -> Result<Option<MemoryScope>, Problem> {
    let Some(table) = item.as_table_like() else {
        return Err(problem(
            at(item),
            Code::NotATable,
            "memory",
            "memory must be a table".to_string(),
        ));
    };
    let mut scope = None;
    for (key, item) in table.iter() {
        if key != "scope" {
            return Err(problem(
                at(item),
                Code::UnknownKey,
                &format!("memory.{key}"),
                format!("unknown key memory.{key}"),
            ));
        }
        scope = match item.as_str().and_then(MemoryScope::parse) {
            Some(MemoryScope::Off) | None => {
                return Err(problem(
                    at(item),
                    Code::BadMemoryScope,
                    "memory.scope",
                    "memory.scope must be persona or session".to_string(),
                ));
            }
            found => found,
        };
    }
    Ok(scope)
}

/// `[appearance]`：只有 `seed`，`#` 加六位十六进制，交回小写的。
pub(super) fn read_appearance(
    item: &Item,
    at: &dyn Fn(&Item) -> Option<usize>,
) -> Result<Option<String>, Problem> {
    let Some(table) = item.as_table_like() else {
        return Err(problem(
            at(item),
            Code::NotATable,
            "appearance",
            "appearance must be a table".to_string(),
        ));
    };
    let mut seed = None;
    for (key, item) in table.iter() {
        if key != "seed" {
            return Err(problem(
                at(item),
                Code::UnknownKey,
                &format!("appearance.{key}"),
                format!("unknown key appearance.{key}"),
            ));
        }
        let Some(color) = item.as_str().and_then(color) else {
            return Err(problem(
                at(item),
                Code::BadSeed,
                "appearance.seed",
                "appearance.seed must be a color like #3368c0".to_string(),
            ));
        };
        seed = Some(color);
    }
    Ok(seed)
}

/// `#rrggbb` 的颜色，大小写都认，交回小写的；别的写法没有（施工 P-6：头照它比对，一律小写）。
pub fn color(text: &str) -> Option<String> {
    let hex = text.strip_prefix('#')?;
    (hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| format!("#{}", hex.to_ascii_lowercase()))
}
