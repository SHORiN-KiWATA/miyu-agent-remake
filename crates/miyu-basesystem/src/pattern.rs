//! 找文件的模式（`10-自带软件.md` 第十节，施工 4-4 下）：照 ripgrep 的 `--glob`，Claude Code、opencode 都是把
//! 模式原样交给它。`glob` 的 `pattern`、`grep` 的 `glob` 都照这个。
//!
//! - 不带 `/` 的照文件名在任何一层找：`*.rs` 找到各层的 `.rs`。
//! - 带 `/` 的照相对搜的那个目录的路径比；开头的 `/` 只是说「从搜的那个目录算起」。
//! - `**` 跨目录，`*`、`?` 不跨；`{a,b}` 二选一；`[abc]` 挑一个字。

use std::path::{Component, Path, PathBuf};

use globset::{GlobBuilder, GlobMatcher};

/// 认好的一个模式。
pub(crate) struct Pattern {
    glob: GlobMatcher,
    /// 照整条相对路径比；不是就只比文件名。
    whole_path: bool,
}

impl Pattern {
    /// 认一个模式。Windows 上的 `\` 当分隔符，和 `/` 一样。
    ///
    /// # Errors
    ///
    /// 写法不对（例如 `[` 没配上 `]`）：交回说哪里不对的那一句。
    pub(crate) fn new(pattern: &str) -> Result<Pattern, String> {
        let pattern = if cfg!(windows) {
            pattern.replace('\\', "/")
        } else {
            pattern.to_string()
        };
        let anchored = pattern.strip_prefix('/');
        let body = anchored.unwrap_or(&pattern);
        let glob = GlobBuilder::new(body)
            .literal_separator(true)
            .build()
            .map_err(|error| error.kind().to_string())?;
        Ok(Pattern {
            glob: glob.compile_matcher(),
            whole_path: anchored.is_some() || body.contains('/'),
        })
    }

    /// 相对搜的那个目录、分隔符写 `/` 的路径 `relative` 对不对得上。
    pub(crate) fn matches(&self, relative: &str) -> bool {
        if self.whole_path {
            self.glob.is_match(relative)
        } else {
            self.glob
                .is_match(relative.rsplit('/').next().unwrap_or(relative))
        }
    }
}

/// 模式本身是绝对路径（或者 `~` 开头）的，拆成前面不带通配的那一截目录，和后面那一截模式（Claude Code 的做法）。
/// 后面那一截从这个目录算起，开头补一个 `/`：不然 `/src/Cargo.toml` 拆成 `Cargo.toml`，会连各层的都找出来。
/// 不是绝对路径的，交回空的。
pub(crate) fn split_absolute(pattern: &str) -> Option<(PathBuf, String)> {
    let path = Path::new(pattern);
    let home_relative = pattern == "~" || pattern.starts_with("~/") || pattern.starts_with("~\\");
    if !path.is_absolute() && !home_relative {
        return None;
    }
    let components: Vec<Component<'_>> = path.components().collect();
    // 只看一段一段的名字：Windows 换真实位置得来的 `\\?\C:` 那个前缀里有 `?`，不是通配（施工中 CI 上查出）。
    let first_glob = components
        .iter()
        .position(
            |part| matches!(part, Component::Normal(name) if has_glob(&name.to_string_lossy())),
        )
        .unwrap_or(components.len().saturating_sub(1));
    let base: PathBuf = components[..first_glob].iter().collect();
    let rest: Vec<String> = components[first_glob..]
        .iter()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    Some((base, format!("/{}", rest.join("/"))))
}

/// 这一段里有没有通配的字。
fn has_glob(part: &str) -> bool {
    part.contains(['*', '?', '[', '{'])
}

#[cfg(test)]
mod tests;
