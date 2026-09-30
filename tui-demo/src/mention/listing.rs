//! 按目录找（蓝图 `tui.md`「`@` 文件列表」第 2 条）：像 shell 补全那样只读那一层目录。`~` 是家目录，别的相对路径照
//! 工作目录；名字照开头对，大小写不论；点开头的藏起来，打了 `.` 才列；目录在前、文件在后，各照名字排。

use std::fs;
use std::path::{Path, PathBuf};

use super::Candidate;

/// 这个词照目录找：带 `/`、`~` 打头，或者只打了 `@`（列工作目录这一层）。
pub fn by_layer(query: &str) -> bool {
    query.is_empty() || query.contains('/') || query.starts_with('~')
}

/// 列那一层里名字照开头对得上的，最多 `limit` 个。显示的字照打的写法（`~/Documents/`、`src/main.rs`）。
pub fn list(query: &str, cwd: &Path, home: Option<&Path>, limit: usize) -> Vec<Candidate> {
    let (typed_dir, prefix) = query.rsplit_once('/').map_or(("", query), |(d, p)| (d, p));
    let typed_dir = if query.contains('/') {
        format!("{typed_dir}/")
    } else {
        String::new()
    };
    let Some(dir) = resolve(&typed_dir, cwd, home) else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let prefix_lower = prefix.to_lowercase();
    let mut found: Vec<(bool, String)> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let shown = !name.starts_with('.') || prefix.starts_with('.');
            let hit = name.to_lowercase().starts_with(&prefix_lower);
            (shown && hit).then(|| (e.path().is_dir(), name))
        })
        .collect();
    found.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then(a.1.to_lowercase().cmp(&b.1.to_lowercase()))
    });
    let lead = typed_dir.chars().count();
    let hits: Vec<usize> = (lead..lead + prefix.chars().count()).collect();
    found
        .into_iter()
        .take(limit)
        .map(|(dir_entry, name)| Candidate {
            shown: format!("{typed_dir}{name}{}", if dir_entry { "/" } else { "" }),
            path: dir.join(&name),
            dir: dir_entry,
            hits: hits.clone(),
        })
        .collect()
}

/// 打的目录换成真的位置：`~` 换成家目录，相对的照工作目录，空的是工作目录。
fn resolve(typed: &str, cwd: &Path, home: Option<&Path>) -> Option<PathBuf> {
    if typed.is_empty() {
        return Some(cwd.to_path_buf());
    }
    if let Some(rest) = typed.strip_prefix('~') {
        let rest = rest.trim_start_matches('/');
        return Some(home?.join(rest));
    }
    Some(cwd.join(typed))
}

#[cfg(test)]
mod tests {
    use super::{by_layer, list};

    #[test]
    fn a_layer_is_read_like_shell_completion() {
        let root = std::env::temp_dir().join(format!("miyu-layer-{}", std::process::id()));
        let docs = root.join("Documents");
        std::fs::create_dir_all(docs.join("reports")).unwrap();
        std::fs::create_dir_all(root.join("downloads")).unwrap();
        for f in [
            "Documents/readme.md",
            "Documents/Report.txt",
            "Documents/.secret",
            "dot.txt",
        ] {
            std::fs::write(root.join(f), b"x").unwrap();
        }
        assert!(by_layer("") && by_layer("src/") && by_layer("~/Doc") && !by_layer("main"));
        let shown = |query: &str| -> Vec<String> {
            list(query, &root, Some(&root), 50)
                .into_iter()
                .map(|c| c.shown)
                .collect()
        };
        assert_eq!(
            shown(""),
            ["Documents/", "downloads/", "dot.txt"],
            "目录在前，大小写不论排"
        );
        assert_eq!(shown("do"), ["Documents/", "downloads/", "dot.txt"]);
        assert_eq!(
            shown("Documents/re"),
            [
                "Documents/reports/",
                "Documents/readme.md",
                "Documents/Report.txt"
            ]
        );
        assert_eq!(
            shown("~/Documents/."),
            ["~/Documents/.secret"],
            "打了点才列点开头的"
        );
        let first = &list("Documents/re", &root, None, 50)[0];
        assert_eq!(first.path, docs.join("reports"));
        assert_eq!(first.hits, [10, 11], "对上的是打的开头那几个字");
        std::fs::remove_dir_all(&root).unwrap_or_default();
    }
}
