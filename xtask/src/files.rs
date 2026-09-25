//! 找 `.rs` 文件。

use std::path::{Path, PathBuf};

/// `dir` 下面所有的 `.rs` 文件，跳过 `target/`，按路径排好。
pub fn rust_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    walk(dir, &mut files)?;
    files.sort();
    Ok(files)
}

fn walk(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let unreadable = |e: std::io::Error| format!("读不了目录 {}：{e}", dir.display());
    for entry in std::fs::read_dir(dir).map_err(unreadable)? {
        let entry = entry.map_err(unreadable)?;
        let path = entry.path();
        if entry.file_type().map_err(unreadable)?.is_dir() {
            if entry.file_name() != "target" {
                walk(&path, files)?;
            }
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
    Ok(())
}
