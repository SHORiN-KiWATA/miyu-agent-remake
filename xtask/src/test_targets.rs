//! 每个测试文件都进了测试程序（施工 0-3 三补）：集成测试并成一个程序的 crate（`Cargo.toml` 写了 `autotests = false`），
//! cargo 不再自己找 `tests/*.rs`。新加的文件忘了在 `tests/all.rs` 添一行 `mod <名>;`、也没在 `Cargo.toml` 另列 `[[test]]`
//! 的，既不编也不跑，门禁照样绿。这里查出来。

use std::path::Path;

/// 查仓库里每个 crate。读不了的目录当没有。
pub fn check(root: &Path, crates: &[&Path]) -> Vec<String> {
    let mut problems = Vec::new();
    for dir in crates {
        let Ok(manifest) = std::fs::read_to_string(dir.join("Cargo.toml")) else {
            continue;
        };
        let tests = dir.join("tests");
        let Ok(entries) = std::fs::read_dir(&tests) else {
            continue;
        };
        let mut files: Vec<String> = entries
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                let main = entry.path().join("main.rs");
                if entry.path().is_dir() && main.is_file() {
                    return Some(format!("{name}/main.rs"));
                }
                name.ends_with(".rs").then_some(name)
            })
            .collect();
        files.sort();
        let all = std::fs::read_to_string(tests.join("all.rs")).ok();
        let label = dir.strip_prefix(root).unwrap_or(dir).join("tests");
        problems.extend(unlisted(&manifest, all.as_deref(), &files).into_iter().map(|file| {
            format!(
                "{} 没进任何测试程序：在 tests/all.rs 添一行 `mod {};`，装日志订阅者的在 Cargo.toml 另列 [[test]]",
                label.join(&file).display(),
                file.trim_end_matches(".rs").trim_end_matches("/main"),
            )
        }));
    }
    problems
}

/// `autotests = false` 的 crate 里，`files`（`tests/` 下的 `<名>.rs`、`<目录>/main.rs`）哪几个既不在 `all.rs` 的
/// `mod <名>;` 里、也不是 `[[test]]` 的 `path`。没关掉自动找的什么都不查。
fn unlisted(manifest: &str, all: Option<&str>, files: &[String]) -> Vec<String> {
    if !manifest
        .lines()
        .any(|line| line.trim() == "autotests = false")
    {
        return Vec::new();
    }
    let declared: Vec<&str> = manifest
        .lines()
        .filter_map(|line| line.trim().strip_prefix("path = \"tests/"))
        .filter_map(|rest| rest.strip_suffix('"'))
        .collect();
    let modules: Vec<&str> = all
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.trim().strip_prefix("mod "))
        .filter_map(|rest| rest.strip_suffix(';'))
        .collect();
    files
        .iter()
        .filter(|file| {
            let name = file.trim_end_matches(".rs");
            !declared.contains(&file.as_str()) && !modules.contains(&name)
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests;
